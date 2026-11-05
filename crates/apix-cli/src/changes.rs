//! The git-aware commands: `apix run --changed` picks the requests a change
//! touches, and `apix diff` shows what changed in them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use apix_core::diff::{self, FileDiff, FileKind, Line, Status};
use apix_core::git::{self, Repo};
use apix_core::{Collection, Project, vfs};
use apix_runner::{Scope, Selection};

use super::Style;

/// What `apix run --changed` compares with when no commit is given: the
/// pull request's base branch on GitHub Actions, otherwise `HEAD`.
fn default_base() -> String {
    match std::env::var("GITHUB_BASE_REF") {
        Ok(base) if !base.trim().is_empty() => format!("origin/{}", base.trim()),
        _ => "HEAD".to_owned(),
    }
}

/// Keeps the requests (all inside `root`) that changed since `rev`, prints
/// which and why, and says what was chosen for the reports.
pub fn select(
    scope: &Scope,
    root: &Path,
    requests: &[PathBuf],
    rev: &str,
    style: &Style,
) -> Result<(Vec<PathBuf>, Selection)> {
    if vfs::encrypted_root(root).is_some() {
        bail!(
            "--changed cannot tell which requests changed in an encrypted collection: git only sees encrypted files"
        );
    }
    let rev = if rev.trim().is_empty() {
        default_base()
    } else {
        rev.trim().to_owned()
    };
    // In a project, its own files and environments count too.
    let base = scope
        .project
        .as_ref()
        .map(|p| p.root.clone())
        .unwrap_or_else(|| root.to_path_buf());
    let repo = Repo::at(&base)
        .with_context(|| format!("{} is not inside a git repository", base.display()))?;
    let since = repo.changes_since(&rev)?;
    if let Some(warning) = &since.warning {
        eprintln!("{}warning:{} {warning}", style.yellow, style.reset);
    }
    let affected = git::affected(root, &base, requests, &since.changes);
    let rel_root = root.strip_prefix(&base).unwrap_or(Path::new(""));
    let removed: Vec<String> = git::removed_requests(&since.changes)
        .iter()
        .filter_map(|p| p.strip_prefix(rel_root).ok())
        .map(git::slashes)
        .collect();

    let label = if rev == "HEAD" {
        "not committed yet".to_owned()
    } else {
        format!("changed since {rev}")
    };
    println!(
        "{}{} of {} requests {label}{}",
        style.dim,
        affected.len(),
        requests.len(),
        style.reset
    );
    for item in &affected {
        let request = item.request.strip_prefix(root).unwrap_or(&item.request);
        let because = item.because.strip_prefix(rel_root).unwrap_or(&item.because);
        let why = if because == request {
            String::new()
        } else {
            format!("  ← {}", git::slashes(because))
        };
        println!(
            "{}  {}{why}{}",
            style.dim,
            git::slashes(request),
            style.reset
        );
    }
    for path in &removed {
        println!("{}  {path} (removed){}", style.dim, style.reset);
    }
    println!();
    let selection = Selection {
        since: rev,
        ran: affected.len(),
        of: requests.len(),
        removed,
        warning: since.warning,
    };
    Ok((affected.into_iter().map(|a| a.request).collect(), selection))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Markdown,
    Json,
}

/// Prints what changed between `from` and `to` (or the files on disk) in
/// the collection or project around `dir`. Returns whether anything did.
pub fn diff(
    dir: &Path,
    from: &str,
    to: Option<&str>,
    format: Format,
    style: &Style,
) -> Result<bool> {
    let base = Collection::find_root(dir)
        .or_else(|| Project::find_root(&std::path::absolute(dir).unwrap_or(dir.to_path_buf())))
        .with_context(|| {
            format!(
                "no apix.toml or apix-project.toml found in {} or its parents",
                dir.display()
            )
        })?;
    if vfs::encrypted_root(&base).is_some() {
        bail!(
            "apix diff cannot compare an encrypted collection: git only sees encrypted files (`git diff` shows which of them changed)"
        );
    }
    let repo = Repo::at(&base)
        .with_context(|| format!("{} is not inside a git repository", base.display()))?;
    let mut files = Vec::new();
    for change in repo.changes_between(from, to)? {
        // Secrets never belong in git; do not print them if one slipped in.
        if change
            .path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with(".local.toml"))
        {
            continue;
        }
        let old = repo.show(from, &change.path)?;
        let new = match to {
            Some(to) => repo.show(to, &change.path)?,
            None => std::fs::read(base.join(&change.path)).ok(),
        };
        files.extend(diff::file(&change.path, old.as_deref(), new.as_deref()));
    }
    diff::pair_moves(&mut files);
    let to_label = to.map_or_else(|| "the working tree".to_owned(), |t| format!("`{t}`"));
    match format {
        Format::Json => {
            let out = serde_json::json!({ "from": from, "to": to, "files": files });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Markdown => print!("{}", markdown(&files, from, &to_label)),
        Format::Text => text(&files, from, &to_label.replace('`', ""), style),
    }
    Ok(!files.is_empty())
}

/// `GET Get user`, `environment dev`, `folder settings` …
fn title(file: &FileDiff) -> String {
    match file.kind {
        FileKind::Request => format!(
            "{} {}",
            file.method.as_deref().unwrap_or("GET"),
            file.name.as_deref().unwrap_or_default()
        ),
        FileKind::Script => "script".to_owned(),
        FileKind::Folder => "folder settings".to_owned(),
        FileKind::Collection => "collection settings".to_owned(),
        FileKind::Project => "project settings".to_owned(),
        FileKind::Environment => format!(
            "environment {}",
            Path::new(&file.path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        ),
        FileKind::Other => "file".to_owned(),
    }
}

fn letter(status: Status) -> char {
    match status {
        Status::Added => 'A',
        Status::Removed => 'D',
        Status::Changed => 'M',
        Status::Moved => 'R',
    }
}

/// One line of at most `max` characters.
fn short(value: &Option<serde_json::Value>, max: usize) -> String {
    let Some(value) = value else {
        return "(none)".to_owned();
    };
    let text = diff::show(value).replace(['\r', '\n'], " ");
    if text.chars().count() > max {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    } else {
        text
    }
}

/// The changed lines with `context` unchanged lines around them; `None`
/// stands for lines left out.
fn hunks(lines: &[Line], context: usize) -> Vec<Option<&Line>> {
    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !matches!(l, Line::Same(_)))
        .map(|(i, _)| i)
        .collect();
    let keep = |i: usize| {
        changed
            .iter()
            .any(|&c| i + context >= c && i <= c + context)
    };
    let mut out = Vec::new();
    let mut skipped = false;
    for (i, line) in lines.iter().enumerate() {
        if keep(i) {
            if skipped && !out.is_empty() {
                out.push(None);
            }
            skipped = false;
            out.push(Some(line));
        } else {
            skipped = true;
        }
    }
    out
}

fn summary(files: &[FileDiff]) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for file in files {
        let noun = match file.kind {
            FileKind::Request => "request",
            FileKind::Script => "script",
            FileKind::Folder => "folder",
            FileKind::Collection => "collection",
            FileKind::Environment => "environment",
            FileKind::Project => "project",
            FileKind::Other => "other file",
        };
        match counts.iter_mut().find(|(n, _)| n == noun) {
            Some((_, count)) => *count += 1,
            None => counts.push((noun.to_owned(), 1)),
        }
    }
    counts
        .into_iter()
        .map(|(noun, n)| {
            if n == 1 {
                format!("1 {noun}")
            } else {
                format!("{n} {noun}s")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn text(files: &[FileDiff], from: &str, to: &str, style: &Style) {
    if files.is_empty() {
        println!("No changes between {from} and {to}.");
        return;
    }
    println!(
        "{}Between {from} and {to}: {}{}\n",
        style.dim,
        summary(files),
        style.reset
    );
    for file in files {
        let colour = match file.status {
            Status::Added => style.green,
            Status::Removed => style.red,
            _ => style.yellow,
        };
        let path = match &file.moved_from {
            Some(old) => format!("{old} → {}", file.path),
            None => file.path.clone(),
        };
        println!(
            "{colour}{}{}  {}  {}{path}{}",
            letter(file.status),
            style.reset,
            title(file),
            style.dim,
            style.reset
        );
        if let Some(note) = &file.note {
            println!("     {}{note}{}", style.dim, style.reset);
        }
        for field in &file.fields {
            match field.multiline() {
                Some((old, new)) => {
                    println!("     {}:", field.field);
                    print_lines(&diff::lines(old, new), "       ", style);
                }
                None => println!(
                    "     {}: {}{}{} → {}{}{}",
                    field.field,
                    style.red,
                    short(&field.old, 100),
                    style.reset,
                    style.green,
                    short(&field.new, 100),
                    style.reset
                ),
            }
        }
        print_lines(&file.lines, "     ", style);
    }
}

fn print_lines(lines: &[Line], indent: &str, style: &Style) {
    for line in hunks(lines, 2) {
        match line {
            None => println!("{indent}{}…{}", style.dim, style.reset),
            Some(Line::Same(t)) => println!("{indent}{}  {t}{}", style.dim, style.reset),
            Some(Line::Removed(t)) => println!("{indent}{}- {t}{}", style.red, style.reset),
            Some(Line::Added(t)) => println!("{indent}{}+ {t}{}", style.green, style.reset),
        }
    }
}

/// For a pull request: one section per file, single values inline and
/// texts as `diff` blocks.
fn markdown(files: &[FileDiff], from: &str, to: &str) -> String {
    let mut out = String::from("<!-- apix-diff -->\n");
    if files.is_empty() {
        out += &format!("### Apix · no changes between `{from}` and {to}\n");
        return out;
    }
    out += &format!(
        "### Apix · changes between `{from}` and {to}\n\n{}\n",
        summary(files)
    );
    for file in files {
        let icon = match file.status {
            Status::Added => "🆕",
            Status::Removed => "🗑️",
            Status::Changed => "✏️",
            Status::Moved => "➡️",
        };
        let path = match &file.moved_from {
            Some(old) => format!("{} → {}", cell(old), cell(&file.path)),
            None => cell(&file.path),
        };
        out += &format!("\n#### {icon} {} <sub>{path}</sub>\n", cell(&title(file)));
        if let Some(note) = &file.note {
            out += &format!("_{}_\n", cell(note));
        }
        let mut blocks = String::new();
        for field in &file.fields {
            match field.multiline() {
                Some((old, new)) => {
                    blocks += &format!("\n`{}`\n", field.field.replace('`', "'"));
                    blocks += &diff_block(&diff::lines(old, new));
                }
                None => {
                    out += &format!(
                        "- `{}`: {} → {}\n",
                        field.field.replace('`', "'"),
                        inline(&field.old),
                        inline(&field.new)
                    );
                }
            }
        }
        out += &blocks;
        if !file.lines.is_empty() {
            out += &diff_block(&file.lines);
        }
    }
    out
}

fn diff_block(lines: &[Line]) -> String {
    let mut out = String::from("```diff\n");
    for line in hunks(lines, 2) {
        let text = match line {
            None => "@@ … @@".to_owned(),
            Some(Line::Same(t)) => format!("  {t}"),
            Some(Line::Removed(t)) => format!("- {t}"),
            Some(Line::Added(t)) => format!("+ {t}"),
        };
        // A line of backticks would end the block early.
        out += &text.replace("```", "`\u{200b}``");
        out.push('\n');
    }
    out += "```\n";
    out
}

fn inline(value: &Option<serde_json::Value>) -> String {
    match value {
        None => "_(none)_".to_owned(),
        Some(_) => format!("`{}`", short(value, 120).replace('`', "'")),
    }
}

/// Text that stays text in Markdown.
fn cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_keep_context_and_mark_gaps() {
        let lines: Vec<Line> = (0..10)
            .map(|i| match i {
                1 => Line::Removed("x".into()),
                8 => Line::Added("y".into()),
                n => Line::Same(n.to_string()),
            })
            .collect();
        let got = hunks(&lines, 1);
        // 0,1,2 … 7,8,9
        assert_eq!(got.len(), 7);
        assert!(got[3].is_none());
    }

    #[test]
    fn markdown_lists_fields_and_blocks() {
        let file = diff::file(
            Path::new("users/get.apix"),
            Some(b"name = \"Get\"\nurl = \"https://x/1\"\n"),
            Some(b"name = \"Get\"\nurl = \"https://x/2\"\n[body]\njson = \"{\\n}\"\n"),
        )
        .unwrap();
        let text = markdown(&[file], "HEAD", "the working tree");
        assert!(text.contains("#### ✏️ GET Get <sub>users/get.apix</sub>"));
        assert!(text.contains("- `url`: `https://x/1` → `https://x/2`"));
        assert!(text.contains("`body.json`\n```diff\n+ {\n+ }\n```"));
    }
}
