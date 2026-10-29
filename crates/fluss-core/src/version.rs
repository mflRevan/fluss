//! Semantic versions, just enough to decide "is this release newer?".
//! `0.2.0`, `v0.2.0` and `0.2.0-beta.1` parse; a pre-release sorts before its release.

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Dot-separated pre-release identifiers (`beta.1`); empty for a release.
    pub pre: String,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        // Build metadata (`+abc`) never affects ordering.
        let s = s.split('+').next()?;
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) if !p.is_empty() => (c, p.to_string()),
            Some(_) => return None,
            None => (s, String::new()),
        };
        let mut it = core.split('.');
        let num = |p: Option<&str>| p.filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).and_then(|p| p.parse().ok());
        let v = Self { major: num(it.next())?, minor: num(it.next())?, patch: num(it.next())?, pre };
        it.next().is_none().then_some(v)
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    /// The version of this build.
    pub fn current() -> Self {
        Self::parse(crate::VERSION).expect("crate version is semver")
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(o.major, o.minor, o.patch)).then_with(|| match (self.pre.is_empty(), o.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => cmp_pre(&self.pre, &o.pre),
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Semver precedence: numeric identifiers compare as numbers and sort before alphanumeric ones.
fn cmp_pre(a: &str, b: &str) -> Ordering {
    let mut ai = a.split('.');
    let mut bi = b.split('.');
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        Ok(())
    }
}

/// `a` is strictly newer than `b`; unparseable versions are never newer.
pub fn newer(a: &str, b: &str) -> bool {
    matches!((Version::parse(a), Version::parse(b)), (Some(a), Some(b)) if a > b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        assert_eq!(Version::parse("v1.2.3"), Some(Version { major: 1, minor: 2, patch: 3, pre: String::new() }));
        assert_eq!(Version::parse("0.2.0-beta.1").unwrap().pre, "beta.1");
        assert_eq!(Version::parse("1.0.0+build.5").unwrap().to_string(), "1.0.0");
        for bad in ["", "1", "1.2", "1.2.3.4", "1.x.3", "1.2.3-", "-1.2.3", "1..3"] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert_eq!(Version::current().to_string(), crate::VERSION);
    }

    #[test]
    fn orders() {
        let v = |s| Version::parse(s).unwrap();
        assert!(v("0.2.0") > v("0.1.9"));
        assert!(v("0.10.0") > v("0.9.0"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.2") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.10") > v("1.0.0-rc.9"));
        assert!(v("1.0.0-beta") > v("1.0.0-alpha.5"));
        assert!(v("1.0.0-alpha.1") > v("1.0.0-alpha"));
        assert_eq!(v("v1.0.0").cmp(&v("1.0.0")), Ordering::Equal);
        assert!(newer("0.2.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
    }
}
