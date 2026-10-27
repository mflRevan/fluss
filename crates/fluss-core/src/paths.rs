//! Where Fluss keeps its files.
//!
//! * config (`config.toml`, `rules.json`): `%APPDATA%\Fluss`, `~/.config/fluss`,
//!   `~/Library/Application Support/Fluss`
//! * data (models, runtimes, logs): `%LOCALAPPDATA%\Fluss`, `~/.local/share/fluss`
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

/// Write a file atomically (temp file + rename) so a crash never leaves half a config.
pub fn write_atomic(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}
