//! The controller: one thread that owns all dictation state. Every other
//! thread (hooks, audio, engine, injector, IPC, tray) talks to it through one
//! channel, so there are no locks on the hot path and nothing here blocks.
//!
//! Life of a dictation:
//! 1. push-to-talk down -> session starts with the pre-roll ring buffer,
//!    model load is triggered if needed (audio keeps buffering meanwhile)
//! 2. audio is segmented into phrases by pauses; the open phrase gets live
//!    previews (shown in the overlay); a closed phrase gets a final decode,
//!    jointly with the previous phrase's tail when continuing (seam repair)
//! 3. finals are post-processed and typed strictly in order
//! 4. release -> post-roll keeps listening; a phrase that already ended in
//!    silence is finalized immediately (often from a preview that already
//!    covered it, so the text appears with ~zero extra decode time)

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use fluss_core::chunk::{self, ChunkLimits};
use fluss_core::config::{Config, Residency};
use fluss_core::continuation::{repair_heuristic, repair_with_context, split_joint, Seam};
use fluss_core::models::{self, EngineKind};
use fluss_core::protocol::*;
use fluss_core::rules::{Matcher, Rule, RuleSet};
use fluss_core::text::{self, Context, Transcript, Word};
use serde_json::{json, Value};

use crate::audio::{AudioCmd, AudioEv};
use crate::dsp::{self, Ring, Vad, BLOCK, SR};
use crate::engine::{self, DecodeOpts, EngineCmd, EngineEv, EngineSpec, Job, JobDone, JobKind};
use crate::hotkeys::{self, HotkeyEv};
use crate::inject::{Edit, InjectEv};
use crate::ipc_server::{Hub, IpcCall, Replier};
use crate::overlay::{OvState, Overlay};
use crate::platform::{self, FocusCtx};
use crate::sounds::{self, Sound};
use crate::tray::TrayCmd;

pub enum Ev {
    Hotkey(HotkeyEv),
    Audio(AudioEv),
    Engine(EngineEv),
    Injected(InjectEv),
    Ipc(IpcCall),
    Install(InstallProgress),
    Tray(TrayCmd),
}

const S: fn(f32) -> usize = |secs| (secs * SR as f32) as usize;
const TAP_MAX: Duration = Duration::from_millis(260);
const RELEASE_PAUSE_MS: u32 = 220;
const SEAM_TAIL_S: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Hold,
    HandsFree,
}

#[derive(Debug, Clone, Default)]
struct Open {
    start: usize,
    speech_start: Option<usize>,
    speech_end: usize,
    voiced_blocks: usize,
    silence: usize,
}

struct Phrase {
    end: usize,
    speech_start: usize,
    speech_end: usize,
    /// Absolute sample where the decoded audio (excluding seam context) begins.
    decode_start: usize,
    boundary_s: Option<f32>,
    result: Option<Result<Transcript, String>>,
}

struct Session {
    id: u64,
    mode: Mode,
    started: Instant,
    /// Session audio; `audio[0]` is absolute sample `base`.
    audio: Vec<f32>,
    base: usize,
    vad: Vad,
    block_pos: usize,
    open: Open,
    phrases: Vec<Phrase>,
    emitted: usize,
    preview_inflight: bool,
    preview_from: usize,
    preview_end: usize,
    last_preview: Option<((usize, usize), Transcript)>,
    preview_text: String,
    capturing: bool,
    released: Option<Instant>,
    stop_at: Option<Instant>,
    tap_since: Option<Instant>,
    ignore_next_up: bool,
    add_rule_after: bool,
    cancelled: bool,
    pending_edits: usize,
    last_done: Option<Instant>,
    text: String,
    raw: String,
    app: String,
}

impl Session {
    fn pos(&self) -> usize {
        self.base + self.audio.len()
    }
    fn slice(&self, a: usize, b: usize) -> Vec<f32> {
        let a = a.max(self.base).min(self.pos());
        let b = b.max(a).min(self.pos());
        self.audio[a - self.base..b - self.base].to_vec()
    }
}

/// What was typed last, for continuation and in-place rule fixes.
struct LastInsert {
    ctx: FocusCtx,
    epoch: u64,
    at: Instant,
    auto_tail: String,
    ended_sentence: bool,
    tail_audio: Vec<f32>,
    words: Vec<Word>,
    env: Vec<f32>,
    text: String,
    flags: Context,
}

pub struct App {
    cfg: Config,
    rules: RuleSet,
    matcher: Matcher,
    tx: Sender<Ev>,
    rx: Receiver<Ev>,
    audio: Sender<AudioCmd>,
    engine: Sender<EngineCmd>,
    inject: Sender<Edit>,
    hub: Arc<Hub>,
    overlay: Overlay,
    tray: crate::tray::TrayHandle,
    started: Instant,
    model: ModelStatus,
    mic: MicStatus,
    mic_open: bool,
    ring: Ring,
    vad_mem: Vad,
    session: Option<Session>,
    next_id: u64,
    last: Option<LastInsert>,
    history: VecDeque<HistoryItem>,
    stats: Stats,
    latencies: VecDeque<u32>,
    last_used: Instant,
    load_at: Option<Instant>,
    unload_at: Option<Instant>,
    installing: Option<String>,
    popup_reply: Vec<Replier>,
    popup_return: u64,
    apply_after_popup: bool,
    apply_at: Option<Instant>,
    record: Option<(Replier, Instant)>,
    file_jobs: HashMap<u64, Replier>,
    edit_tags: HashMap<u64, u64>,
    level_acc: (f32, f32, usize),
    last_level: Instant,
    mic_close_at: Option<Instant>,
    quit: bool,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub struct Handles {
    pub audio: Sender<AudioCmd>,
    pub engine: Sender<EngineCmd>,
    pub inject: Sender<Edit>,
    pub hub: Arc<Hub>,
    pub overlay: Overlay,
    pub tray: crate::tray::TrayHandle,
}

impl App {
    pub fn new(tx: Sender<Ev>, rx: Receiver<Ev>, h: Handles, autostart: bool) -> Self {
        let cfg = Config::load().unwrap_or_else(|e| {
            log::error!("config: {e:#}; using defaults");
            Config::default()
        });
        let rules = RuleSet::load().unwrap_or_else(|e| {
            log::error!("rules: {e:#}");
            RuleSet::default()
        });
        let matcher = rules.matcher();
        let ring = Ring::new(S(cfg.audio.preroll_ms as f32 / 1000.0).max(1));
        let mut app = Self {
            matcher,
            rules,
            tx,
            rx,
            audio: h.audio,
            engine: h.engine,
            inject: h.inject,
            hub: h.hub,
            overlay: h.overlay,
            tray: h.tray,
            started: Instant::now(),
            model: ModelStatus::default(),
            mic: MicStatus::default(),
            mic_open: false,
            ring,
            vad_mem: Vad::default(),
            session: None,
            next_id: 1,
            last: None,
            history: VecDeque::new(),
            stats: Stats::default(),
            latencies: VecDeque::new(),
            last_used: Instant::now(),
            load_at: None,
            unload_at: None,
            installing: None,
            popup_reply: Vec::new(),
            popup_return: 0,
            apply_after_popup: false,
            apply_at: None,
            record: None,
            file_jobs: HashMap::new(),
            edit_tags: HashMap::new(),
            level_acc: (0.0, 0.0, 0),
            last_level: Instant::now(),
            mic_close_at: None,
            quit: false,
            cfg,
        };
        app.model = ModelStatus { id: app.cfg.model.id.clone(), name: EngineSpec::from_config(&app.cfg).name(), ..Default::default() };
        hotkeys::set_bindings(&app.cfg.hotkeys);
        app.overlay.configure(&app.cfg.ui);
        let _ = app.engine.send(EngineCmd::Configure(EngineSpec::from_config(&app.cfg)));
        if app.cfg.general.autostart {
            if let Err(e) = platform::set_autostart(true) {
                log::warn!("autostart: {e:#}");
            }
        }
        if app.cfg.general.onboarded {
            if app.cfg.audio.keep_mic_open {
                app.open_mic();
            }
            if app.cfg.model.residency != Residency::OnDemand {
                let delay = if autostart { app.cfg.general.autostart_delay_s as u64 * 1000 } else { 300 };
                app.load_at = Some(Instant::now() + Duration::from_millis(delay));
            }
        }
        if app.cfg.ui.keep_warm {
            platform::open_ui(&["--hidden"]);
        }
        app
    }

    pub fn run(mut self) {
        while !self.quit {
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(ev) => self.handle(ev),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(_) => break,
            }
            // Drain bursts before ticking.
            while let Ok(ev) = self.rx.try_recv() {
                self.handle(ev);
                if self.quit {
                    break;
                }
            }
            self.tick();
        }
        log::info!("shutting down");
        let _ = self.engine.send(EngineCmd::Shutdown);
        self.overlay.hide();
    }

    fn handle(&mut self, ev: Ev) {
        match ev {
            Ev::Hotkey(h) => self.on_hotkey(h),
            Ev::Audio(a) => self.on_audio(a),
            Ev::Engine(e) => self.on_engine(e),
            Ev::Injected(InjectEv::Done { tag, at }) => self.on_injected(tag, at),
            Ev::Ipc(call) => self.on_ipc(call),
            Ev::Install(p) => self.on_install(p),
            Ev::Tray(t) => self.on_tray(t),
        }
    }

    // ------------------------------------------------------------- status

    fn status(&self) -> Status {
        let phase = match &self.session {
            None => DictationPhase::Idle,
            Some(s) if !s.capturing || s.released.is_some() => DictationPhase::Finishing,
            Some(s) if s.mode == Mode::HandsFree => DictationPhase::HandsFree,
            Some(_) => DictationPhase::Listening,
        };
        let mut model = self.model.clone();
        if model.phase == ModelPhase::Ready && self.cfg.model.residency == Residency::IdleUnload && self.session.is_none() {
            let idle = self.last_used.elapsed().as_secs();
            model.unload_in_s = Some((self.cfg.model.idle_unload_min as u64 * 60).saturating_sub(idle));
        }
        let mut stats = self.stats.clone();
        if !self.latencies.is_empty() {
            let mut v: Vec<u32> = self.latencies.iter().copied().collect();
            v.sort_unstable();
            stats.median_latency_ms = Some(v[v.len() / 2]);
        }
        Status {
            version: fluss_core::VERSION.into(),
            pid: std::process::id(),
            uptime_s: self.started.elapsed().as_secs(),
            phase,
            model,
            mic: self.mic.clone(),
            stats,
            onboarded: self.cfg.general.onboarded,
            config_path: fluss_core::paths::config_file().display().to_string(),
            data_path: fluss_core::paths::data_dir().display().to_string(),
            hotkeys: HotkeyStatus {
                push_to_talk: self.cfg.hotkeys.push_to_talk.to_string(),
                hands_free: self.cfg.hotkeys.hands_free.to_string(),
                add_rule: self.cfg.hotkeys.add_rule.to_string(),
                paused: hotkeys::PAUSED.load(Ordering::Relaxed),
            },
        }
    }

    fn publish(&mut self) {
        let st = self.status();
        self.tray.update(&st);
        self.hub.broadcast("status", serde_json::to_value(&st).unwrap_or_default());
    }

    fn notice(&self, kind: &str, text: &str) {
        self.hub.broadcast("notice", json!({"kind": kind, "text": text}));
    }

    // ------------------------------------------------------------- mic

    fn open_mic(&mut self) {
        if !self.mic_open {
            self.mic_open = true;
            let _ = self.audio.send(AudioCmd::Open { device: self.cfg.audio.device.clone(), gain: self.cfg.audio.gain });
        }
        self.mic_close_at = None;
    }

    fn close_mic(&mut self) {
        if self.mic_open {
            self.mic_open = false;
            self.ring.clear();
            let _ = self.audio.send(AudioCmd::Close);
        }
    }

    fn on_audio(&mut self, a: AudioEv) {
        match a {
            AudioEv::Chunk(c) => self.on_chunk(c),
            AudioEv::Opened { device, sample_rate } => {
                self.mic = MicStatus { device, open: true, sample_rate, error: None };
                if !self.mic_open {
                    self.mic_open = true;
                }
                self.publish();
            }
            AudioEv::Closed => {
                self.mic.open = false;
                self.publish();
            }
            AudioEv::Error(e) => {
                self.mic.error = Some(e.clone());
                self.mic.open = false;
                self.publish();
                if self.session.is_some() {
                    self.overlay.notice(&format!("Microphone problem: {e}"), true);
                }
            }
        }
    }

    fn on_chunk(&mut self, c: Vec<f32>) {
        // Meters (overlay + subscribed UIs).
        let sum: f32 = c.iter().map(|x| x * x).sum();
        let peak = c.iter().fold(0f32, |m, x| m.max(x.abs()));
        self.level_acc.0 += sum;
        self.level_acc.1 = self.level_acc.1.max(peak);
        self.level_acc.2 += c.len();
        let rms_now = (sum / c.len().max(1) as f32).sqrt();
        self.overlay.level.set(rms_now);
        if self.last_level.elapsed() >= Duration::from_millis(40) && self.level_acc.2 > 0 {
            if self.hub.wants_levels() {
                let rms = (self.level_acc.0 / self.level_acc.2 as f32).sqrt();
                self.hub.broadcast_levels(json!({"rms": rms, "peak": self.level_acc.1}));
            }
            self.level_acc = (0.0, 0.0, 0);
            self.last_level = Instant::now();
        }

        let Some(s) = self.session.as_mut() else {
            self.ring.push(&c);
            return;
        };
        if !s.capturing || s.cancelled {
            self.ring.push(&c);
            return;
        }
        s.audio.extend_from_slice(&c);
        self.segment();
    }

    // ------------------------------------------------------------- segmentation

    fn segment(&mut self) {
        let pause_ms = {
            let s = self.session.as_ref().unwrap();
            if s.released.is_some() { RELEASE_PAUSE_MS.min(self.cfg.audio.pause_ms) } else { self.cfg.audio.pause_ms }
        };
        let pause = S(pause_ms as f32 / 1000.0);
        let max_phrase = S(self.cfg.audio.max_phrase_s.max(5) as f32);
        let mut closes = Vec::new();
        {
            let s = self.session.as_mut().unwrap();
            let mut blocks = Vec::new();
            let from = s.block_pos;
            let avail = s.pos() - from;
            let n = avail / BLOCK * BLOCK;
            if n == 0 {
                return;
            }
            let pcm = s.slice(from, from + n);
            s.vad.feed(&pcm, &mut blocks);
            for (i, b) in blocks.iter().enumerate() {
                let bs = from + i * BLOCK;
                let be = bs + BLOCK;
                let o = &mut s.open;
                if b.voiced {
                    if o.speech_start.is_none() {
                        o.speech_start = Some(bs);
                    }
                    o.speech_end = be;
                    o.voiced_blocks += 1;
                    o.silence = 0;
                } else {
                    o.silence += BLOCK;
                    if o.speech_start.is_none() && be - o.start > S(0.45) {
                        // Keep only a little lead-in before speech starts.
                        o.start = be - S(0.4);
                    }
                }
                let voiced = o.voiced_blocks >= 2;
                if (voiced && o.silence >= pause) || be - o.start >= max_phrase {
                    let closed = std::mem::replace(&mut s.open, Open { start: be, ..Default::default() });
                    if closed.voiced_blocks >= 2 {
                        closes.push((closed, be));
                    }
                }
            }
            s.block_pos = from + n;
        }
        for (o, end) in closes {
            self.finalize(o, end);
        }
        self.maybe_preview();
        self.trim_session_audio();
    }

    fn flush_open(&mut self, force: bool) {
        let Some(s) = self.session.as_mut() else { return };
        let o = std::mem::take(&mut s.open);
        let pos = s.pos();
        s.open = Open { start: pos, ..Default::default() };
        if o.voiced_blocks >= 2 || (force && o.voiced_blocks >= 1) {
            self.finalize(o, pos);
        }
    }

    fn finalize(&mut self, o: Open, end: usize) {
        let joint = self.cfg.output.continuation && self.cfg.output.seam_repair && self.engine_is_local();
        let cont_ok = self.continuation_valid(&platform::focus_ctx());
        let s = self.session.as_mut().unwrap();
        let speech_start = o.speech_start.unwrap_or(o.start);
        let speech_end = o.speech_end.max(speech_start + 1).min(end);
        let decode_start = o.start.max(speech_start.saturating_sub(S(0.3)));
        let decode_end = end.min(speech_end + S(0.35));
        let idx = s.phrases.len();

        // A preview that already covered the whole phrase is as good as a final.
        if let Some(((ps, pe), tr)) = &s.last_preview {
            let no_seam = !(joint && (idx > 0 || cont_ok));
            if *ps == o.start && *pe >= speech_end + S(0.25) && no_seam {
                let mut tr = tr.clone();
                let shift = (decode_start - o.start) as f32 / SR as f32;
                for w in tr.words.iter_mut() {
                    w.start = (w.start - shift).max(0.0);
                    w.end = (w.end - shift).max(0.0);
                }
                s.phrases.push(Phrase { end, speech_start, speech_end, decode_start, boundary_s: None, result: Some(Ok(tr)) });
                s.last_preview = None;
                self.emit_ready();
                return;
            }
        }

        // Seam context: previous phrase in this session, or the previous dictation.
        let mut ctx_audio: Vec<f32> = Vec::new();
        if joint {
            if idx > 0 {
                let p = &s.phrases[idx - 1];
                let a = p.speech_start.max(p.speech_end.saturating_sub(S(SEAM_TAIL_S)));
                ctx_audio = s.slice(a, p.speech_end + S(0.12));
            } else if cont_ok {
                if let Some(l) = &self.last {
                    ctx_audio = l.tail_audio.clone();
                }
            }
        }
        let s = self.session.as_mut().unwrap();
        let new_start = if ctx_audio.is_empty() { decode_start } else { decode_start.max(speech_start.saturating_sub(S(0.12))) };
        let mut audio = ctx_audio.clone();
        audio.extend(s.slice(new_start, decode_end));
        let boundary_s = (!ctx_audio.is_empty()).then(|| ctx_audio.len() as f32 / SR as f32);
        s.phrases.push(Phrase { end, speech_start, speech_end, decode_start: new_start, boundary_s, result: None });
        s.last_preview = None;
        let job = Job {
            id: self.next_id,
            session: s.id,
            phrase: idx,
            kind: JobKind::Final,
            audio: Arc::new(audio),
            range: (new_start, decode_end),
            opts: self.decode_opts(),
        };
        self.next_id += 1;
        let _ = self.engine.send(EngineCmd::Job(job));
    }

    fn maybe_preview(&mut self) {
        if self.model.phase != ModelPhase::Ready {
            return;
        }
        let interval = S(self.cfg.audio.preview_interval_ms.max(150) as f32 / 1000.0);
        let opts = self.decode_opts();
        let Some(s) = self.session.as_mut() else { return };
        if s.preview_inflight || !s.capturing {
            return;
        }
        let o = &s.open;
        let Some(ss) = o.speech_start else { return };
        let pos = s.block_pos;
        let first = s.preview_from != o.start;
        let due = if first { pos.saturating_sub(ss) >= S(0.35) } else { pos.saturating_sub(s.preview_end) >= interval };
        // Eager: the speaker went quiet; decode everything now so a release can reuse it.
        let eager = o.silence >= S(0.25) && (first || s.preview_end < o.speech_end + S(0.25));
        if !(due || eager) || o.voiced_blocks < 2 {
            return;
        }
        let range = (o.start, pos);
        let audio = s.slice(range.0, range.1);
        s.preview_inflight = true;
        s.preview_from = o.start;
        s.preview_end = pos;
        let job = Job { id: self.next_id, session: s.id, phrase: usize::MAX, kind: JobKind::Preview, audio: Arc::new(audio), range, opts };
        self.next_id += 1;
        let _ = self.engine.send(EngineCmd::Job(job));
    }

    /// Long hands-free sessions must not grow memory forever.
    fn trim_session_audio(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        if s.audio.len() < S(90.0) {
            return;
        }
        let mut keep_from = s.open.start;
        if let Some(p) = s.phrases.last() {
            keep_from = keep_from.min(p.speech_end.saturating_sub(S(SEAM_TAIL_S + 0.5)));
        }
        keep_from = keep_from.saturating_sub(S(1.0)).max(s.base);
        let cut = keep_from - s.base;
        if cut > S(30.0) {
            s.audio.drain(..cut);
            s.base = keep_from;
        }
    }

    fn decode_opts(&self) -> DecodeOpts {
        let language = match self.cfg.model.language.trim() {
            "" | "auto" => None,
            l => Some(l.to_string()),
        };
        let hotwords = if self.cfg.model.hotwords { self.rules.hotwords(25) } else { vec![] };
        DecodeOpts { hotwords, language }
    }

    fn engine_is_local(&self) -> bool {
        match models::find(&self.cfg.model.id).map(|m| m.engine) {
            Some(EngineKind::Custom) => {
                let u = self.cfg.custom.base_url.to_ascii_lowercase();
                u.contains("://127.") || u.contains("://localhost")
            }
            _ => true,
        }
    }

    // ------------------------------------------------------------- sessions

    fn start_session(&mut self, mode: Mode) {
        if !self.cfg.general.onboarded && !self.mic_open {
            // First use before onboarding finished still works.
        }
        self.last_used = Instant::now();
        self.unload_at = None;
        match self.model.phase {
            ModelPhase::Ready | ModelPhase::Loading => {}
            ModelPhase::Installing => {
                self.overlay.notice("The model is still installing…", true);
                return;
            }
            ModelPhase::NotInstalled => {
                self.overlay.notice("Model not installed. Open Fluss to set it up.", true);
                sounds::play(Sound::Error, &self.cfg.general);
                platform::open_ui(&["--page", "model"]);
                return;
            }
            _ => {
                let _ = self.engine.send(EngineCmd::Load { urgent: true });
            }
        }
        self.load_at = None;
        let mut audio = Vec::new();
        if self.mic_open && self.cfg.audio.keep_mic_open {
            audio = self.ring.last(S(self.cfg.audio.preroll_ms as f32 / 1000.0));
        }
        self.open_mic();
        let ctx = platform::focus_ctx();
        let id = self.next_id;
        self.next_id += 1;
        let start_pos = 0usize;
        let pre = audio.len();
        self.session = Some(Session {
            id,
            mode,
            started: Instant::now(),
            audio,
            base: start_pos,
            vad: self.vad_mem.carry(),
            block_pos: 0,
            open: Open { start: 0, ..Default::default() },
            phrases: Vec::new(),
            emitted: 0,
            preview_inflight: false,
            preview_from: usize::MAX,
            preview_end: 0,
            last_preview: None,
            preview_text: String::new(),
            capturing: true,
            released: None,
            stop_at: None,
            tap_since: None,
            ignore_next_up: false,
            add_rule_after: false,
            cancelled: false,
            pending_edits: 0,
            last_done: None,
            text: String::new(),
            raw: String::new(),
            app: ctx.app.clone(),
        });
        log::debug!("session {id} started ({mode:?}, {pre} samples pre-roll)");
        hotkeys::ACTIVE.store(true, Ordering::Relaxed);
        sounds::play(Sound::Start, &self.cfg.general);
        self.overlay.show(self.overlay_state());
        if pre > 0 {
            self.segment();
        }
        self.publish();
    }

    fn overlay_state(&self) -> OvState {
        let Some(s) = &self.session else { return OvState::Hidden };
        if self.model.phase != ModelPhase::Ready {
            return OvState::Loading { progress: self.model.progress };
        }
        if s.released.is_some() || !s.capturing {
            return OvState::Finishing;
        }
        match s.mode {
            Mode::Hold => OvState::Listening,
            Mode::HandsFree => OvState::HandsFree,
        }
    }

    /// Stop listening (release / hands-free stop). Post-roll keeps capturing.
    fn release(&mut self) {
        let postroll = Duration::from_millis(self.cfg.audio.postroll_ms as u64);
        let Some(s) = self.session.as_mut() else { return };
        if s.released.is_some() {
            return;
        }
        s.released = Some(Instant::now());
        s.stop_at = Some(Instant::now() + postroll);
        s.tap_since = None;
        // Already silent: finalize right now, post-roll only catches stragglers.
        let quiet = s.open.voiced_blocks >= 1 && s.open.silence >= S(RELEASE_PAUSE_MS as f32 / 1000.0);
        if quiet {
            self.flush_open(false);
        }
        self.overlay.show(self.overlay_state());
        self.publish();
    }

    fn stop_capture(&mut self) {
        let Some(s) = self.session.as_mut() else { return };
        if !s.capturing {
            return;
        }
        s.capturing = false;
        self.vad_mem = s.vad.carry();
        self.flush_open(true);
        if !self.cfg.audio.keep_mic_open {
            self.mic_close_at = Some(Instant::now() + Duration::from_secs(4));
        }
        self.maybe_finish();
    }

    fn cancel(&mut self, quiet: bool) {
        if let Some(s) = self.session.take() {
            let _ = self.engine.send(EngineCmd::Cancel(s.id));
            log::debug!("session {} cancelled", s.id);
        }
        hotkeys::ACTIVE.store(false, Ordering::Relaxed);
        if quiet {
            self.overlay.hide();
        } else {
            sounds::play(Sound::Cancel, &self.cfg.general);
            self.overlay.notice("Cancelled", false);
        }
        self.after_session();
        self.publish();
    }

    fn maybe_finish(&mut self) {
        let done = match &self.session {
            Some(s) => !s.capturing && s.emitted == s.phrases.len() && s.pending_edits == 0,
            None => false,
        };
        if !done {
            return;
        }
        let s = self.session.take().unwrap();
        hotkeys::ACTIVE.store(false, Ordering::Relaxed);
        let typed = !s.text.trim().is_empty();
        if typed {
            sounds::play(Sound::Stop, &self.cfg.general);
            let duration_s = s.started.elapsed().as_secs_f32();
            let latency = match (s.released, s.last_done) {
                (Some(r), Some(d)) if d >= r => d.duration_since(r).as_millis() as u32,
                _ => 0,
            };
            if latency > 0 {
                self.record_latency(latency);
            }
            self.stats.dictations += 1;
            self.stats.words += s.text.split_whitespace().count() as u64;
            self.stats.audio_seconds += duration_s as f64;
            if self.cfg.general.history {
                let item = HistoryItem { id: s.id, at: now_ms(), text: s.text.clone(), raw: s.raw.clone(), app: s.app.clone(), duration_s, latency_ms: latency };
                self.hub.broadcast("dictation", serde_json::to_value(&item).unwrap_or_default());
                self.history.push_front(item);
                self.history.truncate(50);
            }
            self.overlay.hide();
        } else if s.add_rule_after {
            self.overlay.hide();
        } else {
            self.overlay.notice("No speech detected", false);
        }
        log::debug!("session {} finished: {:?}", s.id, s.text);
        if s.add_rule_after {
            self.reply_popup();
        }
        self.after_session();
        self.publish();
    }

    fn after_session(&mut self) {
        self.last_used = Instant::now();
        if self.cfg.model.residency == Residency::OnDemand {
            self.unload_at = Some(Instant::now() + Duration::from_secs(20));
        }
    }

    fn record_latency(&mut self, ms: u32) {
        self.latencies.push_back(ms);
        if self.latencies.len() > 50 {
            self.latencies.pop_front();
        }
        self.stats.last_latency_ms = Some(ms);
    }

    // ------------------------------------------------------------- hotkeys

    fn on_hotkey(&mut self, h: HotkeyEv) {
        if let HotkeyEv::Recorded(hk) = &h {
            if let Some((r, _)) = self.record.take() {
                hotkeys::RECORDING.store(false, Ordering::SeqCst);
                match hk {
                    Some(k) => r.ok(json!({"hotkey": k.to_string()})),
                    None => r.err("That isn't a usable shortcut. Use a key, or at least two modifiers (e.g. Ctrl+Win)."),
                }
            }
            return;
        }
        log::debug!("hotkey {h:?}");
        match h {
            HotkeyEv::PttDown => match self.session.as_mut() {
                None => self.start_session(Mode::Hold),
                Some(s) if s.tap_since.is_some() => {
                    // Double tap: lock hands-free.
                    s.tap_since = None;
                    s.mode = Mode::HandsFree;
                    s.ignore_next_up = true;
                    self.overlay.show(self.overlay_state());
                    self.publish();
                }
                Some(s) if s.mode == Mode::HandsFree && s.released.is_none() => {
                    s.ignore_next_up = true;
                    self.release();
                }
                Some(_) => {}
            },
            HotkeyEv::PttUp => {
                let Some(s) = self.session.as_mut() else { return };
                if s.ignore_next_up {
                    s.ignore_next_up = false;
                    return;
                }
                if s.mode != Mode::Hold || s.released.is_some() {
                    return;
                }
                if self.cfg.hotkeys.double_tap_lock && s.started.elapsed() < TAP_MAX && s.open.voiced_blocks < 3 {
                    s.tap_since = Some(Instant::now());
                    return;
                }
                self.release();
            }
            HotkeyEv::HandsFree => match self.session.as_mut() {
                None => self.start_session(Mode::HandsFree),
                Some(s) if s.released.is_none() && s.mode == Mode::Hold => {
                    s.mode = Mode::HandsFree;
                    s.tap_since = None;
                    s.ignore_next_up = true;
                    self.overlay.show(self.overlay_state());
                    self.publish();
                }
                Some(s) if s.released.is_none() => {
                    let _ = s;
                    self.release();
                }
                Some(_) => {}
            },
            HotkeyEv::Cancel => {
                if self.session.is_some() {
                    self.cancel(false);
                }
            }
            HotkeyEv::OtherKey => {
                // Ctrl+Win+Arrow and friends: the user wanted a system shortcut.
                let quiet_cancel = self.session.as_ref().is_some_and(|s| {
                    s.mode == Mode::Hold && s.released.is_none() && s.emitted == 0 && s.started.elapsed() < Duration::from_millis(1500)
                });
                if quiet_cancel {
                    self.cancel(true);
                }
            }
            HotkeyEv::AddRule => self.open_rule_popup(),
            HotkeyEv::Recorded(_) => {}
        }
    }

    // ------------------------------------------------------------- engine results

    fn on_engine(&mut self, e: EngineEv) {
        match e {
            EngineEv::State { id, phase, detail, progress, device, memory_mb, error } => {
                if id != self.cfg.model.id {
                    return; // a late event from the previously selected model
                }
                let was_ready = self.model.phase == ModelPhase::Ready;
                if self.installing.as_deref() == Some(self.cfg.model.id.as_str()) && phase != ModelPhase::Loading {
                    return; // the installer owns the status meanwhile
                }
                self.model.phase = phase;
                self.model.detail = detail;
                self.model.progress = progress;
                if !device.is_empty() || phase != ModelPhase::Loading {
                    self.model.device = device;
                }
                self.model.memory_mb = memory_mb;
                self.model.error = error.clone();
                if phase == ModelPhase::Error {
                    if let Some(e) = &error {
                        self.notice("error", &format!("Model failed to load: {}", e.lines().next().unwrap_or("")));
                        if self.session.is_some() {
                            self.overlay.notice("Model failed to load — see Fluss settings", true);
                        }
                    }
                }
                if phase == ModelPhase::Ready && !was_ready {
                    if let Some(own) = platform::own_memory_mb() {
                        log::info!("daemon working set {own} MB");
                    }
                }
                if self.session.is_some() {
                    self.overlay.show(self.overlay_state());
                    if phase == ModelPhase::Ready {
                        self.maybe_preview();
                    }
                }
                self.publish();
            }
            EngineEv::Done(d) => self.on_job_done(d),
        }
    }

    fn on_job_done(&mut self, d: JobDone) {
        if d.kind == JobKind::File {
            if let Some(r) = self.file_jobs.remove(&d.id) {
                match d.result {
                    Ok(tr) => {
                        let p = text::process(&tr.words, &self.cfg.text, Some(&self.matcher), Context { sentence_start: true, is_final: true });
                        r.ok(json!({"text": p.text, "raw": tr.text, "words": tr.words, "decode_ms": d.decode_ms}));
                    }
                    Err(e) => r.err(&e),
                }
            }
            return;
        }
        let Some(s) = self.session.as_mut() else { return };
        if s.id != d.session || s.cancelled {
            return;
        }
        match d.kind {
            JobKind::Preview => {
                s.preview_inflight = false;
                if let Ok(tr) = d.result {
                    if d.range.0 == s.open.start {
                        s.preview_text = tr.text.clone();
                        s.last_preview = Some((d.range, tr));
                        if self.cfg.ui.overlay_preview {
                            let shown = format!("{}{}", if s.phrases.len() > s.emitted { "… " } else { "" }, s.preview_text);
                            self.overlay.text(&shown);
                        }
                        self.hub.broadcast("partial", json!({"text": s.preview_text}));
                    }
                }
                self.maybe_preview();
            }
            JobKind::Final => {
                if let Some(p) = s.phrases.get_mut(d.phrase) {
                    p.result = Some(d.result);
                }
                self.emit_ready();
                self.maybe_finish();
            }
            JobKind::File => {}
        }
    }

    fn continuation_valid(&self, now: &FocusCtx) -> bool {
        if !self.cfg.output.continuation {
            return false;
        }
        let Some(l) = &self.last else { return false };
        // In test-sink mode the sink file is the text field.
        let test = std::env::var_os("FLUSS_TEST_SINK").is_some();
        (test || (l.ctx == *now && l.epoch == hotkeys::epoch()))
            && l.at.elapsed() < Duration::from_secs(self.cfg.output.continuation_window_s as u64)
    }

    fn emit_ready(&mut self) {
        loop {
            let Some(s) = self.session.as_mut() else { return };
            let k = s.emitted;
            let ready = s.phrases.get(k).and_then(|p| p.result.as_ref()).is_some();
            if !ready {
                break;
            }
            s.emitted += 1;
            let p = &s.phrases[k];
            let tr = match p.result.as_ref().unwrap() {
                Ok(t) => t.clone(),
                Err(e) => {
                    log::warn!("phrase {k} failed: {e}");
                    let msg = format!("Transcription failed: {}", e.lines().next().unwrap_or(""));
                    self.overlay.notice(&msg, true);
                    self.notice("error", &msg);
                    continue;
                }
            };
            let boundary = p.boundary_s;
            let (ctx_words, new_words) = match boundary {
                Some(b) => split_joint(&tr.words, b),
                None => (Vec::new(), tr.words.clone()),
            };
            let tail_audio = {
                let a = p.speech_start.max(p.speech_end.saturating_sub(S(SEAM_TAIL_S)));
                s.slice(a, p.speech_end + S(0.12))
            };
            let env = chunk::envelope(&s.slice(p.decode_start, p.end));
            let gap_s = s.phrases.get(k.wrapping_sub(1)).map(|q| (p.speech_start.saturating_sub(q.speech_end)) as f32 / SR as f32).unwrap_or(1.0);
            let is_final = !s.capturing && k + 1 == s.phrases.len();
            if new_words.is_empty() {
                continue;
            }
            let now_ctx = platform::focus_ctx();
            let cont = self.continuation_valid(&now_ctx);
            let seam = if cont {
                let l = self.last.as_ref().unwrap();
                if boundary.is_some() && !ctx_words.is_empty() {
                    repair_with_context(&l.auto_tail, l.ended_sentence, &ctx_words)
                } else {
                    let gap = if k == 0 { l.at.elapsed().as_secs_f32() } else { gap_s };
                    repair_heuristic(&l.auto_tail, l.ended_sentence, &new_words[0].text, gap)
                }
            } else {
                Seam { sentence_start: true, ..Seam::default() }
            };
            let flags = Context { sentence_start: seam.sentence_start, is_final };
            let out = text::process(&new_words, &self.cfg.text, Some(&self.matcher), flags);
            if out.text.is_empty() && seam.is_noop() {
                continue;
            }
            let sep = if cont && self.cfg.output.smart_spacing && needs_space(&out.text) { " " } else { "" };
            let typed = format!("{}{}{}", seam.insert, sep, out.text);
            if !seam.is_noop() {
                log::debug!("seam: -{} +{:?}", seam.backspaces, seam.insert);
            }
            let tag = self.next_id;
            self.next_id += 1;
            let s = self.session.as_mut().unwrap();
            s.pending_edits += 1;
            self.edit_tags.insert(tag, s.id);
            let _ = self.inject.send(Edit {
                backspaces: seam.backspaces,
                text: typed.clone(),
                method: self.cfg.output.method,
                newline: self.cfg.output.newline,
                tag,
            });
            let raw_text = new_words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
            for _ in 0..seam.backspaces {
                s.text.pop();
            }
            if !s.text.is_empty() && !typed.starts_with(['\n', ',', '.', ' ']) && !s.text.ends_with(['\n', ' ']) {
                s.text.push(' ');
            }
            s.text.push_str(&typed);
            if !s.raw.is_empty() {
                s.raw.push(' ');
            }
            s.raw.push_str(&raw_text);
            if !out.rules_hit.is_empty() {
                self.rules.record_use(&out.rules_hit);
                let _ = self.rules.save();
            }
            let ended_sentence = out.text.trim_end().ends_with(['.', '!', '?', '…']);
            self.last = Some(LastInsert {
                ctx: now_ctx,
                epoch: hotkeys::epoch(),
                at: Instant::now(),
                auto_tail: out.auto_tail.clone(),
                ended_sentence,
                tail_audio,
                words: new_words,
                env,
                text: out.text,
                flags,
            });
        }
    }

    fn on_injected(&mut self, tag: u64, at: Instant) {
        let Some(sid) = self.edit_tags.remove(&tag) else { return };
        if let Some(s) = self.session.as_mut() {
            if s.id == sid {
                s.pending_edits = s.pending_edits.saturating_sub(1);
                s.last_done = Some(at);
                if s.capturing && s.released.is_none() && self.cfg.ui.overlay_preview {
                    self.overlay.text("");
                }
            }
        }
        self.maybe_finish();
    }

    // ------------------------------------------------------------- rule popup

    fn popup_context(&self) -> PopupContext {
        let fresh = self.last.as_ref().filter(|l| l.at.elapsed() < Duration::from_secs(600));
        match fresh {
            Some(l) => PopupContext {
                chunk: chunk::guess(&l.words, Some(&l.env), ChunkLimits::default()),
                return_hwnd: self.popup_return,
                from_dictation: true,
                can_apply_last: self.continuation_valid(&l.ctx),
            },
            None => PopupContext { return_hwnd: self.popup_return, ..Default::default() },
        }
    }

    fn open_rule_popup(&mut self) {
        self.popup_return = platform::foreground_window();
        hotkeys::EPOCH_FROZEN.store(true, Ordering::SeqCst);
        // Ctrl+Win+R from idle starts a push-to-talk hold first; with no speech yet,
        // drop it so the popup opens right away with the previous dictation.
        let empty = self.session.as_ref().is_some_and(|s| s.phrases.is_empty() && s.open.voiced_blocks < 2);
        if empty {
            self.cancel(true);
        }
        if let Some(s) = self.session.as_mut() {
            s.add_rule_after = true;
            if s.released.is_none() {
                self.release();
            }
        }
        platform::open_ui(&["--popup"]);
        if self.session.is_none() {
            let ctx = self.popup_context();
            self.hub.broadcast("popup", serde_json::to_value(&ctx).unwrap_or_default());
        }
    }

    fn reply_popup(&mut self) {
        let ctx = self.popup_context();
        let v = serde_json::to_value(&ctx).unwrap_or_default();
        for r in self.popup_reply.drain(..) {
            r.ok(v.clone());
        }
        self.hub.broadcast("popup", v);
    }

    /// Re-run the last phrase through the pipeline with the new rules and fix it in place.
    fn apply_last_fix(&mut self) {
        let Some(l) = self.last.as_mut() else { return };
        let now = platform::focus_ctx();
        let test = std::env::var_os("FLUSS_TEST_SINK").is_some();
        if !test && (l.ctx != now || l.epoch != hotkeys::epoch()) {
            log::debug!("not fixing last dictation in place: focus or text changed");
            return;
        }
        let out = text::process(&l.words, &self.cfg.text, Some(&self.matcher), l.flags);
        if out.text == l.text {
            return;
        }
        let old: Vec<char> = l.text.chars().collect();
        let new: Vec<char> = out.text.chars().collect();
        let common = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
        let edit = Edit {
            backspaces: old.len() - common,
            text: new[common..].iter().collect(),
            method: self.cfg.output.method,
            newline: self.cfg.output.newline,
            tag: 0,
        };
        l.text = out.text;
        l.auto_tail = out.auto_tail;
        let _ = self.inject.send(edit);
        self.overlay.notice("Fixed", false);
    }

    // ------------------------------------------------------------- timers

    fn tick(&mut self) {
        let now = Instant::now();
        if let Some(t) = self.load_at {
            if now >= t {
                self.load_at = None;
                if engine::installed(&self.cfg.model.id) && self.model.phase != ModelPhase::Ready {
                    let _ = self.engine.send(EngineCmd::Load { urgent: false });
                }
            }
        }
        if let Some(t) = self.unload_at {
            if now >= t && self.session.is_none() {
                self.unload_at = None;
                let _ = self.engine.send(EngineCmd::Unload);
            }
        }
        if self.cfg.model.residency == Residency::IdleUnload
            && self.model.phase == ModelPhase::Ready
            && self.session.is_none()
            && self.last_used.elapsed() >= Duration::from_secs(self.cfg.model.idle_unload_min.max(1) as u64 * 60)
        {
            log::info!("idle for {} min: unloading model", self.cfg.model.idle_unload_min);
            self.last_used = now;
            let _ = self.engine.send(EngineCmd::Unload);
        }
        if let Some(t) = self.mic_close_at {
            if now >= t && self.session.is_none() {
                self.mic_close_at = None;
                self.close_mic();
            }
        }
        if let Some((_, since)) = &self.record {
            if since.elapsed() > Duration::from_secs(12) {
                let (r, _) = self.record.take().unwrap();
                hotkeys::RECORDING.store(false, Ordering::SeqCst);
                r.err("No shortcut pressed");
            }
        }
        if let Some(t) = self.apply_at {
            if now >= t {
                self.apply_at = None;
                self.apply_last_fix();
            }
        }
        // Session timers.
        let mut release = false;
        let mut stop = false;
        let mut cancel_quiet = false;
        if let Some(s) = self.session.as_mut() {
            if let Some(t) = s.tap_since {
                if t.elapsed() >= Duration::from_millis(self.cfg.hotkeys.double_tap_ms as u64) {
                    s.tap_since = None;
                    if s.open.voiced_blocks == 0 && s.phrases.is_empty() {
                        cancel_quiet = true;
                    } else {
                        release = true;
                    }
                }
            }
            if let Some(t) = s.stop_at {
                if now >= t && s.capturing {
                    stop = true;
                }
            }
        }
        if cancel_quiet {
            self.cancel(true);
        } else if release {
            self.release();
        }
        if stop {
            self.stop_capture();
        }
    }

    // ------------------------------------------------------------- install

    fn start_install(&mut self, id: String) -> Result<(), String> {
        if let Some(i) = &self.installing {
            return Err(format!("already installing {i}"));
        }
        if models::find(&id).is_none() {
            return Err(format!("unknown model `{id}`"));
        }
        self.installing = Some(id.clone());
        if id == self.cfg.model.id {
            let _ = self.engine.send(EngineCmd::Unload);
            self.model.phase = ModelPhase::Installing;
            self.model.progress = Some(0.0);
            self.model.detail = "Starting".into();
            self.publish();
        }
        let tx = self.tx.clone();
        let device = self.cfg.model.device;
        std::thread::Builder::new()
            .name("fluss-install".into())
            .spawn(move || {
                #[cfg(windows)]
                let _bg = platform::ThreadPriorityGuard::background(true);
                let send = |step: &str, p: Option<f32>, done: bool, err: Option<String>| {
                    let _ = tx.send(Ev::Install(InstallProgress { id: id.clone(), step: step.into(), progress: p, done, error: err }));
                };
                let progress = |step: &str, p: Option<f32>| send(step, p, false, None);
                match crate::provision::install(&id, device, &progress) {
                    Ok(()) => send("Installed", Some(1.0), true, None),
                    Err(e) => {
                        log::error!("install {id} failed: {e:#}");
                        send("Installation failed", None, true, Some(format!("{e:#}")));
                    }
                }
            })
            .ok();
        Ok(())
    }

    fn on_install(&mut self, p: InstallProgress) {
        self.hub.broadcast("install", serde_json::to_value(&p).unwrap_or_default());
        let current = p.id == self.cfg.model.id;
        if p.done {
            self.installing = None;
            if let Some(e) = &p.error {
                self.notice("error", &format!("Installing failed: {}", e.lines().next().unwrap_or("")));
                if current {
                    self.model.phase = ModelPhase::Error;
                    self.model.error = Some(e.clone());
                }
            } else {
                self.notice("success", &format!("{} is installed", models::find(&p.id).map(|m| m.name).unwrap_or("Model")));
                if current {
                    self.model.phase = ModelPhase::Unloaded;
                    self.model.error = None;
                    let _ = self.engine.send(EngineCmd::Configure(EngineSpec::from_config(&self.cfg)));
                    if self.cfg.model.residency != Residency::OnDemand {
                        let _ = self.engine.send(EngineCmd::Load { urgent: false });
                    }
                }
            }
        } else if current {
            self.model.phase = ModelPhase::Installing;
            self.model.progress = p.progress;
            self.model.detail = p.step.clone();
        }
        if current {
            self.publish();
        }
    }

    // ------------------------------------------------------------- config

    fn apply_config(&mut self, next: Config) -> Result<(), String> {
        let prev = std::mem::replace(&mut self.cfg, next);
        self.cfg.save().map_err(|e| format!("{e:#}"))?;
        if prev.hotkeys != self.cfg.hotkeys {
            hotkeys::set_bindings(&self.cfg.hotkeys);
        }
        let spec_prev = EngineSpec::from_config(&prev);
        let spec = EngineSpec::from_config(&self.cfg);
        if spec_prev != spec || (prev.model.device != self.cfg.model.device) {
            self.model = ModelStatus { id: spec.id.clone(), name: spec.name(), ..Default::default() };
            let _ = self.engine.send(EngineCmd::Configure(spec));
            if self.cfg.general.onboarded && self.cfg.model.residency != Residency::OnDemand && engine::installed(&self.cfg.model.id) {
                let _ = self.engine.send(EngineCmd::Load { urgent: false });
            }
        }
        if prev.model.residency != self.cfg.model.residency {
            match self.cfg.model.residency {
                Residency::Always if self.model.phase == ModelPhase::Unloaded => {
                    let _ = self.engine.send(EngineCmd::Load { urgent: false });
                }
                Residency::OnDemand if self.session.is_none() => self.unload_at = Some(Instant::now() + Duration::from_secs(20)),
                _ => {}
            }
        }
        if prev.audio.device != self.cfg.audio.device && self.mic_open {
            let _ = self.audio.send(AudioCmd::Open { device: self.cfg.audio.device.clone(), gain: self.cfg.audio.gain });
        }
        if prev.audio.gain != self.cfg.audio.gain {
            let _ = self.audio.send(AudioCmd::SetGain(self.cfg.audio.gain));
        }
        if prev.audio.preroll_ms != self.cfg.audio.preroll_ms {
            self.ring = Ring::new(S(self.cfg.audio.preroll_ms as f32 / 1000.0).max(1));
        }
        let want_open = self.cfg.audio.keep_mic_open && self.cfg.general.onboarded;
        if want_open && !self.mic_open {
            self.open_mic();
        } else if !self.cfg.audio.keep_mic_open && self.mic_open && self.session.is_none() && !self.hub.wants_levels() {
            self.close_mic();
        }
        if prev.general.autostart != self.cfg.general.autostart {
            if let Err(e) = platform::set_autostart(self.cfg.general.autostart) {
                log::warn!("autostart: {e:#}");
            }
        }
        if prev.ui != self.cfg.ui {
            self.overlay.configure(&self.cfg.ui);
        }
        if !prev.general.onboarded && self.cfg.general.onboarded && self.cfg.model.residency != Residency::OnDemand && self.model.phase == ModelPhase::Unloaded {
            let _ = self.engine.send(EngineCmd::Load { urgent: false });
        }
        self.hub.broadcast("config", self.cfg.to_value());
        self.publish();
        Ok(())
    }

    fn rules_changed(&mut self) {
        self.matcher = self.rules.matcher();
        if let Err(e) = self.rules.save() {
            log::error!("saving rules: {e:#}");
        }
        self.hub.broadcast("rules", json!({"count": self.rules.rules.len()}));
    }

    // ------------------------------------------------------------- tray

    fn on_tray(&mut self, t: TrayCmd) {
        match t {
            TrayCmd::Open => platform::open_ui(&[]),
            TrayCmd::Dictionary => platform::open_ui(&["--page", "dictionary"]),
            TrayCmd::AddWord => {
                self.popup_return = platform::foreground_window();
                hotkeys::EPOCH_FROZEN.store(true, Ordering::SeqCst);
                platform::open_ui(&["--popup"]);
            }
            TrayCmd::ToggleHandsFree => self.on_hotkey(HotkeyEv::HandsFree),
            TrayCmd::LoadModel => {
                let _ = self.engine.send(EngineCmd::Load { urgent: true });
            }
            TrayCmd::UnloadModel => {
                let _ = self.engine.send(EngineCmd::Unload);
            }
            TrayCmd::PauseHotkeys(p) => {
                hotkeys::PAUSED.store(p, Ordering::Relaxed);
                self.publish();
            }
            TrayCmd::Quit => self.quit = true,
        }
    }

    // ------------------------------------------------------------- IPC

    fn on_ipc(&mut self, call: IpcCall) {
        let IpcCall { method, params, reply } = call;
        let p = |k: &str| params.get(k).cloned().unwrap_or(Value::Null);
        let ps = |k: &str| params.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        match method.as_str() {
            "ping" => reply.ok(json!("pong")),
            "status" => reply.ok(serde_json::to_value(self.status()).unwrap_or_default()),
            "subscribe" => {
                let levels = params.get("levels").and_then(|v| v.as_bool()).unwrap_or(false);
                reply.subscribe(levels);
                if levels && !self.mic_open && self.session.is_none() {
                    self.open_mic();
                    if !self.cfg.audio.keep_mic_open {
                        self.mic_close_at = Some(Instant::now() + Duration::from_secs(120));
                    }
                }
                reply.ok(json!(true));
                self.publish();
            }
            "config.get" => match self.cfg.get_path(&ps("path")) {
                Ok(v) => reply.ok(v),
                Err(e) => reply.err(&format!("{e:#}")),
            },
            "config.set" => {
                let mut next = self.cfg.clone();
                match next.set_path(&ps("path"), p("value")) {
                    Ok(()) => match self.apply_config(next) {
                        Ok(()) => reply.ok(self.cfg.get_path(&ps("path")).unwrap_or_default()),
                        Err(e) => reply.err(&e),
                    },
                    Err(e) => reply.err(&format!("{e:#}")),
                }
            }
            "config.merge" => {
                let mut next = self.cfg.clone();
                match next.merge(&p("patch")) {
                    Ok(()) => match self.apply_config(next) {
                        Ok(()) => reply.ok(self.cfg.to_value()),
                        Err(e) => reply.err(&e),
                    },
                    Err(e) => reply.err(&format!("{e:#}")),
                }
            }
            "config.reset" => {
                let mut next = Config::default();
                next.general.onboarded = self.cfg.general.onboarded;
                match self.apply_config(next) {
                    Ok(()) => reply.ok(self.cfg.to_value()),
                    Err(e) => reply.err(&e),
                }
            }
            "rules.list" => {
                let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
                reply.ok(serde_json::to_value(self.rules.search(&ps("query"), limit)).unwrap_or_default());
            }
            "rules.upsert" => {
                let target = ps("target");
                if target.trim().is_empty() {
                    return reply.err("the replacement text is empty");
                }
                let variants: Vec<String> = params
                    .get("variants")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let out = self.rules.upsert(&target, &variants);
                self.rules_changed();
                if params.get("apply_last").and_then(|v| v.as_bool()).unwrap_or(false) {
                    if hotkeys::EPOCH_FROZEN.load(Ordering::SeqCst) {
                        self.apply_after_popup = true;
                    } else {
                        self.apply_at = Some(Instant::now());
                    }
                }
                self.overlay.notice(&format!("Saved: {}", out.rule.target), false);
                reply.ok(serde_json::to_value(out).unwrap_or_default());
            }
            "rules.replace" => {
                let rule: Result<Rule, _> = serde_json::from_value(p("rule"));
                match rule {
                    Ok(r) => {
                        self.rules.replace(&ps("old_target"), r);
                        self.rules_changed();
                        reply.ok(json!(true));
                    }
                    Err(e) => reply.err(&format!("invalid rule: {e}")),
                }
            }
            "rules.delete" => {
                let ok = self.rules.remove(&ps("target"));
                self.rules_changed();
                reply.ok(json!(ok));
            }
            "rules.delete_variant" => {
                let ok = self.rules.remove_variant(&ps("target"), &ps("variant"));
                self.rules_changed();
                reply.ok(json!(ok));
            }
            "rules.preview" => {
                let tr = Transcript::from_text(&ps("text"), 1.0);
                let out = text::process(&tr.words, &self.cfg.text, Some(&self.matcher), Context { sentence_start: true, is_final: true });
                reply.ok(json!({"text": out.text, "rules": out.rules_hit}));
            }
            "rules.import" => {
                let r: Result<RuleSet, _> = serde_json::from_value(p("rules"));
                match r {
                    Ok(set) => {
                        let n = set.rules.len();
                        for rule in set.rules {
                            self.rules.upsert(&rule.target, &rule.variants);
                        }
                        self.rules_changed();
                        reply.ok(json!({"imported": n}));
                    }
                    Err(e) => reply.err(&format!("invalid rules file: {e}")),
                }
            }
            "model.catalog" => {
                let list: Vec<Value> = models::CATALOG
                    .iter()
                    .map(|m| {
                        let mut v = serde_json::to_value(m).unwrap_or_default();
                        v["installed"] = json!(engine::installed(m.id));
                        v["active"] = json!(m.id == self.cfg.model.id);
                        v["installing"] = json!(self.installing.as_deref() == Some(m.id));
                        v
                    })
                    .collect();
                reply.ok(json!(list));
            }
            "model.use" => {
                let id = ps("id");
                if models::find(&id).is_none() {
                    return reply.err(&format!("unknown model `{id}`"));
                }
                let mut next = self.cfg.clone();
                next.model.id = id;
                match self.apply_config(next) {
                    Ok(()) => reply.ok(json!(true)),
                    Err(e) => reply.err(&e),
                }
            }
            "model.install" => {
                let id = Some(ps("id")).filter(|s| !s.is_empty()).unwrap_or_else(|| self.cfg.model.id.clone());
                match self.start_install(id) {
                    Ok(()) => reply.ok(json!(true)),
                    Err(e) => reply.err(&e),
                }
            }
            "model.load" => {
                let _ = self.engine.send(EngineCmd::Load { urgent: true });
                reply.ok(json!(true));
            }
            "model.unload" => {
                let _ = self.engine.send(EngineCmd::Unload);
                reply.ok(json!(true));
            }
            "devices.list" => reply.ok(serde_json::to_value(crate::audio::list_devices()).unwrap_or_default()),
            "dictation.start" => {
                if self.session.is_none() {
                    let hf = params.get("hands_free").and_then(|v| v.as_bool()).unwrap_or(true);
                    self.start_session(if hf { Mode::HandsFree } else { Mode::Hold });
                }
                reply.ok(json!(true));
            }
            "dictation.stop" => {
                self.release();
                reply.ok(json!(true));
            }
            "dictation.cancel" => {
                if self.session.is_some() {
                    self.cancel(false);
                }
                reply.ok(json!(true));
            }
            "transcribe.file" => {
                let path = std::path::PathBuf::from(ps("path"));
                match dsp::read_wav(&path) {
                    Ok(pcm) => {
                        let id = self.next_id;
                        self.next_id += 1;
                        self.file_jobs.insert(id, reply);
                        let job = Job { id, session: 0, phrase: 0, kind: JobKind::File, audio: Arc::new(pcm), range: (0, 0), opts: self.decode_opts() };
                        let _ = self.engine.send(EngineCmd::Job(job));
                    }
                    Err(e) => reply.err(&format!("{e:#}")),
                }
            }
            "history.list" => reply.ok(serde_json::to_value(&self.history).unwrap_or_default()),
            "history.clear" => {
                self.history.clear();
                reply.ok(json!(true));
            }
            "popup.context" => {
                let finishing = self.session.as_ref().is_some_and(|s| s.add_rule_after);
                if finishing {
                    self.popup_reply.push(reply);
                } else {
                    reply.ok(serde_json::to_value(self.popup_context()).unwrap_or_default());
                }
            }
            "popup.open" => {
                self.open_rule_popup();
                reply.ok(json!(true));
            }
            "popup.closed" => {
                hotkeys::EPOCH_FROZEN.store(false, Ordering::SeqCst);
                if std::mem::take(&mut self.apply_after_popup) {
                    // Let focus settle back into the text field first.
                    self.apply_at = Some(Instant::now() + Duration::from_millis(120));
                }
                reply.ok(json!(true));
            }
            "hotkeys.record" => {
                if let Some((old, _)) = self.record.take() {
                    old.err("superseded");
                }
                hotkeys::RECORDING.store(true, Ordering::SeqCst);
                self.record = Some((reply, Instant::now()));
            }
            "hotkeys.cancel_record" => {
                if let Some((old, _)) = self.record.take() {
                    old.err("cancelled");
                }
                hotkeys::RECORDING.store(false, Ordering::SeqCst);
                reply.ok(json!(true));
            }
            "hotkeys.pause" => {
                let paused = params.get("paused").and_then(|v| v.as_bool()).unwrap_or(true);
                if !paused {
                    // The settings app uses this to abandon a shortcut recording too.
                    if let Some((old, _)) = self.record.take() {
                        old.err("cancelled");
                    }
                    hotkeys::RECORDING.store(false, Ordering::SeqCst);
                }
                hotkeys::PAUSED.store(paused, Ordering::Relaxed);
                reply.ok(json!(true));
                self.publish();
            }
            "endpoint.test" => {
                let cfg = self.cfg.custom.clone();
                std::thread::spawn(move || {
                    let t = Instant::now();
                    let res = crate::engine::openai::OpenAi::new(&cfg).and_then(|mut e| {
                        use crate::engine::Engine;
                        e.transcribe(&test_tone(), &DecodeOpts::default())
                    });
                    match res {
                        Ok(tr) => reply.ok(json!({"ok": true, "latency_ms": t.elapsed().as_millis() as u64, "text": tr.text})),
                        Err(e) => reply.err(&format!("{e:#}")),
                    }
                });
            }
            "autostart.set" => {
                let mut next = self.cfg.clone();
                next.general.autostart = params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                match self.apply_config(next) {
                    Ok(()) => reply.ok(json!(true)),
                    Err(e) => reply.err(&e),
                }
            }
            "ui.open" => {
                let page = ps("page");
                if page.is_empty() {
                    platform::open_ui(&[]);
                } else {
                    platform::open_ui(&["--page", &page]);
                }
                reply.ok(json!(true));
            }
            "logs.path" => reply.ok(json!(fluss_core::paths::logs_dir().display().to_string())),
            "daemon.quit" => {
                reply.ok(json!(true));
                self.quit = true;
            }
            other => reply.err(&format!("unknown method `{other}`")),
        }
    }
}

fn needs_space(t: &str) -> bool {
    !t.starts_with([',', '.', ';', ':', '!', '?', ')', ']', '}', '\n', '%'])
}

/// One second of a soft vowel-like tone for endpoint tests (silence makes some servers 400).
fn test_tone() -> Vec<f32> {
    (0..SR).map(|i| {
        let t = i as f32 / SR as f32;
        0.05 * ((2.0 * std::f32::consts::PI * 220.0 * t).sin() + 0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin())
    }).collect()
}
