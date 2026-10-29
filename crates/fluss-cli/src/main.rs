//! `fluss` — command line control for Fluss.

mod setup;

use std::io::{IsTerminal, Write};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use fluss_core::config::Config;
use fluss_core::ipc::Client;
use fluss_core::protocol::{HistoryItem, InstallProgress, Message, ModelPhase, Status, UpdateStatus};
use fluss_core::rules::{RuleSet, SearchHit};
use serde_json::{json, Value};

const ABOUT: &str = "Fluss — fast, private, local dictation.

Hold your shortcut (default Ctrl+Win), speak, release: text appears where your cursor is.
This CLI controls the Fluss background app: status, settings, your dictionary, models.";

const AFTER: &str = "Examples:
  fluss status                          what Fluss is doing right now
  fluss config set audio.preroll_ms 300 keep 0.3 s of audio from before you press the shortcut
  fluss rules add Omarchy --heard \"on march 9\" --heard \"oh marchy\"
  fluss model use parakeet-v3 && fluss model install
  fluss transcribe meeting.wav

Settings live in the config file (`fluss config path`); every key is listed by `fluss config list`.
Most commands need the background app; `fluss start` launches it (it starts with Windows by default).";

#[derive(Parser)]
#[command(name = "fluss", version, about = ABOUT, after_help = AFTER, propagate_version = true)]
struct Cli {
    /// Machine-readable JSON output (for scripts).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show what Fluss is doing: model, microphone, shortcuts, stats.
    #[command(visible_alias = "st")]
    Status {
        /// Keep watching and print every change (Ctrl+C to stop).
        #[arg(short, long)]
        watch: bool,
    },
    /// Start the Fluss background app.
    Start,
    /// Quit the Fluss background app.
    Stop,
    /// Restart the Fluss background app.
    Restart,
    /// Open the Fluss window (optionally on a page: home, dictionary, shortcuts, model, audio, text, output, advanced).
    Open { page: Option<String> },
    /// Start/stop dictation without the shortcut (types into the focused app).
    #[command(subcommand)]
    Dictate(DictateCmd),
    /// Read and change settings.
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Your dictionary: words and "heard X → write Y" rules.
    #[command(subcommand, visible_alias = "dict")]
    Rules(RulesCmd),
    /// Speech models: list, switch, install, load into memory, unload.
    #[command(subcommand)]
    Model(ModelCmd),
    /// List microphones.
    Devices,
    /// Transcribe a WAV file with the current model and your rules.
    Transcribe {
        /// Path to a .wav file (any sample rate, mono or stereo).
        file: std::path::PathBuf,
        /// Print the raw model output instead of the processed text.
        #[arg(long)]
        raw: bool,
    },
    /// Recent dictations (kept in memory only; never written to disk).
    History {
        /// Forget the history.
        #[arg(long)]
        clear: bool,
        /// How many to show.
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
    /// Show the log file (or follow it).
    Logs {
        /// Keep printing new lines.
        #[arg(short, long)]
        follow: bool,
        /// Only print the path.
        #[arg(long)]
        path: bool,
        /// Number of lines.
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
    /// Start Fluss when you sign in (on/off).
    Autostart { state: OnOff },
    /// Check that everything is set up correctly.
    Doctor,
    /// Check for a new version and install it (Fluss restarts by itself).
    Update {
        /// Only check, don't offer to install.
        #[arg(long)]
        check: bool,
        /// Install the latest version without asking.
        #[arg(long, conflicts_with_all = ["check", "skip"])]
        install: bool,
        /// Stop reminding me about the latest version.
        #[arg(long, conflicts_with = "check")]
        skip: bool,
    },
    /// Remove Fluss from this computer (runs the uninstaller; it asks whether to delete your settings and models).
    Uninstall,
    /// Installer helpers.
    #[command(subcommand, hide = true)]
    Setup(SetupCmd),
    /// Open the quick "add a word" popup (same as the shortcut).
    Popup,
    /// Call any daemon method directly (advanced; see the protocol docs):
    /// `fluss ipc popup.context` or `fluss ipc config.get '{"path":"audio"}'`.
    Ipc {
        method: String,
        /// JSON params.
        params: Option<String>,
    },
}

#[derive(Subcommand)]
enum SetupCmd {
    /// Put this folder on the user's PATH.
    PathAdd,
    /// Take this folder off the user's PATH.
    PathRemove,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Subcommand)]
enum DictateCmd {
    /// Start hands-free dictation.
    Start {
        /// Push-to-talk style instead of hands-free (stops with `fluss dictate stop`).
        #[arg(long)]
        hold: bool,
    },
    /// Finish the current dictation.
    Stop,
    /// Throw the current dictation away.
    Cancel,
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Every setting with its current value.
    List,
    /// Print one setting (`audio.preroll_ms`) or a section (`audio`).
    Get { key: String },
    /// Change a setting: `fluss config set audio.preroll_ms 300`.
    Set { key: String, value: String },
    /// Restore all defaults (your dictionary is kept).
    Reset {
        #[arg(long)]
        yes: bool,
    },
    /// Where the config file is.
    Path,
    /// Open the config file in your editor.
    Edit,
}

#[derive(Subcommand)]
enum RulesCmd {
    /// List rules, optionally filtered by a search.
    #[command(visible_alias = "ls")]
    List { query: Option<String> },
    /// Add a word or a replacement. Without --heard it is a vocabulary word (helps
    /// recognition and fixes its casing).
    Add(AddRule),
    /// Remove a whole rule (by what it writes).
    #[command(visible_alias = "rm")]
    Remove { target: String },
    /// Remove one "heard" variant from a rule.
    RemoveHeard { target: String, heard: String },
    /// Preview how a sentence would come out with your rules and text settings.
    Test { text: String },
    /// Import rules from a JSON file (as written by `export`).
    Import { file: std::path::PathBuf },
    /// Export all rules as JSON (to a file or stdout).
    Export { file: Option<std::path::PathBuf> },
}

#[derive(Args)]
struct AddRule {
    /// What should be written (e.g. "Omarchy").
    target: String,
    /// What the model writes instead (repeatable): --heard "on march 9".
    #[arg(long = "heard", short = 'H')]
    heard: Vec<String>,
}

#[derive(Subcommand)]
enum ModelCmd {
    /// Available models and which one is active.
    #[command(visible_alias = "ls")]
    List,
    /// Switch to a model: phonon-2.5-en, phonon-2.5, parakeet-v3, custom.
    Use { id: String },
    /// Download and set up a model (default: the active one). Shows progress.
    Install { id: Option<String> },
    /// Load the model into memory now.
    Load,
    /// Free the model's memory (it reloads on the next dictation).
    Unload,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{} {e:#}", paint("error:", "31;1"));
        std::process::exit(1);
    }
}

fn color() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn paint(s: &str, code: &str) -> String {
    if color() { format!("\x1b[{code}m{s}\x1b[0m") } else { s.to_string() }
}

fn dim(s: &str) -> String {
    paint(s, "2")
}

fn bold(s: &str) -> String {
    paint(s, "1")
}

fn connect() -> Result<Client> {
    Client::connect().map_err(|_| anyhow!("Fluss is not running. Start it with `fluss start`."))
}

fn daemon_exe() -> std::path::PathBuf {
    let name = if cfg!(windows) { "fluss-daemon.exe" } else { "fluss-daemon" };
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join(name))).filter(|p| p.exists()).unwrap_or_else(|| name.into())
}

fn start_daemon() -> Result<Client> {
    if let Ok(c) = Client::connect() {
        return Ok(c);
    }
    let mut cmd = std::process::Command::new(daemon_exe());
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0000_0008 | 0x0000_0200);
        // The daemon outlives us. If it inherited our stdout/stderr (a pipe when a script
        // or the installer captures output), the reader would wait for it to exit.
        use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT};
        use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
        for h in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            unsafe {
                SetHandleInformation(GetStdHandle(h), HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
    cmd.spawn().with_context(|| format!("starting {}", daemon_exe().display()))?;
    Client::connect_wait(Duration::from_secs(10))
}

/// Ask the daemon to quit and wait until it is gone. Never hangs: a daemon that
/// doesn't answer is reported (the installer runs this before replacing files).
fn stop_daemon() -> Result<bool> {
    let Ok(c) = Client::connect() else { return Ok(false) };
    let _ = c.call_timeout("daemon.quit", Value::Null, Duration::from_secs(3));
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    while std::time::Instant::now() < deadline {
        if Client::connect().is_err() {
            return Ok(true);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    bail!("Fluss did not stop. End `fluss-daemon.exe` in Task Manager, then try again.")
}

/// The background app runs a different version than this command (an update
/// replaced the files but the old process kept running, or a dev build is mixed in).
fn version_skew(st: &Status) -> Option<String> {
    (st.version != fluss_core::VERSION).then(|| format!("the background app is v{} but this command is v{} → fluss restart", st.version, fluss_core::VERSION))
}

fn update_line(u: &UpdateStatus) -> Option<String> {
    let v = u.latest.as_deref()?;
    if u.required {
        Some(paint(&format!("v{v} available; this version is no longer supported → fluss update"), "31"))
    } else if u.available && !u.skipped {
        Some(paint(&format!("v{v} available → fluss update"), "33"))
    } else {
        None
    }
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn run(cli: Cli) -> Result<()> {
    let json_out = cli.json;
    match cli.cmd {
        Cmd::Status { watch } => status(json_out, watch),
        Cmd::Start => {
            let mut c = start_daemon()?;
            let st: Status = serde_json::from_value(c.call("status", Value::Null)?)?;
            println!("Fluss {} is running {}", st.version, dim(&format!("(pid {})", st.pid)));
            Ok(())
        }
        Cmd::Stop => {
            if stop_daemon()? {
                println!("Fluss stopped.");
            } else {
                println!("Fluss is not running.");
            }
            Ok(())
        }
        Cmd::Restart => {
            stop_daemon()?;
            let mut c = start_daemon()?;
            let st: Status = serde_json::from_value(c.call("status", Value::Null)?)?;
            println!("Fluss {} restarted.", st.version);
            Ok(())
        }
        Cmd::Open { page } => {
            let mut c = start_daemon()?;
            c.call("ui.open", json!({"page": page.unwrap_or_default()}))?;
            Ok(())
        }
        Cmd::Popup => {
            let mut c = start_daemon()?;
            c.call("popup.open", Value::Null)?;
            Ok(())
        }
        Cmd::Dictate(d) => {
            let mut c = connect()?;
            match d {
                DictateCmd::Start { hold } => c.call("dictation.start", json!({"hands_free": !hold}))?,
                DictateCmd::Stop => c.call("dictation.stop", Value::Null)?,
                DictateCmd::Cancel => c.call("dictation.cancel", Value::Null)?,
            };
            Ok(())
        }
        Cmd::Config(c) => config(c, json_out),
        Cmd::Rules(r) => rules(r, json_out),
        Cmd::Model(m) => model(m, json_out),
        Cmd::Devices => {
            let mut c = start_daemon()?;
            let v = c.call("devices.list", Value::Null)?;
            if json_out {
                print_json(&v);
                return Ok(());
            }
            let cfg = Config::load().unwrap_or_default();
            for d in v.as_array().cloned().unwrap_or_default() {
                let name = d["name"].as_str().unwrap_or("");
                let mut tags = Vec::new();
                if d["default"].as_bool() == Some(true) {
                    tags.push("system default");
                }
                if name == cfg.audio.device {
                    tags.push("selected");
                }
                println!("  {name} {}", dim(&if tags.is_empty() { String::new() } else { format!("({})", tags.join(", ")) }));
            }
            if cfg.audio.device.is_empty() {
                println!("{}", dim("Using the system default. Choose one with: fluss config set audio.device \"<name>\""));
            }
            Ok(())
        }
        Cmd::Transcribe { file, raw } => {
            let mut c = start_daemon()?;
            let path = std::fs::canonicalize(&file).with_context(|| format!("{} not found", file.display()))?;
            let v = c.call("transcribe.file", json!({"path": path.display().to_string()}))?;
            if json_out {
                print_json(&v);
            } else {
                println!("{}", if raw { v["raw"].as_str().unwrap_or("") } else { v["text"].as_str().unwrap_or("") });
                eprintln!("{}", dim(&format!("decoded in {} ms", v["decode_ms"])));
            }
            Ok(())
        }
        Cmd::History { clear, limit } => {
            let mut c = connect()?;
            if clear {
                c.call("history.clear", Value::Null)?;
                println!("History cleared.");
                return Ok(());
            }
            let v = c.call("history.list", Value::Null)?;
            if json_out {
                print_json(&v);
                return Ok(());
            }
            let items: Vec<HistoryItem> = serde_json::from_value(v)?;
            if items.is_empty() {
                println!("{}", dim("No dictations yet."));
            }
            for h in items.iter().take(limit) {
                println!("{} {}", dim(&format!("{:>5} ms  {:<12}", h.latency_ms, truncate(&h.app, 12))), h.text);
            }
            Ok(())
        }
        Cmd::Logs { follow, path, lines } => {
            let p = fluss_core::paths::daemon_log();
            if path {
                println!("{}", p.display());
                return Ok(());
            }
            let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
            let all: Vec<&str> = text.lines().collect();
            for l in &all[all.len().saturating_sub(lines)..] {
                println!("{l}");
            }
            if follow {
                let mut len = text.len() as u64;
                loop {
                    std::thread::sleep(Duration::from_millis(400));
                    let now = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if now > len {
                        let t = std::fs::read(&p)?;
                        print!("{}", String::from_utf8_lossy(&t[len as usize..]));
                        let _ = std::io::stdout().flush();
                    }
                    len = now;
                }
            }
            Ok(())
        }
        Cmd::Autostart { state } => {
            let on = matches!(state, OnOff::On);
            match Client::connect() {
                Ok(mut c) => {
                    c.call("autostart.set", json!({"enabled": on}))?;
                }
                Err(_) => {
                    let mut cfg = Config::load()?;
                    cfg.general.autostart = on;
                    cfg.save()?;
                }
            }
            println!("Start with Windows: {}", if on { "on" } else { "off" });
            Ok(())
        }
        Cmd::Doctor => doctor(),
        Cmd::Update { check, install, skip } => update(check, install, skip, json_out),
        Cmd::Uninstall => uninstall(),
        Cmd::Setup(SetupCmd::PathAdd) => setup::path(true),
        Cmd::Setup(SetupCmd::PathRemove) => setup::path(false),
        Cmd::Ipc { method, params } => {
            let p: Value = match params {
                Some(s) => serde_json::from_str(&s).context("params must be JSON")?,
                None => Value::Null,
            };
            print_json(&connect()?.call(&method, p)?);
            Ok(())
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}

fn phase_label(p: ModelPhase) -> String {
    match p {
        ModelPhase::Ready => paint("ready", "32"),
        ModelPhase::Loading => paint("loading", "36"),
        ModelPhase::Installing => paint("installing", "36"),
        ModelPhase::Unloaded => paint("sleeping (loads on next dictation)", "33"),
        ModelPhase::NotInstalled => paint("not installed (run `fluss model install`)", "31"),
        ModelPhase::Error => paint("error", "31"),
    }
}

fn print_status(st: &Status) {
    let kind = match st.install.kind.as_str() {
        "dev" => " · dev build",
        "portable" => " · portable",
        _ => "",
    };
    println!("{}  {}", bold("Fluss"), dim(&format!("v{}{kind} · pid {} · up {}", st.version, st.pid, human_secs(st.uptime_s))));
    let state = match st.phase {
        fluss_core::protocol::DictationPhase::Idle => "idle".to_string(),
        fluss_core::protocol::DictationPhase::Listening => paint("listening", "31"),
        fluss_core::protocol::DictationPhase::HandsFree => paint("listening (hands-free)", "33"),
        fluss_core::protocol::DictationPhase::Finishing => paint("typing", "36"),
    };
    println!("  state      {state}");
    let mut m = format!("{} — {}", st.model.name, phase_label(st.model.phase));
    if let Some(p) = st.model.progress {
        m.push_str(&format!(" {:.0}%", p * 100.0));
    }
    if !st.model.detail.is_empty() && st.model.phase != ModelPhase::Ready {
        m.push_str(&dim(&format!(" ({})", st.model.detail)));
    }
    println!("  model      {m}");
    if !st.model.device.is_empty() {
        let mem = st.model.memory_mb.map(|m| format!(" · {m} MB")).unwrap_or_default();
        println!("  runs on    {}{}", st.model.device, mem);
    }
    if let Some(s) = st.model.unload_in_s {
        println!("  unloads in {}", human_secs(s));
    }
    if let Some(e) = &st.model.error {
        println!("  {}  {}", paint("problem", "31"), e.lines().next().unwrap_or(""));
    }
    let mic = if st.mic.open { format!("{} {}", st.mic.device, dim(&format!("({} Hz)", st.mic.sample_rate))) } else { dim("closed (opens when you dictate)") };
    println!("  microphone {mic}");
    if let Some(e) = &st.mic.error {
        println!("  {}  {e}", paint("mic issue", "31"));
    }
    println!(
        "  shortcuts  hold {} · hands-free {} · add word {}{}",
        bold(&st.hotkeys.push_to_talk),
        st.hotkeys.hands_free,
        st.hotkeys.add_rule,
        if st.hotkeys.paused { paint("  (paused)", "33") } else { String::new() }
    );
    let lat = st.stats.median_latency_ms.map(|m| format!(" · median {m} ms after release")).unwrap_or_default();
    println!("  session    {} dictations · {} words{}", st.stats.dictations, st.stats.words, lat);
    if let Some(u) = update_line(&st.update) {
        println!("  update     {u}");
    }
    if let Some(w) = version_skew(st) {
        println!("  {}    {w}", paint("note", "33"));
    }
}

fn human_secs(s: u64) -> String {
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{}h {}m", s / 3600, (s % 3600) / 60)
    }
}

fn status(json_out: bool, watch: bool) -> Result<()> {
    let mut c = connect()?;
    let v = c.call("status", Value::Null)?;
    if json_out && !watch {
        print_json(&v);
        return Ok(());
    }
    let st: Status = serde_json::from_value(v)?;
    print_status(&st);
    if !watch {
        return Ok(());
    }
    c.subscribe()?;
    loop {
        if let Message::Event { event, data } = c.read_message()? {
            match event.as_str() {
                "status" if json_out => println!("{data}"),
                "status" => {
                    let st: Status = serde_json::from_value(data)?;
                    println!();
                    print_status(&st);
                }
                "partial" => println!("{} {}", dim("…"), data["text"].as_str().unwrap_or("")),
                "dictation" => println!("{} {}", paint("✓", "32"), data["text"].as_str().unwrap_or("")),
                "notice" => println!("{} {}", paint("•", "36"), data["text"].as_str().unwrap_or("")),
                _ => {}
            }
        }
    }
}

fn config(c: ConfigCmd, json_out: bool) -> Result<()> {
    let client = Client::connect().ok();
    match c {
        ConfigCmd::List => {
            let cfg = match client {
                Some(mut c) => serde_json::from_value(c.call("config.get", json!({"path": ""}))?)?,
                None => Config::load()?,
            };
            if json_out {
                print_json(&cfg.to_value());
                return Ok(());
            }
            let mut section = String::new();
            for (k, v) in cfg.flatten() {
                let s = k.split('.').next().unwrap_or("").to_string();
                if s != section {
                    println!("{}", bold(&format!("[{s}]")));
                    section = s;
                }
                let val = match &v {
                    Value::String(s) if k.ends_with("api_key") && !s.is_empty() => "\"••••••\"".to_string(),
                    other => other.to_string(),
                };
                println!("  {:<34} {}", k, val);
            }
            Ok(())
        }
        ConfigCmd::Get { key } => {
            let v = match client {
                Some(mut c) => c.call("config.get", json!({"path": key}))?,
                None => Config::load()?.get_path(&key)?,
            };
            match &v {
                Value::String(s) if !json_out => println!("{s}"),
                _ => print_json(&v),
            }
            Ok(())
        }
        ConfigCmd::Set { key, value } => {
            let v = match client {
                Some(mut c) => c.call("config.set", json!({"path": key, "value": value}))?,
                None => {
                    let mut cfg = Config::load()?;
                    cfg.set_path(&key, Value::String(value))?;
                    cfg.save()?;
                    cfg.get_path(&key)?
                }
            };
            println!("{key} = {v}");
            Ok(())
        }
        ConfigCmd::Reset { yes } => {
            if !yes && !confirm("Reset all settings to defaults? Your dictionary is kept.")? {
                return Ok(());
            }
            match client {
                Some(mut c) => {
                    c.call("config.reset", Value::Null)?;
                }
                None => {
                    let old = Config::load().unwrap_or_default();
                    let mut cfg = Config::default();
                    cfg.general.onboarded = old.general.onboarded;
                    cfg.save()?;
                }
            }
            println!("Settings reset.");
            Ok(())
        }
        ConfigCmd::Path => {
            println!("{}", fluss_core::paths::config_file().display());
            Ok(())
        }
        ConfigCmd::Edit => {
            let p = fluss_core::paths::config_file();
            if !p.exists() {
                Config::load()?.save()?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| if cfg!(windows) { "notepad".into() } else { "nano".into() });
            std::process::Command::new(editor).arg(&p).status()?;
            if let Some(mut c) = client {
                // Re-apply the edited file through the daemon (validates it too).
                let cfg = Config::load()?;
                c.call("config.merge", json!({"patch": cfg.to_value()}))?;
            }
            Ok(())
        }
    }
}

fn confirm(q: &str) -> Result<bool> {
    print!("{q} [y/N] ");
    std::io::stdout().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(matches!(s.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn rules(r: RulesCmd, json_out: bool) -> Result<()> {
    let client = Client::connect().ok();
    // Without the daemon, edit the file directly.
    let local = || -> Result<RuleSet> { RuleSet::load() };
    match r {
        RulesCmd::List { query } => {
            let hits: Vec<SearchHit> = match client {
                Some(mut c) => serde_json::from_value(c.call("rules.list", json!({"query": query.clone().unwrap_or_default()}))?)?,
                None => local()?.search(query.as_deref().unwrap_or(""), 500),
            };
            if json_out {
                print_json(&serde_json::to_value(&hits)?);
                return Ok(());
            }
            if hits.is_empty() {
                println!("{}", dim("No rules yet. Add one: fluss rules add Omarchy --heard \"on march 9\" — or press Ctrl+Win+R right after dictating."));
            }
            for h in hits {
                let heard = if h.rule.variants.is_empty() { dim("(vocabulary word)") } else { h.rule.variants.join(dim(", ").as_str()) };
                let uses = if h.rule.uses > 0 { dim(&format!("  ·{}×", h.rule.uses)) } else { String::new() };
                println!("  {:<24} {} {}{}", bold(&h.rule.target), dim("←"), heard, uses);
            }
            Ok(())
        }
        RulesCmd::Add(a) => {
            let out = match client {
                Some(mut c) => c.call("rules.upsert", json!({"target": a.target, "variants": a.heard}))?,
                None => {
                    let mut rs = local()?;
                    let o = rs.upsert(&a.target, &a.heard);
                    rs.save()?;
                    serde_json::to_value(o)?
                }
            };
            if json_out {
                print_json(&out);
                return Ok(());
            }
            let rule = &out["rule"];
            println!(
                "{} {} {} {}",
                if out["created"].as_bool() == Some(true) { paint("added", "32") } else { paint("updated", "36") },
                bold(rule["target"].as_str().unwrap_or("")),
                dim("←"),
                rule["variants"].as_array().map(|v| v.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
            );
            for m in out["moved"].as_array().cloned().unwrap_or_default() {
                println!("  {}", dim(&format!("moved \"{}\" here from \"{}\"", m[0].as_str().unwrap_or(""), m[1].as_str().unwrap_or(""))));
            }
            Ok(())
        }
        RulesCmd::Remove { target } => {
            let ok = match client {
                Some(mut c) => c.call("rules.delete", json!({"target": target}))?.as_bool().unwrap_or(false),
                None => {
                    let mut rs = local()?;
                    let ok = rs.remove(&target);
                    rs.save()?;
                    ok
                }
            };
            if !ok {
                bail!("no rule writes \"{target}\"");
            }
            println!("removed {target}");
            Ok(())
        }
        RulesCmd::RemoveHeard { target, heard } => {
            let ok = match client {
                Some(mut c) => c.call("rules.delete_variant", json!({"target": target, "variant": heard}))?.as_bool().unwrap_or(false),
                None => {
                    let mut rs = local()?;
                    let ok = rs.remove_variant(&target, &heard);
                    rs.save()?;
                    ok
                }
            };
            if !ok {
                bail!("\"{target}\" has no variant \"{heard}\"");
            }
            println!("removed \"{heard}\" from {target}");
            Ok(())
        }
        RulesCmd::Test { text } => {
            let out = match client {
                Some(mut c) => c.call("rules.preview", json!({"text": text}))?,
                None => {
                    let rs = local()?;
                    let cfg = Config::load()?;
                    let tr = fluss_core::text::Transcript::from_text(&text, 1.0);
                    let p = fluss_core::text::process(&tr.words, &cfg.text, Some(&rs.matcher()), fluss_core::text::Context { sentence_start: true, is_final: true });
                    json!({"text": p.text, "rules": p.rules_hit})
                }
            };
            if json_out {
                print_json(&out);
            } else {
                println!("{}", out["text"].as_str().unwrap_or(""));
            }
            Ok(())
        }
        RulesCmd::Import { file } => {
            let set: RuleSet = serde_json::from_str(&std::fs::read_to_string(&file)?).context("not a Fluss rules file")?;
            let n = set.rules.len();
            match client {
                Some(mut c) => {
                    c.call("rules.import", json!({"rules": set}))?;
                }
                None => {
                    let mut rs = local()?;
                    for r in set.rules {
                        rs.upsert(&r.target, &r.variants);
                    }
                    rs.save()?;
                }
            }
            println!("imported {n} rules");
            Ok(())
        }
        RulesCmd::Export { file } => {
            let rs = local()?;
            let s = serde_json::to_string_pretty(&rs)?;
            match file {
                Some(f) => {
                    std::fs::write(&f, s)?;
                    println!("exported {} rules to {}", rs.rules.len(), f.display());
                }
                None => println!("{s}"),
            }
            Ok(())
        }
    }
}

fn model(m: ModelCmd, json_out: bool) -> Result<()> {
    let mut c = start_daemon()?;
    match m {
        ModelCmd::List => {
            let v = c.call("model.catalog", Value::Null)?;
            if json_out {
                print_json(&v);
                return Ok(());
            }
            for m in v.as_array().cloned().unwrap_or_default() {
                let active = m["active"].as_bool() == Some(true);
                let mark = if active { paint("●", "32") } else { " ".into() };
                let inst = if m["engine"] == "custom" {
                    dim("(configure in settings)")
                } else if m["installed"].as_bool() == Some(true) {
                    paint("installed", "32")
                } else {
                    dim(&format!("{} MB download", m["download_mb"]))
                };
                println!("{mark} {:<14} {:<26} {}", bold(m["id"].as_str().unwrap_or("")), m["name"].as_str().unwrap_or(""), inst);
                println!("  {:<14} {}", "", dim(m["tagline"].as_str().unwrap_or("")));
            }
            Ok(())
        }
        ModelCmd::Use { id } => {
            c.call("model.use", json!({"id": id}))?;
            let st: Status = serde_json::from_value(c.call("status", Value::Null)?)?;
            println!("using {} — {}", st.model.name, phase_label(st.model.phase));
            Ok(())
        }
        ModelCmd::Install { id } => {
            let id = id.unwrap_or_default();
            c.subscribe()?;
            c.call("model.install", json!({"id": id}))?;
            let tty = std::io::stderr().is_terminal();
            loop {
                if let Message::Event { event, data } = c.read_message()? {
                    if event != "install" {
                        continue;
                    }
                    let p: InstallProgress = serde_json::from_value(data)?;
                    if json_out {
                        println!("{}", serde_json::to_string(&p)?);
                    } else {
                        let pct = p.progress.map(|x| format!("{:>3.0}%", x * 100.0)).unwrap_or_else(|| "    ".into());
                        if tty {
                            eprint!("\r\x1b[2K  {pct}  {}", p.step);
                        } else {
                            eprintln!("  {pct}  {}", p.step);
                        }
                    }
                    if p.done {
                        if tty && !json_out {
                            eprintln!();
                        }
                        if let Some(e) = p.error {
                            bail!("{e}");
                        }
                        println!("{} {}", paint("✓", "32"), "installed");
                        return Ok(());
                    }
                }
            }
        }
        ModelCmd::Load => {
            c.call("model.load", Value::Null)?;
            c.subscribe()?;
            loop {
                if let Message::Event { event, data } = c.read_message()? {
                    if event == "status" {
                        let st: Status = serde_json::from_value(data)?;
                        match st.model.phase {
                            ModelPhase::Ready => {
                                println!("{} {} on {}", paint("✓", "32"), st.model.name, st.model.device);
                                return Ok(());
                            }
                            ModelPhase::Error | ModelPhase::NotInstalled => bail!("{}", st.model.error.unwrap_or_else(|| "model is not installed".into())),
                            _ => {}
                        }
                    }
                }
            }
        }
        ModelCmd::Unload => {
            c.call("model.unload", Value::Null)?;
            println!("model unloaded");
            Ok(())
        }
    }
}

fn update(check_only: bool, install: bool, skip: bool, json_out: bool) -> Result<()> {
    let mut c = start_daemon()?;
    if skip {
        c.call("update.skip", Value::Null)?;
        println!("Fluss won't remind you about this version. `fluss update` still installs it.");
        return Ok(());
    }
    eprintln!("{}", dim("Checking for updates…"));
    let u: UpdateStatus = serde_json::from_value(c.call("update.check", Value::Null)?)?;
    if json_out {
        print_json(&serde_json::to_value(&u)?);
        return Ok(());
    }
    if let Some(e) = &u.error {
        bail!("couldn't check for updates: {e}");
    }
    let latest = u.latest.clone().unwrap_or_default();
    if !u.available {
        println!("Fluss {} is up to date.", u.current);
        return Ok(());
    }
    println!("{} {} → {}", bold("Update available:"), u.current, paint(&latest, "32"));
    if let Some(m) = &u.message {
        println!("  {m}");
    }
    if let Some(n) = &u.notes_url {
        println!("  {}", dim(&format!("What's new: {n}")));
    }
    if check_only || !(install || confirm("Install it now? Fluss restarts by itself.")?) {
        return Ok(());
    }
    // Watch progress on a second connection; the install call returns right away.
    let mut events = connect()?;
    events.subscribe()?;
    c.call("update.install", Value::Null)?;
    // Events are ordered: the daemon publishes "downloading" right after accepting the
    // install, so an error seen after that belongs to this download (not an older one).
    let mut started = false;
    let mut shown = -1i32;
    loop {
        match events.read_message() {
            Ok(Message::Event { event, data }) if event == "status" => {
                let st: UpdateStatus = serde_json::from_value(data["update"].clone()).unwrap_or_default();
                if let Some(p) = st.downloading {
                    started = true;
                    let pct = (p * 100.0) as i32;
                    if pct / 10 != shown / 10 {
                        shown = pct;
                        eprint!("\r  downloading {pct:>3}%");
                    }
                } else if let Some(e) = st.error.filter(|_| started) {
                    eprintln!();
                    bail!("{e}");
                }
            }
            Ok(Message::Event { event, .. }) if event == "quit" => break,
            Ok(_) => {}
            Err(_) => break, // the daemon quit to let the installer run
        }
    }
    eprintln!();
    println!("Installing Fluss {latest}. It starts again by itself when done.");
    Ok(())
}

fn uninstall() -> Result<()> {
    let dir = std::env::current_exe()?.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let un = dir.join("uninstall.exe");
    if !un.exists() {
        bail!("this copy of Fluss ({}) wasn't set up by the installer, so there is nothing to uninstall. Delete the folder to remove it.", dir.display());
    }
    std::process::Command::new(&un).spawn().with_context(|| format!("starting {}", un.display()))?;
    println!("The uninstaller is open. It asks whether to delete your settings, dictionary and models too.");
    Ok(())
}

fn doctor() -> Result<()> {
    let ok = |s: &str| println!("  {} {s}", paint("✓", "32"));
    let bad = |s: &str| println!("  {} {s}", paint("✗", "31"));
    let warn = |s: &str| println!("  {} {s}", paint("!", "33"));
    println!("{}", bold("Fluss doctor"));
    match Config::load() {
        Ok(_) => ok(&format!("config is valid ({})", fluss_core::paths::config_file().display())),
        Err(e) => bad(&format!("config: {e:#}")),
    }
    match RuleSet::load() {
        Ok(r) => ok(&format!("dictionary: {} rules", r.rules.len())),
        Err(e) => bad(&format!("rules file: {e:#}")),
    }
    match Client::connect() {
        Ok(mut c) => {
            ok("background app is running");
            let st: Status = serde_json::from_value(c.call("status", Value::Null)?)?;
            match version_skew(&st) {
                Some(w) => warn(&w),
                None => ok(&format!("version {} ({})", st.version, if st.install.kind.is_empty() { "unknown install" } else { &st.install.kind })),
            }
            if let (Some(dir), "portable" | "dev") = (&st.install.registered_dir, st.install.kind.as_str()) {
                warn(&format!("running from {} while Fluss {} is installed in {dir}", st.install.dir, st.install.registered_version.as_deref().unwrap_or("")));
            }
            match update_line(&st.update) {
                Some(u) => warn(&u),
                None if st.update.checked_at.is_some() => ok("no updates pending"),
                None => {}
            }
            match st.model.phase {
                ModelPhase::Ready => ok(&format!("{} is loaded on {}", st.model.name, st.model.device)),
                ModelPhase::NotInstalled => bad(&format!("{} is not installed → fluss model install", st.model.name)),
                ModelPhase::Error => bad(&format!("model error: {}", st.model.error.unwrap_or_default())),
                p => warn(&format!("{} is {:?}", st.model.name, p)),
            }
            if st.mic.open {
                ok(&format!("microphone: {}", st.mic.device));
            } else if let Some(e) = st.mic.error {
                bad(&format!("microphone: {e}"));
            } else {
                warn("microphone is closed (it opens when you dictate)");
            }
            let devs = c.call("devices.list", Value::Null)?;
            if devs.as_array().map(|a| a.is_empty()).unwrap_or(true) {
                bad("no microphones found");
            }
            if st.hotkeys.paused {
                warn("shortcuts are paused (tray menu)");
            } else {
                ok(&format!("shortcuts active: hold {}", st.hotkeys.push_to_talk));
            }
        }
        Err(_) => bad("background app is not running → fluss start"),
    }
    Ok(())
}
