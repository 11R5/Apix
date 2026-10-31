//! Types written in a programming language, read closely enough to be
//! messages: fields, their types, and field numbers where the code has them
//! (Go's `protobuf:"…"` tags, Rust's `#[prost(…)]`, ts-proto's encoders,
//! betterproto, `[ProtoMember(n)]`, `@ProtoNumber(n)`). This reads declarations
//! only, not whole programs.

use std::collections::HashSet;

use regex::Regex;

use super::schema::{EnumDef, Field, Msg, Schema, Ty, names_from_key, to_pascal, to_snake};

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a valid pattern")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lang {
    Go,
    Rust,
    TypeScript,
    Kotlin,
    Python,
    Swift,
    CSharp,
    Java,
    Dart,
    Cpp,
}

impl Lang {
    pub(super) fn name(self) -> &'static str {
        match self {
            Lang::Go => "Go",
            Lang::Rust => "Rust",
            Lang::TypeScript => "TypeScript",
            Lang::Kotlin => "Kotlin",
            Lang::Python => "Python",
            Lang::Swift => "Swift",
            Lang::CSharp => "C#",
            Lang::Java => "Java",
            Lang::Dart => "Dart",
            Lang::Cpp => "C++",
        }
    }

    /// `name: Type` rather than `Type name`.
    fn colon_style(self) -> bool {
        matches!(
            self,
            Lang::Rust | Lang::TypeScript | Lang::Kotlin | Lang::Python | Lang::Swift
        )
    }
}

fn detect(text: &str) -> Option<Lang> {
    let has = |p: &str| re(p).is_match(text);
    if has(r"(?m)^\s*type\s+\w+\s+struct\s*\{") {
        return Some(Lang::Go);
    }
    if has(r"(?m)^\s*class\s+\w+\s*(\([^)]*\))?\s*:\s*$") {
        return Some(Lang::Python);
    }
    if has(
        r"\{\s*get\s*;|\bnamespace\s+[\w.]+|\busing\s+System|\brecord\s+\w+\s*\(\s*(string|int|long|bool|double)\b|\bpublic\s+(string|bool|decimal)\s+\w+",
    ) {
        return Some(Lang::CSharp);
    }
    if has(r"\bdata\s+class\b|\bclass\s+\w+\s*\(\s*(val|var)\s") {
        return Some(Lang::Kotlin);
    }
    if has(r"\bstruct\s+\w+[^{;]*\{[^}]*\b(let|var)\s+\w+\s*:") {
        return Some(Lang::Swift);
    }
    if has(r"\bstruct\s+\w+[^{;]*\{\s*(#\[[^\]]*\]\s*)*(pub(\([^)]*\))?\s+)?\w+\s*:\s*\w") {
        return Some(Lang::Rust);
    }
    if has(r"\bstd::|\bint(8|16|32|64)_t\b") {
        return Some(Lang::Cpp);
    }
    if has(r"\bstruct\s+\w+\s*\{\s*\w+\s+\w+\s*;") {
        return Some(Lang::Cpp);
    }
    if has(r"\b(interface|type)\s+\w+[^;]*?(\{|=)")
        || has(r"\bclass\s+\w+[^{]*\{\s*(public\s+|private\s+|readonly\s+)*\w+[?!]?\s*:\s*\w")
    {
        return Some(Lang::TypeScript);
    }
    if has(r"\b(val|var)\s+\w+\s*:\s*\w") {
        return Some(Lang::Kotlin);
    }
    if has(r"\bfinal\s+\w+\??\s+\w+\s*;|\b(int|double|String|bool)\?\s+\w+\s*;") {
        return Some(Lang::Dart);
    }
    if has(
        r"\bimport\s+java|\b(private|protected|public)\s+[\w<>\[\], ]+\s+\w+\s*(=[^;]*)?;|\brecord\s+\w+\s*\(|\bclass\s+\w+[^{]*\{\s*(@\w+(\([^)]*\))?\s*)*\w+(<[^;]*>)?(\[\])?\s+\w+\s*;",
    ) {
        return Some(Lang::Java);
    }
    None
}

/// `text` without comments; strings are left alone.
fn strip_comments(text: &str, lang: Lang) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut ix = 0;
    let mut start = 0;
    let mut string: Option<u8> = None;
    while ix < bytes.len() {
        let c = bytes[ix];
        if let Some(quote) = string {
            if c == b'\\' {
                ix += 1;
            } else if c == quote || (c == b'\n' && quote != b'`') {
                string = None;
            }
            ix += 1;
            continue;
        }
        match c {
            b'"' | b'`' => string = Some(c),
            b'\'' if lang != Lang::Rust => string = Some(c),
            b'#' if lang == Lang::Python => {
                out.push_str(&text[start..ix]);
                while ix < bytes.len() && bytes[ix] != b'\n' {
                    ix += 1;
                }
                start = ix;
                continue;
            }
            b'/' if lang != Lang::Python && bytes.get(ix + 1) == Some(&b'/') => {
                out.push_str(&text[start..ix]);
                while ix < bytes.len() && bytes[ix] != b'\n' {
                    ix += 1;
                }
                start = ix;
                continue;
            }
            b'/' if lang != Lang::Python && bytes.get(ix + 1) == Some(&b'*') => {
                out.push_str(&text[start..ix]);
                ix += 2;
                while ix + 1 < bytes.len() && !(bytes[ix] == b'*' && bytes[ix + 1] == b'/') {
                    ix += 1;
                }
                ix = (ix + 2).min(bytes.len());
                start = ix;
                continue;
            }
            _ => {}
        }
        ix += 1;
    }
    out.push_str(&text[start..]);
    out
}

/// Where the bracket opening at `open` closes.
fn closing(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (a, b) = match bytes[open] {
        b'{' => (b'{', b'}'),
        b'(' => (b'(', b')'),
        b'[' => (b'[', b']'),
        b'<' => (b'<', b'>'),
        _ => return None,
    };
    let mut depth = 0;
    let mut string: Option<u8> = None;
    let mut ix = open;
    while ix < bytes.len() {
        let c = bytes[ix];
        if let Some(quote) = string {
            if c == b'\\' {
                ix += 1;
            } else if c == quote {
                string = None;
            }
        } else if c == b'"' || c == b'`' {
            string = Some(c);
        } else if c == a {
            depth += 1;
        } else if c == b {
            // `=>` closes nothing.
            if !(c == b'>' && ix > 0 && bytes[ix - 1] == b'=') {
                depth -= 1;
                if depth == 0 {
                    return Some(ix);
                }
            }
        }
        ix += 1;
    }
    None
}

/// `text` cut at each of `separators` outside all brackets and strings.
fn split_top(text: &str, separators: &[char]) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut string: Option<char> = None;
    let mut chars = text.chars().peekable();
    let mut previous = ' ';
    while let Some(c) = chars.next() {
        if let Some(quote) = string {
            current.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else if c == quote {
                string = None;
            }
            previous = c;
            continue;
        }
        match c {
            '"' | '`' => string = Some(c),
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if previous != '=' && previous != '-' => depth -= 1,
            _ => {}
        }
        if depth <= 0 && separators.contains(&c) {
            parts.push(std::mem::take(&mut current));
            depth = 0;
        } else {
            current.push(c);
        }
        previous = c;
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Where `what` first appears in `text` outside all brackets and strings.
fn find_top(text: &str, what: char) -> Option<usize> {
    let mut depth = 0i32;
    let mut string: Option<char> = None;
    let mut previous = ' ';
    for (ix, c) in text.char_indices() {
        if let Some(quote) = string {
            if c == quote && previous != '\\' {
                string = None;
            }
        } else {
            if depth == 0 && c == what {
                return Some(ix);
            }
            match c {
                '"' | '`' => string = Some(c),
                '(' | '[' | '{' | '<' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                '>' if previous != '=' && previous != '-' => depth -= 1,
                _ => {}
            }
        }
        previous = c;
    }
    None
}

/// A type, as far as protobuf cares.
#[derive(Debug, Clone)]
struct Parsed {
    ty: Ty,
    repeated: bool,
    optional: bool,
}

impl Parsed {
    fn plain(ty: Ty) -> Self {
        Self {
            ty,
            repeated: false,
            optional: false,
        }
    }
}

fn scalar(name: &str) -> Ty {
    Ty::Scalar(name.to_owned())
}

/// The builder of one source: what it declares, and what it has noticed.
struct Reader<'a> {
    lang: Lang,
    declared: HashSet<String>,
    schema: &'a mut Schema,
    notes: &'a mut Vec<String>,
    /// `type X = string | number;` and the like, by name.
    aliases: Vec<(String, String)>,
}

impl Reader<'_> {
    fn simple_type(&mut self, name: &str, place: &str) -> Option<Ty> {
        let last = name.rsplit([':', '.']).next().unwrap_or(name);
        if self.declared.contains(last) {
            return Some(Ty::Named(last.to_owned()));
        }
        let lower = name.to_ascii_lowercase();
        let lower = lower.trim_start_matches("std::").trim();
        let short = last.to_ascii_lowercase();
        let int_is_64 = matches!(
            self.lang,
            Lang::Go | Lang::Python | Lang::Swift | Lang::Dart
        );
        let found = match lower {
            "long long" | "long long int" => "int64",
            "unsigned int" | "unsigned" => "uint32",
            "unsigned long" | "unsigned long long" => "uint64",
            "interface{}" | "serde_json::value" => "google.protobuf.Value",
            _ => match short.as_str() {
                "string" | "str" | "nsstring" | "charsequence" | "char" | "character"
                | "string_view" | "uuid" | "guid" | "cstring" => "string",
                "bool" | "boolean" => "bool",
                "int" | "integer" if int_is_64 => "int64",
                "int" | "integer" | "int32" | "i32" | "int32_t" | "short" | "int16" | "i16"
                | "int16_t" | "int8" | "i8" | "int8_t" | "sbyte" => "int32",
                "long" | "int64" | "i64" | "int64_t" | "bigint" | "biginteger" | "isize" => "int64",
                "uint" | "uint32" | "u32" | "uint32_t" | "ushort" | "uint16" | "u16"
                | "uint16_t" | "uint8" | "u8" | "uint8_t" | "byte" => "uint32",
                "ulong" | "uint64" | "u64" | "uint64_t" | "usize" => "uint64",
                "float" if self.lang == Lang::Python => "double",
                "float" | "float32" | "f32" | "single" => "float",
                "double" | "float64" | "f64" | "decimal" | "bigdecimal" | "cgfloat" => "double",
                "number" => {
                    self.notes.push(format!(
                        "{place} is a `number`, which could be any numeric type; it was taken for a double, and an integer goes differently on the wire"
                    ));
                    "double"
                }
                "bytes" | "bytearray" | "bytestring" | "uint8array" | "buffer" | "memoryview"
                | "arraybuffer" | "data" => "bytes",
                "date" | "datetime" | "instant" | "time" | "timestamp" | "localdatetime"
                | "offsetdatetime" | "zoneddatetime" | "datetimeoffset" | "systemtime"
                | "nsdate" => "google.protobuf.Timestamp",
                "duration" | "timedelta" | "timespan" => "google.protobuf.Duration",
                "any" | "unknown" | "object" | "dynamic" | "jsonnode" | "jsonelement"
                | "jsonvalue" => "google.protobuf.Value",
                "dict" | "jsonobject" => "google.protobuf.Struct",
                "void" | "none" | "never" | "null" | "undefined" | "nil" | "unit" => return None,
                _ => {
                    return Some(Ty::Named(to_pascal(last)));
                }
            },
        };
        Some(scalar(found))
    }

    /// Reads `Vec<Option<String>>`, `[]*Foo`, `map[string]int`, `string[] | null`, ….
    fn parse_type(&mut self, text: &str, owner: &str, field: &str) -> Option<Parsed> {
        let place = format!("{owner}.{field}");
        let mut s = text.trim();
        for prefix in ["&mut ", "&", "mut ", "readonly ", "const ", "final "] {
            if let Some(rest) = s.strip_prefix(prefix) {
                return self.parse_type(rest, owner, field);
            }
        }
        let mut optional = false;
        if let Some(rest) = s.strip_prefix('*') {
            s = rest.trim();
            optional = true;
        }
        while let Some(rest) = s.strip_suffix(['?', '!', '*', '&']) {
            s = rest.trim();
            optional = true;
        }
        // A union: what is left without null, or a string for a set of literals.
        let parts = split_top(s, &['|']);
        if parts.len() > 1 {
            let real: Vec<&String> = parts
                .iter()
                .filter(|p| !matches!(p.as_str(), "null" | "undefined" | "None" | "nil"))
                .collect();
            if real.len() < parts.len() {
                optional = true;
            }
            if real.len() == 1 {
                let mut inner = self.parse_type(real[0], owner, field)?;
                inner.optional |= optional;
                return Some(inner);
            }
            let literals = real
                .iter()
                .all(|p| p.starts_with(['"', '\'']) || p.starts_with("Literal"));
            if !literals {
                self.notes.push(format!(
                    "{place} can be of several types ({s}), so it was taken for a string"
                ));
            }
            return Some(Parsed {
                ty: scalar("string"),
                repeated: false,
                optional,
            });
        }
        if s.starts_with(['"', '\'']) {
            return Some(Parsed {
                ty: scalar("string"),
                repeated: false,
                optional,
            });
        }
        // Functions are not data.
        if s.contains("=>") || s.contains("->") || s.starts_with("func(") {
            return None;
        }
        // An object written in place.
        if s.starts_with('{') {
            let end = closing(s, 0)?;
            let body = &s[1..end];
            if let Some(map) = self.index_signature(body, owner, field) {
                return Some(map);
            }
            let name = self
                .schema
                .free_name(&format!("{}{}", owner, to_pascal(field)));
            self.schema.messages.push(Msg {
                name: name.clone(),
                fields: Vec::new(),
            });
            self.declared.insert(name.clone());
            let fields = self.read_members(&name, &split_top(body, &[';', ',', '\n']));
            if let Some(message) = self.schema.messages.iter_mut().find(|m| m.name == name) {
                message.fields = fields;
            }
            return Some(Parsed {
                ty: Ty::Named(name),
                repeated: false,
                optional,
            });
        }
        // Go: []T, map[K]V.
        if let Some(rest) = s.strip_prefix("[]") {
            if matches!(rest.trim(), "byte" | "uint8") {
                return Some(Parsed::plain(scalar("bytes")));
            }
            return self.repeated_of(rest, owner, field);
        }
        if s.starts_with("map[")
            && let Some(end) = closing(s, 3)
        {
            let (key, value) = (&s[4..end], &s[end + 1..]);
            return self.map_of(key, value, owner, field);
        }
        // Swift [T], [K: V]; Rust [T; N].
        if s.starts_with('[')
            && let Some(end) = closing(s, 0)
            && end == s.len() - 1
        {
            let inner = &s[1..end];
            if let Some(colon) = find_top(inner, ':') {
                return self.map_of(&inner[..colon], &inner[colon + 1..], owner, field);
            }
            let item = inner.split(';').next().unwrap_or(inner);
            if matches!(item.trim(), "u8" | "UInt8") {
                return Some(Parsed::plain(scalar("bytes")));
            }
            return self.repeated_of(item, owner, field);
        }
        // T[], and T[N].
        if let Some(open) = s.rfind('[')
            && s.ends_with(']')
            && (s[open + 1..s.len() - 1].chars().all(|c| c.is_ascii_digit()))
            && open > 0
        {
            let item = &s[..open];
            if matches!(item.trim(), "byte" | "sbyte" | "uint8_t") {
                return Some(Parsed::plain(scalar("bytes")));
            }
            return self.repeated_of(item, owner, field);
        }
        // Name<…> or Name[…].
        let open = s.find(['<', '[']);
        if let Some(open) = open
            && let Some(end) = closing(s, open)
            && end == s.len() - 1
        {
            let head = s[..open].trim();
            let short = head.rsplit([':', '.']).next().unwrap_or(head);
            let lower = short.to_ascii_lowercase();
            let args = split_top(&s[open + 1..end], &[',']);
            match lower.as_str() {
                "option" | "optional" | "nullable" | "maybe" | "box" | "rc" | "arc"
                | "unique_ptr" | "shared_ptr" | "lazy" | "task" | "promise" | "annotated"
                | "final" | "classvar" | "cow" | "partial" | "required" | "readonly" => {
                    let mut inner = self.parse_type(args.first()?, owner, field)?;
                    inner.optional |= matches!(
                        lower.as_str(),
                        "option" | "optional" | "nullable" | "maybe" | "partial"
                    );
                    return Some(inner);
                }
                "literal" => return Some(Parsed::plain(scalar("string"))),
                "union" => {
                    let joined = args.join(" | ");
                    return self.parse_type(&joined, owner, field);
                }
                "vec"
                | "vector"
                | "list"
                | "ilist"
                | "array"
                | "arraylist"
                | "linkedlist"
                | "set"
                | "hashset"
                | "btreeset"
                | "sortedset"
                | "treeset"
                | "ienumerable"
                | "icollection"
                | "ireadonlylist"
                | "ireadonlycollection"
                | "collection"
                | "iterable"
                | "sequence"
                | "seq"
                | "readonlyarray"
                | "repeatedfield"
                | "mutablelist"
                | "mutableset"
                | "deque"
                | "frozenset"
                | "immutablelist"
                | "tuple"
                | "slice" => {
                    let item = args.first()?;
                    if matches!(item.as_str(), "u8" | "byte" | "Byte") {
                        return Some(Parsed::plain(scalar("bytes")));
                    }
                    return self.repeated_of(item, owner, field);
                }
                "map"
                | "hashmap"
                | "btreemap"
                | "dictionary"
                | "idictionary"
                | "ireadonlydictionary"
                | "dict"
                | "record"
                | "treemap"
                | "linkedhashmap"
                | "mutablemap"
                | "unordered_map"
                | "mapfield"
                | "sortedmap"
                | "mapping"
                | "concurrentdictionary"
                | "indexmap" => {
                    return self.map_of(args.first()?, args.get(1)?, owner, field);
                }
                _ => {
                    self.notes.push(format!(
                        "{place} uses the generic type {s}; protobuf has no generics, so write out what it holds"
                    ));
                    let ty = self.simple_type(head, &place)?;
                    return Some(Parsed {
                        ty,
                        repeated: false,
                        optional,
                    });
                }
            }
        }
        let ty = self.simple_type(s, &place)?;
        Some(Parsed {
            ty,
            repeated: false,
            optional,
        })
    }

    fn index_signature(&mut self, body: &str, owner: &str, field: &str) -> Option<Parsed> {
        let members = split_top(body, &[';', ',', '\n']);
        let [only] = members.as_slice() else {
            return None;
        };
        if !only.starts_with('[') {
            return None;
        }
        let end = closing(only, 0)?;
        let colon = find_top(&only[end + 1..], ':')?;
        let key = only[1..end].split(':').nth(1)?.to_owned();
        let value = only[end + 1 + colon + 1..].to_owned();
        self.map_of(&key, &value, owner, field)
    }

    fn repeated_of(&mut self, item: &str, owner: &str, field: &str) -> Option<Parsed> {
        let inner = self.parse_type(item, owner, field)?;
        if inner.repeated || matches!(inner.ty, Ty::Map(..)) {
            let ty = self.wrap(inner, owner, field);
            return Some(Parsed {
                ty,
                repeated: true,
                optional: false,
            });
        }
        Some(Parsed {
            ty: inner.ty,
            repeated: true,
            optional: false,
        })
    }

    fn map_of(&mut self, key: &str, value: &str, owner: &str, field: &str) -> Option<Parsed> {
        let place = format!("{owner}.{field}");
        let key = self.parse_type(key, owner, field)?;
        let key_ty = match &key.ty {
            Ty::Scalar(name)
                if !matches!(name.as_str(), "double" | "float" | "bytes")
                    && !name.contains('.') =>
            {
                key.ty.clone()
            }
            _ => {
                self.notes.push(format!(
                    "{place} has keys protobuf maps cannot have, so they were taken for strings"
                ));
                scalar("string")
            }
        };
        let value = self.parse_type(value, owner, field)?;
        let value_ty = if value.repeated || matches!(value.ty, Ty::Map(..)) {
            self.wrap(value, owner, field)
        } else {
            value.ty
        };
        Some(Parsed::plain(Ty::Map(Box::new(key_ty), Box::new(value_ty))))
    }

    /// Protobuf has no list of lists or map of lists: a message in between.
    fn wrap(&mut self, inner: Parsed, owner: &str, field: &str) -> Ty {
        let name = self
            .schema
            .free_name(&format!("{owner}{}Item", to_pascal(field)));
        self.notes.push(format!(
            "{owner}.{field} nests lists or maps, which protobuf does not have, so {name} goes in between"
        ));
        self.schema.messages.push(Msg {
            name: name.clone(),
            fields: vec![Field {
                name: "items".into(),
                json_name: None,
                ty: inner.ty,
                repeated: inner.repeated,
                optional: false,
                number: None,
            }],
        });
        self.declared.insert(name.clone());
        Ty::Named(name)
    }

    /// Takes annotations off the front of a member; they are returned too.
    fn take_annotations<'t>(&self, member: &'t str) -> (String, &'t str) {
        let mut rest = member.trim_start();
        let mut taken = String::new();
        loop {
            let open = if rest.starts_with('@') {
                let word_end = rest[1..]
                    .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == ':'))
                    .map_or(rest.len(), |i| i + 1);
                if rest[word_end..].starts_with('(') {
                    Some(word_end)
                } else {
                    taken.push_str(&rest[..word_end]);
                    taken.push(' ');
                    rest = rest[word_end..].trim_start();
                    continue;
                }
            } else if rest.starts_with("#[") {
                Some(1)
            } else if rest.starts_with('[') && matches!(self.lang, Lang::CSharp) {
                Some(0)
            } else {
                None
            };
            let Some(open) = open else {
                return (taken, rest);
            };
            let Some(end) = closing(rest, open) else {
                return (taken, rest);
            };
            taken.push_str(&rest[..=end]);
            taken.push(' ');
            rest = rest[end + 1..].trim_start();
        }
    }

    /// Fields of a message from its members, whichever the language.
    fn read_members(&mut self, owner: &str, members: &[String]) -> Vec<Field> {
        let mut fields = Vec::new();
        let mut pending = String::new();
        let mut last_type: Option<String> = None;
        for member in members {
            let joined = format!("{pending} {member}");
            let (annotations, rest) = self.take_annotations(&joined);
            let rest = rest.to_owned();
            if rest.is_empty() {
                pending = annotations;
                continue;
            }
            pending.clear();
            if let Some(field) = self.read_member(owner, &annotations, &rest, &mut last_type) {
                fields.push(field);
            }
        }
        fields
    }

    fn read_member(
        &mut self,
        owner: &str,
        annotations: &str,
        member: &str,
        last_type: &mut Option<String>,
    ) -> Option<Field> {
        let modifiers = re(
            r"^(?:(?:public|private|protected|internal|readonly|abstract|virtual|override|required|late|transient|volatile|export|declare|lateinit|open|mutable|pub(?:\([^)]*\))?|mut|var|val|let|final)\s+)+",
        );
        let member = modifiers.replace(member, "").into_owned();
        let member = member.trim();
        let first_word = member.split_whitespace().next().unwrap_or_default();
        if matches!(
            first_word,
            "static"
                | "const"
                | "fn"
                | "func"
                | "fun"
                | "def"
                | "function"
                | "class"
                | "struct"
                | "interface"
                | "enum"
                | "record"
                | "typedef"
                | "using"
                | "return"
                | "init"
                | "deinit"
                | "type"
                | "impl"
                | "friend"
                | "constructor"
                | "get"
                | "set"
                | "async"
                | "case"
                | "where"
                | "companion"
                | "object"
        ) || member.ends_with(':')
            || member.starts_with('}')
        {
            return None;
        }
        let (name, ty_text) = if self.lang.colon_style() {
            let colon = find_top(member, ':')?;
            let name = member[..colon].trim().trim_end_matches(['?', '!']);
            let optional_mark = member[..colon].trim().ends_with('?');
            let name = name.trim_matches(['"', '\'']).to_owned();
            if name.is_empty() || name.contains(['(', ' ', '[']) {
                return None;
            }
            let rest = member[colon + 1..].trim();
            let ty = default_cut(rest);
            let ty = if optional_mark {
                format!("{ty}?")
            } else {
                ty.to_owned()
            };
            (name, ty)
        } else {
            let body_free = member
                .find('{')
                .map_or(member, |open| member[..open].trim_end());
            let body_free = default_cut(body_free);
            if find_top(body_free, '(').is_some() || member.contains("=>") {
                return None;
            }
            match body_free
                .rfind(|c: char| c.is_whitespace() || c == '*' || c == '&' || c == '>' || c == ']')
            {
                Some(cut) if cut + 1 < body_free.len() => {
                    let name = body_free[cut + 1..].trim_end_matches("[]");
                    let ty = body_free[..=cut].trim().to_owned();
                    *last_type = Some(ty.clone());
                    (name.to_owned(), ty)
                }
                // `int a, b;`: the second has the type of the first.
                _ if !body_free.is_empty() => (body_free.to_owned(), last_type.clone()?),
                _ => return None,
            }
        };
        if !name
            .chars()
            .all(|c| c.is_alphanumeric() || "_-$.".contains(c))
        {
            return None;
        }
        let parsed = self.parse_type(&ty_text, owner, &name)?;
        Some(self.field_of(owner, &name, annotations, parsed))
    }

    /// A field with what its annotations say about its name and number.
    fn field_of(&mut self, owner: &str, name: &str, annotations: &str, parsed: Parsed) -> Field {
        let renamed = re(
            r#"(?:rename\s*=\s*|JsonPropertyName\(\s*|JsonProperty\(\s*(?:value\s*=\s*)?|SerializedName\(\s*(?:value\s*=\s*)?|SerialName\(\s*|alias\s*=\s*|JsonKey\(\s*name\s*:\s*)["']([^"']+)["']"#,
        )
        .captures(annotations)
        .map(|c| c[1].to_owned());
        let (field_name, json_name) = match &renamed {
            Some(key) => names_from_key(key),
            None => (to_snake(name), None),
        };
        let mut field = Field {
            name: field_name,
            json_name,
            ty: parsed.ty,
            repeated: parsed.repeated,
            optional: parsed.optional,
            number: None,
        };
        self.apply_prost(owner, annotations, &mut field);
        let numbered = re(
            r"(?:ProtoMember|ProtoNumber|Protobuf|ProtoField)\(\s*(?:(?:tag|number|id|fieldNumber)\s*=\s*)?(\d+)",
        )
        .captures(annotations)
        .and_then(|c| c[1].parse().ok());
        if numbered.is_some() {
            field.number = numbered;
        }
        field
    }

    /// `#[prost(string, optional, tag = "2")]`: the type and number, exactly.
    fn apply_prost(&mut self, owner: &str, annotations: &str, field: &mut Field) {
        let Some(inner) = re(r"#\[prost\((.*?)\)\]").captures(annotations) else {
            return;
        };
        let inner = inner[1].to_owned();
        if inner.contains("oneof") {
            self.notes.push(format!(
                "{owner}.{} is a oneof; give its generated enum's fields as a message, or edit the result",
                field.name
            ));
            return;
        }
        if let Some(c) = re(r#"\btag\s*=\s*"(\d+)""#).captures(&inner) {
            field.number = c[1].parse().ok();
        }
        field.repeated = inner.split(',').any(|p| p.trim() == "repeated");
        if let Some(c) = re(r#"\benumeration\s*=\s*"([\w:]+)""#).captures(&inner) {
            let name = c[1].rsplit(':').next().unwrap_or(&c[1]).to_owned();
            field.ty = Ty::Named(name);
            return;
        }
        if let Some(c) =
            re(r#"\bmap\s*=\s*"(\w+),\s*(?:(\w+)|message\(([\w:]+)\)|enumeration\(([\w:]+)\))""#)
                .captures(&inner)
        {
            let value = match (c.get(2), c.get(3), c.get(4)) {
                (Some(scalar_name), _, _) if scalar_name.as_str() != "message" => {
                    scalar(scalar_name.as_str())
                }
                (_, Some(named), _) | (_, _, Some(named)) => Ty::Named(
                    named
                        .as_str()
                        .rsplit(':')
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                ),
                _ => scalar("string"),
            };
            field.ty = Ty::Map(Box::new(scalar(&c[1])), Box::new(value));
            field.repeated = false;
            return;
        }
        let kind = inner.split(',').next().unwrap_or_default().trim();
        let kind = kind.split('=').next().unwrap_or(kind).trim();
        const SCALARS: [&str; 15] = [
            "double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32",
            "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes",
        ];
        if SCALARS.contains(&kind) {
            field.ty = scalar(kind);
        }
    }

    /// Parent fields first, as the type inherits them.
    fn inherit(&mut self, parents: &[(String, Vec<String>)]) {
        for (child, bases) in parents {
            let mut inherited = Vec::new();
            let mut pending: Vec<String> = bases.clone();
            let mut seen = HashSet::new();
            while let Some(base) = pending.pop() {
                if !seen.insert(base.clone()) {
                    continue;
                }
                if let Some((_, more)) = parents.iter().find(|(c, _)| *c == base) {
                    pending.extend(more.iter().cloned());
                }
                if let Some(message) = self.schema.messages.iter().find(|m| m.name == base) {
                    let mut fields = message.fields.clone();
                    fields.append(&mut inherited);
                    inherited = fields;
                }
            }
            if let Some(message) = self.schema.messages.iter_mut().find(|m| m.name == *child) {
                inherited.append(&mut message.fields);
                message.fields = inherited;
            }
        }
    }
}

/// `Type = default` → `Type`, leaving `=>` alone.
fn default_cut(text: &str) -> &str {
    let mut search = 0;
    while let Some(found) = find_top(&text[search..], '=') {
        let at = search + found;
        if text[at + 1..].starts_with('>') {
            search = at + 2;
            continue;
        }
        return text[..at].trim();
    }
    text.trim()
}

/// A declared type in a language with braces or constructor parameters.
struct Block {
    name: String,
    parents: Vec<String>,
    members: Vec<String>,
}

fn identifiers(text: &str) -> Vec<String> {
    re(r"[A-Za-z_][\w.]*")
        .find_iter(text)
        .map(|m| m.as_str().rsplit('.').next().unwrap_or_default().to_owned())
        .filter(|w| {
            !matches!(
                w.as_str(),
                "extends"
                    | "implements"
                    | "with"
                    | "where"
                    | "public"
                    | "private"
                    | "protected"
                    | "virtual"
            )
        })
        .collect()
}

/// Enums, in any language: `A`, `B = 2`, `case c`, `D("d")`.
fn read_enum(name: &str, body: &str) -> EnumDef {
    let mut values = Vec::new();
    for member in split_top(body, &[',', ';', '\n']) {
        let member = member.strip_prefix("case ").unwrap_or(&member).trim();
        for part in split_top(member, &[',']) {
            let (name_part, number) = match find_top(&part, '=') {
                Some(eq) => (
                    part[..eq].trim().to_owned(),
                    part[eq + 1..]
                        .trim()
                        .trim_end_matches(',')
                        .parse::<i32>()
                        .ok(),
                ),
                None => (part.clone(), None),
            };
            let name_part = name_part
                .split(['(', ':', ' '])
                .next()
                .unwrap_or("")
                .trim()
                .to_owned();
            // ts-proto's own marker for values it does not know.
            if name_part.is_empty()
                || name_part == "UNRECOGNIZED"
                || !name_part.chars().all(|c| c.is_alphanumeric() || c == '_')
                || name_part.starts_with(|c: char| c.is_ascii_digit())
            {
                continue;
            }
            values.push((name_part, number));
        }
    }
    EnumDef {
        name: name.to_owned(),
        values,
    }
}

fn read_braced(text: &str, reader: &mut Reader) {
    let mut enum_spans: Vec<(usize, usize)> = Vec::new();
    for c in re(r"\benum\s+(?:class\s+|struct\s+)?(\w+)[^{;]*\{").captures_iter(text) {
        let whole = c.get(0).expect("the whole match");
        let open = whole.end() - 1;
        let Some(end) = closing(text, open) else {
            continue;
        };
        enum_spans.push((whole.start(), end));
        reader
            .schema
            .enums
            .push(read_enum(&c[1], &text[open + 1..end]));
    }
    let mut blocks: Vec<Block> = Vec::new();
    for c in re(r"\b(?:record\s+(?:struct|class)|interface|class|struct|record|type)\s+(\w+)")
        .captures_iter(text)
    {
        let whole = c.get(0).expect("the whole match");
        if enum_spans
            .iter()
            .any(|(a, b)| whole.start() >= *a && whole.start() <= *b)
        {
            continue;
        }
        let mut ix = whole.end();
        let bytes = text.as_bytes();
        // Generic parameters.
        while ix < bytes.len() && bytes[ix].is_ascii_whitespace() {
            ix += 1;
        }
        if bytes.get(ix) == Some(&b'<') {
            let Some(end) = closing(text, ix) else {
                continue;
            };
            ix = end + 1;
        }
        let mut block = Block {
            name: c[1].to_owned(),
            parents: Vec::new(),
            members: Vec::new(),
        };
        let mut found = false;
        // Header up to what opens the body; stops at a declaration's end.
        let header_start = ix;
        while ix < bytes.len() {
            match bytes[ix] {
                b'(' => {
                    let Some(end) = closing(text, ix) else {
                        break;
                    };
                    if !found && text[header_start..ix].trim().is_empty() {
                        block
                            .members
                            .extend(split_top(&text[ix + 1..end], &[',', '\n']));
                        found = true;
                    }
                    ix = end + 1;
                }
                b'=' => {
                    // TypeScript's `type X = …`: an object, parents joined by `&`, or an alias.
                    let rest = text[ix + 1..].trim_start();
                    let rest_at = text.len() - rest.len();
                    let mut search = rest_at;
                    let mut body = None;
                    for part_end in text[rest_at..].char_indices() {
                        let (at, ch) = part_end;
                        if ch == '{' {
                            search = rest_at + at;
                            body = closing(text, search);
                            break;
                        }
                        if ch == ';' || ch == '\n' {
                            break;
                        }
                    }
                    if let Some(end) = body {
                        block.parents.extend(identifiers(&text[rest_at..search]));
                        block
                            .members
                            .extend(split_top(&text[search + 1..end], &[';', ',', '\n']));
                        found = true;
                    } else {
                        let alias = rest.split([';', '\n']).next().unwrap_or_default().trim();
                        reader.alias(&block.name, alias);
                    }
                    break;
                }
                b'{' => {
                    let Some(end) = closing(text, ix) else {
                        break;
                    };
                    block.parents.extend(identifiers(&text[header_start..ix]));
                    block
                        .members
                        .extend(split_top(&text[ix + 1..end], &[';', ',', '\n']));
                    found = true;
                    break;
                }
                b';' => break,
                b'\n' if found => break,
                _ => ix += 1,
            }
        }
        if found && !blocks.iter().any(|b| b.name == block.name) {
            blocks.push(block);
        }
    }
    reader.build(blocks);
}

impl Reader<'_> {
    /// `type X = string | number;` and the like: nothing to declare, and a
    /// message using `X` should see through it.
    fn alias(&mut self, name: &str, target: &str) {
        self.aliases.push((name.to_owned(), target.to_owned()));
    }

    fn build(&mut self, blocks: Vec<Block>) {
        for e in &self.schema.enums {
            self.declared.insert(e.name.clone());
        }
        for block in &blocks {
            self.declared.insert(block.name.clone());
        }
        let mut parents = Vec::new();
        for block in &blocks {
            self.schema.messages.push(Msg {
                name: block.name.clone(),
                fields: Vec::new(),
            });
        }
        for block in blocks {
            let fields = self.read_members(&block.name, &block.members);
            if let Some(message) = self
                .schema
                .messages
                .iter_mut()
                .find(|m| m.name == block.name)
            {
                message.fields = fields;
            }
            let bases: Vec<String> = block
                .parents
                .into_iter()
                .filter(|p| self.declared.contains(p))
                .collect();
            if !bases.is_empty() {
                parents.push((block.name, bases));
            }
        }
        self.inherit(&parents);
    }
}

/// Replaces aliases where they are used, before anything is read.
fn apply_aliases(text: &str, aliases: Vec<(String, String)>) -> String {
    let mut out = text.to_owned();
    for (name, target) in aliases {
        let pattern = re(&format!(r":\s*{}\b", regex::escape(&name)));
        let replacement = format!(": {target}");
        out = pattern
            .replace_all(&out, regex::NoExpand(&replacement))
            .into_owned();
    }
    out
}

fn read_go(text: &str, reader: &mut Reader) {
    // `type Kind int32` with constants of it is an enum.
    for c in re(r"(?m)^\s*type\s+(\w+)\s+(int32|int64|int|uint32|string)\s*$").captures_iter(text) {
        let name = c[1].to_owned();
        let constant = re(&format!(
            r#"(?m)^\s*(\w+)\s+{}\s*=\s*(-?\d+|"[^"]*"|iota)"#,
            regex::escape(&name)
        ));
        let mut values: Vec<(String, Option<i32>)> = constant
            .captures_iter(text)
            .map(|v| (v[1].to_owned(), v[2].parse().ok()))
            .collect();
        for (value, _) in &mut values {
            if let Some(rest) = value.strip_prefix(&format!("{name}_")) {
                *value = rest.to_owned();
            }
        }
        if !values.is_empty() {
            reader.schema.enums.push(EnumDef { name, values });
        }
    }
    let mut blocks: Vec<(String, String)> = Vec::new();
    for c in re(r"(?m)^\s*type\s+(\w+)\s+struct\s*\{").captures_iter(text) {
        let whole = c.get(0).expect("the whole match");
        let open = whole.end() - 1;
        if let Some(end) = closing(text, open) {
            blocks.push((c[1].to_owned(), text[open + 1..end].to_owned()));
        }
    }
    for e in &reader.schema.enums {
        reader.declared.insert(e.name.clone());
    }
    for (name, _) in &blocks {
        reader.declared.insert(name.clone());
        reader.schema.messages.push(Msg {
            name: name.clone(),
            fields: Vec::new(),
        });
    }
    let line = re(r"^([A-Za-z_]\w*(?:\s*,\s*[A-Za-z_]\w*)*)\s+([^`]+?)\s*(`[^`]*`)?$");
    for (name, body) in blocks {
        let tagged = body.contains("protobuf:\"");
        let mut fields = Vec::new();
        for raw in body.lines() {
            let member = raw.trim();
            let Some(c) = line.captures(member) else {
                continue;
            };
            let tag = c.get(3).map_or("", |m| m.as_str());
            if tagged && !tag.contains("protobuf:\"") {
                if tag.contains("protobuf_oneof") {
                    reader.notes.push(format!(
                        "{name} has a oneof, which struct tags do not describe; add its fields to the result"
                    ));
                }
                continue;
            }
            let json = re(r#"json:"([^",]*)"#)
                .captures(tag)
                .map(|j| j[1].to_owned());
            if json.as_deref() == Some("-") {
                continue;
            }
            for one in c[1].split(',') {
                let one = one.trim();
                if !one.starts_with(|ch: char| ch.is_ascii_uppercase()) {
                    continue;
                }
                let go_type = c[2].trim().to_owned();
                let Some(parsed) = reader.parse_type(&go_type, &name, one) else {
                    continue;
                };
                let mut field = Field {
                    name: to_snake(one),
                    json_name: None,
                    ty: parsed.ty,
                    repeated: parsed.repeated,
                    optional: parsed.optional,
                    number: None,
                };
                if let Some(key) = json.as_ref().filter(|j| !j.is_empty()) {
                    (field.name, field.json_name) = names_from_key(key);
                }
                apply_go_tag(tag, &go_type, &mut field);
                fields.push(field);
            }
        }
        if let Some(message) = reader.schema.messages.iter_mut().find(|m| m.name == name) {
            message.fields = fields;
        }
    }
}

/// Go's wire kind and Go type together give the protobuf type.
fn go_kind(wire: &str, go_type: &str) -> Option<&'static str> {
    let bare = go_type.trim_start_matches("[]").trim_start_matches('*');
    Some(match (wire, bare) {
        ("zigzag32", _) => "sint32",
        ("zigzag64", _) => "sint64",
        ("fixed32", "float32") => "float",
        ("fixed32", "int32") => "sfixed32",
        ("fixed32", _) => "fixed32",
        ("fixed64", "float64") => "double",
        ("fixed64", "int64") => "sfixed64",
        ("fixed64", _) => "fixed64",
        ("varint", "bool") => "bool",
        ("varint", "int32") => "int32",
        ("varint", "int64") => "int64",
        ("varint", "uint32") => "uint32",
        ("varint", "uint64") => "uint64",
        ("bytes", "string") => "string",
        ("bytes", "byte") => "bytes",
        _ => return None,
    })
}

fn apply_go_tag(tag: &str, go_type: &str, field: &mut Field) {
    let Some(c) = re(r#"protobuf:"(\w+),(\d+),(opt|req|rep)([^"]*)""#).captures(tag) else {
        return;
    };
    field.number = c[2].parse().ok();
    field.repeated = &c[3] == "rep";
    if let Some(name) = re(r"name=(\w+)").captures(&c[4]) {
        field.name = name[1].to_owned();
        field.json_name = None;
    }
    if let Some(json) = re(r"json=(\w+)").captures(&c[4]) {
        let (_, keep) = (&field.name, json[1].to_owned());
        field.json_name = (keep != super::schema::default_json_name(&field.name)).then_some(keep);
    }
    if let Some(e) = re(r"enum=([\w.]+)").captures(&c[4]) {
        let name = e[1].rsplit('.').next().unwrap_or(&e[1]).to_owned();
        field.ty = Ty::Named(name);
        return;
    }
    if tag.contains("protobuf_key:") {
        let wire = |label: &str| -> Option<String> {
            re(&format!(r#"{label}:"(\w+),"#))
                .captures(tag)
                .map(|w| w[1].to_owned())
        };
        if let (Some(key), Some(value)) = (wire("protobuf_key"), wire("protobuf_val"))
            && let Ty::Map(key_ty, value_ty) = &mut field.ty
        {
            if let Some(found) = go_map_kind(&key, key_ty) {
                **key_ty = found;
            }
            if let Some(found) = go_map_kind(&value, value_ty) {
                **value_ty = found;
            }
        }
        field.repeated = false;
        return;
    }
    if let Some(kind) = go_kind(&c[1], go_type) {
        field.ty = scalar(kind);
        if go_type == "[]byte" {
            field.repeated = false;
        }
    }
}

fn go_map_kind(wire: &str, ty: &Ty) -> Option<Ty> {
    let Ty::Scalar(name) = ty else {
        return None;
    };
    let go = match name.as_str() {
        "float" => "float32",
        "double" => "float64",
        "bytes" => "byte",
        other => other,
    };
    go_kind(wire, go).map(scalar)
}

fn read_python(text: &str, reader: &mut Reader) {
    let header = re(r"^(\s*)class\s+(\w+)\s*(?:\(([^)]*)\))?\s*:\s*$");
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks: Vec<Block> = Vec::new();
    let mut ix = 0;
    while ix < lines.len() {
        let Some(c) = header.captures(lines[ix]) else {
            ix += 1;
            continue;
        };
        let indent = c[1].len();
        let name = c[2].to_owned();
        let bases = c.get(3).map_or("", |m| m.as_str()).to_owned();
        let mut body = Vec::new();
        ix += 1;
        while ix < lines.len() {
            let line = lines[ix];
            if !line.trim().is_empty() {
                let own = line.len() - line.trim_start().len();
                if own <= indent {
                    break;
                }
                if own == indent + 4
                    || (body.is_empty() && own > indent)
                    || line.starts_with(&format!("{}\t", &c[1]))
                {
                    body.push(line.trim().to_owned());
                }
            }
            ix += 1;
        }
        if re(r"\b(Enum|IntEnum|StrEnum|betterproto\.Enum)\b").is_match(&bases) {
            reader.schema.enums.push(read_enum(&name, &body.join("\n")));
            continue;
        }
        blocks.push(Block {
            name,
            parents: identifiers(&bases),
            members: body,
        });
    }
    // betterproto: `name: str = betterproto.string_field(1)`.
    let better = re(
        r"betterproto\.(\w+)_field\(\s*(\d+)(?:\s*,\s*betterproto\.TYPE_(\w+)\s*,\s*betterproto\.TYPE_(\w+))?",
    );
    for block in &mut blocks {
        for member in &mut block.members {
            if let Some(c) = better.captures(member) {
                let tag = format!("@ProtoField({}) ", &c[2]);
                *member = format!("{tag}{member}");
            }
        }
    }
    let kinds: Vec<(String, String, String)> = blocks
        .iter()
        .flat_map(|block| {
            block.members.iter().filter_map(|member| {
                let c = better.captures(member)?;
                let field = member
                    .rsplit_once(") ")
                    .map_or(member.as_str(), |(_, f)| f)
                    .split(':')
                    .next()?
                    .trim()
                    .to_owned();
                Some((block.name.clone(), to_snake(&field), c[1].to_owned()))
            })
        })
        .collect();
    reader.build(blocks);
    // The field function names the exact protobuf type.
    for (message, field, kind) in kinds {
        if kind == "message" || kind == "enum" || kind == "map" {
            continue;
        }
        if let Some(found) = reader
            .schema
            .messages
            .iter_mut()
            .find(|m| m.name == message)
            .and_then(|m| m.fields.iter_mut().find(|f| f.name == field))
        {
            found.ty = scalar(&kind);
        }
    }
}

/// ts-proto writes each field with `writer.uint32(tag)`: the tag is the
/// number shifted by three, and the call after it names the type.
fn apply_ts_proto(text: &str, reader: &mut Reader) {
    let constants = re(r"(?m)^\s*export\s+const\s+(\w+)\b[^=\n]*=\s*\{");
    let starts: Vec<(usize, String)> = constants
        .captures_iter(text)
        .map(|c| (c.get(0).expect("the whole match").start(), c[1].to_owned()))
        .collect();
    let write = re(r"writer\.uint32\((\d+)\)(?:\.(\w+)\()?");
    let of_message = re(r"message\.(\w+)");
    for (ix, (start, name)) in starts.iter().enumerate() {
        let end = starts.get(ix + 1).map_or(text.len(), |n| n.0);
        let segment = &text[*start..end];
        // Only the encoder; the decoder and the rest name fields too.
        let encode_end = segment.find("decode(").unwrap_or(segment.len());
        let lines: Vec<&str> = segment[..encode_end].lines().collect();
        for (line_ix, line) in lines.iter().enumerate() {
            let Some(c) = write.captures(line) else {
                continue;
            };
            let Ok(tag) = c[1].parse::<u32>() else {
                continue;
            };
            let number = tag >> 3;
            // A packed list names its field after `.fork()`, the others on or before.
            let near: Vec<usize> = if line.trim_end().ends_with(".fork();") {
                (line_ix..lines.len().min(line_ix + 3)).collect()
            } else {
                (0..=2)
                    .filter_map(|back| line_ix.checked_sub(back))
                    .collect()
            };
            let field = near
                .into_iter()
                .find_map(|l| of_message.captures(lines[l]).map(|m| m[1].to_owned()));
            let Some(field) = field else {
                continue;
            };
            // A packed list is `.fork()`, then the items on the next line.
            let kind = c
                .get(2)
                .map(|k| k.as_str().to_owned())
                .filter(|k| k != "fork")
                .or_else(|| {
                    lines
                        .get(line_ix + 2)
                        .or(lines.get(line_ix + 1))
                        .and_then(|next| re(r"writer\.(\w+)\(").captures(next))
                        .map(|k| k[1].to_owned())
                });
            let wanted = to_snake(&field);
            if let Some(found) = reader
                .schema
                .messages
                .iter_mut()
                .find(|m| m.name == *name)
                .and_then(|m| m.fields.iter_mut().find(|f| f.name == wanted))
            {
                found.number = Some(number);
                let number_note = format!("{name}.{field} is a `number`");
                reader.notes.retain(|n| !n.starts_with(&number_note));
                const SCALARS: [&str; 15] = [
                    "double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64",
                    "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes",
                ];
                if let Some(kind) = kind.filter(|k| SCALARS.contains(&k.as_str()))
                    && !matches!(found.ty, Ty::Map(..))
                    && !matches!(&found.ty, Ty::Named(_))
                {
                    found.ty = scalar(&kind);
                }
            }
        }
    }
    // A map field's types are on its helper message, which is not part of the API.
    let entries = reader.schema.messages.clone();
    for message in &mut reader.schema.messages {
        for field in &mut message.fields {
            let helper = format!("{}_{}Entry", message.name, to_pascal(&field.name));
            let found = entries.iter().find(|m| m.name == helper);
            if let (Ty::Map(key, value), Some(entry)) = (&mut field.ty, found) {
                let of = |name: &str| {
                    entry
                        .fields
                        .iter()
                        .find(|f| f.name == name)
                        .map(|f| f.ty.clone())
                };
                if let (Some(entry_key), Some(entry_value)) = (of("key"), of("value")) {
                    **key = entry_key;
                    **value = entry_value;
                }
            }
        }
    }
    // Only what has an encoder is a message: the rest is helpers and clients.
    let encoded: HashSet<&String> = starts.iter().map(|(_, name)| name).collect();
    reader
        .schema
        .messages
        .retain(|m| encoded.contains(&m.name) && !m.name.ends_with("Entry"));
    // `export const OrdersServiceName = "shop.v1.Orders"` and its interface.
    let service_name = re(r#"export\s+const\s+(\w+)ServiceName\s*=\s*"([\w.]+)""#);
    let signature =
        re(r"(\w+)\(\s*request:\s*(Observable<)?(\w+)>?[^)]*\)\s*:\s*(Promise|Observable)<(\w+)>");
    for c in service_name.captures_iter(text) {
        let Some(interface) = re(&format!(r"export\s+interface\s+{}\s*\{{", &c[1])).find(text)
        else {
            continue;
        };
        let open = interface.end() - 1;
        let Some(end) = closing(text, open) else {
            continue;
        };
        let (package, service) = c[2].rsplit_once('.').unwrap_or(("", &c[2]));
        reader.schema.package = package.to_owned();
        for rpc in signature.captures_iter(&text[open..end]) {
            reader.schema.rpcs.push(super::schema::Rpc {
                service: service.to_owned(),
                name: rpc[1].to_owned(),
                input: rpc[3].to_owned(),
                output: rpc[5].to_owned(),
                client_stream: rpc.get(2).is_some(),
                server_stream: &rpc[4] == "Observable",
            });
        }
    }
}

/// What was read, and from which language. `None` when no type could be read.
pub(super) fn read(text: &str, notes: &mut Vec<String>) -> Option<(Schema, Lang)> {
    let lang = detect(text)?;
    let text = strip_comments(text, lang);
    let mut schema = Schema::default();
    let mut reader = Reader {
        lang,
        declared: HashSet::new(),
        schema: &mut schema,
        notes,
        aliases: Vec::new(),
    };
    match lang {
        Lang::Go => read_go(&text, &mut reader),
        Lang::Python => read_python(&text, &mut reader),
        _ => {
            // Aliases are known only after a first look.
            let mut first = Schema::default();
            let mut quiet = Vec::new();
            let mut scout = Reader {
                lang,
                declared: HashSet::new(),
                schema: &mut first,
                notes: &mut quiet,
                aliases: Vec::new(),
            };
            read_braced(&text, &mut scout);
            let aliases = std::mem::take(&mut scout.aliases);
            let text = apply_aliases(&text, aliases);
            read_braced(&text, &mut reader);
            if lang == Lang::TypeScript && text.contains("writer.uint32(") {
                apply_ts_proto(&text, &mut reader);
            }
        }
    }
    // Classes without a single field are code, not types.
    let has_fields = schema.messages.iter().any(|m| !m.fields.is_empty());
    has_fields.then_some((schema, lang))
}

/// `"/package.Service/Method"` strings in generated code.
pub(super) fn method_paths(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for c in re(r#"["']/([A-Za-z_][\w.]*\.[A-Za-z_]\w*/[A-Za-z_]\w*)["']"#).captures_iter(text) {
        if !paths.contains(&c[1].to_owned()) {
            paths.push(c[1].to_owned());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_go_but_strings_stay() {
        let text = "a: \"// not a comment\" // gone\n/* gone */ b";
        assert_eq!(
            strip_comments(text, Lang::TypeScript),
            "a: \"// not a comment\" \n b"
        );
        assert_eq!(strip_comments("a: int # gone\n", Lang::Python), "a: int \n");
    }

    #[test]
    fn splits_outside_brackets() {
        assert_eq!(
            split_top("a: Map<string, number>, b: (x, y) => z; c", &[',', ';']),
            ["a: Map<string, number>", "b: (x, y) => z", "c"]
        );
    }
}
