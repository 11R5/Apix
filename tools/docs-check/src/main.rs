//! Checks the documentation translations listed in `Docs/languages.toml`.
//!
//! ```sh
//! cargo run -p docs-check            # list missing and stale translations; exit 1 if any
//! cargo run -p docs-check -- stamps  # print the current stamp for every document
//! cargo run -p docs-check -- site site/docs  # the documentation as web pages
//! ```
//!
//! A translation starts with `<!-- translated from en/<doc> sha256:<16 hex> -->`,
//! the hash of the English file it was made from. When the English file
//! changes, the hash no longer matches and the translation is listed as stale.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod markdown;
mod site;

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Languages {
    source: String,
    documents: Vec<String>,
    #[serde(rename = "language")]
    languages: Vec<Language>,
}

#[derive(Deserialize)]
struct Language {
    code: String,
    #[allow(dead_code)]
    name: String,
    direction: String,
    docs: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Problem {
    Invalid(String),
    Missing(PathBuf),
    NoStamp(PathBuf),
    Stale {
        file: PathBuf,
        stamp: String,
    },
    /// A link or image whose file, or heading, is not there.
    Broken {
        file: PathBuf,
        target: String,
    },
}

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Docs")
}

fn load(docs: &Path) -> Languages {
    let text = std::fs::read_to_string(docs.join("languages.toml"))
        .expect("Docs/languages.toml is readable");
    toml::from_str(&text).expect("Docs/languages.toml is valid")
}

/// The stamp for the English `doc`: the first 16 hex digits of the SHA-256
/// of its text, with line endings normalised so Windows checkouts agree.
fn stamp(docs: &Path, source: &str, doc: &str) -> String {
    let text = std::fs::read_to_string(docs.join(source).join(doc)).unwrap_or_default();
    let digest = Sha256::digest(text.replace("\r\n", "\n").as_bytes());
    let mut hex = String::new();
    for byte in &digest[..8] {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("<!-- translated from {source}/{doc} sha256:{hex} -->")
}

fn check(docs: &Path, config: &Languages) -> Vec<Problem> {
    let mut problems = Vec::new();
    for language in &config.languages {
        if !matches!(language.direction.as_str(), "ltr" | "rtl") {
            problems.push(Problem::Invalid(format!(
                "{}: direction must be ltr or rtl",
                language.code
            )));
        }
    }
    for language in config.languages.iter().filter(|l| l.docs) {
        for doc in &config.documents {
            let file = docs.join(&language.code).join(doc);
            if language.code == config.source {
                if !file.is_file() {
                    problems.push(Problem::Missing(file));
                }
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&file) else {
                problems.push(Problem::Missing(file));
                continue;
            };
            let first = text.lines().next().unwrap_or_default().trim();
            let expected = stamp(docs, &config.source, doc);
            if !first.starts_with("<!-- translated from ") {
                problems.push(Problem::NoStamp(file));
            } else if first != expected {
                problems.push(Problem::Stale {
                    file,
                    stamp: expected,
                });
            }
        }
    }
    problems.extend(check_links(docs, config));
    problems
}

/// Links between the documents (and to images and repository files) in
/// every language: the file must exist and a `#heading` must be one of its
/// headings.
fn check_links(docs: &Path, config: &Languages) -> Vec<Problem> {
    let mut problems = Vec::new();
    for language in config.languages.iter().filter(|l| l.docs) {
        let folder = docs.join(&language.code);
        for doc in &config.documents {
            let file = folder.join(doc);
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let own = markdown::headings(&text);
            for target in markdown::targets(&text) {
                if markdown::is_external(&target) {
                    continue;
                }
                let (path, anchor) = match target.split_once('#') {
                    Some((path, anchor)) => (path, Some(anchor)),
                    None => (target.as_str(), None),
                };
                let ok = if path.is_empty() {
                    anchor.is_none_or(|a| own.iter().any(|h| h.id == a))
                } else {
                    let linked = folder.join(path);
                    match (linked.is_file() || linked.is_dir(), anchor) {
                        (false, _) => false,
                        (true, Some(anchor)) if path.ends_with(".md") => {
                            std::fs::read_to_string(&linked).is_ok_and(|other| {
                                markdown::headings(&other).iter().any(|h| h.id == anchor)
                            })
                        }
                        (true, _) => true,
                    }
                };
                if !ok {
                    problems.push(Problem::Broken {
                        file: file.clone(),
                        target,
                    });
                }
            }
        }
    }
    problems
}

fn main() -> ExitCode {
    let docs = docs_dir();
    let config = load(&docs);
    if std::env::args().nth(1).as_deref() == Some("stamps") {
        for doc in &config.documents {
            println!("{}", stamp(&docs, &config.source, doc));
        }
        return ExitCode::SUCCESS;
    }
    if std::env::args().nth(1).as_deref() == Some("site") {
        let out = PathBuf::from(
            std::env::args()
                .nth(2)
                .unwrap_or_else(|| "site/docs".into()),
        );
        return match site::build(&docs, &config, &out) {
            Ok(pages) => {
                println!("{pages} pages written to {}", out.display());
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("could not write the site: {error}");
                ExitCode::FAILURE
            }
        };
    }
    let problems = check(&docs, &config);
    let show = |file: &PathBuf| {
        let relative = file
            .strip_prefix(&docs)
            .unwrap_or(file)
            .display()
            .to_string();
        format!("Docs/{}", relative.replace('\\', "/"))
    };
    for problem in &problems {
        match problem {
            Problem::Invalid(message) => println!("languages.toml: {message}"),
            Problem::Missing(file) => println!("missing: {}", show(file)),
            Problem::NoStamp(file) => println!("no stamp on the first line: {}", show(file)),
            Problem::Stale { file, stamp } => {
                println!(
                    "older than English: {}\n  after updating it, start it with {stamp}",
                    file.display()
                )
            }
            Problem::Broken { file, target } => {
                println!("broken link in {}: {target}", show(file))
            }
        }
    }
    if problems.is_empty() {
        println!("All documentation translations are up to date.");
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Structure only: every listed document exists, translations carry a
    /// stamp, and directions are valid. Stale translations are reported by
    /// running the tool, not by failing the build.
    #[test]
    fn documents_exist_and_translations_are_stamped() {
        let docs = docs_dir();
        let config = load(&docs);
        assert!(
            config
                .languages
                .iter()
                .any(|l| l.code == config.source && l.docs)
        );
        let structural: Vec<_> = check(&docs, &config)
            .into_iter()
            .filter(|p| !matches!(p, Problem::Stale { .. }))
            .collect();
        assert!(structural.is_empty(), "{structural:#?}");
    }
}
