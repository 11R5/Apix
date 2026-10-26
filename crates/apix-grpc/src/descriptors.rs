//! Where a service's description comes from (`.proto` files, or the server
//! through reflection), the methods it has, an example of a message, and the
//! whole thing as `.proto`-like text.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use prost::Message as _;
use prost_reflect::{
    Cardinality, DescriptorPool, EnumDescriptor, FieldDescriptor, Kind, MessageDescriptor,
    MethodDescriptor,
};
use prost_types::FileDescriptorProto;
use serde_json::{Map, Value};
use tonic::Status;
use tonic::metadata::MetadataMap;
use tonic::transport::Channel;

use crate::GrpcError;

/// Reads `.proto` files (absolute paths). Imports are looked up in
/// `import_paths`, then next to each file; Google's well-known types are built in.
pub fn from_protos(
    protos: &[String],
    import_paths: &[String],
) -> Result<DescriptorPool, GrpcError> {
    let mut includes: Vec<PathBuf> = import_paths.iter().map(PathBuf::from).collect();
    for proto in protos {
        if let Some(dir) = Path::new(proto).parent()
            && !includes.iter().any(|i| i == dir)
        {
            includes.push(dir.to_owned());
        }
    }
    let mut compiler =
        protox::Compiler::new(&includes).map_err(|e| GrpcError::Protos(e.to_string()))?;
    compiler.include_imports(true);
    for proto in protos {
        if !Path::new(proto).is_file() {
            return Err(GrpcError::Protos(format!("{proto} does not exist")));
        }
        compiler
            .open_file(proto)
            .map_err(|e| GrpcError::Protos(format!("{proto}: {e}")))?;
    }
    Ok(compiler.descriptor_pool())
}

/// One question to the reflection service, in either of its versions.
macro_rules! reflection {
    ($name:ident, $pb:path) => {
        async fn $name(
            channel: &Channel,
            metadata: &MetadataMap,
        ) -> Result<Vec<FileDescriptorProto>, Status> {
            use pb::server_reflection_client::ServerReflectionClient;
            use pb::server_reflection_request::MessageRequest;
            use pb::server_reflection_response::MessageResponse;
            use $pb as pb;

            let mut client = ServerReflectionClient::new(channel.clone());
            let mut ask = async |question: MessageRequest| -> Result<MessageResponse, Status> {
                let mut request =
                    tonic::Request::new(tokio_stream::iter([pb::ServerReflectionRequest {
                        host: String::new(),
                        message_request: Some(question),
                    }]));
                *request.metadata_mut() = metadata.clone();
                let mut answers = client.server_reflection_info(request).await?.into_inner();
                match answers.message().await?.and_then(|a| a.message_response) {
                    Some(MessageResponse::ErrorResponse(e)) => {
                        Err(Status::new(e.error_code.into(), e.error_message))
                    }
                    Some(answer) => Ok(answer),
                    None => Err(Status::unknown("the server did not answer")),
                }
            };

            let services = match ask(MessageRequest::ListServices(String::new())).await? {
                MessageResponse::ListServicesResponse(list) => list.service,
                _ => return Err(Status::unknown("unexpected answer to the list of services")),
            };
            let mut files: BTreeMap<String, FileDescriptorProto> = BTreeMap::new();
            let add = |answer: MessageResponse,
                       files: &mut BTreeMap<String, FileDescriptorProto>|
             -> Result<(), Status> {
                let MessageResponse::FileDescriptorResponse(found) = answer else {
                    return Err(Status::unknown("unexpected answer to a file request"));
                };
                for bytes in found.file_descriptor_proto {
                    let file = FileDescriptorProto::decode(bytes.as_slice())
                        .map_err(|e| Status::internal(format!("a broken file descriptor: {e}")))?;
                    files.insert(file.name().to_owned(), file);
                }
                Ok(())
            };
            for service in services
                .iter()
                .map(|s| &s.name)
                .filter(|s| !s.starts_with("grpc.reflection."))
            {
                let answer = ask(MessageRequest::FileContainingSymbol(service.clone())).await?;
                add(answer, &mut files)?;
            }
            // Servers may leave out what a file imports; ask for it by name.
            loop {
                let missing: Vec<String> = files
                    .values()
                    .flat_map(|f| f.dependency.iter())
                    .filter(|d| !files.contains_key(*d))
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if missing.is_empty() {
                    break;
                }
                for name in missing {
                    let answer = ask(MessageRequest::FileByFilename(name)).await?;
                    add(answer, &mut files)?;
                }
            }
            Ok(files.into_values().collect())
        }
    };
}

reflection!(reflect_v1, tonic_reflection::pb::v1);
reflection!(reflect_v1alpha, tonic_reflection::pb::v1alpha);

/// Asks the server what it serves (gRPC server reflection, v1 then v1alpha).
pub async fn reflect(
    channel: &Channel,
    metadata: &MetadataMap,
) -> Result<DescriptorPool, GrpcError> {
    let files = match reflect_v1(channel, metadata).await {
        Err(status) if status.code() == tonic::Code::Unimplemented => {
            reflect_v1alpha(channel, metadata).await
        }
        other => other,
    }
    .map_err(|status| {
        GrpcError::Reflection(if status.code() == tonic::Code::Unimplemented {
            "the server does not offer reflection; add the service's .proto files to the request"
                .into()
        } else {
            format!("reflection failed: {}", crate::status_text(&status))
        })
    })?;
    let mut pool = DescriptorPool::new();
    pool.add_file_descriptor_protos(in_dependency_order(files))
        .map_err(|e| {
            GrpcError::Reflection(format!(
                "the server's descriptions do not fit together: {e}"
            ))
        })?;
    Ok(pool)
}

/// Each file after the files it imports.
fn in_dependency_order(files: Vec<FileDescriptorProto>) -> Vec<FileDescriptorProto> {
    let mut left: BTreeMap<String, FileDescriptorProto> = files
        .into_iter()
        .map(|f| (f.name().to_owned(), f))
        .collect();
    let mut out = Vec::new();
    while !left.is_empty() {
        let ready: Vec<String> = left
            .values()
            .filter(|f| f.dependency.iter().all(|d| !left.contains_key(d)))
            .map(|f| f.name().to_owned())
            .collect();
        if ready.is_empty() {
            // A cycle cannot be valid; hand the rest over as they are.
            out.extend(std::mem::take(&mut left).into_values());
            break;
        }
        for name in ready {
            out.extend(left.remove(&name));
        }
    }
    out
}

/// A method a service offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodInfo {
    /// `package.Service/Method`, as written in a request.
    pub name: String,
    pub input: String,
    pub output: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
}

impl MethodInfo {
    /// "unary", "server streaming", "client streaming" or "bidirectional".
    pub fn kind(&self) -> &'static str {
        match (self.client_streaming, self.server_streaming) {
            (false, false) => "unary",
            (false, true) => "server streaming",
            (true, false) => "client streaming",
            (true, true) => "bidirectional",
        }
    }
}

fn is_reflection(service: &str) -> bool {
    service.starts_with("grpc.reflection.")
}

/// Every method of every service (the reflection service left out), by name.
pub fn methods(pool: &DescriptorPool) -> Vec<MethodInfo> {
    let mut out: Vec<MethodInfo> = pool
        .services()
        .filter(|s| !is_reflection(s.full_name()))
        .flat_map(|s| s.methods().collect::<Vec<_>>())
        .map(|m| MethodInfo {
            name: format!("{}/{}", m.parent_service().full_name(), m.name()),
            input: m.input().full_name().to_owned(),
            output: m.output().full_name().to_owned(),
            client_streaming: m.is_client_streaming(),
            server_streaming: m.is_server_streaming(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The method `name` names: `package.Service/Method` (or with a `.` before
/// the method, as some tools write it).
pub fn find_method(pool: &DescriptorPool, name: &str) -> Option<MethodDescriptor> {
    let name = name.trim().trim_start_matches('/');
    let (service, method) = name.rsplit_once('/').or_else(|| name.rsplit_once('.'))?;
    pool.get_service_by_name(service)?
        .methods()
        .find(|m| m.name() == method)
}

/// A JSON message with every field filled with an example value, to start
/// writing a request from.
pub fn example_message(desc: &MessageDescriptor) -> String {
    serde_json::to_string_pretty(&example(desc, 0)).unwrap_or_default()
}

fn example(desc: &MessageDescriptor, depth: usize) -> Value {
    if let Some(value) = well_known_example(desc.full_name()) {
        return value;
    }
    let mut map = Map::new();
    if depth > 3 {
        return Value::Object(map);
    }
    // Of a oneof, only the first field: setting two is an error.
    let mut oneofs = BTreeSet::new();
    for field in desc.fields() {
        if let Some(oneof) = field.containing_oneof()
            && !field.field_descriptor_proto().proto3_optional()
            && !oneofs.insert(oneof.full_name().to_owned())
        {
            continue;
        }
        map.insert(field.name().to_owned(), field_example(&field, depth));
    }
    Value::Object(map)
}

fn field_example(field: &FieldDescriptor, depth: usize) -> Value {
    if field.is_map() {
        let Kind::Message(entry) = field.kind() else {
            return Value::Object(Map::new());
        };
        let value = entry.map_entry_value_field();
        let mut map = Map::new();
        map.insert("key".into(), kind_example(&value.kind(), depth));
        return Value::Object(map);
    }
    let one = kind_example(&field.kind(), depth);
    if field.cardinality() == Cardinality::Repeated {
        Value::Array(vec![one])
    } else {
        one
    }
}

fn kind_example(kind: &Kind, depth: usize) -> Value {
    match kind {
        Kind::Double | Kind::Float => Value::from(0.0),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 | Kind::Uint32 | Kind::Fixed32 => {
            Value::from(0)
        }
        // 64-bit numbers are strings in protobuf's JSON.
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 | Kind::Uint64 | Kind::Fixed64 => {
            Value::from("0")
        }
        Kind::Bool => Value::Bool(false),
        Kind::String => Value::from(""),
        Kind::Bytes => Value::from(""),
        Kind::Enum(e) => Value::from(
            e.values()
                .next()
                .map(|v| v.name().to_owned())
                .unwrap_or_default(),
        ),
        Kind::Message(m) => example(m, depth + 1),
    }
}

fn well_known_example(name: &str) -> Option<Value> {
    Some(match name {
        "google.protobuf.Timestamp" => Value::from("1970-01-01T00:00:00Z"),
        "google.protobuf.Duration" => Value::from("0s"),
        "google.protobuf.FieldMask" => Value::from(""),
        "google.protobuf.Struct" => Value::Object(Map::new()),
        "google.protobuf.Value" => Value::Null,
        "google.protobuf.ListValue" => Value::Array(Vec::new()),
        "google.protobuf.Empty" => Value::Object(Map::new()),
        "google.protobuf.StringValue" | "google.protobuf.BytesValue" => Value::from(""),
        "google.protobuf.BoolValue" => Value::Bool(false),
        "google.protobuf.Int64Value" | "google.protobuf.UInt64Value" => Value::from("0"),
        "google.protobuf.Int32Value"
        | "google.protobuf.UInt32Value"
        | "google.protobuf.DoubleValue"
        | "google.protobuf.FloatValue" => Value::from(0),
        _ => return None,
    })
}

/// The services, and the messages and enums they use, written the way a
/// `.proto` file writes them (with full names).
pub fn describe(pool: &DescriptorPool) -> String {
    let mut out = String::new();
    let mut messages: BTreeMap<String, MessageDescriptor> = BTreeMap::new();
    let mut enums: BTreeMap<String, EnumDescriptor> = BTreeMap::new();
    for service in pool.services().filter(|s| !is_reflection(s.full_name())) {
        out += &format!("service {} {{\n", service.full_name());
        for m in service.methods() {
            let stream = |yes: bool| if yes { "stream " } else { "" };
            out += &format!(
                "  rpc {}({}{}) returns ({}{});\n",
                m.name(),
                stream(m.is_client_streaming()),
                m.input().full_name(),
                stream(m.is_server_streaming()),
                m.output().full_name()
            );
            collect(&m.input(), &mut messages, &mut enums);
            collect(&m.output(), &mut messages, &mut enums);
        }
        out += "}\n\n";
    }
    for message in messages.values() {
        out += &format!("message {} {{\n", message.full_name());
        for field in message.fields() {
            let label = if field.is_map() {
                ""
            } else if field.cardinality() == Cardinality::Repeated {
                "repeated "
            } else if field.field_descriptor_proto().proto3_optional() {
                "optional "
            } else {
                ""
            };
            out += &format!(
                "  {label}{} {} = {};\n",
                type_name(&field),
                field.name(),
                field.number()
            );
        }
        out += "}\n\n";
    }
    for e in enums.values() {
        out += &format!("enum {} {{\n", e.full_name());
        for v in e.values() {
            out += &format!("  {} = {};\n", v.name(), v.number());
        }
        out += "}\n\n";
    }
    out.trim_end().to_owned() + "\n"
}

/// `message` and the messages and enums its fields use, well-known types
/// left out (they are standard).
fn collect(
    message: &MessageDescriptor,
    messages: &mut BTreeMap<String, MessageDescriptor>,
    enums: &mut BTreeMap<String, EnumDescriptor>,
) {
    let name = message.full_name();
    if name.starts_with("google.protobuf.") || message.is_map_entry() || messages.contains_key(name)
    {
        return;
    }
    messages.insert(name.to_owned(), message.clone());
    for field in message.fields() {
        let kind = if field.is_map() {
            match field.kind() {
                Kind::Message(entry) => entry.map_entry_value_field().kind(),
                other => other,
            }
        } else {
            field.kind()
        };
        match kind {
            Kind::Message(m) => collect(&m, messages, enums),
            Kind::Enum(e) if !e.full_name().starts_with("google.protobuf.") => {
                enums.insert(e.full_name().to_owned(), e);
            }
            _ => {}
        }
    }
}

fn type_name(field: &FieldDescriptor) -> String {
    if field.is_map()
        && let Kind::Message(entry) = field.kind()
    {
        return format!(
            "map<{}, {}>",
            kind_name(&entry.map_entry_key_field().kind()),
            kind_name(&entry.map_entry_value_field().kind())
        );
    }
    kind_name(&field.kind())
}

fn kind_name(kind: &Kind) -> String {
    match kind {
        Kind::Double => "double".into(),
        Kind::Float => "float".into(),
        Kind::Int32 => "int32".into(),
        Kind::Int64 => "int64".into(),
        Kind::Uint32 => "uint32".into(),
        Kind::Uint64 => "uint64".into(),
        Kind::Sint32 => "sint32".into(),
        Kind::Sint64 => "sint64".into(),
        Kind::Fixed32 => "fixed32".into(),
        Kind::Fixed64 => "fixed64".into(),
        Kind::Sfixed32 => "sfixed32".into(),
        Kind::Sfixed64 => "sfixed64".into(),
        Kind::Bool => "bool".into(),
        Kind::String => "string".into(),
        Kind::Bytes => "bytes".into(),
        Kind::Message(m) => m.full_name().to_owned(),
        Kind::Enum(e) => e.full_name().to_owned(),
    }
}
