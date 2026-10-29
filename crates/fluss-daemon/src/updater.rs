//! Update checks and in-place updates.
//!
//! Checks run in the background twice a day (when `general.check_updates` is on)
//! and on request. Nothing is downloaded or installed unless the user asks:
//! `update.install` downloads the installer, verifies its size and SHA-256
//! against the feed, starts it in passive update mode (`/P /UPDATE`) and quits.
//! The installer stops any leftover processes, replaces the files and starts
//! the new version in the background (see `ui/src-tauri/installer-hooks.nsh`).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use crossbeam_channel::Sender;
use fluss_core::protocol::UpdateStatus;
use fluss_core::update::{self, Manifest};
use sha2::{Digest, Sha256};

use crate::app::Ev;
use crate::ipc_server::Replier;
use crate::lifecycle::State;

const INTERVAL: Duration = Duration::from_secs(12 * 3600);
const RETRY: Duration = Duration::from_secs(3600);
/// After startup, so a sign-in isn't slowed down by network work.
const FIRST_CHECK: Duration = Duration::from_secs(45);

pub enum UpdateEv {
    Checked(Result<Manifest, String>),
    Progress(f32),
    Downloaded(Result<PathBuf, String>),
}

pub struct Updater {
    state: State,
    checking: bool,
    error: Option<String>,
    downloading: Option<f32>,
    next_check: Instant,
    waiters: Vec<Replier>,
    /// Reopen the settings window after installing.
    reopen: bool,
    /// Downloaded installer waiting for the current dictation to end.
    ready: Option<PathBuf>,
    updated_from: Option<String>,
    /// Version already announced during this run.
    announced: Option<String>,
}

pub enum Outcome {
    Nothing,
    /// A newer version showed up (not skipped, not yet announced).
    Available(String),
    /// This version is below the supported minimum.
    Unsupported(Option<String>),
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Updater {
    pub fn new(state: State, updated_from: Option<String>) -> Self {
        // Respect the cached check across restarts instead of hitting the network every launch.
        let age = state.checked_at.map(|t| Duration::from_millis(now_ms().saturating_sub(t)));
        let next_check = match age {
            Some(a) if a < INTERVAL => Instant::now() + (INTERVAL - a).max(FIRST_CHECK),
            _ => Instant::now() + FIRST_CHECK,
        };
        Self { state, checking: false, error: None, downloading: None, next_check, waiters: Vec::new(), reopen: false, ready: None, updated_from, announced: None }
    }

    fn latest(&self) -> Option<&Manifest> {
        self.state.manifest.as_ref()
    }

    pub fn status(&self, auto: bool) -> UpdateStatus {
        let current = fluss_core::VERSION;
        let m = self.latest();
        let available = m.is_some_and(|m| m.is_newer_than(current));
        UpdateStatus {
            current: current.into(),
            latest: m.map(|m| m.version.clone()),
            available,
            required: m.is_some_and(|m| m.unsupported(current)),
            skipped: available && self.state.skipped.as_deref() == m.map(|m| m.version.as_str()),
            message: m.and_then(|m| m.message.clone()).filter(|s| !s.is_empty()),
            notes_url: m.and_then(|m| m.notes.clone()),
            checked_at: self.state.checked_at,
            checking: self.checking,
            downloading: self.downloading,
            error: self.error.clone(),
            auto,
            updated_from: self.updated_from.clone(),
        }
    }

    /// Time for a background check?
    pub fn due(&self, auto: bool) -> bool {
        auto && !self.checking && self.downloading.is_none() && Instant::now() >= self.next_check
    }

    /// Start a check; `reply` (if any) gets the status once it finished.
    pub fn check(&mut self, tx: &Sender<Ev>, reply: Option<Replier>) {
        self.waiters.extend(reply);
        if self.checking {
            return;
        }
        self.checking = true;
        let tx = tx.clone();
        std::thread::Builder::new()
            .name("fluss-update-check".into())
            .spawn(move || {
                let r = fetch().map_err(|e| format!("{e:#}"));
                let _ = tx.send(Ev::Update(UpdateEv::Checked(r)));
            })
            .ok();
    }

    /// A check finished. Returns waiters to answer and what (if anything) to tell the user.
    pub fn on_checked(&mut self, r: Result<Manifest, String>) -> (Vec<Replier>, Outcome) {
        self.checking = false;
        match r {
            Ok(m) => {
                log::info!("update check: latest {} (running {})", m.version, fluss_core::VERSION);
                self.error = None;
                self.state.manifest = Some(m);
                self.state.checked_at = Some(now_ms());
                self.state.save();
                self.next_check = Instant::now() + INTERVAL;
            }
            Err(e) => {
                log::warn!("update check failed: {e}");
                self.error = Some(e);
                self.next_check = Instant::now() + RETRY;
            }
        }
        (std::mem::take(&mut self.waiters), self.outcome())
    }

    fn outcome(&mut self) -> Outcome {
        let st = self.status(true);
        let Some(latest) = st.latest.clone() else { return Outcome::Nothing };
        if self.announced.as_deref() == Some(latest.as_str()) {
            return Outcome::Nothing;
        }
        if st.required {
            self.announced = Some(latest);
            return Outcome::Unsupported(st.message);
        }
        if st.available && !st.skipped {
            self.announced = Some(latest.clone());
            return Outcome::Available(latest);
        }
        Outcome::Nothing
    }

    /// `version: None` clears the skip.
    pub fn skip(&mut self, version: Option<String>) {
        self.state.skipped = version;
        self.state.save();
    }

    pub fn latest_version(&self) -> Option<String> {
        self.latest().map(|m| m.version.clone())
    }

    /// Download and verify the latest installer in the background.
    pub fn install(&mut self, tx: &Sender<Ev>, reopen: bool) -> Result<String, String> {
        if self.downloading.is_some() || self.ready.is_some() {
            return Err("The update is already downloading".into());
        }
        let m = self.latest().cloned().ok_or("No update information yet. Check for updates first.")?;
        if !m.is_newer_than(fluss_core::VERSION) {
            return Err(format!("Fluss {} is up to date", fluss_core::VERSION));
        }
        if m.sha256.is_none() {
            return Err("This update can't be verified. Download it from the website instead.".into());
        }
        if !cfg!(windows) {
            return Err("In-app updates are only available on Windows".into());
        }
        self.reopen = reopen;
        self.downloading = Some(0.0);
        self.error = None;
        let tx = tx.clone();
        let version = m.version.clone();
        std::thread::Builder::new()
            .name("fluss-update-download".into())
            .spawn(move || {
                let progress = |_: &str, p: Option<f32>| {
                    if let Some(p) = p {
                        let _ = tx.send(Ev::Update(UpdateEv::Progress(p)));
                    }
                };
                let r = download(&m, &progress).map_err(|e| format!("{e:#}"));
                let _ = tx.send(Ev::Update(UpdateEv::Downloaded(r)));
            })
            .map_err(|e| e.to_string())?;
        Ok(version)
    }

    pub fn on_progress(&mut self, p: f32) {
        if self.downloading.is_some() {
            self.downloading = Some(p.clamp(0.0, 1.0));
        }
    }

    pub fn on_downloaded(&mut self, r: Result<PathBuf, String>) -> Result<(), String> {
        self.downloading = None;
        match r {
            Ok(p) => {
                self.ready = Some(p);
                Ok(())
            }
            Err(e) => {
                log::error!("update download failed: {e}");
                self.error = Some(e.clone());
                Err(e)
            }
        }
    }

    pub fn set_error(&mut self, e: String) {
        self.error = Some(e);
    }

    /// The verified installer, once nothing is in the way of running it.
    pub fn take_ready(&mut self) -> Option<(PathBuf, bool)> {
        self.ready.take().map(|p| (p, self.reopen))
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(format!("Fluss/{}", fluss_core::VERSION))
        .build()
        .into()
}

/// The feed from the website; GitHub's latest release if the website is unreachable.
fn fetch() -> Result<Manifest> {
    let url = update::manifest_url();
    let primary = (|| -> Result<Manifest> {
        let body = agent().get(&url).call()?.body_mut().read_to_string()?;
        let m: Manifest = serde_json::from_str(&body).context("update feed is not valid JSON")?;
        m.validate()?;
        Ok(m)
    })();
    match primary {
        Ok(m) => Ok(m),
        Err(e) => {
            log::info!("update feed {url}: {e:#}; trying GitHub");
            let body = agent().get(update::GITHUB_LATEST_URL).header("Accept", "application/vnd.github+json").call()?.body_mut().read_to_string()?;
            let v: serde_json::Value = serde_json::from_str(&body)?;
            let m = Manifest::from_github(&v).context("GitHub release has no Windows installer")?;
            m.validate()?;
            Ok(m)
        }
    }
}

fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn download(m: &Manifest, progress: crate::provision::Progress) -> Result<PathBuf> {
    let want = m.sha256.as_deref().context("no checksum")?.to_lowercase();
    let dest = fluss_core::paths::updates_dir().join(format!("Fluss_{}_x64-setup.exe", m.version));
    // A previous download may already be here; trust it only if it verifies.
    for attempt in 0..2 {
        crate::provision::download(&m.url, &dest, "Downloading update", 0.0, 1.0, progress)?;
        let size_ok = m.size.is_none_or(|s| std::fs::metadata(&dest).map(|md| md.len() == s).unwrap_or(false));
        if size_ok && sha256_file(&dest)? == want {
            log::info!("update {} downloaded and verified: {}", m.version, dest.display());
            return Ok(dest);
        }
        let _ = std::fs::remove_file(&dest);
        if attempt == 1 {
            bail!("the downloaded update didn't match its checksum; nothing was installed");
        }
        log::warn!("update {}: checksum mismatch, downloading again", m.version);
    }
    unreachable!()
}

/// Run the installer in passive update mode. It stops Fluss, so the caller quits right after.
pub fn launch_installer(path: &Path, reopen: bool) -> Result<()> {
    let mut args = vec!["/P", "/UPDATE"];
    if reopen {
        args.push("/R"); // the installer starts the settings app when it's done
    }
    crate::platform::spawn_independent(path, &args).with_context(|| format!("starting {}", path.display()))?;
    log::info!("started update installer {} {}", path.display(), args.join(" "));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes() {
        let p = std::env::temp_dir().join(format!("fluss-sha-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        std::fs::remove_file(p).unwrap();
    }

    #[test]
    fn status_logic() {
        let m = |v: &str, min: Option<&str>| Manifest { version: v.into(), url: "https://x".into(), min_supported: min.map(String::from), ..Default::default() };
        let mut u = Updater::new(State::default(), None);
        assert!(!u.status(true).available);
        assert!(matches!(u.on_checked(Ok(m("999.0.0", None))).1, Outcome::Available(_)));
        assert!(u.status(true).available);
        // Announced once per run.
        assert!(matches!(u.on_checked(Ok(m("999.0.0", None))).1, Outcome::Nothing));
        u.skip(Some("999.0.1".into()));
        assert!(matches!(u.on_checked(Ok(m("999.0.1", None))).1, Outcome::Nothing));
        assert!(u.status(true).skipped);
        assert!(matches!(u.on_checked(Ok(m("999.0.2", Some("999.0.0")))).1, Outcome::Unsupported(_)));
        assert!(u.status(true).required);
        let e = u.on_checked(Err("offline".into()));
        assert!(matches!(e.1, Outcome::Nothing));
        assert_eq!(u.status(true).error.as_deref(), Some("offline"));
        assert_eq!(u.status(true).latest.as_deref(), Some("999.0.2"), "a failed check keeps the last result");
    }
}
