//! Phonon (Fermion Research) via a managed `fermion serve` child process.
//! Fluss installs its own isolated Python runtime for it (see `provision`),
//! keeps one keep-alive HTTP connection, and kills the server with itself.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use fluss_core::config::Priority;
use fluss_core::text::{Transcript, Word};
use parking_lot::Mutex;
use serde::Deserialize;

use super::{DecodeOpts, Engine, EngineSpec};
use crate::dsp;

pub fn alias(id: &str) -> &'static str {
    match id {
        "phonon-2.5" => "phonon-2.5",
        _ => "phonon-2.5-en",
    }
}

fn repo_dir(id: &str) -> &'static str {
    match id {
        "phonon-2.5" => "FermionResearch__Phonon-2.5",
        _ => "FermionResearch__Phonon-2.5-English",
    }
}

pub fn venv_dir() -> PathBuf {
    fluss_core::paths::runtime_dir().join("phonon")
}

pub fn fermion_exe() -> PathBuf {
    if cfg!(windows) {
        venv_dir().join("Scripts").join("fermion.exe")
    } else {
        venv_dir().join("bin").join("fermion")
    }
}

pub fn cache_dir() -> PathBuf {
    fluss_core::paths::models_dir().join("fermion")
}

pub fn runtime_installed() -> bool {
    fermion_exe().exists()
}

pub fn weights_installed(id: &str) -> bool {
    let d = cache_dir().join("speech").join(repo_dir(id));
    std::fs::read_dir(&d).map(|mut it| it.any(|e| e.map(|e| e.path().is_dir()).unwrap_or(false))).unwrap_or(false)
}

pub fn installed(id: &str) -> bool {
    runtime_installed() && weights_installed(id)
}

/// Environment for every fermion process: keep all caches inside Fluss's data dir.
pub fn env() -> Vec<(String, String)> {
    let models = fluss_core::paths::models_dir();
    vec![
        ("FERMION_CACHE_DIR".into(), cache_dir().display().to_string()),
        ("HF_HOME".into(), models.join("hf").display().to_string()),
        ("HF_HUB_DISABLE_SYMLINKS_WARNING".into(), "1".into()),
        ("HF_HUB_DISABLE_TELEMETRY".into(), "1".into()),
        ("PYTHONUNBUFFERED".into(), "1".into()),
        ("PYTHONIOENCODING".into(), "utf-8".into()),
        ("NO_COLOR".into(), "1".into()),
    ]
}

/// Parse "45%|" style progress out of tqdm/uv output.
pub fn parse_percent(line: &str) -> Option<f32> {
    let i = line.find("%|")?;
    let digits: String = line[..i].chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let n: f32 = digits.chars().rev().collect::<String>().parse().ok()?;
    Some((n / 100.0).clamp(0.0, 1.0))
}

/// Read a child's stderr, splitting on `\r` and `\n` (tqdm redraws with `\r`).
pub fn pump_lines(mut r: impl Read + Send + 'static, mut on_line: impl FnMut(&str) + Send + 'static) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut line = Vec::new();
        loop {
            let n = match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for &b in &buf[..n] {
                if b == b'\n' || b == b'\r' {
                    if !line.is_empty() {
                        on_line(&String::from_utf8_lossy(&line));
                        line.clear();
                    }
                } else {
                    line.push(b);
                }
            }
        }
        if !line.is_empty() {
            on_line(&String::from_utf8_lossy(&line));
        }
    });
}

fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

pub struct Phonon {
    child: Child,
    base: String,
    agent: ureq::Agent,
    pid: u32,
}

#[derive(Deserialize)]
struct VerboseWord {
    word: String,
    start: f32,
    end: f32,
}

#[derive(Deserialize)]
struct Verbose {
    text: String,
    #[serde(default)]
    words: Vec<VerboseWord>,
}

impl Phonon {
    pub fn start(spec: &EngineSpec, progress: &dyn Fn(&str, Option<f32>)) -> Result<Self> {
        let exe = fermion_exe();
        if !exe.exists() {
            bail!("the Phonon runtime is not installed (run `fluss model install`)");
        }
        let port = free_port()?;
        let mut cmd = Command::new(&exe);
        cmd.args(["serve", alias(&spec.id), "--host", "127.0.0.1", "--port", &port.to_string()]);
        if spec.threads > 0 {
            cmd.args(["--threads", &spec.threads.to_string()]);
        }
        cmd.envs(env()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        let low = spec.priority == Priority::Low;
        #[cfg(windows)]
        crate::platform::hide_child(&mut cmd, low);
        let mut child = cmd.spawn().with_context(|| format!("starting {}", exe.display()))?;
        let pid = child.id();
        #[cfg(windows)]
        crate::platform::bind_child_lifetime(&child);

        let tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let last_pct: Arc<Mutex<Option<(String, f32)>>> = Arc::new(Mutex::new(None));
        {
            let tail = tail.clone();
            let last_pct = last_pct.clone();
            pump_lines(child.stderr.take().unwrap(), move |l| {
                let l = l.trim();
                if l.is_empty() {
                    return;
                }
                if let Some(p) = parse_percent(l) {
                    let what = if l.contains("Fetching") || l.contains(".fsq") || l.contains("Download") {
                        "Downloading model"
                    } else {
                        "Preparing model"
                    };
                    *last_pct.lock() = Some((what.into(), p));
                } else {
                    log::info!("phonon: {l}");
                }
                let mut t = tail.lock();
                t.push(l.to_string());
                if t.len() > 12 {
                    t.remove(0);
                }
            });
        }

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .build()
            .into();
        let base = format!("http://127.0.0.1:{port}");
        let t = Instant::now();
        progress("Starting engine", None);
        loop {
            if let Some(status) = child.try_wait()? {
                let msg = tail.lock().join("\n");
                bail!("the Phonon server exited ({status}):\n{msg}");
            }
            if let Ok(r) = agent.get(&format!("{base}/health")).call() {
                if r.status() == 200 {
                    break;
                }
            }
            if let Some((what, p)) = last_pct.lock().take() {
                progress(&what, Some(p));
            }
            if t.elapsed() > Duration::from_secs(600) {
                let _ = child.kill();
                bail!("the Phonon server did not start within 10 minutes");
            }
            std::thread::sleep(Duration::from_millis(80));
        }
        #[cfg(windows)]
        if low {
            crate::platform::set_process_priority_normal(&child);
        }
        let mut me = Self { child, base, agent, pid };
        // First decode compiles kernels; do it now, not on the user's first sentence.
        progress("Warming up", Some(0.99));
        let _ = me.transcribe(&vec![0.0; dsp::SR / 2], &DecodeOpts::default());
        log::info!("phonon ready on port {port} after {:.1}s", t.elapsed().as_secs_f32());
        Ok(me)
    }
}

pub fn multipart(fields: &[(&str, String)], wav: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("fluss{:016x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7));
    let mut body = Vec::with_capacity(wav.len() + 1024);
    for (k, v) in fields {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n").as_bytes());
    }
    body.extend_from_slice(
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

impl Engine for Phonon {
    fn transcribe(&mut self, pcm: &[f32], opts: &DecodeOpts) -> Result<Transcript> {
        let mut fields = vec![
            ("response_format", "verbose_json".to_string()),
            ("timestamp_granularities[]", "word".to_string()),
        ];
        if !opts.hotwords.is_empty() {
            fields.push(("hotwords", opts.hotwords.iter().take(25).cloned().collect::<Vec<_>>().join(",")));
        }
        let (ctype, body) = multipart(&fields, &dsp::wav_bytes(pcm));
        let mut r = self
            .agent
            .post(&format!("{}/v1/audio/transcriptions", self.base))
            .header("Content-Type", &ctype)
            .send(&body[..])
            .map_err(|e| anyhow!("phonon request failed: {e}"))?;
        let text = r.body_mut().read_to_string()?;
        if r.status() != 200 {
            bail!("phonon error {}: {}", r.status(), text.chars().take(300).collect::<String>());
        }
        let v: Verbose = serde_json::from_str(&text).context("phonon response")?;
        if v.words.is_empty() {
            return Ok(Transcript::from_text(&v.text, pcm.len() as f32 / dsp::SR as f32));
        }
        Ok(Transcript {
            text: v.text.trim().to_string(),
            words: v.words.into_iter().map(|w| Word { text: w.word, start: w.start, end: w.end, conf: None }).collect(),
        })
    }

    fn device(&self) -> String {
        if cfg!(target_os = "macos") { "Apple GPU (MLX)".into() } else { "CPU".into() }
    }

    fn memory_mb(&self) -> Option<u64> {
        #[cfg(windows)]
        return crate::platform::children_memory_mb().or_else(|| crate::platform::process_memory_mb(self.pid));
        #[cfg(not(windows))]
        return None;
    }
}

impl Drop for Phonon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent() {
        assert_eq!(super::parse_percent("phonon-2.5-en.fsq:  45%|####"), Some(0.45));
        assert_eq!(super::parse_percent("Fetching 4 files: 100%|██████████| 4/4"), Some(1.0));
        assert_eq!(super::parse_percent("no progress here"), None);
    }
}
