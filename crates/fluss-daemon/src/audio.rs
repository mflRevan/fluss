//! Microphone capture on its own thread. The cpal callback only resamples to
//! 16 kHz mono and hands the chunk to the controller; nothing in the callback
//! can block on the rest of the app.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender};
use fluss_core::protocol::DeviceInfo;

use crate::app::Ev;
use crate::dsp::Resampler;

pub enum AudioCmd {
    /// Open (or re-open) the input device. Empty name = system default.
    Open { device: String, gain: f32 },
    Close,
    SetGain(f32),
}

pub enum AudioEv {
    Chunk(Vec<f32>),
    Opened { device: String, sample_rate: u32 },
    Closed,
    Error(String),
}

/// Mic level for the overlay/meters, stored as f32 bits (lock-free).
#[derive(Clone, Default)]
pub struct Level(Arc<AtomicU32>);

impl Level {
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

pub fn spawn(ev: Sender<Ev>) -> Sender<AudioCmd> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let me = tx.clone();
    std::thread::Builder::new()
        .name("fluss-audio".into())
        .spawn(move || run(rx, me, ev))
        .expect("spawn audio thread");
    tx
}

fn run(rx: Receiver<AudioCmd>, me: Sender<AudioCmd>, ev: Sender<Ev>) {
    let mut stream: Option<cpal::Stream> = None;
    let gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let mut want: Option<String> = None;
    loop {
        // While a device is wanted but failed (unplugged), retry every 2 s.
        let cmd = match (stream.is_none(), want.is_some()) {
            (true, true) => match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(c) => Some(c),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                Err(_) => return,
            },
            _ => match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            },
        };
        match cmd {
            Some(AudioCmd::Open { device, gain: g }) => {
                gain.store(g.to_bits(), Ordering::Relaxed);
                stream = None;
                want = Some(device);
            }
            Some(AudioCmd::Close) => {
                stream = None;
                want = None;
                let _ = ev.send(Ev::Audio(AudioEv::Closed));
                continue;
            }
            Some(AudioCmd::SetGain(g)) => {
                gain.store(g.to_bits(), Ordering::Relaxed);
                continue;
            }
            None => {}
        }
        if let (None, Some(name)) = (&stream, &want) {
            if let Some(path) = name.strip_prefix("file:") {
                // Test/dev input: stream a WAV in real time as if it were a microphone.
                feed_file(std::path::PathBuf::from(path), ev.clone(), rx.clone());
                want = None;
                continue;
            }
            match open(name, ev.clone(), gain.clone(), me.clone()) {
                Ok((s, dev, rate)) => {
                    log::info!("microphone open: {dev} @ {rate} Hz");
                    let _ = ev.send(Ev::Audio(AudioEv::Opened { device: dev, sample_rate: rate }));
                    stream = Some(s);
                }
                Err(e) => {
                    log::warn!("microphone: {e:#}");
                    let _ = ev.send(Ev::Audio(AudioEv::Error(format!("{e:#}"))));
                }
            }
        }
    }
}

/// Plays `path` (then silence) as 10 ms chunks until a Close/Open command arrives.
fn feed_file(path: std::path::PathBuf, ev: Sender<Ev>, rx: Receiver<AudioCmd>) {
    let pcm = match crate::dsp::read_wav(&path) {
        Ok(p) => p,
        Err(e) => {
            let _ = ev.send(Ev::Audio(AudioEv::Error(format!("{e:#}"))));
            return;
        }
    };
    let _ = ev.send(Ev::Audio(AudioEv::Opened { device: format!("file:{}", path.display()), sample_rate: 16_000 }));
    let start = std::time::Instant::now();
    let mut sent = 0usize;
    loop {
        if let Ok(cmd) = rx.try_recv() {
            if matches!(cmd, AudioCmd::Close | AudioCmd::Open { .. }) {
                let _ = ev.send(Ev::Audio(AudioEv::Closed));
                return;
            }
        }
        let due = (start.elapsed().as_secs_f64() * 16_000.0) as usize;
        while sent + 160 <= due {
            let chunk: Vec<f32> = (sent..sent + 160).map(|i| pcm.get(i).copied().unwrap_or(0.0)).collect();
            let _ = ev.send(Ev::Audio(AudioEv::Chunk(chunk)));
            sent += 160;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn device_name(d: &cpal::Device) -> String {
    d.description().map(|x| x.name().to_string()).unwrap_or_else(|_| "Unknown microphone".into())
}

fn find_device(host: &cpal::Host, name: &str) -> anyhow::Result<cpal::Device> {
    if !name.is_empty() {
        if let Ok(devs) = host.input_devices() {
            for d in devs {
                if device_name(&d) == name {
                    return Ok(d);
                }
            }
        }
        log::warn!("microphone `{name}` not found, using the default");
    }
    host.default_input_device().ok_or_else(|| anyhow::anyhow!("no microphone found"))
}

fn open(name: &str, ev: Sender<Ev>, gain: Arc<AtomicU32>, me: Sender<AudioCmd>) -> anyhow::Result<(cpal::Stream, String, u32)> {
    let host = cpal::default_host();
    let dev = find_device(&host, name)?;
    let dev_name = device_name(&dev);
    let supported = dev.default_input_config()?;
    let rate = supported.sample_rate();
    let rate: u32 = rate.into();
    let channels = supported.channels() as usize;
    let fmt = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();

    let err_ev = ev.clone();
    let want = name.to_string();
    let g2 = gain.clone();
    let on_err = move |e: cpal::Error| {
        use cpal::ErrorKind::*;
        match e.kind() {
            // The device went away or changed: report it and reopen (retries every 2 s).
            DeviceNotAvailable | DeviceChanged | StreamInvalidated | DeviceBusy | PermissionDenied | HostUnavailable => {
                log::warn!("microphone lost: {e}; reopening");
                let _ = err_ev.send(Ev::Audio(AudioEv::Error(e.to_string())));
                let _ = me.send(AudioCmd::Open { device: want.clone(), gain: f32::from_bits(g2.load(Ordering::Relaxed)) });
            }
            // Overruns and similar glitches are harmless.
            _ => log::debug!("audio glitch: {e}"),
        }
    };

    let mut rs = Resampler::new(rate, channels);
    let mut out = Vec::with_capacity(1024);
    let mut scratch: Vec<f32> = Vec::new();
    let mut push = move |data: &[f32]| {
        rs.set_gain(f32::from_bits(gain.load(Ordering::Relaxed)));
        out.clear();
        rs.process(data, &mut out);
        if !out.is_empty() {
            let _ = ev.try_send(Ev::Audio(AudioEv::Chunk(out.clone())));
        }
    };

    let stream = match fmt {
        cpal::SampleFormat::F32 => dev.build_input_stream(config, move |d: &[f32], _: &_| push(d), on_err, None)?,
        cpal::SampleFormat::I16 => dev.build_input_stream(
            config,
            move |d: &[i16], _: &_| {
                scratch.clear();
                scratch.extend(d.iter().map(|s| *s as f32 / 32768.0));
                push(&scratch)
            },
            on_err,
            None,
        )?,
        cpal::SampleFormat::I32 => dev.build_input_stream(
            config,
            move |d: &[i32], _: &_| {
                scratch.clear();
                scratch.extend(d.iter().map(|s| *s as f32 / 2147483648.0));
                push(&scratch)
            },
            on_err,
            None,
        )?,
        cpal::SampleFormat::U16 => dev.build_input_stream(
            config,
            move |d: &[u16], _: &_| {
                scratch.clear();
                scratch.extend(d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0));
                push(&scratch)
            },
            on_err,
            None,
        )?,
        other => anyhow::bail!("unsupported sample format {other:?}"),
    };
    stream.play()?;
    Ok((stream, dev_name, rate))
}

pub fn list_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| device_name(&d)).unwrap_or_default();
    let mut out: Vec<DeviceInfo> = host
        .input_devices()
        .map(|it| it.map(|d| device_name(&d)).map(|n| DeviceInfo { default: n == default, name: n }).collect())
        .unwrap_or_default();
    out.dedup_by(|a, b| a.name == b.name);
    out
}
