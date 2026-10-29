//! Helpers the installer runs (`fluss setup …`, hidden from `--help`). Doing this
//! in Rust instead of NSIS/PowerShell keeps paths with quotes or `%VARS%` intact.

use anyhow::Result;

/// Add or remove this executable's folder on the user's `PATH`.
pub fn path(add: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().map(|d| d.display().to_string()).unwrap_or_default();
    let (current, kind) = imp::read_user_path()?;
    let next = if add { fluss_core::install::path_with(&current, &dir) } else { fluss_core::install::path_without(&current, &dir) };
    match next {
        Some(p) => {
            imp::write_user_path(&p, kind)?;
            println!("{} {dir} {} PATH", if add { "added" } else { "removed" }, if add { "to" } else { "from" });
        }
        None => println!("PATH already {} {dir}", if add { "has" } else { "lacks" }),
    }
    Ok(())
}

#[cfg(windows)]
mod imp {
    use anyhow::{bail, Result};
    use windows_sys::Win32::System::Registry::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The raw (unexpanded) user `PATH` and its registry type.
    pub fn read_user_path() -> Result<(String, u32)> {
        let (key, name) = (wide("Environment"), wide("Path"));
        let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
        unsafe {
            let mut len = 0u32;
            let mut ty = 0u32;
            let r = RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), flags, &mut ty, std::ptr::null_mut(), &mut len);
            if r == 2 {
                return Ok((String::new(), REG_EXPAND_SZ)); // ERROR_FILE_NOT_FOUND: no user PATH yet
            }
            if r != 0 {
                bail!("reading the user PATH failed (error {r})");
            }
            let mut buf = vec![0u16; (len as usize).div_ceil(2)];
            let r = RegGetValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), flags, &mut ty, buf.as_mut_ptr() as *mut _, &mut len);
            if r != 0 {
                bail!("reading the user PATH failed (error {r})");
            }
            Ok((String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string(), ty))
        }
    }

    pub fn write_user_path(p: &str, kind: u32) -> Result<()> {
        let (key, name, val) = (wide("Environment"), wide("Path"), wide(p));
        let kind = if kind == REG_SZ { REG_SZ } else { REG_EXPAND_SZ };
        unsafe {
            let r = RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), kind, val.as_ptr() as *const _, (val.len() * 2) as u32);
            if r != 0 {
                bail!("writing the user PATH failed (error {r})");
            }
            // Tell Explorer, so new terminals see the change without signing out.
            let env = wide("Environment");
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, env.as_ptr() as isize, SMTO_ABORTIFHUNG, 2000, std::ptr::null_mut());
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use anyhow::{bail, Result};

    pub fn read_user_path() -> Result<(String, u32)> {
        bail!("only needed on Windows; add the folder to PATH in your shell profile")
    }

    pub fn write_user_path(_: &str, _: u32) -> Result<()> {
        unreachable!()
    }
}
