//! Text tools for showing response bodies of any size: a streaming JSON
//! pretty-printer, an index of display rows, and a one-line JSON tokenizer
//! for syntax colouring. All of them are linear and allocation-light, so a
//! 100 MB body can be prepared in a background thread in well under a second.

use std::ops::Range;

/// Re-indents JSON in a single pass without building a tree, so memory stays
/// close to the output size and numbers keep their exact original spelling.
/// The output matches `serde_json::to_string_pretty` (two-space indent).
///
/// Returns `None` when the input is not well-formed enough to format:
/// unbalanced brackets, an unterminated string, a bad literal or invalid UTF-8.
pub fn pretty_json(input: &[u8]) -> Option<String> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len() + input.len() / 3);
    let mut stack: Vec<u8> = Vec::new();
    let n = input.len();
    let mut i = 0;

    while i < n {
        let b = input[i];
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'"' => {
                let start = i;
                i += 1;
                loop {
                    if i >= n {
                        return None;
                    }
                    match input[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                out.extend_from_slice(&input[start..i]);
            }
            b'{' | b'[' => {
                let close = if b == b'{' { b'}' } else { b']' };
                let mut j = i + 1;
                while j < n && is_ws(input[j]) {
                    j += 1;
                }
                out.push(b);
                if j < n && input[j] == close {
                    out.push(close);
                    i = j + 1;
                } else {
                    stack.push(b);
                    newline(&mut out, stack.len());
                    i += 1;
                }
            }
            b'}' | b']' => {
                let open = if b == b'}' { b'{' } else { b'[' };
                if stack.pop() != Some(open) {
                    return None;
                }
                newline(&mut out, stack.len());
                out.push(b);
                i += 1;
            }
            b',' => {
                if stack.is_empty() {
                    return None;
                }
                out.push(b',');
                newline(&mut out, stack.len());
                i += 1;
            }
            b':' => {
                out.extend_from_slice(b": ");
                i += 1;
            }
            _ => {
                let start = i;
                while i < n && !is_delimiter(input[i]) {
                    i += 1;
                }
                let literal = &input[start..i];
                if !is_literal(literal) {
                    return None;
                }
                out.extend_from_slice(literal);
            }
        }
    }

    if !stack.is_empty() || out.is_empty() {
        return None;
    }
    String::from_utf8(out).ok()
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_delimiter(b: u8) -> bool {
    is_ws(b) || matches!(b, b',' | b':' | b'{' | b'}' | b'[' | b']' | b'"')
}

fn is_literal(lit: &[u8]) -> bool {
    match lit {
        b"true" | b"false" | b"null" => true,
        [first, ..] => {
            (first.is_ascii_digit() || *first == b'-')
                && lit
                    .iter()
                    .all(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.' | b'e' | b'E'))
        }
        [] => false,
    }
}

fn newline(out: &mut Vec<u8>, depth: usize) {
    out.push(b'\n');
    out.resize(out.len() + depth * 2, b' ');
}

/// One display row: a whole line, or a slice of a line too long to lay out
/// at once. Packed into 8 bytes because a big body has millions of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    start: u32,
    /// Zero-based line number in the low 31 bits; the top bit marks a
    /// continuation row, which repeats its line's number.
    line: u32,
}

const CONTINUATION: u32 = 1 << 31;

impl Row {
    fn line(self) -> u32 {
        self.line & !CONTINUATION
    }

    fn is_continuation(self) -> bool {
        self.line & CONTINUATION != 0
    }
}

/// Display rows of a text, so a viewer can lay out only what is on screen.
/// Lines longer than `max_row_bytes` are split at char boundaries.
#[derive(Debug, Clone)]
pub struct RowIndex {
    rows: Vec<Row>,
    len: usize,
    lines: u32,
    widest: usize,
}

impl RowIndex {
    /// Texts are limited to 4 GiB; anything past that is not indexed.
    pub fn new(text: &str, max_row_bytes: usize) -> Self {
        let max_row_bytes = max_row_bytes.max(4);
        let len = text.len().min(u32::MAX as usize);
        let bytes = &text.as_bytes()[..len];
        let mut rows = Vec::with_capacity(len / 40 + 1);
        let (mut widest, mut widest_len) = (0, 0);
        let mut line: u32 = 0;
        let mut pos = 0;

        loop {
            let end = bytes[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(len, |off| pos + off);
            let mut start = pos;
            let mut continuation = false;
            loop {
                let mut stop = (start + max_row_bytes).min(end);
                while stop < end && !text.is_char_boundary(stop) {
                    stop -= 1;
                }
                if stop - start > widest_len {
                    (widest, widest_len) = (rows.len(), stop - start);
                }
                rows.push(Row {
                    start: start as u32,
                    line: if continuation {
                        line | CONTINUATION
                    } else {
                        line
                    },
                });
                if stop >= end {
                    break;
                }
                start = stop;
                continuation = true;
            }
            // Past 2^31 lines the counter would collide with the flag bit.
            if end >= len || line == CONTINUATION - 1 {
                break;
            }
            pos = end + 1;
            line += 1;
        }

        Self {
            rows,
            len,
            lines: line + 1,
            widest,
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Number of logical lines (not display rows).
    pub fn lines(&self) -> u32 {
        self.lines
    }

    /// The index of the widest row, for measuring the viewer's width.
    pub fn widest(&self) -> usize {
        self.widest
    }

    /// The row's text without its line break, its 1-based line number, and
    /// whether it continues the previous row.
    pub fn row<'a>(&self, text: &'a str, ix: usize) -> (&'a str, u32, bool) {
        let row = self.rows[ix];
        let start = row.start as usize;
        let end = match self.rows.get(ix + 1) {
            Some(next) if next.is_continuation() => next.start as usize,
            Some(next) => next.start as usize - 1,
            None => self.len,
        };
        let slice = text[start..end]
            .strip_suffix('\r')
            .unwrap_or(&text[start..end]);
        (slice, row.line() + 1, row.is_continuation())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonToken {
    Key,
    String,
    Number,
    Keyword,
    Punctuation,
}

/// Tokenizes one line of (pretty-printed) JSON for colouring. Ranges are
/// sorted and never overlap. A string cut off by a row split is coloured to
/// the end of the row.
pub fn json_tokens(line: &str) -> Vec<(Range<usize>, JsonToken)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() {
                    match b[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                let end = i.min(b.len());
                let mut j = end;
                while j < b.len() && is_ws(b[j]) {
                    j += 1;
                }
                let kind = if b.get(j) == Some(&b':') {
                    JsonToken::Key
                } else {
                    JsonToken::String
                };
                out.push((start..end, kind));
                i = end;
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                out.push((i..i + 1, JsonToken::Punctuation));
                i += 1;
            }
            c if c == b'-' || c.is_ascii_digit() => {
                let start = i;
                while i < b.len()
                    && (b[i].is_ascii_digit() || matches!(b[i], b'-' | b'+' | b'.' | b'e' | b'E'))
                {
                    i += 1;
                }
                out.push((start..i, JsonToken::Number));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if matches!(&line[start..i], "true" | "false" | "null") {
                    out.push((start..i, JsonToken::Keyword));
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// One piece of an XML document, as `pretty_xml` reads it.
enum XmlPart<'a> {
    /// `<?…?>`, `<!--…-->`, `<!DOCTYPE …>`: kept as written, on a line of its own.
    Verbatim(&'a str),
    Open {
        name: &'a str,
        tag: &'a str,
    },
    Close {
        name: &'a str,
        tag: &'a str,
    },
    SelfClosing(&'a str),
    /// Text or CDATA, trimmed.
    Text(&'a str),
}

/// The end of the tag that starts at `start` (just after its `>`), skipping
/// `>` inside quoted attribute values.
fn tag_end(s: &str, start: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut quote = None;
    for (i, &c) in b.iter().enumerate().skip(start) {
        match (quote, c) {
            (None, b'"' | b'\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, b'>') => return Some(i + 1),
            _ => {}
        }
    }
    None
}

fn tag_name(tag: &str) -> &str {
    let inner = tag.trim_start_matches(['<', '/']);
    let end = inner
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(inner.len());
    &inner[..end]
}

/// HTML elements that never have a closing tag.
const HTML_VOID: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn xml_parts(s: &str, html: bool) -> Option<Vec<XmlPart<'_>>> {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if !rest.starts_with('<') {
            let end = rest.find('<').map_or(s.len(), |n| i + n);
            let text = s[i..end].trim();
            if !text.is_empty() {
                parts.push(XmlPart::Text(text));
            }
            i = end;
            continue;
        }
        let (end, part) = if rest.starts_with("<!--") {
            let end = i + rest.find("-->")? + 3;
            (end, XmlPart::Verbatim(&s[i..end]))
        } else if rest.starts_with("<![CDATA[") {
            let end = i + rest.find("]]>")? + 3;
            (end, XmlPart::Text(&s[i..end]))
        } else if rest.starts_with("<?") {
            let end = i + rest.find("?>")? + 2;
            (end, XmlPart::Verbatim(&s[i..end]))
        } else if rest.starts_with("<!") {
            // A DOCTYPE may carry an internal subset in brackets.
            let mut depth = 0;
            let mut end = None;
            for (n, c) in rest.char_indices() {
                match c {
                    '[' => depth += 1,
                    ']' => depth -= 1,
                    '>' if depth == 0 => {
                        end = Some(i + n + 1);
                        break;
                    }
                    _ => {}
                }
            }
            let end = end?;
            (end, XmlPart::Verbatim(&s[i..end]))
        } else {
            let end = tag_end(s, i)?;
            let tag = &s[i..end];
            let name = tag_name(tag);
            if name.is_empty() {
                return None;
            }
            let void = html && HTML_VOID.iter().any(|v| v.eq_ignore_ascii_case(name));
            let part = if tag.starts_with("</") {
                XmlPart::Close { name, tag }
            } else if tag.ends_with("/>") || void {
                XmlPart::SelfClosing(tag)
            } else {
                XmlPart::Open { name, tag }
            };
            // Script and style hold code, not markup: kept as one piece.
            let raw_text = html
                && matches!(part, XmlPart::Open { .. })
                && (name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style"));
            parts.push(part);
            i = end;
            if raw_text {
                let close = format!("</{}", name.to_ascii_lowercase());
                let len = s[i..].to_ascii_lowercase().find(&close)?;
                let code = s[i..i + len].trim();
                if !code.is_empty() {
                    parts.push(XmlPart::Verbatim(code));
                }
                i += len;
            }
            continue;
        };
        parts.push(part);
        i = end;
    }
    Some(parts)
}

/// Re-indents an XML document, two spaces per level. An element holding
/// only text stays on one line. Comments, CDATA, `<?…?>` and DOCTYPE are
/// kept as written; text is trimmed. Returns `None` for anything that is not
/// well-formed enough to indent (unbalanced or unclosed tags), so the caller
/// shows the body as it came.
pub fn pretty_xml(input: &[u8]) -> Option<String> {
    pretty_markup(input, false)
}

/// Like [`pretty_xml`], for HTML: void elements (`<br>`, `<img>`, `<meta>`…)
/// need no closing tag, `<script>` and `<style>` keep their code as written,
/// and tag names match in any case. HTML that leaves out optional closing
/// tags (`<p>`, `<li>`) is returned as `None`, so it shows unchanged.
pub fn pretty_html(input: &[u8]) -> Option<String> {
    pretty_markup(input, true)
}

fn pretty_markup(input: &[u8], html: bool) -> Option<String> {
    let s = std::str::from_utf8(input).ok()?;
    let parts = xml_parts(s, html)?;
    let same = |a: &str, b: &str| {
        if html {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    let mut stack: Vec<&str> = Vec::new();
    let mut elements = 0;
    let line = |out: &mut String, depth: usize, text: &str| {
        if !out.is_empty() {
            out.push('\n');
        }
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(text);
    };
    let mut i = 0;
    while i < parts.len() {
        match &parts[i] {
            XmlPart::Verbatim(text) | XmlPart::Text(text) => line(&mut out, stack.len(), text),
            XmlPart::SelfClosing(tag) => {
                elements += 1;
                line(&mut out, stack.len(), tag);
            }
            XmlPart::Open { name, tag } => {
                elements += 1;
                // <a>text</a> and <a></a> stay on one line.
                match (parts.get(i + 1), parts.get(i + 2)) {
                    (
                        Some(XmlPart::Text(text)),
                        Some(XmlPart::Close {
                            name: close,
                            tag: end,
                        }),
                    ) if same(close, name) => {
                        line(&mut out, stack.len(), &format!("{tag}{text}{end}"));
                        i += 3;
                        continue;
                    }
                    (
                        Some(XmlPart::Close {
                            name: close,
                            tag: end,
                        }),
                        _,
                    ) if same(close, name) => {
                        line(&mut out, stack.len(), &format!("{tag}{end}"));
                        i += 2;
                        continue;
                    }
                    _ => {}
                }
                line(&mut out, stack.len(), tag);
                stack.push(name);
            }
            XmlPart::Close { name, tag } => {
                if !same(stack.pop()?, name) {
                    return None;
                }
                line(&mut out, stack.len(), tag);
            }
        }
        i += 1;
    }
    (stack.is_empty() && elements > 0).then(|| {
        out.push('\n');
        out
    })
}

/// Bytes as a hex dump, 8 per line so it fits a narrow pane: offset, hex
/// and the printable ASCII. At most `max` bytes; a last line says how many
/// were left out.
pub fn hex_dump(bytes: &[u8], max: usize) -> String {
    use std::fmt::Write as _;
    const PER_LINE: usize = 8;
    let shown = &bytes[..bytes.len().min(max)];
    let mut out = String::with_capacity(shown.len() * 6 + 80);
    for (n, chunk) in shown.chunks(PER_LINE).enumerate() {
        let _ = write!(out, "{:08x}  ", n * PER_LINE);
        for i in 0..PER_LINE {
            match chunk.get(i) {
                Some(b) => {
                    let _ = write!(out, "{b:02x} ");
                }
                None => out.push_str("   "),
            }
        }
        out.push_str(" |");
        out.extend(chunk.iter().map(|&b| {
            if (0x20..0x7f).contains(&b) {
                b as char
            } else {
                '.'
            }
        }));
        out.push_str("|\n");
    }
    if bytes.len() > shown.len() {
        let _ = writeln!(out, "… {} more bytes", bytes.len() - shown.len());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indents_xml_and_keeps_text_elements_on_one_line() {
        let raw = br#"<?xml version="1.0"?><!-- list --><users count="2"><user id="1" note="a>b"><name>Ada</name><tags/></user><user id="2"><name><![CDATA[<Grace>]]></name><empty></empty></user></users>"#;
        assert_eq!(
            pretty_xml(raw).unwrap(),
            r#"<?xml version="1.0"?>
<!-- list -->
<users count="2">
  <user id="1" note="a>b">
    <name>Ada</name>
    <tags/>
  </user>
  <user id="2">
    <name><![CDATA[<Grace>]]></name>
    <empty></empty>
  </user>
</users>
"#
        );
        // Already indented input comes out the same.
        let once = pretty_xml(raw).unwrap();
        assert_eq!(pretty_xml(once.as_bytes()).unwrap(), once);
        let doctype = b"<!DOCTYPE note [<!ELEMENT note (#PCDATA)>]><note>hi</note>";
        assert_eq!(
            pretty_xml(doctype).unwrap(),
            "<!DOCTYPE note [<!ELEMENT note (#PCDATA)>]>\n<note>hi</note>\n"
        );
    }

    #[test]
    fn indents_html_with_void_elements_and_scripts() {
        let raw = b"<!DOCTYPE html><HTML><head><meta charset=\"utf-8\"><title>Apix</title>\
            <script>if (a < b && c > d) { go(); }</script></head>\
            <body><h1 class=\"hero\">Hello</h1><br><img src=\"x.png\"><p>Text</P></body></html>";
        assert_eq!(
            pretty_html(raw).unwrap(),
            "<!DOCTYPE html>
<HTML>
  <head>
    <meta charset=\"utf-8\">
    <title>Apix</title>
    <script>
      if (a < b && c > d) { go(); }
    </script>
  </head>
  <body>
    <h1 class=\"hero\">Hello</h1>
    <br>
    <img src=\"x.png\">
    <p>Text</P>
  </body>
</html>
"
        );
        // Optional closing tags left out: shown as it came.
        assert!(pretty_html(b"<ul><li>a<li>b</ul>").is_none());
        // In XML mode a <br> is an open tag like any other.
        assert!(pretty_xml(b"<p><br></p>").is_none());
    }

    #[test]
    fn leaves_broken_xml_alone() {
        assert!(pretty_xml(b"<a><b></a>").is_none(), "mismatched");
        assert!(pretty_xml(b"<a>").is_none(), "unclosed");
        assert!(pretty_xml(b"<a attr=\"x>").is_none(), "unterminated tag");
        assert!(pretty_xml(b"just text").is_none());
        assert!(pretty_xml(&[0xff, 0xfe]).is_none(), "not UTF-8");
    }

    #[test]
    fn dumps_hex() {
        let dump = hex_dump(b"Hello, Apix!\x00\x01\xff end", 64);
        assert_eq!(
            dump,
            "00000000  48 65 6c 6c 6f 2c 20 41  |Hello, A|\n\
             00000008  70 69 78 21 00 01 ff 20  |pix!... |\n\
             00000010  65 6e 64                 |end|\n"
        );
        assert!(hex_dump(&[0u8; 40], 16).ends_with("… 24 more bytes\n"));
    }

    #[test]
    fn pretty_matches_two_space_layout() {
        let input = br#"{"a":1,"b":[true,null,{}],"c":{"d":"x, y: {z}"},"e":[]}"#;
        let expected = "{\n  \"a\": 1,\n  \"b\": [\n    true,\n    null,\n    {}\n  ],\n  \"c\": {\n    \"d\": \"x, y: {z}\"\n  },\n  \"e\": []\n}";
        assert_eq!(pretty_json(input).as_deref(), Some(expected));
    }

    #[test]
    fn pretty_keeps_numbers_and_escapes_verbatim() {
        let input = br#"[12345678901234567890, 1.50, -0e+3, "q\"\\"]"#;
        assert_eq!(
            pretty_json(input).as_deref(),
            Some("[\n  12345678901234567890,\n  1.50,\n  -0e+3,\n  \"q\\\"\\\\\"\n]")
        );
    }

    #[test]
    fn pretty_rejects_broken_json() {
        for bad in [&b""[..], b"{", b"[1}", b"\"open", b"[nope]", b"1,2"] {
            assert_eq!(pretty_json(bad), None, "{:?}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn rows_split_lines_and_long_lines() {
        let text = "ab\r\ncdefghij\n\nk";
        let rows = RowIndex::new(text, 4);
        let all: Vec<_> = (0..rows.len()).map(|ix| rows.row(text, ix)).collect();
        assert_eq!(
            all,
            [
                ("ab", 1, false),
                ("cdef", 2, false),
                ("ghij", 2, true),
                ("", 3, false),
                ("k", 4, false)
            ]
        );
        assert_eq!(rows.lines(), 4);
        assert_eq!(rows.widest(), 1);
    }

    #[test]
    fn rows_never_split_inside_a_char() {
        let text = "ğüşıöç";
        let rows = RowIndex::new(text, 5);
        let joined: String = (0..rows.len()).map(|ix| rows.row(text, ix).0).collect();
        assert_eq!(joined, text);
        assert!(rows.len() > 1);
    }

    #[test]
    fn tokens_tell_keys_from_values() {
        let line = r#"  "id": 7, "ok": true, "name": "a\"b","#;
        let kinds: Vec<_> = json_tokens(line)
            .into_iter()
            .map(|(r, k)| (&line[r], k))
            .collect();
        assert_eq!(
            kinds,
            [
                ("\"id\"", JsonToken::Key),
                (":", JsonToken::Punctuation),
                ("7", JsonToken::Number),
                (",", JsonToken::Punctuation),
                ("\"ok\"", JsonToken::Key),
                (":", JsonToken::Punctuation),
                ("true", JsonToken::Keyword),
                (",", JsonToken::Punctuation),
                ("\"name\"", JsonToken::Key),
                (":", JsonToken::Punctuation),
                ("\"a\\\"b\"", JsonToken::String),
                (",", JsonToken::Punctuation),
            ]
        );
    }
}
