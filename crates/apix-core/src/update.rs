//! Is there a newer Apix? One GET to GitHub's "latest release" endpoint,
//! compared with this build's version. Nothing is sent but the request
//! itself (no identifiers); nothing is downloaded or installed: the answer
//! carries the release page to open.
//!
//! The request is built and the answer read here, so the app and the CLI
//! share them and the logic is tested without a network.

use serde_json::Value;

use crate::{KeyValue, Method, Request};

/// Where releases are published.
pub const RELEASES_PAGE: &str = "https://github.com/11R5/Apix/releases";
const LATEST_API: &str = "https://api.github.com/repos/11R5/Apix/releases/latest";

/// A version as `major.minor.patch`, with an optional pre-release part
/// (`0.2.0-beta.1`), which sorts before the release itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    numbers: [u64; 3],
    pre: Option<String>,
}

impl Version {
    /// `v1.2.3`, `1.2.3`, `1.2` and `1.2.3-rc.1` all parse; anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let text = text.split('+').next()?; // build metadata does not count
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) if !pre.is_empty() => (core, Some(pre.to_owned())),
            Some(_) => return None,
            None => (text, None),
        };
        let mut numbers = [0u64; 3];
        let mut parts = core.split('.');
        for (i, slot) in numbers.iter_mut().enumerate() {
            match parts.next() {
                Some(p) => *slot = p.parse().ok()?,
                None if i >= 2 => {}
                None => return None,
            }
        }
        if parts.next().is_some() {
            return None;
        }
        Some(Self { numbers, pre })
    }

    /// This build's version.
    pub fn current() -> Self {
        Self::parse(env!("CARGO_PKG_VERSION")).expect("the crate version parses")
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        self.numbers
            .cmp(&other.numbers)
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => pre_cmp(a, b),
            })
    }
}

/// Dot-separated identifiers: numbers by value and below words, words by text.
fn pre_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut a, mut b) = (a.split('.'), b.split('.'));
    loop {
        match (a.next(), b.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c] = self.numbers;
        write!(f, "{a}.{b}.{c}")?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// Its page, to download it from.
    pub url: String,
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// A newer release than this build.
    Newer(Release),
    /// The latest release is this one or older.
    UpToDate { latest: Version },
    /// Nothing has been published yet.
    NoReleases,
}

/// The request to send: GitHub's latest release. `APIX_UPDATE_URL` points it
/// elsewhere (a fork, a mirror, a test server).
pub fn request() -> Request {
    let url = std::env::var("APIX_UPDATE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| LATEST_API.to_owned());
    let mut request = Request::new(Method::Get, url);
    request.name = "Check for updates".into();
    request.headers = vec![
        KeyValue::new("Accept", "application/vnd.github+json"),
        KeyValue::new("User-Agent", concat!("Apix/", env!("CARGO_PKG_VERSION"))),
    ];
    request.settings.timeout_ms = Some(10_000);
    request
}

/// Reads the answer to [`request`] against `current`.
pub fn interpret(status: u16, body: &[u8], current: &Version) -> Result<Check, String> {
    if status == 404 {
        return Ok(Check::NoReleases);
    }
    if !(200..300).contains(&status) {
        let message = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|v| v.get("message")?.as_str().map(str::to_owned))
            .unwrap_or_default();
        return Err(format!("HTTP {status} {message}").trim().to_owned());
    }
    let value: Value = serde_json::from_slice(body).map_err(|e| format!("not a release: {e}"))?;
    let tag = value
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("the release has no tag_name")?;
    let version = Version::parse(tag).ok_or_else(|| format!("not a version: {tag}"))?;
    let url = value
        .get("html_url")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{RELEASES_PAGE}/tag/{tag}"));
    Ok(if version > *current {
        Check::Newer(Release { version, url })
    } else {
        Check::UpToDate { latest: version }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_parse_and_order() {
        assert_eq!(v("v1.2.3").to_string(), "1.2.3");
        assert_eq!(v("1.2").to_string(), "1.2.0");
        assert_eq!(v("1.2.3+build.5").to_string(), "1.2.3");
        assert!(Version::parse("1").is_none());
        assert!(Version::parse("1.2.3.4").is_none());
        assert!(Version::parse("one.two").is_none());
        assert!(Version::parse("1.2.3-").is_none());
        assert!(v("0.2.0") > v("0.1.9"));
        assert!(v("0.10.0") > v("0.9.0"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.2") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.10") > v("1.0.0-rc.9"));
        assert!(v("1.0.0-beta") > v("1.0.0-alpha"));
        assert!(v("1.0.0-alpha.1") > v("1.0.0-alpha"));
        assert!(v("1.0.0-alpha.beta") > v("1.0.0-alpha.1"));
        assert_eq!(v("v0.1.0"), v("0.1.0"));
        assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn answers_are_read() {
        let current = v("0.1.0");
        let newer = br#"{"tag_name":"v0.2.0","html_url":"https://example.test/r/v0.2.0"}"#;
        assert_eq!(
            interpret(200, newer, &current).unwrap(),
            Check::Newer(Release {
                version: v("0.2.0"),
                url: "https://example.test/r/v0.2.0".into()
            })
        );
        let same = br#"{"tag_name":"0.1.0"}"#;
        assert_eq!(
            interpret(200, same, &current).unwrap(),
            Check::UpToDate { latest: v("0.1.0") }
        );
        let pre = br#"{"tag_name":"v0.1.0-rc.1"}"#;
        assert!(matches!(
            interpret(200, pre, &current).unwrap(),
            Check::UpToDate { .. }
        ));
        // No html_url: the page is made from the tag.
        let Check::Newer(r) = interpret(200, br#"{"tag_name":"v9.0.0"}"#, &current).unwrap() else {
            panic!()
        };
        assert_eq!(r.url, format!("{RELEASES_PAGE}/tag/v9.0.0"));
        assert_eq!(interpret(404, b"", &current).unwrap(), Check::NoReleases);
        let limited = br#"{"message":"API rate limit exceeded"}"#;
        assert_eq!(
            interpret(403, limited, &current).unwrap_err(),
            "HTTP 403 API rate limit exceeded"
        );
        assert!(interpret(200, b"<html>", &current).is_err());
        assert!(interpret(200, br#"{"tag_name":"latest"}"#, &current).is_err());
    }

    #[test]
    fn the_request_names_apix() {
        let r = request();
        assert_eq!(r.method, Method::Get);
        assert!(
            r.headers
                .iter()
                .any(|h| h.key == "User-Agent" && h.value.starts_with("Apix/"))
        );
    }
}
