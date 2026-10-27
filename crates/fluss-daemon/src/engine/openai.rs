//! Any OpenAI-compatible `POST {base}/audio/transcriptions` endpoint
//! (OpenAI, Groq, Fireworks, whisper.cpp server, speaches, `fermion serve`, ...).

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use fluss_core::config::CustomEndpoint;
use fluss_core::text::{Transcript, Word};
use serde::Deserialize;

use super::phonon::multipart;
use super::{DecodeOpts, Engine};
use crate::dsp;

pub struct OpenAi {
    cfg: CustomEndpoint,
    agent: ureq::Agent,
    remote: bool,
}

#[derive(Deserialize)]
struct W {
    word: String,
    start: f32,
    end: f32,
}

#[derive(Deserialize)]
struct Resp {
    text: String,
    #[serde(default)]
    words: Vec<W>,
}

impl OpenAi {
    pub fn new(cfg: &CustomEndpoint) -> Result<Self> {
        if cfg.base_url.trim().is_empty() {
            bail!("set a base URL for the custom endpoint (Settings → Model → Custom endpoint)");
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(cfg.timeout_s.max(3) as u64)))
            .http_status_as_error(false)
            .build()
            .into();
        let host = cfg.base_url.to_ascii_lowercase();
        let remote = !(host.contains("://127.") || host.contains("://localhost") || host.contains("://[::1]"));
        Ok(Self { cfg: cfg.clone(), agent, remote })
    }

    fn url(&self) -> String {
        let b = self.cfg.base_url.trim().trim_end_matches('/');
        if b.ends_with("/audio/transcriptions") { b.to_string() } else { format!("{b}/audio/transcriptions") }
    }
}

impl Engine for OpenAi {
    fn transcribe(&mut self, pcm: &[f32], opts: &DecodeOpts) -> Result<Transcript> {
        let mut fields: Vec<(&str, String)> = Vec::new();
        if !self.cfg.model.trim().is_empty() {
            fields.push(("model", self.cfg.model.trim().to_string()));
        }
        if self.cfg.word_timestamps {
            fields.push(("response_format", "verbose_json".into()));
            fields.push(("timestamp_granularities[]", "word".into()));
        } else {
            fields.push(("response_format", "json".into()));
        }
        if let Some(l) = &opts.language {
            fields.push(("language", l.clone()));
        }
        if !opts.hotwords.is_empty() {
            // OpenAI-style vocabulary hint.
            fields.push(("prompt", opts.hotwords.join(", ")));
        }
        let (ctype, body) = multipart(&fields, &dsp::wav_bytes(pcm));
        let mut req = self.agent.post(&self.url()).header("Content-Type", &ctype);
        if !self.cfg.api_key.trim().is_empty() {
            req = req.header("Authorization", &format!("Bearer {}", self.cfg.api_key.trim()));
        }
        let mut r = req.send(&body[..]).map_err(|e| anyhow!("request to {} failed: {e}", self.url()))?;
        let text = r.body_mut().read_to_string()?;
        if r.status() != 200 {
            bail!("endpoint error {}: {}", r.status(), text.chars().take(300).collect::<String>());
        }
        let v: Resp = serde_json::from_str(&text).with_context(|| format!("unexpected response: {}", text.chars().take(200).collect::<String>()))?;
        if v.words.is_empty() {
            return Ok(Transcript::from_text(&v.text, pcm.len() as f32 / dsp::SR as f32));
        }
        // Some servers return bare words; re-attach the punctuation from `text`.
        let words: Vec<Word> = v.words.into_iter().map(|w| Word { text: w.word.trim().to_string(), start: w.start, end: w.end, conf: None }).collect();
        Ok(Transcript { words: reattach_punct(&v.text, words), text: v.text.trim().to_string() })
    }

    fn supports_previews(&self) -> bool {
        self.cfg.live_preview || !self.remote
    }

    fn device(&self) -> String {
        if self.remote { "Remote".into() } else { "Local server".into() }
    }
}

/// If `words` lack punctuation that `text` has, copy each text token onto the word
/// at the same position when the letters agree.
fn reattach_punct(text: &str, mut words: Vec<Word>) -> Vec<Word> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    if toks.len() != words.len() {
        return words;
    }
    for (w, t) in words.iter_mut().zip(toks) {
        if fluss_core::text::normalize_word(&w.text) == fluss_core::text::normalize_word(t) {
            w.text = t.to_string();
        }
    }
    words
}
