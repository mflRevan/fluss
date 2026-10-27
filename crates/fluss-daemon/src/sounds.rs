//! Short, soft UI sounds synthesized in memory (no asset files).

use fluss_core::config::General;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sound {
    Start,
    Stop,
    Cancel,
    Error,
}

const SR: u32 = 44_100;

fn synth(sound: Sound, volume: f32) -> Vec<u8> {
    // (frequency, start s, length s)
    let notes: &[(f32, f32, f32)] = match sound {
        Sound::Start => &[(659.25, 0.0, 0.07), (987.77, 0.045, 0.11)],
        Sound::Stop => &[(987.77, 0.0, 0.07), (659.25, 0.045, 0.12)],
        Sound::Cancel => &[(523.25, 0.0, 0.09)],
        Sound::Error => &[(311.13, 0.0, 0.09), (261.63, 0.08, 0.14)],
    };
    let total = notes.iter().map(|n| n.1 + n.2).fold(0.0, f32::max) + 0.02;
    let n = (total * SR as f32) as usize;
    let mut pcm = vec![0f32; n];
    for &(f, start, len) in notes {
        let a = (start * SR as f32) as usize;
        let m = (len * SR as f32) as usize;
        for i in 0..m.min(n - a) {
            let t = i as f32 / SR as f32;
            let attack = (t / 0.006).min(1.0);
            let decay = (-t * 7.0 / len).exp();
            let w = 2.0 * std::f32::consts::PI * f * t;
            pcm[a + i] += (w.sin() + 0.18 * (2.0 * w).sin()) * attack * decay;
        }
    }
    let gain = 0.22 * volume;
    let mut v = Vec::with_capacity(44 + n * 2);
    let data_len = (n * 2) as u32;
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&SR.to_le_bytes());
    v.extend_from_slice(&(SR * 2).to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        v.extend_from_slice(&(((s * gain).clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

pub fn play(sound: Sound, g: &General) {
    if !g.sounds || g.sound_volume == 0 {
        return;
    }
    imp::play(sound, g.sound_volume);
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::HashMap;
    use std::sync::OnceLock;

    use parking_lot::Mutex;
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    // PlaySound(SND_ASYNC) reads the buffer while playing, so buffers live forever
    // (at most 4 sounds x a few volume levels).
    static CACHE: OnceLock<Mutex<HashMap<(Sound, u8), &'static [u8]>>> = OnceLock::new();

    pub fn play(sound: Sound, volume: u8) {
        let buf = {
            let mut c = CACHE.get_or_init(Default::default).lock();
            *c.entry((sound, volume)).or_insert_with(|| Box::leak(synth(sound, volume as f32 / 100.0).into_boxed_slice()))
        };
        unsafe {
            let _ = PlaySoundW(PCWSTR(buf.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn play(_sound: Sound, _volume: u8) {
        let _ = synth;
    }
}
