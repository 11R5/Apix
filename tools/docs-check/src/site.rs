//! The documentation as web pages, for GitHub Pages next to the download
//! page: `cargo run -p docs-check -- site <out>` writes
//! `<out>/<language>/<document>.html` for every language with documents, a
//! language picker on every page, the pictures, and `<out>/index.html`,
//! which opens the visitor's language.

use std::fmt::Write as _;
use std::path::{Component, Path};

use pulldown_cmark::{CowStr, Event, Parser, Tag, TagEnd, html};

use crate::{Languages, markdown, stamp};

const REPO: &str = "https://github.com/11R5/Apix";

/// The few words of the page itself, per language.
struct Words {
    contents: &'static str,
    download: &'static str,
    source: &'static str,
    behind: &'static str,
}

fn words(code: &str) -> Words {
    let (contents, download, source, behind) = match code {
        "tr" => (
            "İçindekiler",
            "İndir",
            "GitHub'da kaynak",
            "Bu çeviri İngilizce metnin eski bir sürümünden; bazı bölümler eksik ya da eski olabilir.",
        ),
        "de" => (
            "Inhalt",
            "Herunterladen",
            "Quelle auf GitHub",
            "Diese Übersetzung beruht auf einer älteren Fassung des englischen Textes; manches kann fehlen oder veraltet sein.",
        ),
        "fr" => (
            "Sommaire",
            "Télécharger",
            "Source sur GitHub",
            "Cette traduction suit une version plus ancienne du texte anglais ; certaines parties peuvent manquer ou être dépassées.",
        ),
        "es" => (
            "Contenido",
            "Descargar",
            "Código en GitHub",
            "Esta traducción sigue una versión anterior del texto en inglés; puede faltar algo o estar desactualizado.",
        ),
        "ru" => (
            "Содержание",
            "Скачать",
            "Исходники на GitHub",
            "Этот перевод сделан с более старой версии английского текста; что-то может отсутствовать или устареть.",
        ),
        "ja" => (
            "目次",
            "ダウンロード",
            "GitHub のソース",
            "この翻訳は英語版の古い版をもとにしています。一部が欠けていたり古くなっていたりする場合があります。",
        ),
        "zh-CN" => (
            "目录",
            "下载",
            "GitHub 上的源码",
            "此译文基于较旧的英文版本，部分内容可能缺失或已过时。",
        ),
        "ar" => (
            "المحتويات",
            "تنزيل",
            "المصدر على GitHub",
            "هذه الترجمة مأخوذة من نسخة أقدم من النص الإنجليزي؛ قد يكون بعض الأجزاء ناقصًا أو قديمًا.",
        ),
        _ => (
            "Contents",
            "Download",
            "Source on GitHub",
            "This translation follows an older version of the English text; parts may be missing or out of date.",
        ),
    };
    Words {
        contents,
        download,
        source,
        behind,
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Where a link in `Docs/<language>/<doc>` points on the site: another
/// document becomes its page, a picture stays next to the pages, anything
/// else in the repository opens on GitHub.
fn rewrite(target: &str, language: &str, config: &Languages) -> String {
    if markdown::is_external(target) || target.starts_with('#') {
        return target.to_string();
    }
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (target, String::new()),
    };
    // The path from the repository's root.
    let mut parts: Vec<String> = vec!["Docs".into(), language.into()];
    for component in Path::new(path).components() {
        match component {
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    let documented = |code: &str| config.languages.iter().any(|l| l.code == code && l.docs);
    if parts.len() == 3
        && parts[0] == "Docs"
        && documented(&parts[1])
        && let Some(stem) = parts[2].strip_suffix(".md")
        && config.documents.contains(&parts[2])
    {
        return format!("../{}/{stem}.html{anchor}", parts[1]);
    }
    if parts.len() == 4 && parts[0] == "Docs" && parts[2] == "images" && documented(&parts[1]) {
        return format!("../{}/images/{}", parts[1], parts[3]);
    }
    let kind = if Path::new(path).extension().is_some() {
        "blob"
    } else {
        "tree"
    };
    format!("{REPO}/{kind}/main/{}{anchor}", parts.join("/"))
}

/// One document as HTML: headings with GitHub's anchors, links rewritten.
fn body(text: &str, language: &str, config: &Languages) -> String {
    let headings = markdown::headings(text);
    let mut next = headings.iter();
    let events = Parser::new_ext(text, markdown::options()).map(|event| match event {
        Event::Start(Tag::Heading { level, .. }) => {
            let id = next.next().map(|h| h.id.as_str()).unwrap_or_default();
            Event::Html(CowStr::from(format!(
                "<{level} id=\"{0}\"><a class=\"anchor\" href=\"#{0}\" aria-hidden=\"true\">#</a>",
                escape(id)
            )))
        }
        Event::End(TagEnd::Heading(level)) => Event::Html(CowStr::from(format!("</{level}>"))),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: rewrite(&dest_url, language, config).into(),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: rewrite(&dest_url, language, config).into(),
            title,
            id,
        }),
        // The translation stamp.
        Event::Html(html) if html.trim_start().starts_with("<!-- translated from") => {
            Event::Html("".into())
        }
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

/// The title of a document: its first heading.
fn title(text: &str) -> String {
    markdown::headings(text)
        .into_iter()
        .next()
        .map(|h| h.text)
        .unwrap_or_else(|| "Apix".into())
}

/// Writes the site into `out`.
pub fn build(docs: &Path, config: &Languages, out: &Path) -> std::io::Result<usize> {
    let languages: Vec<_> = config.languages.iter().filter(|l| l.docs).collect();
    let mut pages = 0;
    for language in &languages {
        let folder = docs.join(&language.code);
        let target = out.join(&language.code);
        std::fs::create_dir_all(&target)?;
        if let Ok(images) = std::fs::read_dir(folder.join("images")) {
            std::fs::create_dir_all(target.join("images"))?;
            for image in images.flatten() {
                std::fs::copy(image.path(), target.join("images").join(image.file_name()))?;
            }
        }
        let texts: Vec<(String, String)> = config
            .documents
            .iter()
            .map(|doc| {
                let text = std::fs::read_to_string(folder.join(doc)).unwrap_or_default();
                (doc.clone(), text)
            })
            .collect();
        let word = words(&language.code);
        for (doc, text) in &texts {
            let stem = doc.trim_end_matches(".md");
            let tabs: String = texts
                .iter()
                .map(|(other, other_text)| {
                    let other_stem = other.trim_end_matches(".md");
                    format!(
                        "<a href=\"{other_stem}.html\"{}>{}</a>",
                        if other == doc {
                            " aria-current=\"page\""
                        } else {
                            ""
                        },
                        escape(&title(other_text))
                    )
                })
                .collect();
            let picker: String = languages
                .iter()
                .map(|l| {
                    format!(
                        "<option value=\"../{0}/{stem}.html\" lang=\"{0}\"{1}>{2}</option>",
                        l.code,
                        if l.code == language.code {
                            " selected"
                        } else {
                            ""
                        },
                        escape(&l.name)
                    )
                })
                .collect();
            let mut contents = String::new();
            for heading in markdown::headings(text).iter().filter(|h| h.level == 2) {
                let _ = write!(
                    contents,
                    "<li><a href=\"#{}\">{}</a></li>",
                    escape(&heading.id),
                    escape(&heading.text)
                );
            }
            let behind = language.code != config.source
                && text.lines().next().map(str::trim) != Some(&stamp(docs, &config.source, doc));
            let notice = if behind {
                format!("<p class=\"behind\">{}</p>", escape(word.behind))
            } else {
                String::new()
            };
            let page = PAGE
                .replace("{lang}", &language.code)
                .replace("{dir}", &language.direction)
                .replace("{title}", &escape(&title(text)))
                .replace("{tabs}", &tabs)
                .replace("{picker}", &picker)
                .replace("{contents_label}", word.contents)
                .replace("{contents}", &contents)
                .replace("{download}", word.download)
                .replace("{source_label}", word.source)
                .replace(
                    "{source}",
                    &format!("{REPO}/blob/main/Docs/{}/{doc}", language.code),
                )
                .replace("{notice}", &notice)
                .replace("{body}", &body(text, &language.code, config));
            std::fs::write(target.join(format!("{stem}.html")), page)?;
            pages += 1;
        }
    }
    let codes: Vec<String> = languages
        .iter()
        .map(|l| format!("\"{}\"", l.code))
        .collect();
    std::fs::write(
        out.join("index.html"),
        INDEX
            .replace("{codes}", &codes.join(","))
            .replace("{source}", &config.source),
    )?;
    Ok(pages)
}

const INDEX: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Apix documentation</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<script>
(function () {
  var codes = [{codes}], pick = "{source}";
  try { var saved = localStorage.getItem("apix-docs-lang"); if (codes.indexOf(saved) >= 0) pick = saved; else saved = null; } catch (e) { saved = null; }
  if (!saved) {
    var wanted = navigator.languages || [navigator.language || ""];
    for (var i = 0; i < wanted.length && pick === "{source}"; i++) {
      var w = (wanted[i] || "").toLowerCase();
      for (var j = 0; j < codes.length; j++) {
        var c = codes[j].toLowerCase();
        if (w === c || w.split("-")[0] === c.split("-")[0]) { pick = codes[j]; break; }
      }
    }
  }
  location.replace(pick + "/user-guide.html" + location.hash);
})();
</script></head>
<body><p><a href="{source}/user-guide.html">Apix documentation</a></p></body></html>
"#;

const PAGE: &str = r#"<!doctype html>
<html lang="{lang}" dir="{dir}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Apix</title>
<link rel="icon" type="image/png" href="../../favicon.png">
<style>
:root {
  --bg: #070b12; --panel: #0e1824; --line: rgba(120, 200, 230, 0.16);
  --text: #e6f1f7; --muted: #8ea3b3; --accent: #22d3ee; --accent-ink: #04212a;
  --code: #0a131d; --warn: #fbbf24;
}
@media (prefers-color-scheme: light) {
  :root {
    --bg: #f6fafc; --panel: #ffffff; --line: rgba(20, 80, 110, 0.16);
    --text: #0d1b26; --muted: #4f6474; --accent: #0891b2; --accent-ink: #ffffff;
    --code: #eef4f8; --warn: #b45309;
  }
}
* { box-sizing: border-box; }
html { scroll-padding-top: 70px; }
body { margin: 0; background: var(--bg); color: var(--text);
  font: 16px/1.65 system-ui, -apple-system, "Segoe UI", "Noto Sans", "Noto Sans Arabic", "Noto Sans JP", "Noto Sans SC", Roboto, sans-serif; }
a { color: var(--accent); text-decoration: none; }
a:hover { text-decoration: underline; }
header { position: sticky; top: 0; z-index: 2; background: color-mix(in srgb, var(--bg) 88%, transparent);
  backdrop-filter: blur(10px); -webkit-backdrop-filter: blur(10px); border-bottom: 1px solid var(--line); }
.bar { max-width: 1240px; margin: 0 auto; padding: 10px 16px; display: flex; align-items: center; gap: 14px; flex-wrap: wrap; }
.brand { display: flex; align-items: center; gap: 8px; font-weight: 700; color: var(--text); }
.brand img { width: 26px; height: 26px; border-radius: 6px; }
nav.docs { display: flex; gap: 2px; flex-wrap: wrap; flex: 1; min-width: 0; }
nav.docs a { color: var(--muted); padding: 5px 10px; border-radius: 8px; font-size: 14px; }
nav.docs a:hover { color: var(--text); background: var(--panel); text-decoration: none; }
nav.docs a[aria-current] { color: var(--accent-ink); background: var(--accent); }
.tools { display: flex; gap: 8px; align-items: center; }
.tools a { font-size: 14px; color: var(--muted); }
select { font: inherit; font-size: 14px; color: var(--text); background: var(--panel); border: 1px solid var(--line);
  border-radius: 8px; padding: 4px 8px; }
.layout { max-width: 1240px; margin: 0 auto; padding: 0 16px; display: grid;
  grid-template-columns: 250px minmax(0, 1fr); gap: 40px; }
aside { position: sticky; top: 64px; align-self: start; max-height: calc(100vh - 64px); overflow: auto; padding: 24px 0; font-size: 14px; }
aside h2 { font-size: 12px; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); margin: 0 0 8px; }
aside ul { list-style: none; margin: 0; padding: 0; }
aside li a { display: block; padding: 4px 8px; border-radius: 6px; color: var(--muted); }
aside li a:hover { background: var(--panel); color: var(--text); text-decoration: none; }
main { padding: 16px 0 80px; min-width: 0; max-width: 860px; }
main h1 { font-size: 34px; line-height: 1.2; margin: 16px 0 12px; }
main h2 { font-size: 25px; margin: 44px 0 10px; padding-top: 8px; border-top: 1px solid var(--line); }
main h3 { font-size: 19px; margin: 30px 0 8px; }
main h4 { font-size: 16px; margin: 24px 0 6px; }
.anchor { opacity: 0; margin-inline-end: 6px; margin-inline-start: -18px; font-weight: 400; color: var(--muted); }
h1:hover .anchor, h2:hover .anchor, h3:hover .anchor, h4:hover .anchor { opacity: 1; text-decoration: none; }
main img { max-width: 100%; height: auto; border-radius: 10px; border: 1px solid var(--line); }
code { font: 0.88em/1.5 ui-monospace, "Cascadia Code", Menlo, Consolas, monospace; background: var(--code);
  border: 1px solid var(--line); border-radius: 5px; padding: 0.1em 0.35em; unicode-bidi: isolate; direction: ltr; }
pre { background: var(--code); border: 1px solid var(--line); border-radius: 10px; padding: 12px 14px; overflow: auto; direction: ltr; text-align: left; }
pre code { background: none; border: 0; padding: 0; font-size: 13.5px; }
table { border-collapse: collapse; display: block; overflow: auto; max-width: 100%; font-size: 14.5px; margin: 14px 0; }
th, td { border: 1px solid var(--line); padding: 6px 10px; text-align: start; vertical-align: top; }
th { background: var(--panel); }
blockquote { margin: 14px 0; padding: 4px 14px; border-inline-start: 3px solid var(--accent); color: var(--muted); }
.behind { border-inline-start: 3px solid var(--warn); padding: 8px 12px; background: var(--panel); border-radius: 6px; color: var(--muted); font-size: 14px; }
footer { color: var(--muted); font-size: 14px; border-top: 1px solid var(--line); margin-top: 50px; padding-top: 16px; }
aside li a.here { color: var(--accent); background: var(--panel); }
/* On a narrow screen the document's own table of contents is enough. */
@media (max-width: 860px) {
  .layout { grid-template-columns: minmax(0, 1fr); gap: 0; }
  aside { display: none; }
}
</style>
</head>
<body>
<header><div class="bar">
  <a class="brand" href="../../"><img src="../../favicon.png" alt="">Apix</a>
  <nav class="docs">{tabs}</nav>
  <div class="tools">
    <a href="../../">{download}</a>
    <select aria-label="Language" onchange="try{localStorage.setItem('apix-docs-lang',this.selectedOptions[0].lang)}catch(e){};location.href=this.value">{picker}</select>
  </div>
</div></header>
<div class="layout">
  <aside><h2>{contents_label}</h2><ul>{contents}</ul></aside>
  <main>
{notice}
{body}
<footer><a href="{source}">{source_label}</a> · Apache-2.0</footer>
  </main>
</div>
<script>
// The section being read, marked in the contents.
(function () {
  var links = [].slice.call(document.querySelectorAll("aside a"));
  var heads = links.map(function (a) { return document.getElementById(decodeURIComponent(a.hash.slice(1))); });
  function mark() {
    var at = -1;
    for (var i = 0; i < heads.length; i++) if (heads[i] && heads[i].getBoundingClientRect().top < 120) at = i;
    links.forEach(function (a, i) { a.classList.toggle("here", i === at); });
    if (at >= 0 && links[at].scrollIntoView && !mark.busy) {
      var box = links[at].getBoundingClientRect(), side = links[at].closest("aside").getBoundingClientRect();
      if (box.top < side.top || box.bottom > side.bottom) links[at].scrollIntoView({ block: "nearest" });
    }
  }
  addEventListener("scroll", mark, { passive: true });
  mark();
})();
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{docs_dir, load};

    #[test]
    fn links_on_the_site() {
        let config = load(&docs_dir());
        assert_eq!(
            rewrite("workflows.md#runner", "en", &config),
            "../en/workflows.html#runner"
        );
        assert_eq!(
            rewrite("images/main.png", "tr", &config),
            "../tr/images/main.png"
        );
        assert_eq!(
            rewrite("../en/images/main.png", "tr", &config),
            "../en/images/main.png"
        );
        assert_eq!(
            rewrite("../../examples/ci/github-actions.yml", "en", &config),
            format!("{REPO}/blob/main/examples/ci/github-actions.yml")
        );
        assert_eq!(
            rewrite("../../examples/jsonplaceholder", "en", &config),
            format!("{REPO}/tree/main/examples/jsonplaceholder")
        );
        assert_eq!(rewrite("#top", "en", &config), "#top");
        assert_eq!(
            rewrite("https://e.com/a.md", "en", &config),
            "https://e.com/a.md"
        );
    }

    #[test]
    fn the_site_is_built() {
        let docs = docs_dir();
        let config = load(&docs);
        let out = std::env::temp_dir().join(format!("apix-docs-site-{}", std::process::id()));
        let pages = build(&docs, &config, &out).unwrap();
        assert!(pages >= config.documents.len());
        let guide = std::fs::read_to_string(out.join("en/user-guide.html")).unwrap();
        assert!(guide.contains("<h2 id=\"installing-apix\">"));
        assert!(guide.contains("<select"));
        assert!(!guide.contains("{body}"));
        assert!(out.join("en/images/main.png").is_file());
        assert!(out.join("index.html").is_file());
        let _ = std::fs::remove_dir_all(out);
    }
}
