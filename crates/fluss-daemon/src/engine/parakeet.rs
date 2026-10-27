//! NVIDIA Parakeet TDT 0.6B v3 in-process through ONNX Runtime.
//!
//! Pipeline (istupakov/parakeet-tdt-0.6b-v3-onnx export): `nemo128.onnx`
//! (log-mel features, CPU) -> encoder (GPU via DirectML/CoreML, or int8 on CPU)
//! -> greedy TDT decode with `decoder_joint` (small, always CPU: one call per
//! emitted token or skipped frame, where GPU launch overhead would dominate).
//! ONNX Runtime itself is loaded dynamically, only when this engine is used.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use fluss_core::config::Device;
use fluss_core::text::{Transcript, Word};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use super::{DecodeOpts, Engine, EngineSpec};

pub const REPO: &str = "istupakov/parakeet-tdt-0.6b-v3-onnx";
const FRAME_S: f32 = 0.08; // 8x subsampling of 10 ms hops
const MAX_SYMBOLS_PER_FRAME: usize = 10;
/// DirectML compiles a fast graph for the first input shape only; every other
/// shape takes a slow path (~6x). So on the GPU all audio is padded to one fixed
/// window and the true length is passed in (the encoder masks the padding).
const GPU_WINDOW: usize = 32 * 16_000;

pub fn model_dir() -> PathBuf {
    fluss_core::paths::models_dir().join("parakeet-tdt-0.6b-v3")
}

pub fn ort_dir() -> PathBuf {
    fluss_core::paths::runtime_dir().join("onnxruntime")
}

pub fn ort_dylib() -> PathBuf {
    let name = if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    ort_dir().join(name)
}

/// Files needed for the GPU (fp32) and CPU (int8) variants.
pub fn files(gpu: bool) -> Vec<&'static str> {
    let mut v = vec!["config.json", "vocab.txt", "nemo128.onnx", "decoder_joint-model.int8.onnx"];
    if gpu {
        v.extend(["encoder-model.onnx", "encoder-model.onnx.data"]);
    } else {
        v.push("encoder-model.int8.onnx");
    }
    v
}

pub fn gpu_supported() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

pub fn wants_gpu(device: Device) -> bool {
    match device {
        Device::Cpu => false,
        Device::Gpu | Device::Auto => gpu_supported(),
    }
}

fn current_device() -> Device {
    fluss_core::config::Config::load().map(|c| c.model.device).unwrap_or(Device::Auto)
}

pub fn installed() -> bool {
    installed_for(current_device())
}

pub fn installed_for(device: Device) -> bool {
    let d = model_dir();
    ort_dylib().exists() && files(wants_gpu(device)).iter().all(|f| d.join(f).exists())
}

static ORT_INIT: OnceLock<Result<(), String>> = OnceLock::new();

fn init_ort() -> Result<()> {
    ORT_INIT
        .get_or_init(|| {
            let lib = ort_dylib();
            // DirectML.dll must be found next to onnxruntime.dll.
            #[cfg(windows)]
            crate::platform::add_dll_dir(&ort_dir());
            ort::init_from(&lib)
                .map_err(|e| format!("loading {}: {e}", lib.display()))
                .map(|b| {
                    let _ = b.with_name("fluss").commit();
                })
        })
        .clone()
        .map_err(|e| anyhow!(e))
}

pub struct Parakeet {
    pre: Session,
    enc: Session,
    dec: Session,
    vocab: Vec<String>,
    blank: usize,
    device: String,
    fixed_window: bool,
}

fn cpu_session(path: &Path, threads: usize) -> Result<Session> {
    let r: ort::Result<Session> = (|| {
        let mut b = Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?.with_intra_threads(threads)?;
        b.commit_from_file(path)
    })();
    r.map_err(|e| anyhow!("loading {}: {e}", path.display()))
}

fn gpu_session(path: &Path, threads: usize) -> Result<Session> {
    let r: ort::Result<Session> = (|| {
        // Some encoder ops run on the CPU even with a GPU provider; give them real threads.
        let b = Session::builder()?.with_optimization_level(GraphOptimizationLevel::Level3)?.with_intra_threads(threads)?;
        #[cfg(windows)]
        let b = b
            .with_memory_pattern(false)?
            .with_parallel_execution(false)?
            .with_execution_providers([ort::ep::DirectML::default()
                // Pick the discrete GPU on machines that also have an iGPU.
                .with_performance_preference(ort::ep::directml::PerformancePreference::HighPerformance)
                .with_device_filter(ort::ep::directml::DeviceFilter::Gpu)
                .build()
                .error_on_failure()])?;
        #[cfg(target_os = "macos")]
        let b = b.with_execution_providers([ort::ep::CoreML::default().build().error_on_failure()])?;
        let mut b = b;
        b.commit_from_file(path)
    })();
    r.map_err(|e| anyhow!("loading {}: {e}", path.display()))
}

impl Parakeet {
    pub fn load(spec: &EngineSpec, progress: &dyn Fn(&str, Option<f32>)) -> Result<Self> {
        init_ort()?;
        let dir = model_dir();
        let threads = if spec.threads > 0 {
            spec.threads as usize
        } else {
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8)
        };
        let vocab = load_vocab(&dir.join("vocab.txt"))?;
        let blank = vocab.iter().position(|t| t == "<blk>").unwrap_or(vocab.len() - 1);
        progress("Loading model", Some(0.1));
        let pre = cpu_session(&dir.join("nemo128.onnx"), 1)?;
        let dec = cpu_session(&dir.join("decoder_joint-model.int8.onnx"), 1)?;
        progress("Loading encoder", Some(0.3));
        let gpu = wants_gpu(spec.device);
        let (enc, device) = if gpu {
            match gpu_session(&dir.join("encoder-model.onnx"), threads) {
                Ok(s) => (s, if cfg!(windows) { "GPU (DirectML)" } else { "GPU (CoreML)" }.to_string()),
                Err(e) if spec.device == Device::Auto && dir.join("encoder-model.int8.onnx").exists() => {
                    log::warn!("GPU unavailable ({e:#}), using CPU");
                    (cpu_session(&dir.join("encoder-model.int8.onnx"), threads)?, "CPU".to_string())
                }
                Err(e) => return Err(e),
            }
        } else {
            (cpu_session(&dir.join("encoder-model.int8.onnx"), threads)?, "CPU".to_string())
        };
        let fixed_window = device.starts_with("GPU (DirectML)");
        let mut me = Self { pre, enc, dec, vocab, blank, device, fixed_window };
        progress("Warming up", Some(0.9));
        // The first run compiles the GPU graph; do it now, not on the first sentence.
        me.transcribe(&vec![0.0; 16_000], &DecodeOpts::default())?;
        Ok(me)
    }
}

fn load_vocab(path: &Path) -> Result<Vec<String>> {
    let s = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut v: Vec<(usize, String)> = Vec::new();
    for line in s.lines() {
        let Some((tok, id)) = line.rsplit_once(' ') else { continue };
        v.push((id.trim().parse()?, tok.to_string()));
    }
    v.sort_by_key(|(i, _)| *i);
    Ok(v.into_iter().map(|(_, t)| t).collect())
}

fn softmax_max(logits: &[f32]) -> (usize, f32) {
    let (mut best, mut bv) = (0usize, f32::NEG_INFINITY);
    for (i, &v) in logits.iter().enumerate() {
        if v > bv {
            bv = v;
            best = i;
        }
    }
    let sum: f32 = logits.iter().map(|v| (v - bv).exp()).sum();
    (best, 1.0 / sum)
}

impl Engine for Parakeet {
    fn transcribe(&mut self, pcm: &[f32], _opts: &DecodeOpts) -> Result<Transcript> {
        if pcm.len() < 1600 {
            return Ok(Transcript::default());
        }
        let t0 = std::time::Instant::now();
        let n = pcm.len();
        let wave = Tensor::from_array(([1usize, n], pcm.to_vec()))?;
        let lens = Tensor::from_array(([1usize], vec![n as i64]))?;
        let out = self.pre.run(ort::inputs!["waveforms" => wave, "waveforms_lens" => lens])?;
        let (fshape, feats) = out["features"].try_extract_tensor::<f32>()?;
        let (_, flens) = out["features_lens"].try_extract_tensor::<i64>()?;
        let mut fshape: Vec<usize> = fshape.iter().map(|d| *d as usize).collect();
        let (bins, frames_in) = (fshape[1], fshape[2]);
        let window_frames = GPU_WINDOW / 160 + 1;
        let feats = if self.fixed_window && frames_in <= window_frames {
            // Pad features (frames past `length` are zero, exactly what padded audio produces).
            let mut padded = vec![0f32; bins * window_frames];
            for b in 0..bins {
                padded[b * window_frames..b * window_frames + frames_in].copy_from_slice(&feats[b * frames_in..(b + 1) * frames_in]);
            }
            fshape[2] = window_frames;
            Tensor::from_array((fshape, padded))?
        } else {
            Tensor::from_array((fshape, feats.to_vec()))?
        };
        let flens = Tensor::from_array(([1usize], vec![flens[0]]))?;
        drop(out);
        let t_pre = t0.elapsed();

        let out = self.enc.run(ort::inputs!["audio_signal" => feats, "length" => flens])?;
        let (eshape, enc) = out["outputs"].try_extract_tensor::<f32>()?;
        let (_, elens) = out["encoded_lengths"].try_extract_tensor::<i64>()?;
        if eshape.len() != 3 {
            bail!("unexpected encoder output shape {eshape:?}");
        }
        let (dim, frames) = (eshape[1] as usize, eshape[2] as usize);
        let t_max = (elens[0] as usize).min(frames);
        let enc = enc.to_vec();
        drop(out);
        let t_enc = t0.elapsed();

        let vocab_n = self.blank + 1;
        let mut s1 = vec![0f32; 2 * 640];
        let mut s2 = vec![0f32; 2 * 640];
        let mut tokens: Vec<(usize, usize, f32)> = Vec::new(); // (token, frame, prob)
        let (mut t, mut emitted) = (0usize, 0usize);
        let mut frame = vec![0f32; dim];
        while t < t_max {
            for (c, f) in frame.iter_mut().enumerate() {
                *f = enc[c * frames + t];
            }
            let last = tokens.last().map(|x| x.0).unwrap_or(self.blank) as i32;
            let out = self.dec.run(ort::inputs![
                "encoder_outputs" => Tensor::from_array(([1usize, dim, 1], frame.clone()))?,
                "targets" => Tensor::from_array(([1usize, 1], vec![last]))?,
                "target_length" => Tensor::from_array(([1usize], vec![1i32]))?,
                "input_states_1" => Tensor::from_array(([2usize, 1, 640], s1.clone()))?,
                "input_states_2" => Tensor::from_array(([2usize, 1, 640], s2.clone()))?,
            ])?;
            let (_, logits) = out["outputs"].try_extract_tensor::<f32>()?;
            let (tok, prob) = softmax_max(&logits[..vocab_n]);
            let dur = logits[vocab_n..].iter().enumerate().fold((0usize, f32::NEG_INFINITY), |b, (i, &v)| if v > b.1 { (i, v) } else { b }).0;
            if tok != self.blank {
                let (_, n1) = out["output_states_1"].try_extract_tensor::<f32>()?;
                let (_, n2) = out["output_states_2"].try_extract_tensor::<f32>()?;
                s1.copy_from_slice(n1);
                s2.copy_from_slice(n2);
                tokens.push((tok, t, prob));
                emitted += 1;
            }
            if dur > 0 {
                t += dur;
                emitted = 0;
            } else if tok == self.blank || emitted >= MAX_SYMBOLS_PER_FRAME {
                t += 1;
                emitted = 0;
            }
        }
        log::debug!("parakeet: {:.1}s audio, features {} ms, encoder {} ms, decoder {} ms ({} frames)", n as f32 / 16000.0, t_pre.as_millis(), (t_enc - t_pre).as_millis(), (t0.elapsed() - t_enc).as_millis(), t_max);
        Ok(self.to_transcript(&tokens, t_max))
    }

    fn device(&self) -> String {
        self.device.clone()
    }

    fn memory_mb(&self) -> Option<u64> {
        Some(if self.device.starts_with("GPU") { 2600 } else { 1100 })
    }
}

impl Parakeet {
    fn to_transcript(&self, tokens: &[(usize, usize, f32)], t_max: usize) -> Transcript {
        let mut words: Vec<Word> = Vec::new();
        for (i, &(tok, frame, prob)) in tokens.iter().enumerate() {
            let piece = &self.vocab[tok];
            let starts_word = piece.starts_with('\u{2581}') || words.is_empty();
            let text = piece.trim_start_matches('\u{2581}');
            let next_frame = tokens.get(i + 1).map(|x| x.1).unwrap_or(t_max).max(frame + 1);
            let end = (next_frame.min(frame + 4)) as f32 * FRAME_S;
            if starts_word && !text.is_empty() {
                words.push(Word { text: text.to_string(), start: frame as f32 * FRAME_S, end, conf: Some(prob) });
            } else if let Some(w) = words.last_mut() {
                w.text.push_str(text);
                w.end = end;
                w.conf = Some(w.conf.unwrap_or(1.0).min(prob));
            }
        }
        let text = words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
        Transcript { text, words }
    }
}
