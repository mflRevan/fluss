//! Daemon <-> client protocol: newline-delimited JSON over a local socket
//! (named pipe on Windows). Clients send [`Request`]s; the daemon answers with
//! [`Message::Response`] and, after `subscribe`, pushes [`Message::Event`]s.
//!
//! Methods (params in braces):
//! - `status`                          -> [`Status`]
//! - `subscribe {levels?}`             -> ok, then events: `status` ([`Status`]), `install` ([`InstallProgress`]),
//!   `partial` ({text}), `dictation` ([`HistoryItem`]), `notice` ({kind, text}), `config` (full config),
//!   `rules` ({count}), `level` ({rms, peak}, ~25 Hz, only with `levels: true`), `popup` ([`PopupContext`])
//! - `hotkeys.record {}`               -> {hotkey} once the user presses and releases a combination (10 s timeout);
//!   daemon hotkeys are suspended meanwhile. `hotkeys.pause {paused}` suspends them explicitly.
//! - `endpoint.test {}`                -> {ok, latency_ms, text} using the custom endpoint settings
//! - `config.get {path?}` / `config.set {path, value}` / `config.merge {patch}` / `config.reset`
//! - `rules.list {query?, limit?}` / `rules.upsert {target, variants, apply_last?}` /
//!   `rules.replace {old_target, rule}` / `rules.delete {target}` / `rules.delete_variant {target, variant}` /
//!   `rules.preview {text}`
//! - `model.catalog` / `model.use {id}` / `model.install {id?}` / `model.load` / `model.unload`
//! - `devices.list`
//! - `dictation.start {hands_free?}` / `dictation.stop` / `dictation.cancel`
//! - `transcribe.file {path}`
//! - `history.list` / `history.clear`
//! - `popup.context` (rule popup prefill) / `popup.closed`
//! - `autostart.set {enabled}`
//! - `update.status` -> [`UpdateStatus`] / `update.check` -> [`UpdateStatus`] once the check finished /
//!   `update.install {reopen?}` (download, verify, run the installer, quit) / `update.skip {version?}`
//!   (`version: null` un-skips). Progress and results arrive as `status` events (`Status.update`).
//! - `ui.open {page?}` / `daemon.quit` / `logs.path`

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chunk::ChunkGuess;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    Response {
        id: u64,
        #[serde(default)]
        result: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Event {
        event: String,
        #[serde(default)]
        data: Value,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DictationPhase {
    #[default]
    Idle,
    /// Holding push-to-talk.
    Listening,
    /// Hands-free (locked) dictation.
    HandsFree,
    /// Released; post-roll + last phrase being transcribed.
    Finishing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ModelPhase {
    NotInstalled,
    Installing,
    #[default]
    Unloaded,
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelStatus {
    pub id: String,
    pub name: String,
    pub phase: ModelPhase,
    /// 0..1 while installing/loading when known.
    #[serde(default)]
    pub progress: Option<f32>,
    /// Human-readable step ("Downloading model", "Warming up", ...).
    #[serde(default)]
    pub detail: String,
    /// Where it runs: "GPU (DirectML)", "CPU", "Remote".
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub memory_mb: Option<u64>,
    /// Seconds until the idle timeout unloads the model (when enabled).
    #[serde(default)]
    pub unload_in_s: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MicStatus {
    pub device: String,
    pub open: bool,
    pub sample_rate: u32,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Stats {
    pub dictations: u64,
    pub words: u64,
    pub audio_seconds: f64,
    /// Median release-to-text latency of recent dictations.
    pub median_latency_ms: Option<u32>,
    pub last_latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Status {
    pub version: String,
    pub pid: u32,
    pub uptime_s: u64,
    pub phase: DictationPhase,
    pub model: ModelStatus,
    pub mic: MicStatus,
    pub stats: Stats,
    pub onboarded: bool,
    pub config_path: String,
    pub data_path: String,
    pub hotkeys: HotkeyStatus,
    #[serde(default)]
    pub install: InstallInfo,
    #[serde(default)]
    pub update: UpdateStatus,
}

/// Where the running daemon came from.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct InstallInfo {
    /// `installed` (by the installer), `dev` (a cargo build) or `portable`.
    pub kind: String,
    /// Directory of the running executable.
    pub dir: String,
    /// What the installer registered, if Fluss is installed for this user.
    #[serde(default)]
    pub registered_dir: Option<String>,
    #[serde(default)]
    pub registered_version: Option<String>,
}

/// Update check and download state.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UpdateStatus {
    pub current: String,
    /// Newest released version, once known.
    #[serde(default)]
    pub latest: Option<String>,
    /// `latest` is newer than `current`.
    pub available: bool,
    /// This version is below the supported minimum: updating is strongly recommended.
    pub required: bool,
    /// The user chose to skip `latest` (no reminders; still listed on the About page).
    pub skipped: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub notes_url: Option<String>,
    /// Unix ms of the last successful check.
    #[serde(default)]
    pub checked_at: Option<u64>,
    pub checking: bool,
    /// Download progress 0..1 while an update downloads.
    #[serde(default)]
    pub downloading: Option<f32>,
    #[serde(default)]
    pub error: Option<String>,
    /// Automatic checks are on.
    pub auto: bool,
    /// This run is the first after an update from that version.
    #[serde(default)]
    pub updated_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct HotkeyStatus {
    pub push_to_talk: String,
    pub hands_free: String,
    pub add_rule: String,
    /// Hotkeys are paused (e.g. while the settings app records a new shortcut).
    pub paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryItem {
    pub id: u64,
    /// Unix milliseconds.
    pub at: u64,
    pub text: String,
    pub raw: String,
    pub app: String,
    pub duration_s: f32,
    pub latency_ms: u32,
}

/// Everything the rule popup needs to open pre-filled.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PopupContext {
    pub chunk: ChunkGuess,
    /// Window to return focus to when the popup closes (Windows HWND as integer).
    pub return_hwnd: u64,
    /// The popup was opened mid-dictation / right after one (vs. from scratch).
    pub from_dictation: bool,
    /// The last dictation can still be fixed in place if a rule is saved.
    pub can_apply_last: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstallProgress {
    pub id: String,
    pub step: String,
    pub progress: Option<f32>,
    pub done: bool,
    pub error: Option<String>,
}
