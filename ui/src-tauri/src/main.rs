// Fluss settings app: settings window, onboarding and the "fix a word" popup.
// All state lives in the daemon; this process is a thin view that talks to it
// over local IPC and exits when its windows close (unless `ui.keep_warm`).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::time::Duration;

use fluss_core::ipc::Client;
use fluss_core::protocol::Message;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

const PAGES: &[&str] = &["home", "dictionary", "shortcuts", "model", "audio", "text", "output", "advanced", "about"];

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
        .inner_size(1080.0, 720.0)
        .min_inner_size(880.0, 600.0)
        .decorations(false)
        .center()
        .visible(false) // the page shows itself once rendered (no white flash)
        .background_color(tauri::webview::Color(7, 11, 16, 255))
        .build()
        .ok()
}

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
        app.exit(0);
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
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| handle_args(app, &argv)))
        .manage(Pool::default())
        .invoke_handler(tauri::generate_handler![daemon_call, restore_focus, popup_finish, maybe_exit, open_path, app_quit])
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
                        app.exit(0);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Fluss settings");
}
