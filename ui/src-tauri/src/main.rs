// Fluss settings app: settings window, onboarding and the "fix a word" popup.
// All state lives in the daemon; this process is a thin view that talks to it
// over local IPC and exits when its windows close (unless `ui.keep_warm`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use fluss_core::ipc::Client;
use fluss_core::protocol::Message;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

const PAGES: &[&str] = &["home", "dictionary", "shortcuts", "model", "audio", "text", "output", "advanced", "about"];

/// We are restarting the daemon ourselves: its `quit` event must not close this app.
static RESTARTING: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- daemon connection

/// Idle connections to the daemon. Each call checks one out, so a long call
/// (`hotkeys.record` waits for a key press) never blocks the others.
#[derive(Default, Clone)]
struct Pool(Arc<Mutex<Vec<Client>>>);

fn spawn_daemon() {
    let name = if cfg!(windows) { "fluss-daemon.exe" } else { "fluss-daemon" };
    let local = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.exists());
    let mut cmd = std::process::Command::new(local.unwrap_or_else(|| name.into()));
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    let _ = cmd.spawn();
}

fn connect_or_spawn() -> anyhow_like::Result<Client> {
    if let Ok(c) = Client::connect() {
        return Ok(c);
    }
    spawn_daemon();
    Client::connect_wait(Duration::from_secs(8)).map_err(|e| e.to_string())
}

/// Tiny local alias so we don't need anyhow as a direct dependency.
mod anyhow_like {
    pub type Result<T> = std::result::Result<T, String>;
}

fn is_transport_error(e: &(dyn std::error::Error + 'static), msg: &str) -> bool {
    e.downcast_ref::<std::io::Error>().is_some() || e.downcast_ref::<serde_json::Error>().is_some() || msg.contains("closed the connection")
}

fn pooled_call(pool: &Pool, method: &str, params: Value) -> Result<Value, String> {
    for _ in 0..2 {
        let mut client = match pool.0.lock().pop() {
            Some(c) => c,
            None => connect_or_spawn()?,
        };
        match client.call(method, params.clone()) {
            Ok(v) => {
                pool.0.lock().push(client);
                return Ok(v);
            }
            Err(e) => {
                let msg = e.to_string();
                if is_transport_error(e.as_ref(), &msg) {
                    continue; // drop the broken connection and retry once
                }
                pool.0.lock().push(client);
                return Err(msg);
            }
        }
    }
    Err("Fluss is not responding".into())
}

#[tauri::command]
async fn daemon_call(app: AppHandle, method: String, params: Value) -> Result<Value, String> {
    let pool = app.state::<Pool>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || pooled_call(&pool, &method, params))
        .await
        .map_err(|e| e.to_string())?
}

/// Subscribed connection that forwards daemon events to the frontend; reconnects with backoff.
fn event_loop(app: AppHandle) {
    let mut backoff = Duration::from_millis(200);
    loop {
        if let Ok(mut c) = connect_or_spawn() {
            if c.call("subscribe", json!({ "levels": true })).is_ok() {
                let _ = app.emit("daemon-connection", json!({ "connected": true }));
                backoff = Duration::from_millis(200);
                while let Ok(msg) = c.read_message() {
                    if let Message::Event { event, data } = msg {
                        // A warm instance opens the popup straight from the event (no process launch).
                        // The daemon is quitting on purpose: close with it instead of restarting it.
                        if event == "quit" {
                            if RESTARTING.load(Ordering::SeqCst) {
                                break;
                            }
                            quit(&app);
                            return;
                        }
                        if event == "popup" {
                            let h = app.clone();
                            let _ = app.run_on_main_thread(move || show_popup(&h));
                        }
                        let _ = app.emit("daemon-event", json!({ "event": event, "data": data }));
                    }
                }
            }
            let _ = app.emit("daemon-connection", json!({ "connected": false }));
        }
        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(Duration::from_secs(3));
    }
}

/// Exit, and make sure the process is really gone even if teardown stalls.
fn quit(app: &AppHandle) {
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(2));
        hard_exit();
    });
    app.exit(0);
}

/// End the process without DLL teardown: a normal exit can deadlock in WebView2
/// shutdown and leave a hung instance that swallows every later launch.
fn hard_exit() -> ! {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        TerminateProcess(GetCurrentProcess(), 0);
    }
    std::process::exit(0)
}

/// A previous instance that hangs (e.g. stuck while exiting) would still own the
/// single-instance lock, so launching Fluss would silently do nothing. Stop it.
#[cfg(windows)]
fn kill_hung_instances() {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowExW, GetWindowThreadProcessId, SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_NULL};
    let title: Vec<u16> = "com.mflrevan.fluss-siw\0".encode_utf16().collect();
    let me = std::process::id();
    let mut after = std::ptr::null_mut();
    unsafe {
        loop {
            let hwnd = FindWindowExW(std::ptr::null_mut(), after, std::ptr::null(), title.as_ptr());
            if hwnd.is_null() {
                break;
            }
            after = hwnd;
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 || pid == me {
                continue;
            }
            let mut res = 0usize;
            if SendMessageTimeoutW(hwnd, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 1500, &mut res) != 0 {
                continue; // alive: the single-instance plugin hands our arguments to it
            }
            let h = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, pid);
            if !h.is_null() {
                TerminateProcess(h, 1);
                WaitForSingleObject(h, 3000);
                CloseHandle(h);
            }
        }
    }
}

#[cfg(not(windows))]
fn kill_hung_instances() {}

fn keep_warm() -> bool {
    fluss_core::config::Config::load().map(|c| c.ui.keep_warm).unwrap_or(false)
}

// ---------------------------------------------------------------- windows

fn main_window(app: &AppHandle, page: Option<&str>) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window("main") {
        return Some(w);
    }
    let url = match page {
        Some(p) => format!("index.html?page={p}"),
        None => "index.html".into(),
    };
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App(url.into()))
        .title("Fluss")
        .inner_size(1200.0, 760.0)
        .min_inner_size(1120.0, 720.0)
        .decorations(false)
        .center()
        .visible(false) // the page shows itself once rendered (no white flash)
        .background_color(tauri::webview::Color(0, 0, 0, 255))
        .build()
        .ok()
        .inspect(set_taskbar_icon)
}

/// Tauri only sets the small window icon, so the taskbar falls back to the shell's
/// cached icon for the exe path, which can be stale after an update. Set the
/// large icon from our own exe resource.
#[cfg(windows)]
fn set_taskbar_icon(w: &WebviewWindow) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{LoadImageW, SendMessageW, ICON_BIG, IMAGE_ICON, LR_DEFAULTCOLOR, WM_SETICON};
    let Ok(hwnd) = w.hwnd() else { return };
    unsafe {
        // tauri-build embeds the app icon under this resource id.
        let id = tauri::utils::platform::WINDOWS_APP_ICON_RESOURCE_ID as usize as *const u16;
        let icon = LoadImageW(GetModuleHandleW(std::ptr::null()), id, IMAGE_ICON, 64, 64, LR_DEFAULTCOLOR);
        if !icon.is_null() {
            SendMessageW(hwnd.0 as _, WM_SETICON, ICON_BIG as usize, icon as isize);
        }
    }
}

#[cfg(not(windows))]
fn set_taskbar_icon(_w: &WebviewWindow) {}

fn show_main(app: &AppHandle, page: Option<&str>) {
    let existed = app.get_webview_window("main").is_some();
    let Some(w) = main_window(app, page) else { return };
    if !existed {
        // Safety net: if the page never signals it's ready, show the window anyway.
        let w2 = w.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(2500));
            if !w2.is_visible().unwrap_or(true) {
                let _ = w2.show();
            }
        });
    }
    if existed {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        // A fresh window reads `?page`; an existing one gets an event.
        if let Some(p) = page {
            let _ = w.emit("navigate", p);
        }
    }
}

fn popup_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window("popup") {
        return Some(w);
    }
    WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("index.html".into()))
        .title("Fluss — fix a word")
        .inner_size(600.0, 330.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()
        .ok()
}

/// Center the popup horizontally on the monitor under the mouse, a third of the way down.
fn place_popup(app: &AppHandle, w: &WebviewWindow) {
    let Ok(size) = w.outer_size() else { return };
    let cursor = app.cursor_position().ok();
    let monitors = app.available_monitors().unwrap_or_default();
    let m = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let (p, s) = (m.position(), m.size());
                c.x >= p.x as f64 && c.y >= p.y as f64 && c.x < (p.x + s.width as i32) as f64 && c.y < (p.y + s.height as i32) as f64
            })
        })
        .cloned()
        .or_else(|| w.primary_monitor().ok().flatten());
    if let Some(m) = m {
        let (p, s) = (m.position(), m.size());
        let x = p.x + (s.width as i32 - size.width as i32) / 2;
        let y = p.y + (s.height as f64 * 0.28) as i32;
        let _ = w.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn show_popup(app: &AppHandle) {
    let Some(w) = popup_window(app) else { return };
    place_popup(app, &w);
    let _ = w.show();
    #[cfg(windows)]
    if let Ok(h) = w.hwnd() {
        force_foreground(h.0 as isize);
    }
    let _ = w.set_focus();
    let _ = w.emit("popup-show", ());
}

fn handle_args(app: &AppHandle, args: &[String]) {
    let has = |f: &str| args.iter().any(|a| a == f);
    let page = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).map(String::as_str).filter(|p| PAGES.contains(p));
    if has("--popup") {
        show_popup(app);
    } else if has("--hidden") {
        // Resident with no window (keep_warm): pre-create the popup so it opens instantly.
        popup_window(app);
    } else {
        show_main(app, page);
    }
}

// ---------------------------------------------------------------- focus (Windows)

/// Bring a window to the front even when another process owns the foreground.
#[cfg(windows)]
fn force_foreground(hwnd: isize) {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };
    let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
    unsafe {
        if IsWindow(hwnd) == 0 {
            return;
        }
        let fg = GetForegroundWindow();
        let me = GetCurrentThreadId();
        let other = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let attached = other != 0 && other != me && AttachThreadInput(me, other, 1) != 0;
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        if attached {
            AttachThreadInput(me, other, 0);
        }
    }
}

#[cfg(not(windows))]
fn force_foreground(_hwnd: isize) {}

#[tauri::command]
fn restore_focus(hwnd: u64) {
    if hwnd != 0 {
        force_foreground(hwnd as isize);
    }
}

/// Popup done: hide it and give focus back to the app the user was typing in.
#[tauri::command]
fn popup_finish(app: AppHandle, hwnd: u64) {
    if let Some(w) = app.get_webview_window("popup") {
        let _ = w.hide();
    }
    if hwnd != 0 {
        force_foreground(hwnd as isize);
    }
}

/// Exit when nothing is visible and the user didn't ask to keep the app warm.
#[tauri::command]
fn maybe_exit(app: AppHandle) {
    let main_visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if !main_visible && !keep_warm() {
        quit(&app);
    }
}

#[tauri::command]
fn open_path(path: String) {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(&path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(&path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&path);
        c
    };
    let _ = cmd.spawn();
}

#[tauri::command]
fn app_quit(app: AppHandle) {
    quit(&app);
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Replace the running daemon with the one next to this app (after an update
/// left an old one running, or a different build was started).
#[tauri::command]
async fn restart_daemon(app: AppHandle) -> Result<(), String> {
    let pool = app.state::<Pool>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        RESTARTING.store(true, Ordering::SeqCst);
        let res = (|| {
            pool.0.lock().clear();
            if let Ok(c) = Client::connect() {
                let _ = c.call_timeout("daemon.quit", json!({}), Duration::from_secs(3));
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(6);
            while Client::connect().is_ok() {
                if std::time::Instant::now() > deadline {
                    return Err("The background app did not stop. End fluss-daemon.exe in Task Manager.".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            connect_or_spawn().map(drop)
        })();
        RESTARTING.store(false, Ordering::SeqCst);
        res
    })
    .await
    .map_err(|e| e.to_string())?
}

fn main() {
    kill_hung_instances();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| handle_args(app, &argv)))
        .manage(Pool::default())
        .invoke_handler(tauri::generate_handler![daemon_call, restore_focus, popup_finish, maybe_exit, open_path, app_quit, app_version, restart_daemon])
        .setup(|app| {
            let handle = app.handle().clone();
            let args: Vec<String> = std::env::args().collect();
            handle_args(&handle, &args);
            let h = handle.clone();
            std::thread::Builder::new().name("daemon-events".into()).spawn(move || event_loop(h))?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } if keep_warm() => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                WindowEvent::Destroyed => {
                    let app = window.app_handle();
                    if !keep_warm() {
                        quit(app);
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start Fluss settings")
        .run(|_, event| {
            if let RunEvent::Exit = event {
                hard_exit();
            }
        });
}
