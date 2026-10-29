//! User configuration (`config.toml`). Every field has a default, so a partial or
//! old file always loads; unknown keys are ignored. Edits from the CLI and the
//! settings app go through [`Config::set_path`], which validates by round-tripping
//! through serde, so an invalid value is rejected instead of written.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hotkey::Hotkey;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub model: ModelCfg,
    pub custom: CustomEndpoint,
    pub audio: Audio,
    pub hotkeys: Hotkeys,
    pub output: Output,
    pub text: TextCfg,
    pub ui: Ui,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct General {
    /// Start Fluss when you sign in.
    pub autostart: bool,
    /// Seconds to wait after an autostart before warming the model (keeps login snappy).
    pub autostart_delay_s: u32,
    /// Onboarding has been completed.
    pub onboarded: bool,
    /// Short start/stop sounds.
    pub sounds: bool,
    /// Sound volume 0..=100.
    pub sound_volume: u8,
    /// Keep a short in-memory history of recent dictations (never written to disk).
    pub history: bool,
    /// Look for new versions in the background (twice a day). Nothing is installed without asking.
    pub check_updates: bool,
}

impl Default for General {
    fn default() -> Self {
        Self { autostart: true, autostart_delay_s: 4, onboarded: false, sounds: true, sound_volume: 45, history: true, check_updates: true }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Device {
    /// GPU when the engine supports it here, else CPU.
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Residency {
    /// Load once, keep in memory forever (fastest).
    Always,
    /// Unload after `idle_unload_min` minutes without dictation.
    IdleUnload,
    /// Load for each dictation, unload right after (least memory, slow start).
    OnDemand,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    Low,
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelCfg {
    /// Model id from the catalog (`fluss model list`), or `custom`.
    pub id: String,
    pub device: Device,
    pub residency: Residency,
    pub idle_unload_min: u32,
    /// Process priority while the model loads (loading never competes with your apps).
    pub load_priority: Priority,
    /// CPU threads for inference, 0 = automatic.
    pub threads: u32,
    /// Spoken language for multilingual models (`auto` or an ISO code like `de`).
    pub language: String,
    /// Feed your dictionary words to the model as hotwords (when supported).
    pub hotwords: bool,
}

impl Default for ModelCfg {
    fn default() -> Self {
        Self {
            id: "phonon-2.5-en".into(),
            device: Device::Auto,
            residency: Residency::Always,
            idle_unload_min: 20,
            load_priority: Priority::Low,
            threads: 0,
            language: "auto".into(),
            hotwords: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EndpointKind {
    /// `POST {base}/audio/transcriptions` (OpenAI, Groq, Fireworks, whisper.cpp, speaches, `fermion serve`, ...).
    OpenaiTranscriptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CustomEndpoint {
    pub kind: EndpointKind,
    /// e.g. `https://api.openai.com/v1` or `http://127.0.0.1:8000/v1`
    pub base_url: String,
    pub api_key: String,
    /// Model name sent with each request (`whisper-1`, `whisper-large-v3-turbo`, ...). Empty = omit.
    pub model: String,
    /// Re-transcribe while you speak for a live preview (costs extra requests on paid APIs).
    pub live_preview: bool,
    /// Ask for word timestamps (`verbose_json`); disable for servers that reject it.
    pub word_timestamps: bool,
    pub timeout_s: u32,
}

impl Default for CustomEndpoint {
    fn default() -> Self {
        Self {
            kind: EndpointKind::OpenaiTranscriptions,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            live_preview: false,
            word_timestamps: true,
            timeout_s: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Audio {
    /// Input device name; empty = system default.
    pub device: String,
    /// Audio kept from *before* you pressed the shortcut (needs the mic open in the background).
    pub preroll_ms: u32,
    /// Audio still captured *after* you release the shortcut.
    pub postroll_ms: u32,
    /// Keep the microphone stream open between dictations. Required for pre-roll; also
    /// removes the device-open delay. When off, pre-roll is disabled.
    pub keep_mic_open: bool,
    /// A pause this long closes a phrase (it gets finalized and typed).
    pub pause_ms: u32,
    /// Hard cap for one phrase.
    pub max_phrase_s: u32,
    /// How often the live preview refreshes.
    pub preview_interval_ms: u32,
    /// Input gain multiplier (1.0 = unchanged).
    pub gain: f32,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            device: String::new(),
            preroll_ms: 500,
            postroll_ms: 500,
            keep_mic_open: true,
            pause_ms: 700,
            max_phrase_s: 28,
            preview_interval_ms: 450,
            gain: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Hotkeys {
    /// Hold to dictate; release to finish.
    pub push_to_talk: Hotkey,
    /// Start hands-free dictation (also: press while holding push-to-talk). Press push-to-talk to stop.
    pub hands_free: Hotkey,
    /// Add a word/replacement rule, prefilled with what you just said.
    pub add_rule: Hotkey,
    /// Cancel the running dictation (only active while dictating).
    pub cancel: Hotkey,
    /// Double-tap push-to-talk to lock hands-free mode.
    pub double_tap_lock: bool,
    /// Max gap between the two taps.
    pub double_tap_ms: u32,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            push_to_talk: Hotkey::parse("Ctrl+Win").unwrap(),
            hands_free: Hotkey::parse("Ctrl+Win+Space").unwrap(),
            add_rule: Hotkey::parse("Ctrl+Win+R").unwrap(),
            cancel: Hotkey::parse("Escape").unwrap(),
            double_tap_lock: true,
            double_tap_ms: 320,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InsertMethod {
    /// Type characters directly (no clipboard involvement). Best for most apps.
    Type,
    /// Paste through the clipboard (restored afterwards). Best for terminals/remote desktops.
    Paste,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NewlineKey {
    /// Shift+Enter: a line break in chat apps instead of "send".
    ShiftEnter,
    Enter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Output {
    pub method: InsertMethod,
    pub newline: NewlineKey,
    /// Continue the previous dictation when you dictate again into the same text field
    /// without touching anything in between (fixes premature punctuation at the seam).
    pub continuation: bool,
    /// Max seconds between dictations for them to count as one continuous text.
    pub continuation_window_s: u32,
    /// Re-transcribe the seam with context to repair punctuation (more accurate, ~1 extra decode).
    pub seam_repair: bool,
    /// Add a space before a continued dictation.
    pub smart_spacing: bool,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            method: InsertMethod::Type,
            newline: NewlineKey::ShiftEnter,
            continuation: true,
            continuation_window_s: 600,
            seam_repair: true,
            smart_spacing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Casing {
    /// As the model writes it.
    Natural,
    /// all lowercase (dictionary words and ACRONYMS keep their case).
    Lowercase,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TextCfg {
    pub casing: Casing,
    /// "comma", "new line", "question mark", ... become symbols.
    pub spoken_punctuation: bool,
    /// Remove um / uh / erm.
    pub remove_fillers: bool,
    /// Drop the period at the very end of a dictation (chat style).
    pub drop_final_period: bool,
    /// Apply your dictionary (replacement rules).
    pub apply_rules: bool,
}

impl Default for TextCfg {
    fn default() -> Self {
        Self {
            casing: Casing::Natural,
            spoken_punctuation: true,
            remove_fillers: true,
            drop_final_period: false,
            apply_rules: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayPosition {
    Bottom,
    Top,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ui {
    pub theme: Theme,
    pub overlay: bool,
    pub overlay_position: OverlayPosition,
    /// Show the live preview text in the overlay.
    pub overlay_preview: bool,
    /// Keep the settings app resident (hidden) so the rule popup opens instantly (~60 MB).
    pub keep_warm: bool,
    pub reduce_motion: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            overlay: true,
            overlay_position: OverlayPosition::Bottom,
            overlay_preview: true,
            keep_warm: false,
            reduce_motion: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            model: ModelCfg::default(),
            custom: CustomEndpoint::default(),
            audio: Audio::default(),
            hotkeys: Hotkeys::default(),
            output: Output::default(),
            text: TextCfg::default(),
            ui: Ui::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from(&crate::paths::config_file())
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).with_context(|| format!("parsing {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&crate::paths::config_file())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        let body = toml::to_string_pretty(self)?;
        let text = format!("# Fluss configuration. Edit here, in the app, or with `fluss config set`.\n\n{body}");
        crate::paths::write_atomic(path, text.as_bytes())?;
        Ok(())
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("config serializes")
    }

    /// Read one setting by dotted path (`audio.preroll_ms`). Empty path = whole config.
    pub fn get_path(&self, path: &str) -> Result<Value> {
        let v = self.to_value();
        if path.is_empty() {
            return Ok(v);
        }
        v.pointer(&pointer(path)).cloned().ok_or_else(|| anyhow!("unknown setting `{path}`"))
    }

    /// Set one setting by dotted path. Strings are coerced to the field's type
    /// (`"300"` -> 300, `"true"` -> true) so the CLI can pass raw text.
    pub fn set_path(&mut self, path: &str, value: Value) -> Result<()> {
        let mut root = self.to_value();
        let slot = root.pointer_mut(&pointer(path)).ok_or_else(|| anyhow!("unknown setting `{path}`"))?;
        if slot.is_object() {
            bail!("`{path}` is a section; set one of its fields (see `fluss config list`)");
        }
        *slot = coerce(slot, value);
        let next: Config = serde_json::from_value(root).map_err(|e| anyhow!("invalid value for `{path}`: {e}"))?;
        *self = next;
        Ok(())
    }

    /// Merge a partial JSON object into the config (used by the settings app).
    pub fn merge(&mut self, patch: &Value) -> Result<()> {
        let mut root = self.to_value();
        merge_json(&mut root, patch);
        *self = serde_json::from_value(root).map_err(|e| anyhow!("invalid settings: {e}"))?;
        Ok(())
    }

    /// All leaf settings as (`dotted.path`, value), for `fluss config list`.
    pub fn flatten(&self) -> Vec<(String, Value)> {
        let mut out = Vec::new();
        flatten_into(&self.to_value(), String::new(), &mut out);
        out
    }
}

fn pointer(path: &str) -> String {
    format!("/{}", path.trim().replace('.', "/"))
}

fn coerce(current: &Value, value: Value) -> Value {
    let Value::String(s) = &value else { return value };
    let t = s.trim();
    match current {
        Value::Bool(_) => match t.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Value::Bool(true),
            "false" | "off" | "no" | "0" => Value::Bool(false),
            _ => value,
        },
        Value::Number(_) => {
            if let Ok(i) = t.parse::<i64>() {
                Value::from(i)
            } else if let Ok(f) = t.parse::<f64>() {
                Value::from(f)
            } else {
                value
            }
        }
        _ => value,
    }
}

fn merge_json(dst: &mut Value, patch: &Value) {
    match (dst, patch) {
        (Value::Object(d), Value::Object(p)) => {
            for (k, v) in p {
                match d.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge_json(slot, v),
                    _ => {
                        d.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (d, p) => *d = p.clone(),
    }
}

fn flatten_into(v: &Value, prefix: String, out: &mut Vec<(String, Value)>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                let p = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten_into(v, p, out);
            }
        }
        _ => out.push((prefix, v.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_roundtrip_through_toml() {
        let c = Config::default();
        let s = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(c, back);
        let m = if cfg!(target_os = "macos") { "Cmd" } else { "Win" };
        assert!(s.contains(&format!("push_to_talk = \"Ctrl+{m}\"")));
    }

    #[test]
    fn partial_file_loads_with_defaults() {
        let c: Config = toml::from_str("[audio]\npreroll_ms = 250\n").unwrap();
        assert_eq!(c.audio.preroll_ms, 250);
        assert_eq!(c.audio.postroll_ms, 500);
        assert_eq!(c.model.id, "phonon-2.5-en");
    }

    #[test]
    fn set_path_coerces_and_validates() {
        let mut c = Config::default();
        c.set_path("audio.preroll_ms", Value::String("300".into())).unwrap();
        assert_eq!(c.audio.preroll_ms, 300);
        c.set_path("text.remove_fillers", Value::String("off".into())).unwrap();
        assert!(!c.text.remove_fillers);
        c.set_path("hotkeys.push_to_talk", Value::String("ctrl+shift+space".into())).unwrap();
        assert_eq!(c.hotkeys.push_to_talk.to_string(), "Ctrl+Shift+Space");
        assert!(c.set_path("model.residency", Value::String("sometimes".into())).is_err());
        assert!(c.set_path("hotkeys.push_to_talk", Value::String("Ctrl+Nope".into())).is_err());
        assert!(c.set_path("audio.nope", Value::from(1)).is_err());
        assert!(c.set_path("audio", Value::from(1)).is_err());
    }

    #[test]
    fn merge_patch() {
        let mut c = Config::default();
        c.merge(&serde_json::json!({"ui": {"theme": "dark"}, "model": {"idle_unload_min": 5}})).unwrap();
        assert_eq!(c.ui.theme, Theme::Dark);
        assert_eq!(c.model.idle_unload_min, 5);
        assert!(c.ui.overlay);
    }
}
