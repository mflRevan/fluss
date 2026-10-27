//! Continuation: when you dictate again into the same field (or keep talking
//! after a pause in hands-free mode), the model has already ended the previous
//! phrase with a period it guessed. Fluss re-decodes the seam with context:
//! the tail of the previous audio + the new audio in one pass, so the model
//! itself decides how the two connect ("release. So" vs "release, so").
//! Only the punctuation at the seam is edited; words already typed are never
//! rewritten.

use crate::text::{normalize_word, split_punct, Word};

const SENTENCE_END: &[char] = &['.', '!', '?', '…'];
const AUTO_PUNCT: &[char] = &['.', ',', ';', ':', '!', '?'];

/// An edit at the end of the previously typed text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Seam {
    /// Characters to delete at the end of what was typed before.
    pub backspaces: usize,
    /// Punctuation to type in their place.
    pub insert: String,
    /// The new text starts a sentence.
    pub sentence_start: bool,
}

impl Seam {
    pub fn is_noop(&self) -> bool {
        self.backspaces == 0 && self.insert.is_empty()
    }
}

/// Split a joint decode (previous tail + gap + new audio) at `boundary_s`.
/// Returns (context words, new words with times rebased to `boundary_s`).
pub fn split_joint(words: &[Word], boundary_s: f32) -> (Vec<Word>, Vec<Word>) {
    let mut ctx = Vec::new();
    let mut new = Vec::new();
    for w in words {
        let mid = 0.5 * (w.start + w.end);
        if mid < boundary_s {
            ctx.push(w.clone());
        } else {
            let mut w = w.clone();
            w.start = (w.start - boundary_s).max(0.0);
            w.end = (w.end - boundary_s).max(0.0);
            new.push(w);
        }
    }
    (ctx, new)
}

fn auto_punct_of(word: &str) -> String {
    let (_, _, trail) = split_punct(word);
    trail.chars().filter(|c| AUTO_PUNCT.contains(c)).collect()
}

/// Model-driven repair: compare the punctuation we typed after the previous
/// phrase (`prev_auto_tail`, only the model's own punctuation) with what the
/// joint decode put after the same word.
pub fn repair_with_context(prev_auto_tail: &str, prev_ended_sentence: bool, ctx_words: &[Word]) -> Seam {
    let Some(last) = ctx_words.last() else {
        return Seam { sentence_start: prev_ended_sentence, ..Seam::default() };
    };
    if prev_auto_tail.is_empty() {
        // The user ended it themselves (spoken "period", a rule, a symbol): respect it.
        return Seam { sentence_start: prev_ended_sentence, ..Seam::default() };
    }
    let joint = auto_punct_of(&last.text);
    let sentence_start = joint.chars().any(|c| SENTENCE_END.contains(&c));
    if joint == prev_auto_tail {
        return Seam { sentence_start, ..Seam::default() };
    }
    Seam { backspaces: prev_auto_tail.chars().count(), insert: joint, sentence_start }
}

/// Words that continue a sentence rather than start one.
const CONTINUERS: &[&str] = &[
    "and", "or", "nor", "but", "so", "because", "cause", "which", "that", "who", "whom", "whose", "where", "when",
    "while", "whereas", "if", "then", "than", "to", "with", "without", "for", "of", "plus", "although", "though",
    "unless", "until", "since", "as", "like", "including", "especially", "except", "or", "into", "onto", "from",
];
const COMMA_BEFORE: &[&str] = &["but", "so", "although", "though", "whereas", "especially", "including", "except"];

/// Fallback when no joint decode is available (remote endpoints, seam repair off):
/// undo a guessed period when the next phrase clearly continues the sentence.
pub fn repair_heuristic(prev_auto_tail: &str, prev_ended_sentence: bool, first_new_word: &str, gap_s: f32) -> Seam {
    if prev_auto_tail != "." || gap_s > 2.5 {
        return Seam { sentence_start: prev_ended_sentence, ..Seam::default() };
    }
    let w = normalize_word(first_new_word);
    if !CONTINUERS.contains(&w.as_str()) {
        return Seam { sentence_start: true, ..Seam::default() };
    }
    let insert = if COMMA_BEFORE.contains(&w.as_str()) { ",".to_string() } else { String::new() };
    Seam { backspaces: 1, insert, sentence_start: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f32, end: f32) -> Word {
        Word { text: text.into(), start, end, conf: None }
    }

    #[test]
    fn split_rebases_new_words() {
        let words = vec![w("release,", 0.1, 0.6), w("so", 1.05, 1.2), w("we", 1.2, 1.3)];
        let (ctx, new) = split_joint(&words, 1.0);
        assert_eq!(ctx.len(), 1);
        assert_eq!(new[0].text, "so");
        assert!((new[0].start - 0.05).abs() < 1e-5);
    }

    #[test]
    fn context_repair_replaces_premature_period() {
        let s = repair_with_context(".", true, &[w("release,", 0.0, 0.5)]);
        assert_eq!(s, Seam { backspaces: 1, insert: ",".into(), sentence_start: false });
        let s = repair_with_context(".", true, &[w("release", 0.0, 0.5)]);
        assert_eq!(s, Seam { backspaces: 1, insert: "".into(), sentence_start: false });
        let s = repair_with_context(".", true, &[w("release.", 0.0, 0.5)]);
        assert!(s.is_noop() && s.sentence_start);
    }

    #[test]
    fn context_repair_respects_user_punctuation() {
        let s = repair_with_context("", true, &[w("release,", 0.0, 0.5)]);
        assert!(s.is_noop() && s.sentence_start);
    }

    #[test]
    fn heuristic() {
        assert_eq!(repair_heuristic(".", true, "and", 0.8), Seam { backspaces: 1, insert: "".into(), sentence_start: false });
        assert_eq!(repair_heuristic(".", true, "But", 0.8), Seam { backspaces: 1, insert: ",".into(), sentence_start: false });
        assert!(repair_heuristic(".", true, "Tomorrow", 0.8).is_noop());
        assert!(repair_heuristic(".", true, "and", 5.0).is_noop());
        assert!(repair_heuristic("?", true, "and", 0.5).is_noop());
    }
}
