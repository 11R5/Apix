//! A service description from what a user happens to have, not only `.proto`
//! files: descriptor sets, code generated from a `.proto`, types written by
//! hand in a programming language, or an example message in JSON.
//!
//! On the wire a field is a number and a method is `package.Service/Method`.
//! Descriptor sets and most generated code hold them exactly. Hand-written
//! types and JSON do not, so what comes from them is a guess and says so
//! (`Certainty`); a server that offers reflection is believed over a guess.

mod compare;
mod embedded;
mod json;
mod printer;
mod schema;
mod source;

use std::path::{Path, PathBuf};

use prost::Message as _;
use prost_reflect::{DescriptorPool, DynamicMessage};
use prost_types::{FileDescriptorProto, FileDescriptorSet};
use protox::file::{
    ChainFileResolver, File, FileResolver, GoogleFileResolver, IncludeFileResolver,
};
use serde::Serialize as _;

pub use compare::compare;
use schema::Schema;

use crate::GrpcError;

/// What to convert.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConvertInput<'a> {
    pub source: &'a str,
    /// An example of the answer, or its type, when `source` is only the request.
    pub response: Option<&'a str>,
    /// `package.Service/Method`: what to call, where the source does not say.
    pub method: Option<&'a str>,
}

/// How far a definition can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certainty {
    /// It is the service's own description.
    Exact,
    /// Field numbers and types come from the source; the calls may not.
    NumbersFromSource,
    /// Field numbers were made up in order of appearance.
    Guessed,
}

#[derive(Debug, Clone)]
pub struct Conversion {
    /// `.proto` text, or a descriptor set as JSON where that would lose
    /// something. Goes in `[grpc] definition`.
    pub definition: String,
    /// What the source turned out to be: "TypeScript", "JSON example", ….
    pub format: String,
    pub certainty: Certainty,
    /// Choices made on the way, for the user to check.
    pub notes: Vec<String>,
    pub pool: DescriptorPool,
}

impl Conversion {
    /// `guessed` in `[grpc]`: a server's reflection goes before it.
    pub fn is_guess(&self) -> bool {
        self.certainty != Certainty::Exact
    }
}

/// `.proto` text from memory, with its imports found elsewhere.
struct InMemory {
    name: String,
    source: String,
}

impl FileResolver for InMemory {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        if name == self.name {
            File::from_source(name, &self.source)
        } else {
            Err(protox::Error::file_not_found(name))
        }
    }
}

const DEFINITION_FILE: &str = "definition.proto";

fn compile_text(text: &str, import_paths: &[String]) -> Result<DescriptorPool, GrpcError> {
    let mut resolver = ChainFileResolver::new();
    resolver.add(InMemory {
        name: DEFINITION_FILE.into(),
        source: text.to_owned(),
    });
    resolver.add(GoogleFileResolver::new());
    for path in import_paths {
        resolver.add(IncludeFileResolver::new(PathBuf::from(path)));
    }
    let mut compiler = protox::Compiler::with_file_resolver(resolver);
    compiler.include_imports(true);
    compiler
        .open_file(DEFINITION_FILE)
        .map_err(|e| GrpcError::Protos(format!("the definition is not valid protobuf: {e}")))?;
    Ok(compiler.descriptor_pool())
}

/// A definition as `[grpc] definition` holds it: `.proto` text, or a
/// descriptor set as JSON.
pub fn compile_definition(
    text: &str,
    import_paths: &[String],
) -> Result<DescriptorPool, GrpcError> {
    if text.trim_start().starts_with('{') {
        pool_of(set_from_json(text)?)
    } else {
        compile_text(text, import_paths)
    }
}

fn descriptor_set_type() -> prost_reflect::MessageDescriptor {
    DescriptorPool::global()
        .get_message_by_name("google.protobuf.FileDescriptorSet")
        .expect("the global pool has descriptor.proto")
}

/// A descriptor set from protobuf's JSON (`buf build -o set.json`).
pub fn set_from_json(text: &str) -> Result<FileDescriptorSet, GrpcError> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let message = DynamicMessage::deserialize(descriptor_set_type(), &mut deserializer)
        .map_err(|e| GrpcError::Protos(format!("not a descriptor set in JSON: {e}")))?;
    FileDescriptorSet::decode(message.encode_to_vec().as_slice())
        .map_err(|e| GrpcError::Protos(format!("not a descriptor set: {e}")))
}

fn set_to_json(set: &FileDescriptorSet) -> Result<String, GrpcError> {
    let message = DynamicMessage::decode(descriptor_set_type(), set.encode_to_vec().as_slice())
        .map_err(|e| GrpcError::Protos(e.to_string()))?;
    let mut out = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut out, serde_json::ser::PrettyFormatter::new());
    message
        .serialize(&mut serializer)
        .map_err(|e| GrpcError::Protos(e.to_string()))?;
    String::from_utf8(out).map_err(|e| GrpcError::Protos(e.to_string()))
}

/// What a set imports and does not hold, apart from Google's own types.
fn missing_imports(files: &[FileDescriptorProto]) -> Vec<String> {
    let mut missing: Vec<String> = files
        .iter()
        .flat_map(|f| &f.dependency)
        .filter(|d| !d.starts_with("google/protobuf/") && !files.iter().any(|f| f.name() == *d))
        .cloned()
        .collect();
    missing.sort();
    missing.dedup();
    missing
}

/// The file of Google's that declares a type.
fn google_file_of(type_name: &str) -> Option<String> {
    let name = type_name.strip_prefix(".google.protobuf.")?;
    let file = match name.split('.').next()? {
        "Any" => "any",
        "Duration" => "duration",
        "Empty" => "empty",
        "FieldMask" => "field_mask",
        "Struct" | "Value" | "ListValue" | "NullValue" => "struct",
        "Timestamp" => "timestamp",
        "DoubleValue" | "FloatValue" | "Int64Value" | "UInt64Value" | "Int32Value"
        | "UInt32Value" | "BoolValue" | "StringValue" | "BytesValue" => "wrappers",
        _ => return None,
    };
    Some(format!("google/protobuf/{file}.proto"))
}

fn type_names_of(message: &prost_types::DescriptorProto, out: &mut Vec<String>) {
    out.extend(message.field.iter().filter_map(|f| f.type_name.clone()));
    for nested in &message.nested_type {
        type_names_of(nested, out);
    }
}

/// protobuf-es leaves imports out of the description it embeds: the ones of
/// Google's own types can be told from the types the file uses.
fn restore_google_imports(file: &mut FileDescriptorProto) {
    let mut names = Vec::new();
    for message in &file.message_type {
        type_names_of(message, &mut names);
    }
    for method in file.service.iter().flat_map(|s| &s.method) {
        names.extend(method.input_type.clone());
        names.extend(method.output_type.clone());
    }
    for name in names {
        if let Some(wanted) = google_file_of(&name)
            && !file.dependency.contains(&wanted)
        {
            file.dependency.push(wanted);
        }
    }
}

/// A pool from a set; Google's own types are added if it left them out.
fn pool_of(set: FileDescriptorSet) -> Result<DescriptorPool, GrpcError> {
    let mut files = set.file;
    files.iter_mut().for_each(restore_google_imports);
    let missing = missing_imports(&files);
    if !missing.is_empty() {
        return Err(GrpcError::Protos(format!(
            "the description imports {} and does not hold it; give that file too",
            missing.join(", ")
        )));
    }
    let google = GoogleFileResolver::new();
    loop {
        let wanted: Vec<String> = files
            .iter()
            .flat_map(|f| &f.dependency)
            .filter(|d| !files.iter().any(|f| f.name() == *d))
            .cloned()
            .collect();
        if wanted.is_empty() {
            break;
        }
        for name in wanted {
            if files.iter().any(|f| f.name() == name) {
                continue;
            }
            let file = google
                .open_file(&name)
                .map_err(|e| GrpcError::Protos(e.to_string()))?;
            files.push(file.file_descriptor_proto().clone());
        }
    }
    let mut pool = DescriptorPool::new();
    pool.add_file_descriptor_protos(crate::descriptors::in_dependency_order(files))
        .map_err(|e| GrpcError::Protos(format!("the descriptions do not fit together: {e}")))?;
    Ok(pool)
}

/// A binary descriptor set (`protoc --descriptor_set_out`, `buf build`).
pub fn set_from_bytes(bytes: &[u8]) -> Result<FileDescriptorSet, GrpcError> {
    let set = FileDescriptorSet::decode(bytes)
        .map_err(|e| GrpcError::Protos(format!("not a descriptor set: {e}")))?;
    if set.file.is_empty() || set.file.iter().any(|f| !f.name().contains('.')) {
        return Err(GrpcError::Protos("not a descriptor set".into()));
    }
    Ok(set)
}

/// A description file, by its extension: `.proto`s are for the compiler, so
/// this is everything else.
pub fn pool_from_set_file(path: &Path) -> Result<DescriptorPool, GrpcError> {
    let bytes = std::fs::read(path).map_err(|e| GrpcError::File {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let named = |e: GrpcError| GrpcError::Protos(format!("{}: {e}", path.display()));
    let is_json = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    let set = if is_json {
        set_from_json(&String::from_utf8_lossy(&bytes)).map_err(named)?
    } else {
        set_from_bytes(&bytes).map_err(named)?
    };
    pool_of(set).map_err(named)
}

/// The files as a definition: `.proto` text when one file says it all and
/// the text proves the same, the set as JSON otherwise.
fn from_files(
    files: Vec<FileDescriptorProto>,
    format: String,
    mut notes: Vec<String>,
) -> Result<Conversion, GrpcError> {
    let mut files = files;
    files.iter_mut().for_each(restore_google_imports);
    let set = FileDescriptorSet { file: files };
    let pool = pool_of(set.clone())?;
    let own: Vec<&FileDescriptorProto> = set
        .file
        .iter()
        .filter(|f| !f.name().starts_with("google/protobuf/"))
        .collect();
    let printed = match own.as_slice() {
        [only]
            if only
                .dependency
                .iter()
                .all(|d| d.starts_with("google/protobuf/")) =>
        {
            printer::print(only).filter(|text| same_as_printed(only, text))
        }
        _ => None,
    };
    let definition = match printed {
        Some(text) => text,
        None => {
            notes.push("Kept as a descriptor set in JSON: it spans several files, or holds what .proto text here would lose".into());
            set_to_json(&set)?
        }
    };
    Ok(Conversion {
        definition,
        format,
        certainty: Certainty::Exact,
        notes,
        pool,
    })
}

/// Whether printed text compiles back to the same calls and messages.
fn same_as_printed(file: &FileDescriptorProto, text: &str) -> bool {
    let Ok(pool) = compile_text(text, &[]) else {
        return false;
    };
    let Some(back) = pool.get_file_by_name(DEFINITION_FILE) else {
        return false;
    };
    printer::normalized(file) == printer::normalized(back.file_descriptor_proto())
}

fn is_descriptor_set(json: &json::Json) -> bool {
    let json::Json::Obj(items) = json else {
        return false;
    };
    let Some((_, json::Json::Arr(files))) = items.iter().find(|(k, _)| k == "file") else {
        return false;
    };
    !files.is_empty()
        && files.iter().all(|f| match f {
            json::Json::Obj(fields) => fields.iter().any(|(k, _)| k == "name"),
            _ => false,
        })
}

/// `{{variable}}`s in a message are not JSON; they stand for some value.
fn without_variables(text: &str) -> String {
    regex::Regex::new(r"\{\{[^{}]*\}\}")
        .expect("a valid pattern")
        .replace_all(text, "0")
        .into_owned()
}

fn looks_like_proto(text: &str) -> bool {
    let has = |p: &str| {
        regex::Regex::new(p)
            .expect("a valid pattern")
            .is_match(text)
    };
    has(r#"(?m)^\s*(syntax|edition)\s*=\s*""#)
        || has(r"(?s)\bmessage\s+\w+\s*\{[^}]*=\s*\d+\s*(\[[^\]]*\])?\s*;")
        || has(r"(?s)\bservice\s+\w+\s*\{\s*(option[^;]*;\s*)*rpc\b")
        || has(r"(?s)\bmessage\s+\w+\s*\{\s*\}")
}

fn from_proto_text(text: &str, method: Option<&str>) -> Result<Conversion, GrpcError> {
    let mut notes = Vec::new();
    let has_syntax = regex::Regex::new(r#"(?m)^\s*(syntax|edition)\s*=\s*""#)
        .expect("a valid pattern")
        .is_match(text);
    let definition = if has_syntax {
        text.to_owned()
    } else {
        notes.push("The text has no `syntax` line, so proto3 was assumed".to_owned());
        format!("syntax = \"proto3\";\n\n{text}")
    };
    let pool = compile_text(&definition, &[])?;
    if let Some(method) = method.filter(|m| !m.trim().is_empty())
        && crate::find_method(&pool, method).is_none()
    {
        notes.push(format!("The text has no method {method}"));
    }
    Ok(Conversion {
        definition,
        format: "protobuf".into(),
        certainty: Certainty::Exact,
        notes,
        pool,
    })
}

/// Reads the source as types or as JSON, whichever it is, into `schema`.
/// Says what it was, and whether a JSON example was a list (a stream).
fn read_into(
    text: &str,
    root: &str,
    schema: &mut Schema,
    notes: &mut Vec<String>,
) -> Option<(String, bool)> {
    let trimmed = text.trim();
    if trimmed.starts_with(['{', '['])
        && let Ok(value) = json::parse(&without_variables(trimmed))
    {
        let list = json::infer(&value, root, schema, notes);
        return Some(("a JSON example".to_owned(), list));
    }
    let (read, lang) = source::read(trimmed, notes)?;
    schema.absorb(read);
    Some((lang.name().to_owned(), false))
}

/// Turns the source into a definition, working out what it is.
pub fn convert(input: &ConvertInput) -> Result<Conversion, GrpcError> {
    let text = input.source.trim_start_matches('\u{feff}').trim();
    if text.is_empty() {
        return Err(GrpcError::Protos(
            "nothing to convert: paste protobuf, generated code, types or a JSON example".into(),
        ));
    }
    // A descriptor set in JSON is its own thing; other JSON is an example.
    if text.starts_with('{')
        && let Ok(value) = json::parse(text)
        && is_descriptor_set(&value)
    {
        return from_files(
            set_from_json(text)?.file,
            "a descriptor set in JSON".into(),
            Vec::new(),
        );
    }
    if !text.starts_with(['{', '[']) && looks_like_proto(text) {
        return from_proto_text(text, input.method);
    }
    let files = embedded::extract(text);
    if !files.is_empty() {
        return from_files(
            files,
            "generated code (its embedded description)".into(),
            Vec::new(),
        );
    }
    if is_generated_without_description(text) {
        return Err(GrpcError::Protos(
            "this generated file does not hold the service's description. Use the one that does: _pb2.py in Python, the outer class's .java, .cs, .pb.cc in C++, .pb.go, _pb.rb, GPBMetadata's .php, or a descriptor set".into(),
        ));
    }
    from_types(text, input)
}

/// `protoc`'s own output that only uses the description (`.pb.h`, Kotlin's
/// builders, Objective-C, Python's `.pyi`): read as types it would be nonsense.
fn is_generated_without_description(text: &str) -> bool {
    text.contains("Generated by the protocol buffer compiler")
        || (text.contains("_FIELD_NUMBER: _ClassVar[int]"))
}

fn from_types(text: &str, input: &ConvertInput) -> Result<Conversion, GrpcError> {
    let method = input
        .method
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned);
    let method_name = method
        .as_deref()
        .and_then(schema::method_path)
        .map(|(_, _, name)| name);
    let request_root = method_name
        .as_ref()
        .map_or_else(|| "Request".to_owned(), |m| format!("{m}Request"));
    let response_root = method_name
        .as_ref()
        .map_or_else(|| "Response".to_owned(), |m| format!("{m}Response"));

    let mut notes = Vec::new();
    let mut schema = Schema::default();
    let (format, client_stream) = read_into(text, &request_root, &mut schema, &mut notes)
        .ok_or_else(|| {
            let not_json = (text.starts_with(['{', '[']))
                .then(|| json::parse(&without_variables(text)).err())
                .flatten()
                .map(|e| format!(" It is not valid JSON either: {e}."))
                .unwrap_or_default();
            let only_calls = if source::method_paths(text).is_empty() {
                ""
            } else {
                " It names calls but no messages: the code generated for the messages is what describes them."
            };
            GrpcError::Protos(format!(
                "no types could be read from this. Paste protobuf, generated code, types in a programming language, or a JSON example.{not_json}{only_calls}"
            ))
        })?;
    let mut server_stream = false;
    if let Some(response) = input.response.map(str::trim).filter(|r| !r.is_empty()) {
        let (_, list) =
            read_into(response, &response_root, &mut schema, &mut notes).ok_or_else(|| {
                GrpcError::Protos("no types could be read from the answer's example".into())
            })?;
        server_stream = list;
    }
    let mut paths: Vec<String> = method.into_iter().collect();
    for found in source::method_paths(text) {
        if !paths.iter().any(|p| p.trim_matches('/') == found) {
            paths.push(found);
        }
    }
    if paths.is_empty() && schema.rpcs.is_empty() {
        notes.push("Nothing says what to call: add the method (package.Service/Method) or an rpc to the result".into());
    }
    schema.add_rpcs(&paths, client_stream, server_stream, &mut notes);
    let numbered = schema.all_numbered() && schema.messages.iter().any(|m| !m.fields.is_empty());
    let certainty = if numbered {
        Certainty::NumbersFromSource
    } else {
        Certainty::Guessed
    };
    let header = match certainty {
        Certainty::Guessed => format!(
            "Made by Apix from {format}. The field numbers are guesses, in order of\nappearance: what goes on the wire depends on them, so check them against the server."
        ),
        _ => format!("Made by Apix from {format}, with its field numbers."),
    };
    let definition = schema.render(&header, &mut notes);
    let pool = compile_text(&definition, &[]).map_err(|e| {
        GrpcError::Protos(format!(
            "the result is not valid protobuf ({e}):\n{definition}"
        ))
    })?;
    Ok(Conversion {
        definition,
        format,
        certainty,
        notes,
        pool,
    })
}

/// `convert` for a file's bytes: a binary descriptor set, or text.
pub fn convert_bytes(
    bytes: &[u8],
    response: Option<&str>,
    method: Option<&str>,
) -> Result<Conversion, GrpcError> {
    if let Ok(text) = std::str::from_utf8(bytes)
        && !text.contains('\0')
    {
        // Binary sets can happen to be valid UTF-8; try them after the text.
        let converted = convert(&ConvertInput {
            source: text,
            response,
            method,
        });
        if converted.is_ok() {
            return converted;
        }
        if let Ok(set) = set_from_bytes(bytes) {
            return from_files(set.file, "a descriptor set".into(), Vec::new());
        }
        return converted;
    }
    let files = embedded::extract_bytes(bytes);
    if !files.is_empty() {
        return from_files(
            files,
            "generated code (its embedded description)".into(),
            Vec::new(),
        );
    }
    let set = set_from_bytes(bytes)?;
    from_files(set.file, "a descriptor set".into(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_paths_split() {
        assert_eq!(
            schema::method_path("/a.b.Svc/Do"),
            Some(("a.b".into(), "Svc".into(), "Do".into()))
        );
        assert_eq!(
            schema::method_path("Svc/Do"),
            Some(("".into(), "Svc".into(), "Do".into()))
        );
        assert_eq!(schema::method_path("nothing"), None);
    }

    #[test]
    fn names_become_protobuf_names() {
        assert_eq!(schema::to_snake("firstName"), "first_name");
        assert_eq!(schema::to_snake("HTTPServer"), "http_server");
        assert_eq!(schema::to_snake("userID"), "user_id");
        assert_eq!(schema::to_snake("first-name"), "first_name");
        assert_eq!(schema::to_snake("1st"), "f_1st");
        assert_eq!(
            schema::names_from_key("firstName"),
            ("first_name".into(), None)
        );
        assert_eq!(
            schema::names_from_key("userID"),
            ("user_id".into(), Some("userID".into()))
        );
    }
}
