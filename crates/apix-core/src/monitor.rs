//! Monitors: a collection (or a folder or request of it) run on a schedule,
//! kept in `apix.toml` so they are shared through git like the requests:
//!
//! ```toml
//! [[monitor]]
//! name = "Health"
//! every = "5m"
//! run = "users"            # a folder or request; empty: the whole collection
//! environment = "staging"
//! webhook = "{{alerts_url}}"
//! ```
//!
//! `apix monitor` runs them while it lasts (or once, for a scheduler of the
//! system or a CI schedule), and the desktop app while it is open. Running
//! them is `apix-runner`'s job; this module only describes them.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The shortest interval a monitor may have.
pub const MIN_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Monitor {
    pub name: String,
    /// How often: `30s`, `5m`, `1h`, `1d`, or together like `1h30m`.
    pub every: String,
    /// A folder or request file inside the collection, relative to it (with
    /// `/`); empty for the whole collection.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Told when the monitor starts failing and when it passes again;
    /// `{{variables}}` are filled in, so the address can stay a secret.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub webhook: String,
    /// Off: listed, but not run on its schedule.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

fn is_yes(value: &bool) -> bool {
    *value
}

impl Monitor {
    pub fn new(name: impl Into<String>, every: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            every: every.into(),
            run: String::new(),
            environment: None,
            webhook: String::new(),
            enabled: true,
        }
    }

    /// The interval, or why `every` cannot be read.
    pub fn interval(&self) -> Result<Duration, String> {
        parse_every(&self.every)
    }
}

/// Reads `30s`, `5m`, `2h`, `1d`, `1h30m` (spaces allowed between parts).
/// At least [`MIN_EVERY`].
pub fn parse_every(text: &str) -> Result<Duration, String> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if text.is_empty() {
        return Err("no interval given (for example 5m)".into());
    }
    let mut total = Duration::ZERO;
    let mut number = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let unit = match c.to_ascii_lowercase() {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86_400,
            _ => return Err(format!("`{text}`: use s, m, h or d, as in 30s, 5m, 1h30m")),
        };
        let n: u64 = number
            .parse()
            .map_err(|_| format!("`{text}`: a number must come before each unit"))?;
        number.clear();
        total += Duration::from_secs(n.saturating_mul(unit));
    }
    if !number.is_empty() {
        return Err(format!("`{text}`: say the unit, as in {number}m"));
    }
    if total < MIN_EVERY {
        return Err(format!("`{text}` is too often; the least is 10s"));
    }
    Ok(total)
}

/// `5m`, `1h 30m`, `1d` …, the way an interval reads best.
pub fn describe_every(every: Duration) -> String {
    let mut secs = every.as_secs();
    let mut parts = Vec::new();
    for (unit, size) in [("d", 86_400), ("h", 3600), ("m", 60), ("s", 1)] {
        if secs >= size {
            parts.push(format!("{}{unit}", secs / size));
            secs %= size;
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals() {
        assert_eq!(parse_every("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_every("5m"), Ok(Duration::from_secs(300)));
        assert_eq!(parse_every("1h30m"), Ok(Duration::from_secs(5400)));
        assert_eq!(parse_every("1h 30m"), Ok(Duration::from_secs(5400)));
        assert_eq!(parse_every("1D"), Ok(Duration::from_secs(86_400)));
        assert!(parse_every("5").unwrap_err().contains("unit"));
        assert!(parse_every("5x").is_err());
        assert!(parse_every("m").is_err());
        assert!(parse_every("3s").unwrap_err().contains("too often"));
        assert!(parse_every("").is_err());
        assert_eq!(describe_every(Duration::from_secs(5400)), "1h 30m");
        assert_eq!(describe_every(Duration::from_secs(86_400)), "1d");
    }

    #[test]
    fn in_a_collection_file() {
        let text = r#"
name = "API"

[[monitor]]
name = "Health"
every = "5m"
run = "users"
environment = "staging"

[[monitor]]
name = "Nightly"
every = "1d"
enabled = false
"#;
        let meta: crate::store::CollectionMeta = toml::from_str(text).unwrap();
        assert_eq!(meta.monitors.len(), 2);
        assert_eq!(meta.monitors[0].run, "users");
        assert!(meta.monitors[0].enabled);
        assert!(!meta.monitors[1].enabled);
        let back = toml::to_string_pretty(&meta).unwrap();
        let again: crate::store::CollectionMeta = toml::from_str(&back).unwrap();
        assert_eq!(again.monitors, meta.monitors);
        assert!(!back.contains("enabled = true"));
    }
}
