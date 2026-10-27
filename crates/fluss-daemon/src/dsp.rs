//! Audio DSP: resampling to 16 kHz mono and phrase segmentation (VAD).

pub const SR: usize = 16_000;
/// VAD block: 50 ms.
pub const BLOCK: usize = SR / 20;

/// Biquad low-pass (RBJ cookbook), Direct Form I.
#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn lowpass(fs: f32, fc: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * fc / fs;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 - c) / 2.0 / a0,
            b1: (1.0 - c) / a0,
            b2: (1.0 - c) / 2.0 / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha) / a0,
            ..Default::default()
        }
    }

    #[inline]
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Streaming resampler: mono mixdown, 4th-order Butterworth anti-alias filter
/// at 7.2 kHz, then linear interpolation to 16 kHz. Cheap and plenty for ASR.
pub struct Resampler {
    channels: usize,
    step: f64,
    pos: f64,
    prev: f32,
    filters: [Biquad; 2],
    filter: bool,
    gain: f32,
}

impl Resampler {
    pub fn new(in_rate: u32, channels: usize) -> Self {
        let fs = in_rate as f32;
        let filter = in_rate as usize > SR;
        Self {
            channels: channels.max(1),
            step: in_rate as f64 / SR as f64,
            pos: 0.0,
            prev: 0.0,
            filters: [Biquad::lowpass(fs, 7200.0, 0.5412), Biquad::lowpass(fs, 7200.0, 1.3066)],
            filter,
            gain: 1.0,
        }
    }

    pub fn set_gain(&mut self, g: f32) {
        self.gain = g.clamp(0.1, 8.0);
    }

    /// Interleaved input frames -> 16 kHz mono appended to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let ch = self.channels;
        for frame in input.chunks_exact(ch) {
            let mut x = frame.iter().sum::<f32>() / ch as f32 * self.gain;
            if self.step == 1.0 {
                out.push(x.clamp(-1.0, 1.0));
                continue;
            }
            if self.filter {
                let y = self.filters[0].run(x);
                x = self.filters[1].run(y);
            }
            // Emit every output sample whose position falls between prev and x.
            while self.pos <= 1.0 {
                let t = self.pos as f32;
                out.push((self.prev + (x - self.prev) * t).clamp(-1.0, 1.0));
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = x;
        }
    }
}

/// Energy VAD with an adaptive gate, in 50 ms blocks (mirrors Fermion's live
/// segmenter: a block is speech when its RMS exceeds both a noise floor and a
/// fraction of the loudest block heard so far).
pub struct Vad {
    pub peak: f32,
    noise_floor: f32,
    ratio: f32,
    pending: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub voiced: bool,
}

impl Default for Vad {
    fn default() -> Self {
        Self { peak: 0.0, noise_floor: 0.004, ratio: 0.18, pending: Vec::with_capacity(BLOCK) }
    }
}

impl Vad {
    /// Feed samples; returns completed blocks.
    pub fn feed(&mut self, samples: &[f32], out: &mut Vec<Block>) {
        for &s in samples {
            self.pending.push(s);
            if self.pending.len() == BLOCK {
                let rms = (self.pending.iter().map(|x| x * x).sum::<f32>() / BLOCK as f32).sqrt();
                self.peak = self.peak.max(rms);
                let gate = self.noise_floor.max(self.ratio * self.peak);
                out.push(Block { voiced: rms > gate });
                self.pending.clear();
            }
        }
    }

    /// Carry the loudness memory into a new session, slowly decayed, so the gate
    /// is sensible from the first block.
    pub fn carry(&self) -> Self {
        Self { peak: self.peak * 0.7, ..Self::default() }
    }
}

/// Fixed-capacity ring of the most recent samples (pre-roll).
pub struct Ring {
    buf: Vec<f32>,
    head: usize,
    len: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Self { buf: vec![0.0; cap.max(1)], head: 0, len: 0 }
    }

    pub fn push(&mut self, xs: &[f32]) {
        let cap = self.buf.len();
        for &x in xs {
            self.buf[self.head] = x;
            self.head = (self.head + 1) % cap;
            self.len = (self.len + 1).min(cap);
        }
    }

    /// The last `n` samples, oldest first.
    pub fn last(&self, n: usize) -> Vec<f32> {
        let n = n.min(self.len);
        let cap = self.buf.len();
        let start = (self.head + cap - n) % cap;
        (0..n).map(|i| self.buf[(start + i) % cap]).collect()
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }
}

/// 16-bit PCM WAV bytes for HTTP engines.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(SR as u32).to_le_bytes());
    v.extend_from_slice(&(SR as u32 * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

/// Read a WAV file (PCM16/float32, any rate/channels) as 16 kHz mono.
pub fn read_wav(path: &std::path::Path) -> anyhow::Result<Vec<f32>> {
    use anyhow::{bail, Context};
    let b = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        bail!("{} is not a WAV file", path.display());
    }
    let (mut fmt, mut ch, mut rate, mut bits, mut data) = (0u16, 1u16, 16000u32, 16u16, None);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " && body.len() >= 16 {
            fmt = u16::from_le_bytes([body[0], body[1]]);
            ch = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            bits = u16::from_le_bytes([body[14], body[15]]);
            if fmt == 0xFFFE && body.len() >= 26 {
                fmt = u16::from_le_bytes([body[24], body[25]]);
            }
        } else if id == b"data" {
            data = Some(body);
        }
        i += 8 + len + (len & 1);
    }
    let data = data.context("WAV has no data chunk")?;
    let interleaved: Vec<f32> = match (fmt, bits) {
        (1, 16) => data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect(),
        (3, 32) => data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
        (1, 32) => data.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap()) as f32 / 2147483648.0).collect(),
        _ => bail!("unsupported WAV encoding (format {fmt}, {bits} bit); use 16-bit PCM or float"),
    };
    let mut out = Vec::new();
    Resampler::new(rate, ch as usize).process(&interleaved, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_48k_to_16k_length_and_tone() {
        let n = 48_000;
        let input: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let mut out = Vec::new();
        let mut r = Resampler::new(48_000, 1);
        for c in input.chunks(480) {
            r.process(c, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2, "{}", out.len());
        let rms = (out[1000..].iter().map(|x| x * x).sum::<f32>() / (out.len() - 1000) as f32).sqrt();
        assert!((rms - 0.3535).abs() < 0.02, "rms {rms}");
    }

    #[test]
    fn antialias_kills_high_tone() {
        let input: Vec<f32> = (0..48_000).map(|i| (2.0 * std::f32::consts::PI * 15_000.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
        let mut out = Vec::new();
        Resampler::new(48_000, 1).process(&input, &mut out);
        let rms = (out[1000..].iter().map(|x| x * x).sum::<f32>() / (out.len() - 1000) as f32).sqrt();
        assert!(rms < 0.03, "rms {rms}");
    }

    #[test]
    fn stereo_mixdown_and_44k() {
        let input: Vec<f32> = (0..44_100 * 2).map(|i| if i % 2 == 0 { 0.2 } else { 0.4 }).collect();
        let mut out = Vec::new();
        Resampler::new(44_100, 2).process(&input, &mut out);
        assert!((out.len() as i64 - 16_000).abs() <= 2);
        assert!((out[8000] - 0.3).abs() < 1e-3);
    }

    #[test]
    fn ring_keeps_latest() {
        let mut r = Ring::new(4);
        r.push(&[1.0, 2.0, 3.0]);
        assert_eq!(r.last(10), vec![1.0, 2.0, 3.0]);
        r.push(&[4.0, 5.0, 6.0]);
        assert_eq!(r.last(4), vec![3.0, 4.0, 5.0, 6.0]);
        assert_eq!(r.last(2), vec![5.0, 6.0]);
    }

    #[test]
    fn vad_gates_relative_to_peak() {
        let mut v = Vad::default();
        let mut blocks = Vec::new();
        v.feed(&vec![0.2; BLOCK], &mut blocks);
        v.feed(&vec![0.02; BLOCK], &mut blocks);
        v.feed(&vec![0.001; BLOCK], &mut blocks);
        assert_eq!(blocks.iter().map(|b| b.voiced).collect::<Vec<_>>(), vec![true, false, false]);
    }

    #[test]
    fn wav_roundtrip() {
        let s: Vec<f32> = (0..1600).map(|i| (i as f32 / 100.0).sin() * 0.5).collect();
        let dir = std::env::temp_dir().join("fluss-test-wav");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.wav");
        std::fs::write(&p, wav_bytes(&s)).unwrap();
        let back = read_wav(&p).unwrap();
        assert_eq!(back.len(), s.len());
        assert!((back[500] - s[500]).abs() < 1e-3);
    }
}
