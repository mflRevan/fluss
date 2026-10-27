//! Installing engines: everything lands under Fluss's data dir and can be
//! removed by deleting it. Phonon gets an isolated Python runtime built with
//! `uv` (uv-managed Python, never the system one); Parakeet gets ONNX Runtime
//! and the model files.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use fluss_core::config::Device;
use fluss_core::models::{self, EngineKind};

use crate::engine::{parakeet, phonon};

const UV_VERSION: &str = "0.12.24";
const FERMION_VERSION: &str = "0.2.15";
const ORT_VERSION: &str = "1.24.4";
const DIRECTML_VERSION: &str = "1.15.4";

pub type Progress<'a> = &'a dyn Fn(&str, Option<f32>);

pub fn install(id: &str, device: Device, progress: Progress) -> Result<()> {
    match models::find(id).map(|m| m.engine) {
        Some(EngineKind::Phonon) => install_phonon(id, progress),
        Some(EngineKind::Parakeet) => install_parakeet(device, progress),
        Some(EngineKind::Custom) => Ok(()),
        None => bail!("unknown model `{id}`"),
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(120)))
        .build()
        .into()
}

/// Download `url` to `dest` (via a `.part` file), reporting progress within [lo, hi].
pub fn download(url: &str, dest: &Path, what: &str, lo: f32, hi: f32, progress: Progress) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    if let Some(d) = dest.parent() {
        std::fs::create_dir_all(d)?;
    }
    let part = dest.with_extension("part");
    let mut resp = agent().get(url).call().with_context(|| format!("downloading {url}"))?;
    let total: Option<u64> = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
    let mut f = std::fs::File::create(&part)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut got: u64 = 0;
    let mut last = std::time::Instant::now();
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n])?;
        got += n as u64;
        if last.elapsed() > Duration::from_millis(150) {
            last = std::time::Instant::now();
            let frac = total.map(|t| got as f32 / t.max(1) as f32);
            let mb = got as f32 / 1_048_576.0;
            let label = match total {
                Some(t) => format!("{what} ({mb:.0} / {:.0} MB)", t as f32 / 1_048_576.0),
                None => format!("{what} ({mb:.0} MB)"),
            };
            progress(&label, frac.map(|p| lo + (hi - lo) * p));
        }
    }
    f.flush()?;
    drop(f);
    if let Some(t) = total {
        if got != t {
            bail!("download of {url} was incomplete ({got} of {t} bytes)");
        }
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

fn unzip_entries(zip_path: &Path, wanted: &[(&str, &Path)]) -> Result<()> {
    let f = std::fs::File::open(zip_path)?;
    let mut z = zip::ZipArchive::new(f)?;
    for (inner, out) in wanted {
        let mut e = z.by_name(inner).with_context(|| format!("{inner} missing in {}", zip_path.display()))?;
        if let Some(d) = out.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut o = std::fs::File::create(out)?;
        std::io::copy(&mut e, &mut o)?;
    }
    Ok(())
}

// ------------------------------------------------------------------ Phonon

fn uv_dir() -> PathBuf {
    fluss_core::paths::runtime_dir().join("uv")
}

fn uv_exe() -> PathBuf {
    uv_dir().join(if cfg!(windows) { "uv.exe" } else { "uv" })
}

fn ensure_uv(progress: Progress) -> Result<()> {
    if uv_exe().exists() {
        return Ok(());
    }
    progress("Getting the installer (uv)", Some(0.01));
    #[cfg(windows)]
    {
        let url = format!("https://github.com/astral-sh/uv/releases/download/{UV_VERSION}/uv-x86_64-pc-windows-msvc.zip");
        let zip = uv_dir().join("uv.zip");
        download(&url, &zip, "Getting the installer", 0.01, 0.05, progress)?;
        unzip_entries(&zip, &[("uv.exe", &uv_exe())])?;
        let _ = std::fs::remove_file(zip);
    }
    #[cfg(not(windows))]
    {
        std::fs::create_dir_all(uv_dir())?;
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("curl -LsSf https://astral.sh/uv/{UV_VERSION}/install.sh | sh"))
            .env("UV_UNMANAGED_INSTALL", uv_dir())
            .stdout(Stdio::null())
            .status()?;
        if !status.success() {
            bail!("installing uv failed");
        }
    }
    Ok(())
}

fn uv_env(cmd: &mut Command) {
    let rt = fluss_core::paths::runtime_dir();
    cmd.env("UV_PYTHON_INSTALL_DIR", rt.join("python"))
        .env("UV_CACHE_DIR", rt.join("uv-cache"))
        .env("UV_PYTHON_PREFERENCE", "only-managed")
        .env("UV_NO_PROGRESS", "1")
        .env("NO_COLOR", "1")
        .envs(phonon::env());
}

/// Run a command, streaming stderr lines to the log and `on_line`.
fn run_logged(mut cmd: Command, what: &str, on_line: impl FnMut(&str) + Send + 'static) -> Result<()> {
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    crate::platform::hide_child(&mut cmd, false);
    let mut child = cmd.spawn().with_context(|| format!("{what}: could not start"))?;
    let tail = std::sync::Arc::new(parking_lot::Mutex::new(Vec::<String>::new()));
    let t2 = tail.clone();
    let mut on_line = on_line;
    phonon::pump_lines(child.stderr.take().unwrap(), move |l| {
        log::info!("install: {l}");
        let mut t = t2.lock();
        t.push(l.to_string());
        if t.len() > 15 {
            t.remove(0);
        }
        drop(t);
        on_line(l);
    });
    let status = child.wait()?;
    std::thread::sleep(Duration::from_millis(50));
    if !status.success() {
        bail!("{what} failed:\n{}", tail.lock().join("\n"));
    }
    Ok(())
}

fn install_phonon(id: &str, progress: Progress) -> Result<()> {
    if !phonon::runtime_installed() {
        ensure_uv(progress)?;
        let venv = phonon::venv_dir();
        progress("Setting up Python", Some(0.06));
        let mut c = Command::new(uv_exe());
        c.args(["venv", "--python", "3.12", "--allow-existing"]).arg(&venv);
        uv_env(&mut c);
        run_logged(c, "creating the Python environment", |_| {})?;

        progress("Installing PyTorch (CPU)", Some(0.15));
        let mut c = Command::new(uv_exe());
        c.args(["pip", "install", "--python"]).arg(&venv);
        if cfg!(target_os = "macos") {
            c.args(["torch"]);
        } else {
            c.args(["torch", "--index-url", "https://download.pytorch.org/whl/cpu"]);
        }
        uv_env(&mut c);
        run_logged(c, "installing PyTorch", |_| {})?;

        progress("Installing Phonon", Some(0.55));
        let mut c = Command::new(uv_exe());
        c.args(["pip", "install", "--python"]).arg(&venv);
        c.arg(format!("fermion-research=={FERMION_VERSION}"));
        c.args(["safetensors", "soundfile", "scipy", "zstandard"]);
        if cfg!(target_os = "macos") {
            c.args(["mlx", "mlx-audio", "mlx-lm"]);
        }
        uv_env(&mut c);
        run_logged(c, "installing Phonon", |_| {})?;
        if !phonon::runtime_installed() {
            bail!("the Phonon runtime did not install correctly");
        }
    }
    if !phonon::weights_installed(id) {
        // A one-second transcription downloads, verifies and unpacks the weights.
        progress("Downloading model", Some(0.65));
        let probe = fluss_core::paths::runtime_dir().join("probe.wav");
        std::fs::write(&probe, crate::dsp::wav_bytes(&vec![0.0; 16_000]))?;
        let mut c = Command::new(phonon::fermion_exe());
        c.args(["transcribe", phonon::alias(id)]).arg(&probe);
        c.envs(phonon::env());
        let (tx, rx) = crossbeam_channel::unbounded::<f32>();
        let res = std::thread::scope(|s| {
            let h = s.spawn(move || {
                run_logged(c, "downloading the model", move |l| {
                    if let Some(p) = phonon::parse_percent(l) {
                        let _ = tx.send(p);
                    }
                })
            });
            while !h.is_finished() {
                if let Ok(p) = rx.recv_timeout(Duration::from_millis(100)) {
                    progress("Downloading model", Some(0.65 + 0.33 * p));
                }
            }
            h.join().unwrap()
        });
        let _ = std::fs::remove_file(probe);
        res?;
        if !phonon::weights_installed(id) {
            bail!("the model download did not complete");
        }
    }
    progress("Installed", Some(1.0));
    Ok(())
}

// ------------------------------------------------------------------ Parakeet

fn install_ort(progress: Progress) -> Result<()> {
    if parakeet::ort_dylib().exists() {
        return Ok(());
    }
    let dir = parakeet::ort_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(windows)]
    {
        let ort = dir.join("ort.nupkg");
        download(
            &format!("https://api.nuget.org/v3-flatcontainer/microsoft.ml.onnxruntime.directml/{ORT_VERSION}/microsoft.ml.onnxruntime.directml.{ORT_VERSION}.nupkg"),
            &ort,
            "Getting ONNX Runtime",
            0.0,
            0.08,
            progress,
        )?;
        unzip_entries(&ort, &[("runtimes/win-x64/native/onnxruntime.dll", &dir.join("onnxruntime.dll"))])?;
        let _ = std::fs::remove_file(ort);
        let dml = dir.join("dml.nupkg");
        download(
            &format!("https://api.nuget.org/v3-flatcontainer/microsoft.ai.directml/{DIRECTML_VERSION}/microsoft.ai.directml.{DIRECTML_VERSION}.nupkg"),
            &dml,
            "Getting DirectML",
            0.08,
            0.12,
            progress,
        )?;
        unzip_entries(&dml, &[("bin/x64-win/DirectML.dll", &dir.join("DirectML.dll"))])?;
        let _ = std::fs::remove_file(dml);
    }
    #[cfg(not(windows))]
    {
        let _ = ORT_VERSION;
        let _ = DIRECTML_VERSION;
        bail!("automatic ONNX Runtime install is only implemented on Windows; place {} manually", parakeet::ort_dylib().display());
    }
    #[allow(unreachable_code)]
    Ok(())
}

fn install_parakeet(device: Device, progress: Progress) -> Result<()> {
    install_ort(progress)?;
    let gpu = parakeet::wants_gpu(device);
    let files = parakeet::files(gpu);
    let dir = parakeet::model_dir();
    // Weight progress by rough size so the bar moves evenly.
    let size = |f: &str| -> f32 {
        match f {
            "encoder-model.onnx.data" => 2435.0,
            "encoder-model.int8.onnx" => 652.0,
            "decoder_joint-model.int8.onnx" => 18.0,
            "encoder-model.onnx" => 42.0,
            _ => 0.5,
        }
    };
    let total: f32 = files.iter().map(|f| size(f)).sum();
    let mut done = 0.0;
    for f in files {
        let lo = 0.12 + 0.88 * done / total;
        done += size(f);
        let hi = 0.12 + 0.88 * done / total;
        download(&format!("https://huggingface.co/{}/resolve/main/{f}", parakeet::REPO), &dir.join(f), "Downloading Parakeet", lo, hi, progress)?;
    }
    progress("Installed", Some(1.0));
    Ok(())
}
