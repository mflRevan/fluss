//! Install, upgrade and autostart bookkeeping.
//!
//! * `state.json` remembers the last version that ran, so the first start of a
//!   new version can back up the user's files and say "Updated to …".
//! * [`install_info`] tells an installed copy from a dev build or a portable one.
//! * Autostart is repaired on startup only by the installed copy: a dev build or
//!   an isolated `FLUSS_HOME` instance must never re-point the user's sign-in entry.

use std::path::{Path, PathBuf};

use fluss_core::protocol::InstallInfo;
use fluss_core::update::Manifest;
use fluss_core::version::Version;
use serde::{Deserialize, Serialize};

/// Small app-owned state (not settings: nothing here is user-editable).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub last_version: Option<String>,
    /// Last update feed result and when it was fetched (unix ms).
    pub manifest: Option<Manifest>,
    pub checked_at: Option<u64>,
    /// A version the user chose not to be reminded about.
    pub skipped: Option<String>,
    /// Where it was loaded from; a default (in-memory) state is never written.
    #[serde(skip)]
    file: Option<PathBuf>,
}

impl State {
    pub fn load() -> Self {
        let file = fluss_core::paths::state_file();
        let mut s: Self = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        s.file = Some(file);
        s
    }

    pub fn save(&self) {
        let Some(file) = &self.file else { return };
        let res = serde_json::to_vec_pretty(self).map_err(std::io::Error::other).and_then(|b| fluss_core::paths::write_atomic(file, &b));
        if let Err(e) = res {
            log::warn!("saving state: {e}");
        }
    }
}

/// Run once at startup. Returns the version that ran before, if it differs from this one.
pub fn on_start(state: &mut State) -> Option<String> {
    clean_updates_dir();
    let prev = state.last_version.clone();
    if prev.as_deref() == Some(fluss_core::VERSION) {
        return None;
    }
    state.last_version = Some(fluss_core::VERSION.into());
    state.save();
    let prev = prev?;
    let dir = fluss_core::paths::backups_dir().join(&prev);
    match backup_user_files(&dir) {
        Ok(()) => log::info!("version changed {prev} -> {}; settings backed up to {}", fluss_core::VERSION, dir.display()),
        Err(e) => log::warn!("version changed {prev} -> {}; backup failed: {e}", fluss_core::VERSION),
    }
    prune_backups(5);
    Some(prev)
}

/// Copy `config.toml` and `rules.json` (whatever exists) into `dir`.
fn backup_user_files(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for f in [fluss_core::paths::config_file(), fluss_core::paths::rules_file()] {
        if f.exists() {
            std::fs::copy(&f, dir.join(f.file_name().unwrap_or_default()))?;
        }
    }
    Ok(())
}

/// Keep the `keep` newest backup folders.
fn prune_backups(keep: usize) {
    let Ok(rd) = std::fs::read_dir(fluss_core::paths::backups_dir()) else { return };
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> =
        rd.flatten().filter(|e| e.path().is_dir()).map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path())).collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, d) in dirs.into_iter().skip(keep) {
        let _ = std::fs::remove_dir_all(d);
    }
}

/// Installers for this version or older are done with. One that is still
/// running (it starts us after an update) stays locked; the next start gets it.
fn clean_updates_dir() {
    let Ok(rd) = std::fs::read_dir(fluss_core::paths::updates_dir()) else { return };
    let current = Version::current();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let newer = installer_version(&name).is_some_and(|v| v > current) && name.ends_with(".exe");
        if !newer {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// `Fluss_0.2.0_x64-setup.exe` -> 0.2.0
pub fn installer_version(name: &str) -> Option<Version> {
    Version::parse(name.strip_prefix("Fluss_")?.split('_').next()?)
}

pub fn install_info() -> InstallInfo {
    let dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let reg = crate::platform::registered_install();
    let kind = fluss_core::install::kind_of(&dir, reg.as_ref().map(|r| Path::new(&r.0)));
    InstallInfo {
        kind: kind.into(),
        dir: dir.display().to_string(),
        registered_dir: reg.as_ref().map(|r| r.0.clone()),
        registered_version: reg.map(|r| r.1),
    }
}

/// Isolated test instances (`FLUSS_HOME`) never touch the real autostart entry.
pub fn isolated() -> bool {
    std::env::var_os("FLUSS_HOME").is_some_and(|v| !v.is_empty())
}

/// Make the sign-in entry point at this executable, but only from the installed
/// copy (or a portable one): a dev build would otherwise hijack it.
pub fn repair_autostart(install: &InstallInfo) {
    if isolated() || install.kind == "dev" {
        log::info!("autostart: left alone ({} build)", if isolated() { "isolated" } else { "dev" });
        return;
    }
    if let Err(e) = crate::platform::ensure_autostart() {
        log::warn!("autostart: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_names() {
        assert_eq!(installer_version("Fluss_0.2.0_x64-setup.exe").unwrap().to_string(), "0.2.0");
        assert_eq!(installer_version("Fluss_0.2.0_x64-setup.exe.part").unwrap().to_string(), "0.2.0");
        assert!(installer_version("Fluss-setup.exe").is_none());
        assert!(installer_version("notes.txt").is_none());
    }
}
