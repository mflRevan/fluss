//! The model catalog: what Fluss can run and how.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EngineKind {
    /// Fermion Research Phonon, served by a managed `fermion serve` process.
    Phonon,
    /// NVIDIA Parakeet TDT 0.6B v3 via ONNX Runtime, in-process (GPU via DirectML/CoreML or CPU).
    Parakeet,
    /// Any OpenAI-compatible transcription endpoint.
    Custom,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub vendor: &'static str,
    pub engine: EngineKind,
    pub tagline: &'static str,
    pub languages: &'static str,
    pub multilingual: bool,
    /// Approximate download size of the model itself.
    pub download_mb: u32,
    /// Approximate resident memory once loaded.
    pub memory_mb: u32,
    pub gpu: bool,
    pub recommended: bool,
    /// Engine-specific handle (Fermion alias / HF repo).
    pub source: &'static str,
}

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "phonon-2.5-en",
        name: "Phonon 2.5 English",
        vendor: "Fermion Research",
        engine: EngineKind::Phonon,
        tagline: "Tiny, fast and accurate. The best default for English.",
        languages: "English",
        multilingual: false,
        download_mb: 158,
        memory_mb: 1500,
        gpu: cfg!(target_os = "macos"),
        recommended: true,
        source: "phonon-2.5-en",
    },
    ModelInfo {
        id: "phonon-2.5",
        name: "Phonon 2.5 Multilingual",
        vendor: "Fermion Research",
        engine: EngineKind::Phonon,
        tagline: "25 European languages in the same tiny package.",
        languages: "25 languages (bg, cs, da, de, el, en, es, et, fi, fr, hr, hu, it, lt, lv, mt, nl, pl, pt, ro, ru, sk, sl, sv, uk)",
        multilingual: true,
        download_mb: 170,
        memory_mb: 1600,
        gpu: cfg!(target_os = "macos"),
        recommended: false,
        source: "phonon-2.5",
    },
    ModelInfo {
        id: "parakeet-v3",
        name: "Parakeet TDT 0.6B v3",
        vendor: "NVIDIA",
        engine: EngineKind::Parakeet,
        tagline: "The full-size original. Runs on your GPU and stays in VRAM.",
        languages: "25 languages",
        multilingual: true,
        download_mb: 2560,
        memory_mb: 2600,
        gpu: cfg!(any(windows, target_os = "macos")),
        recommended: false,
        source: "istupakov/parakeet-tdt-0.6b-v3-onnx",
    },
    ModelInfo {
        id: "custom",
        name: "Custom endpoint",
        vendor: "You",
        engine: EngineKind::Custom,
        tagline: "Any OpenAI-compatible /audio/transcriptions server, local or online.",
        languages: "Depends on the server",
        multilingual: true,
        download_mb: 0,
        memory_mb: 0,
        gpu: false,
        recommended: false,
        source: "",
    },
];

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id.eq_ignore_ascii_case(id))
}
