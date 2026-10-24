//! Variables from outside the collection, for CI where secrets live in the
//! pipeline rather than in `.local.toml` files or a keychain:
//!
//! - `APIX_VAR_<NAME>` environment variables: `APIX_VAR_TOKEN` fills `{{token}}`.
//! - a dotenv file (`--env-file .env`): `TOKEN=…` lines.
//!
//! CI systems write secret names in capitals, collections usually do not, so
//! a name matches a variable the collection already has whatever its case;
//! other names are used as written.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// The environment variable prefix that marks an Apix variable.
pub const PREFIX: &str = "APIX_VAR_";

/// `NAME=value` lines: `#` comments and blank lines are skipped, `export ` in
/// front is allowed, values may be in double quotes (with `\n`, `\t`, `\"`,
/// `\\`) or single quotes (taken as written); an unquoted value ends at ` #`.
pub fn parse_env_file(text: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for (ix, raw) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((name, value)) = line.split_once('=') else {
            bail!("line {}: expected NAME=value", ix + 1);
        };
        let name = name.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            bail!("line {}: \"{name}\" is not a variable name", ix + 1);
        }
        let value = value.trim();
        let value = if let Some(rest) = value.strip_prefix('"') {
            let Some(end) = closing_quote(rest) else {
                bail!("line {}: the value's quote is never closed", ix + 1);
            };
            unescape(&rest[..end])
        } else if let Some(rest) = value.strip_prefix('\'') {
            let Some(end) = rest.find('\'') else {
                bail!("line {}: the value's quote is never closed", ix + 1);
            };
            rest[..end].to_owned()
        } else {
            match value.find(" #") {
                Some(comment) => value[..comment].trim_end().to_owned(),
                None => value.to_owned(),
            }
        };
        out.push((name.to_owned(), value));
    }
    Ok(out)
}

/// Where the closing `"` is, skipping escaped ones.
fn closing_quote(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (ix, c) in text.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '"' => return Some(ix),
            _ => {}
        }
    }
    None
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

pub fn load_env_file(path: &Path) -> Result<Vec<(String, String)>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_env_file(&text).with_context(|| format!("reading {}", path.display()))
}

/// The `APIX_VAR_*` entries of `environment` (pass `std::env::vars()`).
pub fn from_environment(
    environment: impl IntoIterator<Item = (String, String)>,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = environment
        .into_iter()
        .filter_map(|(key, value)| {
            let name = key.strip_prefix(PREFIX)?;
            (!name.is_empty()).then(|| (name.to_owned(), value))
        })
        .collect();
    out.sort();
    out
}

/// Renames each entry to the variable it means: an exact name wins, then one
/// the collection knows that differs only in case.
pub fn match_known<'a>(
    entries: Vec<(String, String)>,
    known: impl IntoIterator<Item = &'a str> + Clone,
) -> Vec<(String, String)> {
    entries
        .into_iter()
        .map(|(name, value)| {
            let known = known.clone();
            let mut same_case = known.clone().into_iter();
            if same_case.any(|k| k == name) {
                return (name, value);
            }
            let found = known
                .into_iter()
                .find(|k| k.eq_ignore_ascii_case(&name))
                .map(str::to_owned);
            (found.unwrap_or(name), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_dotenv_files() {
        let text = "\u{feff}# secrets\n\nTOKEN=abc123\nexport BASE_URL = https://x.test # comment\nQUOTED=\"a \\\"b\\\"\\nc\"\nRAW='$not {{expanded}}'\nEMPTY=\nHASH=a#b\n";
        assert_eq!(
            parse_env_file(text).unwrap(),
            [
                ("TOKEN".to_string(), "abc123".to_string()),
                ("BASE_URL".into(), "https://x.test".into()),
                ("QUOTED".into(), "a \"b\"\nc".into()),
                ("RAW".into(), "$not {{expanded}}".into()),
                ("EMPTY".into(), String::new()),
                ("HASH".into(), "a#b".into()),
            ]
        );
        assert!(parse_env_file("NOEQUALS\n").is_err());
        assert!(parse_env_file("A=\"open\n").is_err());
        assert!(parse_env_file("TWO WORDS=1\n").is_err());
    }

    #[test]
    fn takes_prefixed_environment_variables() {
        let env = [
            ("PATH".to_string(), "/bin".to_string()),
            ("APIX_VAR_TOKEN".into(), "s3cret".into()),
            ("APIX_VAR_".into(), "ignored".into()),
            ("APIX_VAR_base_url".into(), "https://x.test".into()),
        ];
        assert_eq!(
            from_environment(env),
            [
                ("TOKEN".to_string(), "s3cret".to_string()),
                ("base_url".into(), "https://x.test".into()),
            ]
        );
    }

    #[test]
    fn names_follow_the_collection() {
        let known = ["token", "Token2", "API_KEY"];
        let entries = vec![
            ("TOKEN".to_string(), "1".to_string()),
            ("TOKEN2".into(), "2".into()),
            ("API_KEY".into(), "3".into()),
            ("NEW_ONE".into(), "4".into()),
        ];
        let names: Vec<String> = match_known(entries, known.iter().copied())
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(names, ["token", "Token2", "API_KEY", "NEW_ONE"]);
    }
}
