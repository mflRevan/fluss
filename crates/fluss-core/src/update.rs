//! Update feed. The release workflow publishes `update.json` on the website:
//!
//! ```json
//! { "version": "0.2.0", "url": "https://github.com/.../Fluss_0.2.0_x64-setup.exe",
//!   "sha256": "…", "size": 12345678, "notes": "https://github.com/.../releases/tag/v0.2.0",
//!   "date": "2026-11-02", "min_supported": "0.1.0", "message": "" }
//! ```
//!
//! `min_supported` and `message` are hand-edited in `site/update.json`: running a
//! version below `min_supported` shows "no longer supported" with the message.
//! If the website can't be reached, the latest GitHub release is used instead.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::version::{newer, Version};

pub const MANIFEST_URL: &str = "https://fluss-mauve.vercel.app/update.json";
pub const GITHUB_LATEST_URL: &str = "https://api.github.com/repos/mflRevan/fluss/releases/latest";

/// The feed URL; `FLUSS_UPDATE_URL` overrides it (testing, mirrors).
pub fn manifest_url() -> String {
    std::env::var("FLUSS_UPDATE_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| MANIFEST_URL.into())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Manifest {
    pub version: String,
    /// Installer download (Windows x64 NSIS).
    pub url: String,
    /// Lowercase hex SHA-256 of the installer.
    pub sha256: Option<String>,
    pub size: Option<u64>,
    /// Release notes page.
    pub notes: Option<String>,
    pub date: Option<String>,
    /// Versions older than this are no longer supported.
    pub min_supported: Option<String>,
    /// Shown with an update prompt (e.g. why updating matters).
    pub message: Option<String>,
}

impl Manifest {
    /// Reject anything we would not want to download and run.
    pub fn validate(&self) -> Result<()> {
        let Some(v) = Version::parse(&self.version) else { bail!("update feed: bad version `{}`", self.version) };
        if v.is_prerelease() {
            bail!("update feed: pre-release {v} is not offered");
        }
        // Plain http only on this machine (a local test feed); never over the network.
        let local = self.url.starts_with("http://127.0.0.1:") || self.url.starts_with("http://localhost:");
        if !self.url.starts_with("https://") && !local {
            bail!("update feed: installer URL must be https");
        }
        if let Some(h) = &self.sha256 {
            if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("update feed: bad sha256");
            }
        }
        if let Some(m) = &self.min_supported {
            if Version::parse(m).is_none() {
                bail!("update feed: bad min_supported `{m}`");
            }
        }
        Ok(())
    }

    /// Build a manifest from GitHub's `releases/latest` response.
    pub fn from_github(v: &Value) -> Option<Self> {
        let tag = v["tag_name"].as_str()?;
        let asset = v["assets"].as_array()?.iter().find(|a| a["name"].as_str().is_some_and(|n| n.ends_with("_x64-setup.exe")))?;
        Some(Self {
            version: tag.trim_start_matches('v').to_string(),
            url: asset["browser_download_url"].as_str()?.to_string(),
            sha256: asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase),
            size: asset["size"].as_u64(),
            notes: v["html_url"].as_str().map(String::from),
            date: v["published_at"].as_str().map(|d| d.chars().take(10).collect()),
            min_supported: None,
            message: None,
        })
    }

    /// A newer version than `current` is out.
    pub fn is_newer_than(&self, current: &str) -> bool {
        newer(&self.version, current)
    }

    /// `current` is below the supported minimum.
    pub fn unsupported(&self, current: &str) -> bool {
        self.min_supported.as_deref().is_some_and(|m| newer(m, current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn m(v: &str) -> Manifest {
        Manifest { version: v.into(), url: "https://example.com/Fluss_x64-setup.exe".into(), ..Default::default() }
    }

    #[test]
    fn validates() {
        assert!(m("0.2.0").validate().is_ok());
        assert!(m("0.2.0-rc.1").validate().is_err());
        assert!(m("latest").validate().is_err());
        assert!(Manifest { url: "http://x".into(), ..m("0.2.0") }.validate().is_err());
        assert!(Manifest { url: "http://localhost.evil.com/x".into(), ..m("0.2.0") }.validate().is_err());
        assert!(Manifest { url: "http://127.0.0.1:8000/x".into(), ..m("0.2.0") }.validate().is_ok());
        assert!(Manifest { sha256: Some("abc".into()), ..m("0.2.0") }.validate().is_err());
        assert!(Manifest { sha256: Some("a".repeat(64)), ..m("0.2.0") }.validate().is_ok());
        assert!(Manifest { min_supported: Some("soon".into()), ..m("0.2.0") }.validate().is_err());
    }

    #[test]
    fn compares() {
        let x = Manifest { min_supported: Some("0.2.0".into()), ..m("0.3.0") };
        assert!(x.is_newer_than("0.2.5"));
        assert!(!x.is_newer_than("0.3.0"));
        assert!(x.unsupported("0.1.9"));
        assert!(!x.unsupported("0.2.0"));
        assert!(!m("0.3.0").unsupported("0.0.1"));
    }

    #[test]
    fn reads_github_release() {
        let v = json!({
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/mflRevan/fluss/releases/tag/v0.2.0",
            "published_at": "2026-11-02T10:00:00Z",
            "assets": [
                {"name": "Fluss-setup.exe", "browser_download_url": "https://x/Fluss-setup.exe"},
                {"name": "Fluss_0.2.0_x64-setup.exe", "browser_download_url": "https://x/Fluss_0.2.0_x64-setup.exe",
                 "size": 42, "digest": "sha256:ABCDEF"}
            ]
        });
        let r = Manifest::from_github(&v).unwrap();
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.url, "https://x/Fluss_0.2.0_x64-setup.exe");
        assert_eq!(r.sha256.as_deref(), Some("abcdef"));
        assert_eq!(r.size, Some(42));
        assert_eq!(r.date.as_deref(), Some("2026-11-02"));
        assert!(Manifest::from_github(&json!({"tag_name": "v1.0.0", "assets": []})).is_none());
    }
}
