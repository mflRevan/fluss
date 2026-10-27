//! "What did I just say?" — find the spoken chunk a correction rule should
//! cover. When an unknown word ("Omarchy") is transcribed as several known ones
//! ("on March 9"), the last *word* is wrong but the last *chunk* is right.
//!
//! Each word boundary gets a strength from timing (pauses), audio energy
//! (dips between words), and punctuation. Each word gets an "oddness" from
//! what misrecognitions look like: unusually short words, low confidence or
//! unstable live hypotheses, digits, mid-sentence capitals, and short function
//! words glued to them. The chunk grows backwards from the last word while
//! the words are odd and the boundary is weak, within word and time limits.

use serde::{Deserialize, Serialize};

use crate::text::{normalize_word, split_punct, Word};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChunkGuess {
    /// The recent words shown in the rule popup (punctuation stripped).
    pub context: Vec<String>,
    /// Suggested chunk: `context[start..end]`.
    pub start: usize,
    pub end: usize,
    /// `context[start..end]` joined.
    pub text: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ChunkLimits {
    pub max_words: usize,
    pub max_seconds: f32,
    /// How many recent words the popup shows for manual adjustment.
    pub context_words: usize,
}

impl Default for ChunkLimits {
    fn default() -> Self {
        Self { max_words: 4, max_seconds: 2.2, context_words: 8 }
    }
}

const FUNCTION_WORDS: &[&str] = &[
    "a", "an", "the", "on", "in", "at", "of", "to", "and", "or", "for", "by", "my", "oh", "o", "uh", "is", "it",
    "i", "a.", "as", "be", "we", "he", "she", "me", "no", "so", "do", "up", "um",
];

fn median(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.25;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Energy envelope: RMS per 10 ms frame, frame 0 at t = 0.
fn env_slice(env: &[f32], a: f32, b: f32) -> &[f32] {
    let i = ((a.max(0.0) * 100.0) as usize).min(env.len());
    let j = ((b.max(0.0) * 100.0).ceil() as usize).clamp(i, env.len());
    &env[i..j]
}

fn boundary_strength(prev: &Word, next: &Word, env: Option<&[f32]>) -> f32 {
    let gap = (next.start - prev.end).max(0.0);
    let mut s = (gap / 0.18).min(1.5);
    let (_, _, trail) = split_punct(&prev.text);
    if trail.chars().any(|c| matches!(c, '.' | '!' | '?')) {
        s += 2.5;
    } else if trail.chars().any(|c| matches!(c, ',' | ';' | ':')) {
        s += 1.2;
    }
    if let Some(env) = env {
        let words = env_slice(env, prev.start, next.end);
        let around = env_slice(env, prev.end - 0.03, next.start + 0.03);
        if !words.is_empty() && !around.is_empty() {
            let reference = words.iter().sum::<f32>() / words.len() as f32;
            let floor = around.iter().cloned().fold(f32::INFINITY, f32::min);
            if reference > 1e-6 {
                let dip = (1.0 - floor / reference).clamp(0.0, 1.0);
                if dip > 0.85 {
                    s += 0.6; // near-silence between them: two separate words
                } else if dip < 0.5 {
                    s -= 0.25; // continuous voicing: likely one spoken word
                }
            }
        }
    }
    s
}

fn oddness(words: &[Word], i: usize, med: f32, count_short: bool) -> f32 {
    let w = &words[i];
    let (_, core, _) = split_punct(&w.text);
    let norm = normalize_word(&core);
    let dur = w.end - w.start;
    let short = dur < 0.7 * med;
    let mut o = 0.0;
    if short && count_short {
        o += 0.35;
    }
    if let Some(c) = w.conf {
        if c < 0.8 {
            o += (0.8 - c) * 2.5;
        } else if c >= 0.9 {
            o -= 1.0; // the model was sure about this one
        }
    }
    if norm.chars().any(|c| c.is_ascii_digit()) {
        o += 0.45;
    }
    let prev_ends = i == 0 || {
        let (_, _, t) = split_punct(&words[i - 1].text);
        t.chars().any(|c| matches!(c, '.' | '!' | '?'))
    };
    if !prev_ends && core.chars().next().is_some_and(char::is_uppercase) && core != "I" {
        o += 0.3;
    }
    o
}

/// Guess the chunk at the end of `words` (the latest phrase, times in seconds).
pub fn guess(words: &[Word], env: Option<&[f32]>, limits: ChunkLimits) -> ChunkGuess {
    let words: Vec<Word> = words.iter().filter(|w| !normalize_word(&w.text).is_empty()).cloned().collect();
    let n = words.len();
    if n == 0 {
        return ChunkGuess::default();
    }
    let ctx_from = n.saturating_sub(limits.context_words.max(1));
    let med = median(words.iter().map(|w| w.end - w.start).filter(|d| *d > 0.0).collect()).max(0.12);

    let mut start = n - 1;
    let floor = n.saturating_sub(limits.max_words.max(1)).max(ctx_from);
    while start > floor {
        let j = start - 1;
        let b = boundary_strength(&words[j], &words[j + 1], env);
        if b >= 1.2 {
            break;
        }
        if words[n - 1].end - words[j].start > limits.max_seconds {
            break;
        }
        // Short function words only glue onto a chunk that already looks fragmented
        // ("on" + "March 9"), never onto a single normal word ("and" + "Omarchi").
        let fragmented = n - start >= 2 || words[start..].iter().any(|w| w.text.chars().any(|c| c.is_ascii_digit()) || w.end - w.start < 0.7 * med);
        let (_, jcore, _) = split_punct(&words[j].text);
        let jshort = words[j].end - words[j].start < 0.7 * med;
        let glue = if fragmented && jshort && FUNCTION_WORDS.contains(&normalize_word(&jcore).as_str()) { 0.25 } else { 0.0 };
        let gain = oddness(&words, j, med, fragmented) + glue + 0.5 * oddness(&words, start, med, true) - b;
        if gain < 0.3 {
            break;
        }
        start = j;
    }

    let context: Vec<String> = words[ctx_from..]
        .iter()
        .map(|w| {
            let (_, core, _) = split_punct(&w.text);
            core
        })
        .collect();
    let (s, e) = (start - ctx_from, n - ctx_from);
    let text = context[s..e].join(" ");
    ChunkGuess { context, start: s, end: e, text }
}

/// Per-frame RMS (10 ms frames) of 16 kHz audio, for [`guess`].
pub fn envelope(samples: &[f32]) -> Vec<f32> {
    samples
        .chunks(160)
        .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len().max(1) as f32).sqrt())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f32, end: f32) -> Word {
        Word { text: text.into(), start, end, conf: None }
    }

    #[test]
    fn misheard_name_spans_several_words() {
        let words = vec![w("I", 0.0, 0.12), w("use", 0.2, 0.45), w("on", 0.62, 0.7), w("March", 0.7, 0.95), w("9.", 0.95, 1.1)];
        let g = guess(&words, None, ChunkLimits::default());
        assert_eq!(g.text, "on March 9");
        assert_eq!(g.context, vec!["I", "use", "on", "March", "9"]);
        assert_eq!((g.start, g.end), (2, 5));
    }

    #[test]
    fn function_word_does_not_glue_to_single_name() {
        let words = vec![w("Yesterday", 0.0, 0.5), w("I", 0.5, 0.6), w("installed", 0.6, 1.1), w("Hipperland", 1.15, 1.8), w("and", 1.85, 2.0), w("Omarchi,", 2.0, 2.6)];
        assert_eq!(guess(&words, None, ChunkLimits::default()).text, "Omarchi");
    }

    #[test]
    fn ordinary_last_word_stays_single() {
        let words = vec![w("Let", 0.0, 0.2), w("me", 0.2, 0.35), w("know", 0.35, 0.6), w("if", 0.6, 0.7), w("anything", 0.75, 1.1), w("looks", 1.1, 1.35), w("off.", 1.35, 1.6)];
        let g = guess(&words, None, ChunkLimits::default());
        assert_eq!(g.text, "off");
    }

    #[test]
    fn sentence_boundary_stops_growth() {
        let words = vec![w("Done.", 0.0, 0.3), w("on", 0.3, 0.38), w("March", 0.38, 0.6), w("9", 0.6, 0.75)];
        let g = guess(&words, None, ChunkLimits::default());
        assert_eq!(g.text, "on March 9");
        let words = vec![w("We", 0.0, 0.2), w("use", 0.2, 0.4), w("Hyper", 0.4, 0.6), w("Land", 0.6, 0.8)];
        assert_eq!(guess(&words, None, ChunkLimits::default()).text, "Hyper Land");
    }

    #[test]
    fn low_confidence_glues_words() {
        let mut words = vec![w("deploy", 0.0, 0.4), w("to", 0.4, 0.5), w("cooper", 0.5, 0.8), w("netties", 0.8, 1.2)];
        for (w, c) in words.iter_mut().zip([0.97, 0.95, 0.4, 0.5]) {
            w.conf = Some(c);
        }
        let g = guess(&words, None, ChunkLimits::default());
        assert_eq!(g.text, "cooper netties");
    }

    #[test]
    fn energy_dip_separates_words() {
        // Two odd words but a clear silence between them.
        let words = vec![w("call", 0.0, 0.3), w("Bob", 0.3, 0.5), w("7", 0.5, 0.6)];
        let mut env = vec![0.2f32; 70];
        for e in env.iter_mut().take(52).skip(48) {
            *e = 0.001;
        }
        let g = guess(&words, Some(&env), ChunkLimits::default());
        assert_eq!(g.text, "7");
        let g = guess(&words, None, ChunkLimits::default());
        assert_eq!(g.text, "Bob 7");
    }

    #[test]
    fn limits_respected() {
        let words: Vec<Word> = (0..10).map(|i| w(&format!("{i}"), i as f32 * 0.1, i as f32 * 0.1 + 0.1)).collect();
        let g = guess(&words, None, ChunkLimits { max_words: 3, ..Default::default() });
        assert_eq!(g.end - g.start, 3);
        assert_eq!(g.context.len(), 8);
    }

    #[test]
    fn empty() {
        assert_eq!(guess(&[], None, ChunkLimits::default()), ChunkGuess::default());
    }
}
