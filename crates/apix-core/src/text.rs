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

#[cfg(test)]
mod tests {
    use super::*;

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
