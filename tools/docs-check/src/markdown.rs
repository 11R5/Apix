//! What the checks and the docs site need from a Markdown document: its
//! headings with GitHub's anchors, and the links and images in it.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// GitHub's anchor for a heading: lower case, punctuation dropped (letters
/// of every script, digits, `_`, `-` and combining marks stay), spaces
/// become hyphens.
pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() || is_mark(c) => Some(c),
            _ => None,
        })
        .collect()
}

/// Combining marks that GitHub keeps in anchors (the ranges the documents'
/// languages use).
fn is_mark(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F      // Latin diacritics (and Turkish İ lower-cased)
        | 0x0610..=0x061A    // Arabic
        | 0x064B..=0x065F
        | 0x0670
        | 0x3099..=0x309A) // Japanese voicing marks
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// The anchor, with `-1`, `-2` … added for repeats, as GitHub does.
    pub id: String,
}

/// The headings of a document, in order.
pub fn headings(markdown: &str) -> Vec<Heading> {
    let mut found = Vec::new();
    let mut current: Option<(u8, String)> = None;
    for event in Parser::new_ext(markdown, options()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as u8, String::new()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, heading)) = current.as_mut() {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    found.push((level, text));
                }
            }
            _ => {}
        }
    }
    let mut used: Vec<String> = Vec::new();
    found
        .into_iter()
        .map(|(level, text)| {
            let base = slug(&text);
            let mut id = base.clone();
            let mut n = 0;
            while used.contains(&id) {
                n += 1;
                id = format!("{base}-{n}");
            }
            used.push(id.clone());
            Heading {
                level,
                text: text.trim().to_string(),
                id,
            }
        })
        .collect()
}

/// Every link and image target in a document.
pub fn targets(markdown: &str) -> Vec<String> {
    Parser::new_ext(markdown, options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. }) => Some(dest_url.to_string()),
            _ => None,
        })
        .collect()
}

/// Whether a target leaves the documentation (a web address, an e-mail).
pub fn is_external(target: &str) -> bool {
    target.contains("://") || target.starts_with("mailto:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_like_github() {
        assert_eq!(slug("Installing Apix"), "installing-apix");
        assert_eq!(
            slug("Let an AI agent use your collection (MCP)"),
            "let-an-ai-agent-use-your-collection-mcp"
        );
        assert_eq!(
            slug("Only what changed: git-aware runs"),
            "only-what-changed-git-aware-runs"
        );
        assert_eq!(slug("Apix'i yükleme"), "apixi-yükleme");
        assert_eq!(slug("Установка Apix"), "установка-apix");
        assert_eq!(slug("インストール、設定"), "インストール設定");
        assert_eq!(slug("`apix run` options"), "apix-run-options");
    }

    #[test]
    fn repeated_headings_are_numbered() {
        let found = headings("# A\n\n## Notes\n\ntext\n\n## Notes\n\n### `code` here\n");
        let ids: Vec<_> = found.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["a", "notes", "notes-1", "code-here"]);
        assert_eq!(found[3].level, 3);
    }

    #[test]
    fn links_and_images() {
        let found = targets(
            "See [x](other.md#a) and ![p](images/p.png).\n\n```\n[not](a link)\n```\n<https://e.com>",
        );
        assert_eq!(found, ["other.md#a", "images/p.png", "https://e.com"]);
    }
}
