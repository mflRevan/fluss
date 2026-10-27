//! Text post-processing: engine words in, typed text out.
//!
//! The pipeline works on tokens (leading punctuation, core, trailing punctuation)
//! rather than raw strings, so rules, spoken punctuation, filler removal and
//! casing compose without fighting over spaces and commas:
//!
//! 1. dictionary rules (`on march 9` -> `Omarchy`), longest match first
//! 2. spoken punctuation (`comma`, `new line`, `question mark`, ...)
//! 3. filler removal (`um`, `uh`, ...)
//! 4. casing (natural / lowercase) and sentence-start capitalization
//! 5. rendering with correct spacing

use serde::{Deserialize, Serialize};

use crate::config::{Casing, TextCfg};
use crate::rules::Matcher;

/// One word as the engine produced it (punctuation attached), with timing in
/// seconds relative to the start of the decoded audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f32,
    pub end: f32,
    /// Model confidence 0..1 when the engine provides it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conf: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub words: Vec<Word>,
}

impl Transcript {
    /// Build word entries from plain text when an engine gives no timestamps:
    /// spread the words evenly over `duration`.
    pub fn from_text(text: &str, duration: f32) -> Self {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let n = parts.len().max(1) as f32;
        let words = parts
            .iter()
            .enumerate()
            .map(|(i, w)| Word {
                text: (*w).to_string(),
                start: duration * i as f32 / n,
                end: duration * (i + 1) as f32 / n,
                conf: None,
            })
            .collect();
        Self { text: text.trim().to_string(), words }
    }
}

const LEAD: &[char] = &['(', '[', '{', '"', '\'', '“', '‘', '¿', '¡', '«'];
const TRAIL: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '}', '"', '\'', '”', '’', '…', '»'];
const SENTENCE_END: &[char] = &['.', '!', '?', '…'];
/// Punctuation an ASR model inserts on its own between words.
const AUTO_PUNCT: &[char] = &['.', ',', ';', ':', '!', '?'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glue {
    /// Normal word: space on both sides.
    None,
    /// Attach to the previous token (`,` `.` `)` `%`).
    Left,
    /// Attach to the next token (`(` `$` `#`).
    Right,
    /// Attach on both sides (`/` `_` `-` `@` and line breaks).
    Both,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub lead: String,
    pub core: String,
    pub trail: String,
    pub glue: Glue,
    /// Inserted by a rule or a symbol: casing and later stages leave it alone.
    pub protected: bool,
    /// A line break token (`\n` or `\n\n`).
    pub newline: bool,
    pub start: f32,
    pub end: f32,
}

impl Tok {
    fn word(raw: &str, start: f32, end: f32) -> Self {
        let (lead, core, trail) = split_punct(raw);
        Self { lead, core, trail, glue: Glue::None, protected: false, newline: false, start, end }
    }

    fn symbol(s: &str, glue: Glue, start: f32, end: f32) -> Self {
        Self {
            lead: String::new(),
            core: s.to_string(),
            trail: String::new(),
            glue,
            protected: true,
            newline: s.starts_with('\n'),
            start,
            end,
        }
    }

    pub fn norm(&self) -> String {
        normalize_word(&self.core)
    }

    fn is_word(&self) -> bool {
        !self.newline && self.core.chars().any(char::is_alphanumeric)
    }
}

/// Split a raw engine token into (leading punct, core, trailing punct).
/// A token made only of punctuation keeps it all as the core.
pub fn split_punct(raw: &str) -> (String, String, String) {
    let chars: Vec<char> = raw.chars().collect();
    let mut a = 0;
    while a < chars.len() && LEAD.contains(&chars[a]) {
        a += 1;
    }
    let mut b = chars.len();
    while b > a && TRAIL.contains(&chars[b - 1]) {
        b -= 1;
    }
    // Keep a trailing period that belongs to an abbreviation like "U.S." or "e.g."
    if b < chars.len() && chars[b] == '.' && b > a + 1 && chars[a..b].contains(&'.') {
        b += 1;
    }
    if a == b {
        return (String::new(), raw.to_string(), String::new());
    }
    (chars[..a].iter().collect(), chars[a..b].iter().collect(), chars[b..].iter().collect())
}

/// Lowercase, keep letters/digits and inner apostrophes/hyphens. Used for all
/// matching so `March,` == `march` and `Don't` == `don't`.
pub fn normalize_word(s: &str) -> String {
    let t: String = s
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '’' || *c == '-')
        .map(|c| if c == '’' { '\'' } else { c })
        .flat_map(char::to_lowercase)
        .collect();
    t.trim_matches(|c| c == '\'' || c == '-').to_string()
}

/// Normalize a phrase into matching tokens (`"On March, 9."` -> `["on","march","9"]`).
pub fn normalize_phrase(s: &str) -> Vec<String> {
    s.split_whitespace().map(normalize_word).filter(|w| !w.is_empty()).collect()
}

pub fn tokenize(words: &[Word]) -> Vec<Tok> {
    words.iter().filter(|w| !w.text.trim().is_empty()).map(|w| Tok::word(w.text.trim(), w.start, w.end)).collect()
}

/// Context the pipeline needs about what is already in the text field.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    /// The text before the cursor ends a sentence (or the field is empty/new).
    pub sentence_start: bool,
    /// This is the last chunk of the dictation (for `drop_final_period`).
    pub is_final: bool,
}

/// What the pipeline produced.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Processed {
    pub text: String,
    /// Trailing punctuation the *model* put at the end (not spoken by the user and
    /// not from a rule). Continuation may replace it.
    pub auto_tail: String,
    /// Rule targets that fired (for usage stats).
    pub rules_hit: Vec<String>,
}

pub fn process(words: &[Word], cfg: &TextCfg, rules: Option<&Matcher>, ctx: Context) -> Processed {
    let mut toks = tokenize(words);
    let mut hits = Vec::new();
    if cfg.apply_rules {
        if let Some(m) = rules {
            hits = apply_rules(&mut toks, m);
        }
    }
    if cfg.spoken_punctuation {
        apply_spoken_punctuation(&mut toks);
    }
    if cfg.remove_fillers {
        remove_fillers(&mut toks);
    }
    fix_sentence_case(&mut toks, ctx.sentence_start);
    if cfg.casing == Casing::Lowercase {
        lowercase(&mut toks);
    }
    if cfg.drop_final_period && ctx.is_final {
        if let Some(t) = toks.iter_mut().rev().find(|t| !t.newline) {
            if t.trail == "." {
                t.trail.clear();
            }
        }
    }
    let auto_tail = match toks.last() {
        Some(t) if !t.protected && !t.newline => t.trail.chars().filter(|c| AUTO_PUNCT.contains(c)).collect(),
        _ => String::new(),
    };
    Processed { text: render(&toks), auto_tail, rules_hit: hits }
}

/// Render tokens with correct spacing.
pub fn render(toks: &[Tok]) -> String {
    let mut out = String::new();
    let mut prev_glue_right = true; // no space at the very start
    for t in toks {
        let piece = format!("{}{}{}", t.lead, t.core, t.trail);
        if piece.is_empty() {
            continue;
        }
        let attach = prev_glue_right || matches!(t.glue, Glue::Left | Glue::Both);
        if !attach {
            out.push(' ');
        }
        out.push_str(&piece);
        prev_glue_right = matches!(t.glue, Glue::Right | Glue::Both);
    }
    // Spaces never survive at line starts/ends.
    out.replace(" \n", "\n").replace("\n ", "\n")
}

// ---------------------------------------------------------------- rules

fn apply_rules(toks: &mut Vec<Tok>, m: &Matcher) -> Vec<String> {
    let mut hits = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        if toks[i].protected || !toks[i].is_word() {
            i += 1;
            continue;
        }
        let norms: Vec<String> = toks[i..].iter().take(m.max_len()).map(|t| t.norm()).collect();
        if let Some((len, target)) = m.longest_match(&norms) {
            // Never let a match swallow a sentence boundary inside it.
            let span_ok = toks[i..i + len - 1].iter().all(|t| !t.trail.chars().any(|c| SENTENCE_END.contains(&c)));
            if span_ok {
                let lead = toks[i].lead.clone();
                let trail = toks[i + len - 1].trail.clone();
                let (start, end) = (toks[i].start, toks[i + len - 1].end);
                let mut t = Tok::word(target, start, end);
                t.core = target.to_string();
                t.lead = lead;
                t.trail = trail;
                t.protected = true;
                toks.splice(i..i + len, [t]);
                hits.push(target.to_string());
            }
        }
        i += 1;
    }
    hits
}

// ---------------------------------------------------------------- spoken punctuation

struct Spoken {
    phrase: &'static [&'static str],
    out: &'static str,
    glue: Glue,
    /// The word right before must not be one of these (e.g. "a period of time").
    not_after: &'static [&'static str],
    /// Only when the previous token is a number (`50 percent`).
    after_number: bool,
}

const PERIOD_GUARD: &[&str] = &[
    "a", "the", "this", "that", "each", "every", "any", "some", "one", "same", "trial", "grace", "waiting",
    "time", "cooling", "probation", "incubation", "rest", "long", "short", "given", "certain", "whole",
    "entire", "transition", "reporting", "billing", "notice", "free", "test", "my", "your", "her", "his",
];

const SPOKEN: &[Spoken] = &[
    Spoken { phrase: &["new", "paragraph"], out: "\n\n", glue: Glue::Both, not_after: &[], after_number: false },
    Spoken { phrase: &["next", "paragraph"], out: "\n\n", glue: Glue::Both, not_after: &[], after_number: false },
    Spoken { phrase: &["new", "line"], out: "\n", glue: Glue::Both, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["newline"], out: "\n", glue: Glue::Both, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["next", "line"], out: "\n", glue: Glue::Both, not_after: &["the"], after_number: false },
    Spoken { phrase: &["question", "mark"], out: "?", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["exclamation", "mark"], out: "!", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["exclamation", "point"], out: "!", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["full", "stop"], out: ".", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["period"], out: ".", glue: Glue::Left, not_after: PERIOD_GUARD, after_number: false },
    Spoken { phrase: &["comma"], out: ",", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["semicolon"], out: ";", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["semi", "colon"], out: ";", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["colon"], out: ":", glue: Glue::Left, not_after: &["a", "the", "my", "your", "his", "her"], after_number: false },
    Spoken { phrase: &["ellipsis"], out: "...", glue: Glue::Left, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["dot", "dot", "dot"], out: "...", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["open", "parenthesis"], out: "(", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["open", "paren"], out: "(", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["close", "parenthesis"], out: ")", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["close", "paren"], out: ")", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["open", "bracket"], out: "[", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["close", "bracket"], out: "]", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["open", "brace"], out: "{", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["close", "brace"], out: "}", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["open", "quote"], out: "\"", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["close", "quote"], out: "\"", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["end", "quote"], out: "\"", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["unquote"], out: "\"", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["em", "dash"], out: "—", glue: Glue::Both, not_after: &[], after_number: false },
    Spoken { phrase: &["hyphen"], out: "-", glue: Glue::Both, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["forward", "slash"], out: "/", glue: Glue::Both, not_after: &[], after_number: false },
    Spoken { phrase: &["backslash"], out: "\\", glue: Glue::Both, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["underscore"], out: "_", glue: Glue::Both, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["at", "sign"], out: "@", glue: Glue::Both, not_after: &[], after_number: false },
    Spoken { phrase: &["ampersand"], out: "&", glue: Glue::None, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["hashtag"], out: "#", glue: Glue::Right, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["hash", "sign"], out: "#", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["dollar", "sign"], out: "$", glue: Glue::Right, not_after: &[], after_number: false },
    Spoken { phrase: &["percent", "sign"], out: "%", glue: Glue::Left, not_after: &[], after_number: false },
    Spoken { phrase: &["percent"], out: "%", glue: Glue::Left, not_after: &[], after_number: true },
    Spoken { phrase: &["asterisk"], out: "*", glue: Glue::None, not_after: &["a", "the"], after_number: false },
    Spoken { phrase: &["plus", "sign"], out: "+", glue: Glue::None, not_after: &[], after_number: false },
    Spoken { phrase: &["equals", "sign"], out: "=", glue: Glue::None, not_after: &[], after_number: false },
];

fn apply_spoken_punctuation(toks: &mut Vec<Tok>) {
    let mut i = 0;
    while i < toks.len() {
        if toks[i].protected || !toks[i].is_word() {
            i += 1;
            continue;
        }
        let mut matched = None;
        for sp in SPOKEN {
            let n = sp.phrase.len();
            if i + n > toks.len() {
                continue;
            }
            let ok = (0..n).all(|k| !toks[i + k].protected && toks[i + k].norm() == sp.phrase[k]);
            if !ok {
                continue;
            }
            // Inner words of a phrase must not be split by a sentence end ("new. Line").
            if toks[i..i + n - 1].iter().any(|t| t.trail.chars().any(|c| SENTENCE_END.contains(&c))) {
                continue;
            }
            let prev = i.checked_sub(1).map(|p| toks[p].norm()).unwrap_or_default();
            if !prev.is_empty() && sp.not_after.contains(&prev.as_str()) {
                continue;
            }
            if sp.after_number && !prev.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                continue;
            }
            matched = Some((n, sp));
            break;
        }
        let Some((n, sp)) = matched else {
            i += 1;
            continue;
        };
        let (start, end) = (toks[i].start, toks[i + n - 1].end);
        // The model's own punctuation around a spoken symbol is noise: "Hello, comma, world" -> "Hello, world".
        let strips_prev = match sp.glue {
            Glue::Left => true,
            Glue::Both => !sp.out.starts_with('\n'),
            _ => false,
        };
        if i > 0 && strips_prev {
            let p = &mut toks[i - 1];
            if !p.protected {
                p.trail = p.trail.chars().filter(|c| !AUTO_PUNCT.contains(c)).collect();
            }
        }
        let following_trail = toks[i + n - 1].trail.clone();
        let mut sym = Tok::symbol(sp.out, sp.glue, start, end);
        // Keep closing quotes/brackets the model attached after the spoken word.
        sym.trail = following_trail.chars().filter(|c| !AUTO_PUNCT.contains(c)).collect();
        toks.splice(i..i + n, [sym]);
        // The next word was capitalized only because the model thought a sentence ended here.
        if matches!(sp.out, "," | ";" | ":" | "-" | "—" | "/" | "_" | "@" | "&" | "(" | "\"") {
            if let Some(next) = toks.get_mut(i + 1) {
                if !next.protected && starts_sentence_style(&next.core) {
                    decapitalize(&mut next.core);
                }
            }
        }
        i += 1;
    }
}

fn starts_sentence_style(core: &str) -> bool {
    let mut cs = core.chars();
    let Some(first) = cs.next() else { return false };
    // "I", acronyms and words with inner capitals are left alone.
    first.is_uppercase() && core != "I" && !core.starts_with("I'") && cs.all(|c| !c.is_uppercase())
}

fn decapitalize(core: &mut String) {
    let mut cs = core.chars();
    if let Some(f) = cs.next() {
        *core = f.to_lowercase().chain(cs).collect();
    }
}

fn capitalize(core: &mut String) {
    let mut cs = core.chars();
    if let Some(f) = cs.next() {
        *core = f.to_uppercase().chain(cs).collect();
    }
}

// ---------------------------------------------------------------- fillers

const FILLERS: &[&str] = &["um", "umm", "uh", "uhh", "uhm", "erm", "er", "hmm", "mm", "mmm", "ehm"];

fn remove_fillers(toks: &mut Vec<Tok>) {
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.protected || !FILLERS.contains(&t.norm().as_str()) {
            i += 1;
            continue;
        }
        let filler = toks.remove(i);
        let ends_sentence = filler.trail.chars().any(|c| SENTENCE_END.contains(&c));
        if i > 0 {
            let prev = &mut toks[i - 1];
            if ends_sentence && !prev.protected && !prev.trail.chars().any(|c| SENTENCE_END.contains(&c)) {
                // "We went, um." -> "We went."
                prev.trail = prev.trail.trim_end_matches([',', ';', ':']).to_string();
                prev.trail.push('.');
            } else if filler.trail.contains(',') && prev.trail == "," {
                // "I, um, think" -> "I think"
                prev.trail.clear();
            }
        }
        // Keep a leading quote/bracket the filler carried.
        if !filler.lead.is_empty() {
            if let Some(next) = toks.get_mut(i) {
                next.lead = format!("{}{}", filler.lead, next.lead);
            }
        }
    }
}

// ---------------------------------------------------------------- casing

fn fix_sentence_case(toks: &mut [Tok], sentence_start: bool) {
    let mut at_start = sentence_start;
    for t in toks.iter_mut() {
        if t.newline {
            continue; // a line break neither starts nor ends a sentence by itself
        }
        // Mid-sentence words keep the model's casing (proper nouns).
        if at_start && t.is_word() && !t.protected {
            capitalize(&mut t.core);
        }
        if t.is_word() || t.protected {
            let ends = t.trail.chars().any(|c| SENTENCE_END.contains(&c))
                || (t.protected && SENTENCE_END.iter().any(|c| t.core.ends_with(*c)));
            at_start = ends;
        }
    }
    // A continuation (not a sentence start) must not begin with a stray capital.
    if !sentence_start {
        if let Some(first) = toks.iter_mut().find(|t| t.is_word()) {
            if !first.protected && starts_sentence_style(&first.core) && is_common_word(&first.core) {
                decapitalize(&mut first.core);
            }
        }
    }
}

/// Words that are never capitalized mid-sentence; used to undo the model's
/// "new segment = new sentence" capitalization without touching proper nouns.
fn is_common_word(core: &str) -> bool {
    const COMMON: &[&str] = &[
        "a", "an", "the", "and", "but", "or", "so", "because", "which", "that", "who", "whom", "whose", "where",
        "when", "while", "if", "then", "than", "to", "with", "without", "for", "of", "in", "on", "at", "as", "by",
        "from", "into", "about", "after", "before", "until", "since", "though", "although", "unless", "like",
        "it", "its", "it's", "this", "these", "those", "there", "here", "we", "you", "they", "he", "she", "my",
        "your", "our", "their", "his", "her", "is", "are", "was", "were", "be", "been", "have", "has", "had",
        "do", "does", "did", "not", "also", "just", "even", "maybe", "probably", "really", "very", "too",
        "still", "now", "yet", "both", "all", "any", "some", "more", "most", "other", "such", "what", "how",
        "why", "let", "can", "could", "would", "should", "will", "might", "must", "plus", "especially",
    ];
    COMMON.contains(&normalize_word(core).as_str())
}

fn lowercase(toks: &mut [Tok]) {
    for t in toks.iter_mut() {
        if t.protected || t.newline {
            continue;
        }
        let letters: Vec<char> = t.core.chars().filter(|c| c.is_alphabetic()).collect();
        let acronym = letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase());
        if !acronym {
            t.core = t.core.to_lowercase();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, RuleSet};

    fn words(s: &str) -> Vec<Word> {
        Transcript::from_text(s, 1.0).words
    }

    fn run(s: &str) -> String {
        process(&words(s), &TextCfg::default(), None, Context { sentence_start: true, is_final: true }).text
    }

    #[test]
    fn split_punct_cases() {
        assert_eq!(split_punct("(hello),"), ("(".into(), "hello".into(), "),".into()));
        assert_eq!(split_punct("U.S."), ("".into(), "U.S.".into(), "".into()));
        assert_eq!(split_punct("—"), ("".into(), "—".into(), "".into()));
        assert_eq!(split_punct("don't."), ("".into(), "don't".into(), ".".into()));
    }

    #[test]
    fn passthrough() {
        assert_eq!(run("Hey team, quick update on the release."), "Hey team, quick update on the release.");
    }

    #[test]
    fn spoken_punctuation_basic() {
        assert_eq!(run("Hello comma how are you question mark"), "Hello, how are you?");
        assert_eq!(run("Hello, comma, how are you? Question mark."), "Hello, how are you?");
        assert_eq!(run("first line new line second line"), "First line\nsecond line");
        assert_eq!(run("Intro. New paragraph. Body text."), "Intro.\n\nBody text.");
        assert_eq!(run("it grew 50 percent this year"), "It grew 50% this year");
        assert_eq!(run("Send it to me at sign example dot com"), "Send it to me@example dot com");
    }

    #[test]
    fn period_guard() {
        assert_eq!(run("It took a period of time"), "It took a period of time");
        assert_eq!(run("That's it period"), "That's it.");
        assert_eq!(run("Done period Next thing"), "Done. Next thing");
    }

    #[test]
    fn comma_decapitalizes_model_sentence_start() {
        assert_eq!(run("I went home. Comma. Then I slept."), "I went home, then I slept.");
    }

    #[test]
    fn fillers() {
        assert_eq!(run("Um, I think, uh, we should go."), "I think we should go.");
        assert_eq!(run("We went, um."), "We went.");
        assert_eq!(run("so umm yeah"), "So yeah");
    }

    #[test]
    fn rules_apply_longest_first_and_keep_punct() {
        let mut rs = RuleSet::default();
        rs.rules.push(Rule::new("Omarchy", &["on march 9", "oh marchy"]));
        rs.rules.push(Rule::new("March", &["march"]));
        let m = rs.matcher();
        let out = process(&words("I use on March 9, daily."), &TextCfg::default(), Some(&m), Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "I use Omarchy, daily.");
        assert_eq!(out.rules_hit, vec!["Omarchy".to_string()]);
    }

    #[test]
    fn rules_do_not_cross_sentences() {
        let mut rs = RuleSet::default();
        rs.rules.push(Rule::new("Omarchy", &["on march"]));
        let m = rs.matcher();
        let out = process(&words("It was on. March came."), &TextCfg::default(), Some(&m), Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "It was on. March came.");
    }

    #[test]
    fn target_casing_enforced_without_variants() {
        let mut rs = RuleSet::default();
        rs.rules.push(Rule::new("GitHub", &[]));
        let m = rs.matcher();
        let out = process(&words("push it to github."), &TextCfg::default(), Some(&m), Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "Push it to GitHub.");
    }

    #[test]
    fn lowercase_mode_keeps_acronyms_and_rules() {
        let mut rs = RuleSet::default();
        rs.rules.push(Rule::new("Omarchy", &["oh marchy"]));
        let m = rs.matcher();
        let cfg = TextCfg { casing: Casing::Lowercase, ..TextCfg::default() };
        let out = process(&words("The API runs on Oh Marchy. Nice."), &cfg, Some(&m), Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "the API runs on Omarchy. nice.");
    }

    #[test]
    fn drop_final_period() {
        let cfg = TextCfg { drop_final_period: true, ..TextCfg::default() };
        let out = process(&words("See you soon."), &cfg, None, Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "See you soon");
        let out = process(&words("Really?"), &cfg, None, Context { sentence_start: true, is_final: true });
        assert_eq!(out.text, "Really?");
    }

    #[test]
    fn continuation_context_lowercases_common_first_word() {
        let out = process(&words("And then we left."), &TextCfg::default(), None, Context { sentence_start: false, is_final: true });
        assert_eq!(out.text, "and then we left.");
        let out = process(&words("Berlin is next."), &TextCfg::default(), None, Context { sentence_start: false, is_final: true });
        assert_eq!(out.text, "Berlin is next.");
    }

    #[test]
    fn auto_tail_reports_model_punct_only() {
        let out = process(&words("Hello there."), &TextCfg::default(), None, Context { sentence_start: true, is_final: true });
        assert_eq!(out.auto_tail, ".");
        let out = process(&words("Hello there period"), &TextCfg::default(), None, Context { sentence_start: true, is_final: true });
        assert_eq!(out.auto_tail, "");
    }
}
