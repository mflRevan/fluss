//! Small OS helpers (Windows first; other platforms get safe fallbacks).

use std::path::{Path, PathBuf};

/// The text field the user is typing into, as far as we can tell cheaply.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusCtx {
    pub window: u64,
    pub control: u64,
    pub app: String,
}

/// Path of a sibling executable (`fluss-settings`, `fluss`) next to this one.
pub fn sibling_exe(name: &str) -> PathBuf {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(&file))).filter(|p| p.exists()).unwrap_or_else(|| PathBuf::from(file))
}

/// Launch the settings app with arguments (it is single-instance and forwards them).
pub fn open_ui(args: &[&str]) {
    let exe = sibling_exe("fluss-settings");
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0008 | 0x0000_0200); // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        // Let the settings app / rule popup take focus.
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(windows::Win32::UI::WindowsAndMessaging::ASFW_ANY);
        }
    }
    if let Err(e) = cmd.spawn() {
        log::warn!("could not open {}: {e}", exe.display());
    }
}

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub use other::*;

#[cfg(windows)]
mod win {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::os::windows::process::CommandExt;
    use std::sync::OnceLock;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
    use windows::Win32::System::JobObjects::*;
    use windows::Win32::System::LibraryLoader::SetDllDirectoryW;
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Registry::*;
    use windows::Win32::System::Threading::*;
    use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    use windows::Win32::UI::WindowsAndMessaging::*;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}
    static JOB: OnceLock<Option<Job>> = OnceLock::new();

    pub fn init_process() {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        job();
    }

    fn job() -> Option<HANDLE> {
        JOB.get_or_init(|| unsafe {
            let h = CreateJobObjectW(None, PCWSTR::null()).ok()?;
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            Some(Job(h))
        })
        .as_ref()
        .map(|j| j.0)
    }

    /// No console window; optionally start at below-normal priority.
    pub fn hide_child(cmd: &mut std::process::Command, low: bool) {
        let mut flags = CREATE_NO_WINDOW;
        if low {
            flags |= BELOW_NORMAL_PRIORITY_CLASS.0;
        }
        cmd.creation_flags(flags);
    }

    /// Kill the child when Fluss exits, even if Fluss crashes.
    pub fn bind_child_lifetime(child: &std::process::Child) {
        if let Some(j) = job() {
            unsafe {
                let _ = AssignProcessToJobObject(j, HANDLE(child.as_raw_handle()));
            }
        }
    }

    pub fn set_process_priority_normal(child: &std::process::Child) {
        unsafe {
            let _ = SetPriorityClass(HANDLE(child.as_raw_handle()), NORMAL_PRIORITY_CLASS);
        }
    }

    pub fn process_memory_mb(pid: u32) -> Option<u64> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut c = PROCESS_MEMORY_COUNTERS::default();
            let ok = GetProcessMemoryInfo(h, &mut c, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32).is_ok();
            let _ = CloseHandle(h);
            ok.then_some(c.WorkingSetSize as u64 / 1_048_576)
        }
    }

    /// Working set of every child process Fluss started (engine servers and their children).
    pub fn children_memory_mb() -> Option<u64> {
        let j = job()?;
        unsafe {
            #[repr(C)]
            struct List {
                assigned: u32,
                listed: u32,
                ids: [usize; 64],
            }
            let mut l = List { assigned: 0, listed: 0, ids: [0; 64] };
            QueryInformationJobObject(Some(j), JobObjectBasicProcessIdList, &mut l as *mut _ as *mut _, std::mem::size_of::<List>() as u32, None).ok()?;
            let total: u64 = l.ids[..(l.listed as usize).min(64)].iter().filter_map(|pid| process_memory_mb(*pid as u32)).sum();
            Some(total)
        }
    }

    pub fn own_memory_mb() -> Option<u64> {
        process_memory_mb(std::process::id())
    }

    pub fn add_dll_dir(dir: &Path) {
        unsafe {
            let _ = SetDllDirectoryW(&HSTRING::from(dir.as_os_str()));
        }
    }

    /// Lowers this thread (CPU + I/O) while a model loads in the background.
    pub struct ThreadPriorityGuard(bool);

    impl ThreadPriorityGuard {
        pub fn background(enable: bool) -> Self {
            if enable {
                unsafe {
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
                }
            }
            Self(enable)
        }
    }

    impl Drop for ThreadPriorityGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                }
            }
        }
    }

    pub fn raise_thread_priority() {
        unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
        }
    }

    fn exe_name(pid: u32) -> String {
        unsafe {
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
            let mut buf = [0u16; MAX_PATH as usize * 2];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
            let _ = CloseHandle(h);
            if !ok {
                return String::new();
            }
            let p = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
            p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        }
    }

    pub fn focus_ctx() -> FocusCtx {
        unsafe {
            let fg = GetForegroundWindow();
            if fg.0.is_null() {
                return FocusCtx::default();
            }
            let mut pid = 0u32;
            let tid = GetWindowThreadProcessId(fg, Some(&mut pid));
            let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
            let control = if GetGUIThreadInfo(tid, &mut gti).is_ok() { gti.hwndFocus.0 as u64 } else { 0 };
            FocusCtx { window: fg.0 as u64, control, app: exe_name(pid) }
        }
    }

    pub fn foreground_window() -> u64 {
        unsafe { GetForegroundWindow().0 as u64 }
    }

    /// End the process now, skipping DLL teardown. A normal exit runs every
    /// loaded DLL's detach code (ONNX Runtime, DirectML, audio drivers), which
    /// can deadlock when a model is mid-load and leave a zombie that holds the
    /// IPC endpoint so Fluss can't start again. Children die with the job object.
    pub fn hard_exit(code: i32) -> ! {
        log::logger().flush();
        unsafe {
            let _ = TerminateProcess(GetCurrentProcess(), code as u32);
        }
        std::process::exit(code)
    }

    /// Stop other Fluss daemons that hold the IPC endpoint but no longer answer.
    pub fn kill_stale_daemons() {
        let me = std::process::id();
        let mut cmd = std::process::Command::new("taskkill");
        cmd.args(["/F", "/IM", "fluss-daemon.exe", "/FI", &format!("PID ne {me}")])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.status();
    }

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    use fluss_core::install::{RUN_VALUE, UNINSTALL_KEY};

    /// A REG_SZ / REG_EXPAND_SZ value under HKCU, unexpanded.
    fn reg_string(key: &str, value: &str) -> Option<String> {
        unsafe {
            let (k, v) = (HSTRING::from(key), HSTRING::from(value));
            let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
            let mut len = 0u32;
            RegGetValueW(HKEY_CURRENT_USER, &k, &v, flags, None, None, Some(&mut len)).ok().ok()?;
            let mut buf = vec![0u16; (len as usize).div_ceil(2)];
            RegGetValueW(HKEY_CURRENT_USER, &k, &v, flags, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len)).ok().ok()?;
            Some(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string())
        }
    }

    /// (install dir, version) registered by the installer for this user.
    pub fn registered_install() -> Option<(String, String)> {
        let dir = reg_string(UNINSTALL_KEY, "InstallLocation")?.trim_matches('"').to_string();
        let ver = reg_string(UNINSTALL_KEY, "DisplayVersion").unwrap_or_default();
        (!dir.is_empty()).then_some((dir, ver))
    }

    fn autostart_command() -> anyhow::Result<String> {
        Ok(format!("\"{}\" --autostart", std::env::current_exe()?.display()))
    }

    pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
        if crate::lifecycle::isolated() {
            log::info!("autostart: not changed by an isolated (FLUSS_HOME) instance");
            return Ok(());
        }
        unsafe {
            if enabled {
                let wide: Vec<u16> = autostart_command()?.encode_utf16().chain(std::iter::once(0)).collect();
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    &HSTRING::from(RUN_KEY),
                    &HSTRING::from(RUN_VALUE),
                    REG_SZ.0,
                    Some(wide.as_ptr() as *const _),
                    (wide.len() * 2) as u32,
                )
                .ok()?;
            } else {
                let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN_KEY), &HSTRING::from(RUN_VALUE));
            }
        }
        Ok(())
    }

    /// Point the sign-in entry at this executable if it points anywhere else.
    pub fn ensure_autostart() -> anyhow::Result<()> {
        let want = autostart_command()?;
        let have = reg_string(RUN_KEY, RUN_VALUE);
        if have.as_deref() != Some(want.as_str()) {
            log::info!("autostart: {} -> {want}", have.as_deref().unwrap_or("(none)"));
            set_autostart(true)?;
        }
        Ok(())
    }

    /// Start a program that must outlive Fluss (an update installer): detached,
    /// and outside any job object Fluss itself may be running in.
    pub fn spawn_independent(exe: &Path, args: &[&str]) -> std::io::Result<()> {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let run = |flags: u32| {
            std::process::Command::new(exe)
                .args(args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(flags)
                .spawn()
                .map(drop)
        };
        // Breaking away is refused when our job doesn't allow it; plain detached is the fallback.
        run(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB).or_else(|_| run(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP))
    }
}

#[cfg(not(windows))]
mod other {
    use super::*;

    pub fn init_process() {}

    pub fn own_memory_mb() -> Option<u64> {
        None
    }
    pub fn raise_thread_priority() {}
    pub fn focus_ctx() -> FocusCtx {
        FocusCtx::default()
    }
    pub fn foreground_window() -> u64 {
        0
    }
    pub fn hard_exit(code: i32) -> ! {
        log::logger().flush();
        std::process::exit(code)
    }
    pub fn kill_stale_daemons() {}
    pub fn registered_install() -> Option<(String, String)> {
        None
    }
    pub fn ensure_autostart() -> anyhow::Result<()> {
        set_autostart(true)
    }
    pub fn spawn_independent(exe: &Path, args: &[&str]) -> std::io::Result<()> {
        std::process::Command::new(exe).args(args).spawn().map(drop)
    }
    pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
        if crate::lifecycle::isolated() {
            return Ok(());
        }
        let exe = std::env::current_exe()?;
        #[cfg(target_os = "linux")]
        {
            let dir = dirs_autostart();
            let f = dir.join("fluss.desktop");
            if enabled {
                std::fs::create_dir_all(&dir)?;
                std::fs::write(&f, format!("[Desktop Entry]\nType=Application\nName=Fluss\nExec=\"{}\" --autostart\nX-GNOME-Autostart-enabled=true\n", exe.display()))?;
            } else {
                let _ = std::fs::remove_file(f);
            }
        }
        #[cfg(target_os = "macos")]
        {
            let f = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("Library/LaunchAgents/com.mflrevan.fluss.plist");
            if enabled {
                std::fs::write(&f, format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>com.mflrevan.fluss</string><key>ProgramArguments</key><array><string>{}</string><string>--autostart</string></array><key>RunAtLoad</key><true/></dict></plist>"#, exe.display()))?;
            } else {
                let _ = std::fs::remove_file(f);
            }
        }
        let _ = (enabled, exe);
        Ok(())
    }
    #[cfg(target_os = "linux")]
    fn dirs_autostart() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".config"))
            .join("autostart")
    }
}
