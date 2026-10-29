//! Where Fluss keeps its files.
//!
//! * config (`config.toml`, `rules.json`): `%APPDATA%\Fluss`, `~/.config/fluss`,
//!   `~/Library/Application Support/Fluss`
//! * data (models, runtimes, logs, update downloads, backups, `state.json`):
//!   `%LOCALAPPDATA%\Fluss`, `~/.local/share/fluss`
//!
//! `FLUSS_HOME` overrides both (handy for tests and portable installs).

use std::path::PathBuf;

fn home_override() -> Option<PathBuf> {
    std::env::var_os("FLUSS_HOME").filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn app_dir(base: Option<PathBuf>) -> PathBuf {
    let name = if cfg!(any(windows, target_os = "macos")) { "Fluss" } else { "fluss" };
    base.unwrap_or_else(|| PathBuf::from(".")).join(name)
}

pub fn config_dir() -> PathBuf {
    home_override().unwrap_or_else(|| app_dir(dirs::config_dir()))
}

pub fn data_dir() -> PathBuf {
    home_override().unwrap_or_else(|| app_dir(dirs::data_local_dir()))
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn rules_file() -> PathBuf {
    config_dir().join("rules.json")
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

/// Managed Python runtime for the Phonon engine (created with `uv`).
pub fn runtime_dir() -> PathBuf {
    data_dir().join("runtime")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn daemon_log() -> PathBuf {
    logs_dir().join("fluss-daemon.log")
}

/// Small bookkeeping the app keeps for itself (last run version, update check cache).
pub fn state_file() -> PathBuf {
    data_dir().join("state.json")
}

/// Downloaded update installers (removed once they are no longer needed).
pub fn updates_dir() -> PathBuf {
    data_dir().join("updates")
}

/// Copies of `config.toml` and `rules.json` taken before a version change.
pub fn backups_dir() -> PathBuf {
    data_dir().join("backups")
}

/// Name of the local IPC endpoint (a named pipe on Windows, a socket elsewhere).
/// Scoped by `FLUSS_HOME` so a test instance never talks to the real daemon.
pub fn ipc_name() -> String {
    match home_override() {
        Some(h) => {
            let mut hash: u32 = 2166136261;
            for b in h.to_string_lossy().bytes() {
                hash = (hash ^ b as u32).wrapping_mul(16777619);
            }
            format!("fluss-ipc-{hash:08x}")
        }
        None => "fluss-ipc".to_string(),
    }
}

/// Write a file atomically (flushed temp file + rename) so a crash or power loss
/// never leaves half a config. The temp name is per process, so the CLI and the
/// daemon writing at the same moment can't clobber each other's temp file.
pub fn write_atomic(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Move an unreadable file aside (`config.toml` -> `config.toml.broken-<unix secs>`)
/// so defaults can be used without the next save destroying what the user had.
pub fn quarantine(path: &std::path::Path) -> std::io::Result<PathBuf> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let to = path.with_file_name(format!("{name}.broken-{secs}"));
    std::fs::rename(path, &to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_and_quarantine() {
        let dir = std::env::temp_dir().join(format!("fluss-paths-{}", std::process::id()));
        let f = dir.join("config.toml");
        write_atomic(&f, b"a = 1").unwrap();
        write_atomic(&f, b"a = 2").unwrap();
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "a = 2");
        let moved = quarantine(&f).unwrap();
        assert!(!f.exists() && moved.exists());
        assert!(moved.file_name().unwrap().to_string_lossy().starts_with("config.toml.broken-"));
        // No temp files left behind.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
