//! The personal dictionary: replacement rules mapping what the model *heard*
//! (one or more variants) to what you *meant* (the target).
//!
//! A rule with no variants is a plain vocabulary word: it is sent to the model as
//! a hotword and its casing is enforced (`github` -> `GitHub`).
//! Every variant belongs to exactly one target; adding it to another rule moves it.

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::text::normalize_phrase;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub target: String,
    #[serde(default)]
    pub variants: Vec<String>,
    /// Unix seconds.
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub uses: u64,
}

impl Rule {
    pub fn new(target: &str, variants: &[&str]) -> Self {
        Self { target: target.to_string(), variants: variants.iter().map(|s| s.to_string()).collect(), created: 0, uses: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertOutcome {
    pub rule: Rule,
    pub created: bool,
    /// Variants that were moved here from other rules: (variant, previous target).
    pub moved: Vec<(String, String)>,
    /// Variants that were already on this rule.
    pub existing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub rule: Rule,
    pub score: u32,
    /// Which variant matched the query (if any), for highlighting.
    pub matched_variant: Option<String>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn same_phrase(a: &str, b: &str) -> bool {
    normalize_phrase(a) == normalize_phrase(b)
}

impl RuleSet {
    pub fn load() -> Result<Self> {
        Self::load_from(&crate::paths::rules_file())
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).with_context(|| format!("parsing {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&crate::paths::rules_file())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        crate::paths::write_atomic(path, serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(())
    }

    /// Find a rule by target (case-insensitive, punctuation-insensitive).
    pub fn get(&self, target: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.target.trim().eq_ignore_ascii_case(target.trim()))
    }

    /// Add variants to the rule for `target` (creating it). The target's casing is
    /// updated to what was typed last. Variants equal to the target are dropped.
    pub fn upsert(&mut self, target: &str, variants: &[String]) -> UpsertOutcome {
        let target = target.trim().to_string();
        let mut moved = Vec::new();
        let mut existing = Vec::new();
        let clean: Vec<String> = variants
            .iter()
            .map(|v| v.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|v| !normalize_phrase(v).is_empty() && !same_phrase(v, &target))
            .collect();

        // Move variants away from other targets.
        for r in self.rules.iter_mut() {
            if r.target.trim().eq_ignore_ascii_case(&target) {
                continue;
            }
            r.variants.retain(|v| {
                let hit = clean.iter().any(|c| same_phrase(c, v));
                if hit {
                    moved.push((v.clone(), r.target.clone()));
                }
                !hit
            });
        }

        let idx = self.rules.iter().position(|r| r.target.trim().eq_ignore_ascii_case(&target));
        let created = idx.is_none();
        let idx = idx.unwrap_or_else(|| {
            self.rules.push(Rule { target: target.clone(), variants: vec![], created: now(), uses: 0 });
            self.rules.len() - 1
        });
        let rule = &mut self.rules[idx];
        rule.target = target;
        for v in clean {
            if rule.variants.iter().any(|x| same_phrase(x, &v)) {
                existing.push(v);
            } else {
                rule.variants.push(v);
            }
        }
        UpsertOutcome { rule: rule.clone(), created, moved, existing }
    }

    /// Replace a rule wholesale (edit dialog). `old_target` identifies it.
    pub fn replace(&mut self, old_target: &str, rule: Rule) {
        self.rules.retain(|r| !r.target.trim().eq_ignore_ascii_case(old_target.trim()));
        let vars: Vec<String> = rule.variants.clone();
        let mut out = self.upsert(&rule.target, &vars).rule;
        out.created = if rule.created == 0 { now() } else { rule.created };
        out.uses = rule.uses;
        if let Some(r) = self.rules.iter_mut().find(|r| r.target == out.target) {
            *r = out;
        }
    }

    pub fn remove(&mut self, target: &str) -> bool {
        let n = self.rules.len();
        self.rules.retain(|r| !r.target.trim().eq_ignore_ascii_case(target.trim()));
        self.rules.len() != n
    }

    pub fn remove_variant(&mut self, target: &str, variant: &str) -> bool {
        let Some(r) = self.rules.iter_mut().find(|r| r.target.trim().eq_ignore_ascii_case(target.trim())) else {
            return false;
        };
        let n = r.variants.len();
        r.variants.retain(|v| !same_phrase(v, variant));
        r.variants.len() != n
    }

    pub fn record_use(&mut self, targets: &[String]) {
        for t in targets {
            if let Some(r) = self.rules.iter_mut().find(|r| &r.target == t) {
                r.uses += 1;
            }
        }
    }

    /// Words to bias the model towards: rule targets, most used first.
    pub fn hotwords(&self, max: usize) -> Vec<String> {
        let mut rs: Vec<&Rule> = self.rules.iter().filter(|r| r.target.chars().any(char::is_alphabetic)).collect();
        rs.sort_by(|a, b| b.uses.cmp(&a.uses).then(b.created.cmp(&a.created)));
        rs.into_iter().take(max).map(|r| r.target.clone()).collect()
    }

    pub fn matcher(&self) -> Matcher {
        Matcher::new(self)
    }

    /// Ranked search over targets and variants. Empty query = everything, newest first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let q = query.trim().to_lowercase();
        let mut hits: Vec<SearchHit> = Vec::new();
        for r in &self.rules {
            if q.is_empty() {
                hits.push(SearchHit { rule: r.clone(), score: 0, matched_variant: None });
                continue;
            }
            let t = r.target.to_lowercase();
            let mut best = score(&t, &q).map(|s| s + 20);
            let mut mv = None;
            for v in &r.variants {
                if let Some(s) = score(&v.to_lowercase(), &q) {
                    if best.is_none_or(|b| s > b) {
                        best = Some(s);
                        mv = Some(v.clone());
                    }
                }
            }
            if let Some(s) = best {
                hits.push(SearchHit { rule: r.clone(), score: s, matched_variant: mv });
            }
        }
        if q.is_empty() {
            hits.sort_by(|a, b| b.rule.created.cmp(&a.rule.created).then(a.rule.target.cmp(&b.rule.target)));
        } else {
            hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.rule.target.len().cmp(&b.rule.target.len())));
        }
        hits.truncate(limit);
        hits
    }
}

/// Match quality of `q` in `s`: exact > prefix > word prefix > substring > fuzzy subsequence.
fn score(s: &str, q: &str) -> Option<u32> {
    if s == q {
        return Some(100);
    }
    if s.starts_with(q) {
        return Some(80);
    }
    if s.split_whitespace().any(|w| w.starts_with(q)) {
        return Some(65);
    }
    if s.contains(q) {
        return Some(50);
    }
    // Fuzzy: all query chars in order, scored by how tight they are.
    let sc: Vec<char> = s.chars().collect();
    let mut pos = 0usize;
    let mut first = None;
    for qc in q.chars().filter(|c| !c.is_whitespace()) {
        let found = sc[pos..].iter().position(|c| *c == qc)?;
        if first.is_none() {
            first = Some(pos + found);
        }
        pos += found + 1;
    }
    let span = pos - first.unwrap_or(0);
    let tight = (q.len() * 30 / span.max(1)).min(30) as u32;
    Some(5 + tight)
}

/// Compiled lookup: first normalized word -> candidate phrases (longest first).
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    by_first: HashMap<String, Vec<(Vec<String>, String)>>,
    max_len: usize,
}

impl Matcher {
    pub fn new(set: &RuleSet) -> Self {
        let mut m = Matcher::default();
        for r in &set.rules {
            let mut phrases: Vec<Vec<String>> = r.variants.iter().map(|v| normalize_phrase(v)).collect();
            // The target itself enforces its casing ("github" -> "GitHub").
            let t = normalize_phrase(&r.target);
            if !t.is_empty() && t.iter().all(|w| w.chars().any(char::is_alphabetic)) {
                phrases.push(t);
            }
            for p in phrases.into_iter().filter(|p| !p.is_empty()) {
                m.max_len = m.max_len.max(p.len());
                m.by_first.entry(p[0].clone()).or_default().push((p, r.target.clone()));
            }
        }
        for v in m.by_first.values_mut() {
            v.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        }
        m
    }

    pub fn max_len(&self) -> usize {
        self.max_len.max(1)
    }

    pub fn is_empty(&self) -> bool {
        self.by_first.is_empty()
    }

    /// Longest phrase matching the start of `words` (already normalized).
    pub fn longest_match(&self, words: &[String]) -> Option<(usize, &str)> {
        let first = words.first()?;
        let cands = self.by_first.get(first)?;
        for (p, target) in cands {
            if p.len() <= words.len() && p.iter().zip(words).all(|(a, b)| a == b) {
                return Some((p.len(), target.as_str()));
            }
        }
        None
    }
}

/// Normalized form used to check whether two phrases are the same rule input.
pub fn phrase_key(s: &str) -> String {
    normalize_phrase(s).join(" ")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_merges_and_moves() {
        let mut rs = RuleSet::default();
        let o = rs.upsert("Omarchy", &["on march 9".into(), "Omarchy".into()]);
        assert!(o.created);
        assert_eq!(o.rule.variants, vec!["on march 9"]);
        let o = rs.upsert("omarchy", &["oh marchy".into(), "On March, 9".into()]);
        assert!(!o.created);
        assert_eq!(o.rule.target, "omarchy");
        assert_eq!(o.rule.variants, vec!["on march 9", "oh marchy"]);
        assert_eq!(o.existing, vec!["On March, 9"]);
        let o = rs.upsert("Oh Marky", &["oh marchy".into()]);
        assert_eq!(o.moved, vec![("oh marchy".to_string(), "omarchy".to_string())]);
        assert_eq!(rs.get("OMARCHY").unwrap().variants, vec!["on march 9"]);
    }

    #[test]
    fn search_ranks_and_finds_variants() {
        let mut rs = RuleSet::default();
        rs.upsert("Omarchy", &["on march 9".into()]);
        rs.upsert("Hyprland", &["hyper land".into()]);
        rs.upsert("Kubernetes", &["cooper netties".into()]);
        let h = rs.search("omar", 10);
        assert_eq!(h[0].rule.target, "Omarchy");
        let h = rs.search("hyper", 10);
        assert_eq!(h[0].rule.target, "Hyprland");
        assert_eq!(h[0].matched_variant.as_deref(), Some("hyper land"));
        let h = rs.search("kbnts", 10);
        assert_eq!(h[0].rule.target, "Kubernetes");
        assert_eq!(rs.search("", 10).len(), 3);
        assert!(rs.search("zzz", 10).is_empty());
    }

    #[test]
    fn matcher_longest_first() {
        let mut rs = RuleSet::default();
        rs.upsert("A", &["on march".into()]);
        rs.upsert("B", &["on march 9".into()]);
        let m = rs.matcher();
        let w: Vec<String> = ["on", "march", "9", "x"].iter().map(|s| s.to_string()).collect();
        assert_eq!(m.longest_match(&w), Some((3, "B")));
    }

    #[test]
    fn hotwords_by_use() {
        let mut rs = RuleSet::default();
        rs.upsert("Alpha", &[]);
        rs.upsert("Beta", &[]);
        rs.record_use(&["Beta".into()]);
        assert_eq!(rs.hotwords(1), vec!["Beta"]);
    }
}
