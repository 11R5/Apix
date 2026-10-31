//! What a guess is made of: messages, enums and calls read from code or JSON,
//! on their way to `.proto` text.

use std::collections::{BTreeSet, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Ty {
    /// `string`, `int32`, …, or a well-known type by its full name.
    Scalar(String),
    /// A message or an enum of the source.
    Named(String),
    Map(Box<Ty>, Box<Ty>),
}

#[derive(Debug, Clone)]
pub(super) struct Field {
    pub name: String,
    /// The name in JSON, when protobuf would not make it from `name`.
    pub json_name: Option<String>,
    pub ty: Ty,
    pub repeated: bool,
    pub optional: bool,
    /// As the source says; a guess otherwise.
    pub number: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Msg {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct EnumDef {
    pub name: String,
    pub values: Vec<(String, Option<i32>)>,
}

#[derive(Debug, Clone)]
pub(super) struct Rpc {
    pub service: String,
    pub name: String,
    pub input: String,
    pub output: String,
    pub client_stream: bool,
    pub server_stream: bool,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Schema {
    pub package: String,
    pub messages: Vec<Msg>,
    pub enums: Vec<EnumDef>,
    pub rpcs: Vec<Rpc>,
}

/// `package.Service/Method` (or with a `.` before the method), split.
pub(super) fn method_path(path: &str) -> Option<(String, String, String)> {
    let path = path.trim().trim_matches('/');
    let (service, method) = path.rsplit_once('/').or_else(|| path.rsplit_once('.'))?;
    let (package, service) = service.rsplit_once('.').unwrap_or(("", service));
    let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !word(service) || !word(method) || !package.split('.').all(|p| p.is_empty() || word(p)) {
        return None;
    }
    Some((package.to_owned(), service.to_owned(), method.to_owned()))
}

/// `firstName`, `First-Name`, `HTTPServer` → `first_name`, `first_name`, `http_server`.
pub(super) fn to_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (ix, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let after_lower =
                ix > 0 && (chars[ix - 1].is_ascii_lowercase() || chars[ix - 1].is_ascii_digit());
            let before_lower = chars.get(ix + 1).is_some_and(char::is_ascii_lowercase);
            let after_upper = ix > 0 && chars[ix - 1].is_ascii_uppercase();
            if (after_lower || (after_upper && before_lower)) && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_owned();
    if out.is_empty() {
        "field".into()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("f_{out}")
    } else {
        out
    }
}

/// `first_name`, `first-name` → `FirstName`.
pub(super) fn to_pascal(name: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(if upper { c.to_ascii_uppercase() } else { c });
            upper = false;
        } else {
            upper = true;
        }
    }
    if out.is_empty() {
        "Message".into()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("M{out}")
    } else {
        out
    }
}

/// The JSON name protobuf gives a field by itself.
pub(super) fn default_json_name(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '_' {
            upper = true;
        } else {
            out.push(if upper { c.to_ascii_uppercase() } else { c });
            upper = false;
        }
    }
    out
}

/// A field's protobuf name for the name it has in JSON. A JSON name that
/// protobuf would not make by itself is kept, so messages can go on using it.
pub(super) fn names_from_key(key: &str) -> (String, Option<String>) {
    let name = to_snake(key);
    let keeps = key == name || key == default_json_name(&name);
    (name, (!keeps).then(|| key.to_owned()))
}

fn upper_snake(name: &str) -> String {
    to_snake(name).to_ascii_uppercase()
}

fn import_of(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "google.protobuf.Timestamp" => "google/protobuf/timestamp.proto",
        "google.protobuf.Duration" => "google/protobuf/duration.proto",
        "google.protobuf.Value" | "google.protobuf.Struct" | "google.protobuf.ListValue" => {
            "google/protobuf/struct.proto"
        }
        "google.protobuf.Empty" => "google/protobuf/empty.proto",
        _ => return None,
    })
}

fn ty_text(ty: &Ty) -> String {
    match ty {
        Ty::Scalar(name) | Ty::Named(name) => name.clone(),
        Ty::Map(key, value) => format!("map<{}, {}>", ty_text(key), ty_text(value)),
    }
}

fn collect_imports(ty: &Ty, imports: &mut BTreeSet<&'static str>) {
    match ty {
        Ty::Scalar(name) => imports.extend(import_of(name)),
        Ty::Named(_) => {}
        Ty::Map(key, value) => {
            collect_imports(key, imports);
            collect_imports(value, imports);
        }
    }
}

fn collect_named(ty: &Ty, names: &mut Vec<String>) {
    match ty {
        Ty::Scalar(_) => {}
        Ty::Named(name) => names.push(name.clone()),
        Ty::Map(key, value) => {
            collect_named(key, names);
            collect_named(value, names);
        }
    }
}

impl Schema {
    pub(super) fn message_named(&self, name: &str) -> bool {
        self.messages.iter().any(|m| m.name == name)
    }

    /// A message name nothing has yet: `name`, or `name2`, `name3`, ….
    pub(super) fn free_name(&self, name: &str) -> String {
        let taken = |n: &str| self.message_named(n) || self.enums.iter().any(|e| e.name == n);
        if !taken(name) {
            return name.to_owned();
        }
        (2..)
            .map(|i| format!("{name}{i}"))
            .find(|n| !taken(n))
            .unwrap_or_default()
    }

    /// Adds what is new here from `other`.
    pub(super) fn absorb(&mut self, other: Schema) {
        if self.package.is_empty() {
            self.package = other.package;
        }
        self.rpcs.extend(other.rpcs);
        for message in other.messages {
            if !self.message_named(&message.name) {
                self.messages.push(message);
            }
        }
        for e in other.enums {
            if !self.enums.iter().any(|mine| mine.name == e.name) {
                self.enums.push(e);
            }
        }
    }

    fn referenced(&self) -> HashSet<String> {
        let mut names = Vec::new();
        for message in &self.messages {
            for field in &message.fields {
                collect_named(&field.ty, &mut names);
            }
        }
        names.into_iter().collect()
    }

    /// Whether every field's number comes from the source.
    pub(super) fn all_numbered(&self) -> bool {
        self.messages
            .iter()
            .flat_map(|m| &m.fields)
            .all(|f| f.number.is_some())
    }

    /// The message a call takes: named after it, or that looks like a request.
    fn pick_input(&self, method: &str) -> Option<String> {
        let tops = self.top_messages();
        let exact = ["Request", "Req", "Input", "Args", "Params"]
            .iter()
            .map(|suffix| format!("{method}{suffix}"))
            .find(|name| tops.contains(name));
        exact
            .or_else(|| {
                tops.iter()
                    .find(|name| {
                        name.ends_with("Request")
                            || name.ends_with("Req")
                            || name.ends_with("Input")
                    })
                    .cloned()
            })
            .or_else(|| tops.iter().find(|name| name.starts_with(method)).cloned())
            .or_else(|| tops.first().cloned())
    }

    /// The message a call answers with, if the source seems to have one.
    fn pick_output(&self, method: &str, input: &str) -> Option<String> {
        let tops: Vec<String> = self
            .top_messages()
            .into_iter()
            .filter(|n| n != input)
            .collect();
        let endings = ["Response", "Reply", "Res", "Result", "Output"];
        let stem = ["Request", "Req", "Input", "Args", "Params"]
            .iter()
            .find_map(|suffix| input.strip_suffix(suffix))
            .unwrap_or(input);
        [method, stem]
            .iter()
            .flat_map(|base| endings.iter().map(move |e| format!("{base}{e}")))
            .find(|name| tops.contains(name))
            .or_else(|| {
                tops.iter()
                    .find(|name| endings.iter().any(|e| name.ends_with(e)))
                    .cloned()
            })
            .or_else(|| tops.first().cloned())
    }

    /// Messages no other message uses, in the order of the source.
    fn top_messages(&self) -> Vec<String> {
        let used = self.referenced();
        let tops: Vec<String> = self
            .messages
            .iter()
            .filter(|m| !used.contains(&m.name))
            .map(|m| m.name.clone())
            .collect();
        if tops.is_empty() {
            self.messages.iter().map(|m| m.name.clone()).collect()
        } else {
            tops
        }
    }

    /// A call for each method path; what it takes and answers with is
    /// worked out from the names, and said so.
    pub(super) fn add_rpcs(
        &mut self,
        paths: &[String],
        client_stream: bool,
        server_stream: bool,
        notes: &mut Vec<String>,
    ) {
        for path in paths {
            let Some((package, service, method)) = method_path(path) else {
                notes.push(format!(
                    "`{path}` is not a method path (package.Service/Method) and was left out"
                ));
                continue;
            };
            if self.rpcs.is_empty() {
                self.package = package.clone();
            } else if self.package != package {
                notes.push(format!(
                    "`{path}` is in another package than the first method and was left out"
                ));
                continue;
            }
            if self
                .rpcs
                .iter()
                .any(|r| r.service == service && r.name == method)
            {
                continue;
            }
            self.free_service_name(&service, notes);
            let input = self.pick_input(&method).unwrap_or_else(|| {
                let name = self.free_name(&format!("{method}Request"));
                self.messages.push(Msg {
                    name: name.clone(),
                    fields: Vec::new(),
                });
                name
            });
            let output = match self.pick_output(&method, &input) {
                Some(name) => {
                    notes.push(format!(
                        "{method} was taken to take {input} and answer with {name}: check both"
                    ));
                    name
                }
                None => {
                    let name = self.free_name(&format!("{method}Response"));
                    notes.push(format!(
                        "Nothing says what {method} answers with, so {name} is empty: what the server sends is shown as unknown fields until you add them"
                    ));
                    self.messages.push(Msg {
                        name: name.clone(),
                        fields: Vec::new(),
                    });
                    name
                }
            };
            self.rpcs.push(Rpc {
                service,
                name: method,
                input,
                output,
                client_stream,
                server_stream,
            });
        }
    }

    /// A service's name cannot also be a message's: generated code often has
    /// an empty class or interface for the service itself, which goes away.
    fn free_service_name(&mut self, service: &str, notes: &mut Vec<String>) {
        let Some(ix) = self.messages.iter().position(|m| m.name == service) else {
            return;
        };
        if self.messages[ix].fields.is_empty() {
            self.messages.remove(ix);
            return;
        }
        let name = self.free_name(&format!("{service}Message"));
        notes.push(format!(
            "The message {service} has the service's name, so it became {name}"
        ));
        self.messages[ix].name = name.clone();
        fn rename(ty: &mut Ty, from: &str, to: &str) {
            match ty {
                Ty::Named(named) if named == from => *named = to.to_owned(),
                Ty::Map(key, value) => {
                    rename(key, from, to);
                    rename(value, from, to);
                }
                _ => {}
            }
        }
        for field in self.messages.iter_mut().flat_map(|m| &mut m.fields) {
            rename(&mut field.ty, service, &name);
        }
    }

    /// Types the source uses without describing them become empty messages.
    fn add_missing_types(&mut self, notes: &mut Vec<String>) {
        let mut missing: Vec<String> = Vec::new();
        for name in self.referenced() {
            if !self.message_named(&name) && !self.enums.iter().any(|e| e.name == name) {
                missing.push(name);
            }
        }
        missing.sort();
        for name in missing {
            notes.push(format!(
                "The source uses {name} without describing it, so it is an empty message"
            ));
            self.messages.push(Msg {
                name,
                fields: Vec::new(),
            });
        }
    }

    fn number_fields(&mut self, notes: &mut Vec<String>) {
        for message in &mut self.messages {
            let mut used: HashSet<u32> = HashSet::new();
            let mut names: HashSet<String> = HashSet::new();
            message.fields.retain(|f| names.insert(f.name.clone()));
            for field in &mut message.fields {
                if let Some(number) = field.number
                    && !used.insert(number)
                {
                    notes.push(format!(
                        "{}.{} has the number {number} of another field, so it got a new one",
                        message.name, field.name
                    ));
                    field.number = None;
                }
            }
            let mut next = 1;
            for field in &mut message.fields {
                if field.number.is_some() {
                    continue;
                }
                // 19000-19999 are protobuf's own.
                while used.contains(&next) || (19000..20000).contains(&next) {
                    next += 1;
                }
                field.number = Some(next);
                used.insert(next);
            }
        }
    }

    /// Enum values share a package's scope, and proto3 wants a first value of 0.
    fn fix_enums(&mut self, notes: &mut Vec<String>) {
        let mut seen: HashSet<String> = HashSet::new();
        let duplicates: Vec<bool> = self
            .enums
            .iter()
            .map(|e| e.values.iter().any(|(v, _)| !seen.insert(v.clone())))
            .collect();
        let any_duplicate = duplicates.iter().any(|d| *d);
        for e in &mut self.enums {
            let prefix = upper_snake(&e.name);
            if any_duplicate {
                for (value, _) in &mut e.values {
                    *value = format!("{prefix}_{value}");
                }
            }
            let mut next = 0;
            let mut numbers: Vec<i32> = Vec::new();
            for (_, number) in &mut e.values {
                let n = number.unwrap_or(next);
                *number = Some(n);
                numbers.push(n);
                next = n + 1;
            }
            if numbers.first() != Some(&0) {
                let position = numbers.iter().position(|n| *n == 0);
                match position {
                    Some(ix) => {
                        let zero = e.values.remove(ix);
                        e.values.insert(0, zero);
                    }
                    None => {
                        notes.push(format!(
                            "{} had no value 0, which proto3 needs, so {prefix}_UNSPECIFIED was added",
                            e.name
                        ));
                        e.values
                            .insert(0, (format!("{prefix}_UNSPECIFIED"), Some(0)));
                    }
                }
            }
        }
    }

    /// The whole thing as a `.proto` file.
    pub(super) fn render(&mut self, header: &str, notes: &mut Vec<String>) -> String {
        self.add_missing_types(notes);
        self.number_fields(notes);
        self.fix_enums(notes);

        let mut imports = BTreeSet::new();
        for field in self.messages.iter().flat_map(|m| &m.fields) {
            collect_imports(&field.ty, &mut imports);
        }
        let mut out = String::new();
        for line in header.lines() {
            out += &format!("// {line}\n");
        }
        out += "syntax = \"proto3\";\n";
        if !self.package.is_empty() {
            out += &format!("\npackage {};\n", self.package);
        }
        if !imports.is_empty() {
            out += "\n";
            for import in imports {
                out += &format!("import \"{import}\";\n");
            }
        }
        let mut services: Vec<&str> = Vec::new();
        for rpc in &self.rpcs {
            if !services.contains(&rpc.service.as_str()) {
                services.push(&rpc.service);
            }
        }
        for service in services {
            out += &format!("\nservice {service} {{\n");
            for rpc in self.rpcs.iter().filter(|r| r.service == service) {
                let stream = |yes: bool| if yes { "stream " } else { "" };
                out += &format!(
                    "  rpc {}({}{}) returns ({}{});\n",
                    rpc.name,
                    stream(rpc.client_stream),
                    rpc.input,
                    stream(rpc.server_stream),
                    rpc.output
                );
            }
            out += "}\n";
        }
        for message in &self.messages {
            out += &format!("\nmessage {} {{\n", message.name);
            for field in &message.fields {
                let is_message = matches!(&field.ty, Ty::Named(name) if self.message_named(name))
                    || matches!(&field.ty, Ty::Scalar(name) if name.contains('.'));
                let label = if field.repeated {
                    "repeated "
                } else if field.optional && !is_message && !matches!(field.ty, Ty::Map(..)) {
                    "optional "
                } else {
                    ""
                };
                let json = field
                    .json_name
                    .as_ref()
                    .map(|j| {
                        format!(
                            " [json_name = \"{}\"]",
                            j.replace('\\', "\\\\").replace('"', "\\\"")
                        )
                    })
                    .unwrap_or_default();
                out += &format!(
                    "  {label}{} {} = {}{json};\n",
                    ty_text(&field.ty),
                    field.name,
                    field.number.unwrap_or_default()
                );
            }
            out += "}\n";
        }
        for e in &self.enums {
            out += &format!("\nenum {} {{\n", e.name);
            for (value, number) in &e.values {
                out += &format!("  {value} = {};\n", number.unwrap_or_default());
            }
            out += "}\n";
        }
        out
    }
}
