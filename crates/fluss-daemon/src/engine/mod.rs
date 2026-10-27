//! Speech engines and the engine thread.
//!
//! The engine thread owns the loaded model. It always serves final decodes
//! before live previews, and keeps only the newest preview (older ones are
//! stale the moment new audio arrives), so previews can never delay text.

pub mod openai;
pub mod parakeet;
pub mod phonon;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use fluss_core::config::{Config, CustomEndpoint, Device, Priority};
use fluss_core::models::{self, EngineKind};
use fluss_core::protocol::ModelPhase;
use fluss_core::text::Transcript;

use crate::app::Ev;

#[derive(Debug, Clone, Default)]
pub struct DecodeOpts {
    pub hotwords: Vec<String>,
    /// ISO code or None for auto.
    pub language: Option<String>,
}

pub trait Engine: Send {
    fn transcribe(&mut self, pcm: &[f32], opts: &DecodeOpts) -> Result<Transcript>;
    /// Live previews are cheap enough to run continuously.
    fn supports_previews(&self) -> bool {
        true
    }
    /// Where it runs, for the UI ("CPU", "GPU (DirectML)", "Remote").
    fn device(&self) -> String;
    fn memory_mb(&self) -> Option<u64> {
        None
    }
}

/// Everything needed to (re)load an engine; compared to decide on reloads.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineSpec {
    pub id: String,
    pub device: Device,
    pub threads: u32,
    pub priority: Priority,
    pub custom: CustomEndpoint,
}

impl EngineSpec {
    pub fn from_config(c: &Config) -> Self {
        Self {
            id: c.model.id.clone(),
            device: c.model.device,
            threads: c.model.threads,
            priority: c.model.load_priority,
            custom: c.custom.clone(),
        }
    }

    pub fn name(&self) -> String {
        models::find(&self.id).map(|m| m.name.to_string()).unwrap_or_else(|| self.id.clone())
    }
}

/// Is the model for `id` present on disk (or not needed)?
pub fn installed(id: &str) -> bool {
    match models::find(id).map(|m| m.engine) {
        Some(EngineKind::Phonon) => phonon::installed(id),
        Some(EngineKind::Parakeet) => parakeet::installed(),
        Some(EngineKind::Custom) => true,
        None => false,
    }
}

fn load(spec: &EngineSpec, progress: &dyn Fn(&str, Option<f32>)) -> Result<Box<dyn Engine>> {
    match models::find(&spec.id).map(|m| m.engine) {
        Some(EngineKind::Phonon) => Ok(Box::new(phonon::Phonon::start(spec, progress)?)),
        Some(EngineKind::Parakeet) => Ok(Box::new(parakeet::Parakeet::load(spec, progress)?)),
        Some(EngineKind::Custom) => Ok(Box::new(openai::OpenAi::new(&spec.custom)?)),
        None => anyhow::bail!("unknown model `{}`", spec.id),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Preview,
    Final,
    /// One-shot file transcription for the CLI.
    File,
}

pub struct Job {
    pub id: u64,
    pub session: u64,
    pub phrase: usize,
    pub kind: JobKind,
    pub audio: Arc<Vec<f32>>,
    /// Sample range of the phrase inside the session audio (previews).
    pub range: (usize, usize),
    pub opts: DecodeOpts,
}

pub struct JobDone {
    pub id: u64,
    pub session: u64,
    pub phrase: usize,
    pub kind: JobKind,
    pub range: (usize, usize),
    pub result: Result<Transcript, String>,
    pub decode_ms: u32,
}

pub enum EngineCmd {
    Configure(EngineSpec),
    /// Load now. `urgent` = the user is waiting (normal priority, no delay).
    Load { urgent: bool },
    Unload,
    Job(Job),
    /// Drop queued work for a session (cancelled dictation).
    Cancel(u64),
    Shutdown,
}

pub enum EngineEv {
    State { id: String, phase: ModelPhase, detail: String, progress: Option<f32>, device: String, memory_mb: Option<u64>, error: Option<String> },
    Done(JobDone),
}

pub fn spawn(ev: Sender<Ev>) -> Sender<EngineCmd> {
    let (tx, rx) = crossbeam_channel::unbounded();
    std::thread::Builder::new()
        .name("fluss-engine".into())
        .spawn(move || EngineThread { rx, ev, spec: None, engine: None, finals: VecDeque::new(), preview: None }.run())
        .expect("spawn engine thread");
    tx
}

struct EngineThread {
    rx: Receiver<EngineCmd>,
    ev: Sender<Ev>,
    spec: Option<EngineSpec>,
    engine: Option<Box<dyn Engine>>,
    finals: VecDeque<Job>,
    preview: Option<Job>,
}

impl EngineThread {
    fn state(&self, phase: ModelPhase, detail: &str, progress: Option<f32>, error: Option<String>) {
        let (device, mem) = match &self.engine {
            Some(e) => (e.device(), e.memory_mb()),
            None => (String::new(), None),
        };
        let _ = self.ev.send(Ev::Engine(EngineEv::State {
            id: self.spec.as_ref().map(|s| s.id.clone()).unwrap_or_default(),
            phase,
            detail: detail.into(),
            progress,
            device,
            memory_mb: mem,
            error,
        }));
    }

    fn accept(&mut self, cmd: EngineCmd, want_load: &mut Option<bool>) -> bool {
        match cmd {
            EngineCmd::Configure(spec) => {
                if self.spec.as_ref() != Some(&spec) {
                    let was_loaded = self.engine.is_some();
                    self.engine = None;
                    self.spec = Some(spec);
                    if was_loaded {
                        *want_load = Some(false);
                    } else {
                        self.idle_state();
                    }
                }
            }
            EngineCmd::Load { urgent } => {
                *want_load = Some(urgent || want_load.unwrap_or(false));
            }
            EngineCmd::Unload => {
                *want_load = None;
                if self.engine.take().is_some() {
                    log::info!("model unloaded");
                }
                self.idle_state();
            }
            EngineCmd::Job(j) => match j.kind {
                JobKind::Preview => self.preview = Some(j),
                _ => self.finals.push_back(j),
            },
            EngineCmd::Cancel(s) => {
                self.finals.retain(|j| j.session != s);
                if self.preview.as_ref().is_some_and(|j| j.session == s) {
                    self.preview = None;
                }
            }
            EngineCmd::Shutdown => return false,
        }
        true
    }

    fn idle_state(&self) {
        let id = self.spec.as_ref().map(|s| s.id.clone()).unwrap_or_default();
        if installed(&id) {
            self.state(ModelPhase::Unloaded, "", None, None);
        } else {
            self.state(ModelPhase::NotInstalled, "", None, None);
        }
    }

    fn run(mut self) {
        let mut want_load: Option<bool> = None;
        loop {
            // Block only when there is nothing to do.
            let idle = self.finals.is_empty() && self.preview.is_none() && want_load.is_none();
            let first = if idle {
                match self.rx.recv() {
                    Ok(c) => Some(c),
                    Err(_) => return,
                }
            } else {
                None
            };
            if let Some(c) = first {
                if !self.accept(c, &mut want_load) {
                    return;
                }
            }
            while let Ok(c) = self.rx.try_recv() {
                if !self.accept(c, &mut want_load) {
                    return;
                }
            }

            let has_work = !self.finals.is_empty() || self.preview.is_some();
            if self.engine.is_none() && (want_load.is_some() || !self.finals.is_empty()) {
                let urgent = want_load.unwrap_or(true) || !self.finals.is_empty();
                want_load = None;
                self.do_load(urgent);
                continue;
            }
            want_load = None;
            if !has_work {
                continue;
            }
            let Some(job) = self.finals.pop_front().or_else(|| self.preview.take()) else { continue };
            let Some(engine) = self.engine.as_mut() else {
                // Load failed: fail finals so the session can end cleanly.
                self.reply(job, Err("model is not loaded".into()), 0);
                continue;
            };
            if job.kind == JobKind::Preview && !engine.supports_previews() {
                continue;
            }
            let t = Instant::now();
            let r = engine.transcribe(&job.audio, &job.opts).map_err(|e| format!("{e:#}"));
            let ms = t.elapsed().as_millis() as u32;
            if let Err(e) = &r {
                log::warn!("decode failed: {e}");
            }
            self.reply(job, r, ms);
        }
    }

    fn reply(&self, job: Job, result: Result<Transcript, String>, decode_ms: u32) {
        let _ = self.ev.send(Ev::Engine(EngineEv::Done(JobDone {
            id: job.id,
            session: job.session,
            phrase: job.phrase,
            kind: job.kind,
            range: job.range,
            result,
            decode_ms,
        })));
    }

    fn do_load(&mut self, urgent: bool) {
        let Some(spec) = self.spec.clone() else { return };
        if !installed(&spec.id) {
            self.state(ModelPhase::NotInstalled, "Model not installed", None, None);
            // Fail anything waiting so sessions end.
            let jobs: Vec<Job> = self.finals.drain(..).collect();
            for j in jobs {
                self.reply(j, Err("model is not installed".into()), 0);
            }
            return;
        }
        let low = !urgent && spec.priority == Priority::Low;
        #[cfg(windows)]
        let _prio = crate::platform::ThreadPriorityGuard::background(low);
        let t = Instant::now();
        self.state(ModelPhase::Loading, "Loading model", None, None);
        let ev = self.ev.clone();
        let id = spec.id.clone();
        let progress = move |detail: &str, p: Option<f32>| {
            let _ = ev.send(Ev::Engine(EngineEv::State {
                id: id.clone(),
                phase: ModelPhase::Loading,
                detail: detail.into(),
                progress: p,
                device: String::new(),
                memory_mb: None,
                error: None,
            }));
        };
        match load(&spec, &progress) {
            Ok(e) => {
                log::info!("model {} ready on {} in {:.1}s", spec.id, e.device(), t.elapsed().as_secs_f32());
                self.engine = Some(e);
                self.state(ModelPhase::Ready, "", None, None);
            }
            Err(e) => {
                log::error!("loading {} failed: {e:#}", spec.id);
                self.state(ModelPhase::Error, "Could not load the model", None, Some(format!("{e:#}")));
                let jobs: Vec<Job> = self.finals.drain(..).collect();
                for j in jobs {
                    self.reply(j, Err(format!("model failed to load: {e:#}")), 0);
                }
                self.preview = None;
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

/// Load a model in-process and time repeated decodes (developer tool).
pub fn bench(id: &str, wav: &std::path::Path) -> Result<()> {
    let mut cfg = Config::load().unwrap_or_default();
    cfg.model.id = id.to_string();
    let spec = EngineSpec::from_config(&cfg);
    let t = Instant::now();
    let mut e = load(&spec, &|step, p| eprintln!("  {step} {p:?}"))?;
    println!("loaded {} on {} in {} ms", spec.name(), e.device(), t.elapsed().as_millis());
    let pcm = crate::dsp::read_wav(wav)?;
    for secs in [1.0f32, 3.0, pcm.len() as f32 / 16000.0] {
        let n = ((secs * 16000.0) as usize).min(pcm.len());
        let mut times = Vec::new();
        let mut text = String::new();
        for _ in 0..5 {
            let t = Instant::now();
            text = e.transcribe(&pcm[..n], &DecodeOpts::default())?.text;
            times.push(t.elapsed().as_secs_f32() * 1000.0);
        }
        println!("{secs:>5.1}s audio: {:?} ms  {:?}", times.iter().map(|x| x.round() as u32).collect::<Vec<_>>(), text.chars().take(50).collect::<String>());
    }
    Ok(())
}
