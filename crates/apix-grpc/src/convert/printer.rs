//! A file description back to `.proto` text, for reading and editing. Only
//! what matters for a call is written; what cannot be written gives `None`,
//! and the description stays as it is.

use prost_types::field_descriptor_proto::{Label, Type};
use prost_types::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
};

use super::schema::default_json_name;

pub(super) fn print(file: &FileDescriptorProto) -> Option<String> {
    let syntax = match file.syntax() {
        "" | "proto2" => "proto2",
        "proto3" => "proto3",
        // Editions change how fields are encoded through features.
        _ => return None,
    };
    if !file.extension.is_empty() {
        return None;
    }
    let mut out = format!("syntax = \"{syntax}\";\n");
    if !file.package().is_empty() {
        out += &format!("\npackage {};\n", file.package());
    }
    if !file.dependency.is_empty() {
        out += "\n";
        for dependency in &file.dependency {
            out += &format!("import \"{dependency}\";\n");
        }
    }
    for service in &file.service {
        out += &format!("\nservice {} {{\n", service.name());
        for method in &service.method {
            let stream = |yes: bool| if yes { "stream " } else { "" };
            out += &format!(
                "  rpc {}({}{}) returns ({}{});\n",
                method.name(),
                stream(method.client_streaming()),
                method.input_type(),
                stream(method.server_streaming()),
                method.output_type()
            );
        }
        out += "}\n";
    }
    for message in &file.message_type {
        out += "\n";
        print_message(message, syntax, 0, &mut out)?;
    }
    for e in &file.enum_type {
        out += "\n";
        print_enum(e, 0, &mut out);
    }
    Some(out)
}

fn print_enum(e: &EnumDescriptorProto, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    *out += &format!("{pad}enum {} {{\n", e.name());
    if e.options.as_ref().is_some_and(|o| o.allow_alias()) {
        *out += &format!("{pad}  option allow_alias = true;\n");
    }
    for value in &e.value {
        *out += &format!("{pad}  {} = {};\n", value.name(), value.number());
    }
    *out += &format!("{pad}}}\n");
}

fn scalar_name(ty: Type) -> Option<&'static str> {
    Some(match ty {
        Type::Double => "double",
        Type::Float => "float",
        Type::Int64 => "int64",
        Type::Uint64 => "uint64",
        Type::Int32 => "int32",
        Type::Fixed64 => "fixed64",
        Type::Fixed32 => "fixed32",
        Type::Bool => "bool",
        Type::String => "string",
        Type::Bytes => "bytes",
        Type::Uint32 => "uint32",
        Type::Sfixed32 => "sfixed32",
        Type::Sfixed64 => "sfixed64",
        Type::Sint32 => "sint32",
        Type::Sint64 => "sint64",
        Type::Group | Type::Message | Type::Enum => return None,
    })
}

fn type_text(field: &FieldDescriptorProto) -> Option<String> {
    match field.r#type() {
        Type::Group => None,
        Type::Message | Type::Enum => Some(field.type_name().to_owned()),
        scalar => scalar_name(scalar).map(str::to_owned),
    }
}

fn field_options(field: &FieldDescriptorProto) -> String {
    let mut options = Vec::new();
    if let Some(packed) = field.options.as_ref().and_then(|o| o.packed) {
        options.push(format!("packed = {packed}"));
    }
    if let Some(json) = &field.json_name
        && *json != default_json_name(field.name())
    {
        options.push(format!(
            "json_name = \"{}\"",
            json.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    if let Some(default) = &field.default_value {
        let value = match field.r#type() {
            Type::String | Type::Bytes => {
                format!("\"{}\"", default.replace('\\', "\\\\").replace('"', "\\\""))
            }
            _ => default.clone(),
        };
        options.push(format!("default = {value}"));
    }
    if options.is_empty() {
        String::new()
    } else {
        format!(" [{}]", options.join(", "))
    }
}

fn print_field(
    message: &DescriptorProto,
    field: &FieldDescriptorProto,
    syntax: &str,
    pad: &str,
    out: &mut String,
) -> Option<()> {
    // A map is a repeated field of a generated nested message.
    let entry = (field.r#type() == Type::Message && field.label() == Label::Repeated)
        .then(|| {
            let name = field.type_name().rsplit('.').next().unwrap_or_default();
            message
                .nested_type
                .iter()
                .find(|n| n.name() == name && n.options.as_ref().is_some_and(|o| o.map_entry()))
        })
        .flatten();
    if let Some(entry) = entry {
        let of = |number: i32| entry.field.iter().find(|f| f.number() == number);
        *out += &format!(
            "{pad}map<{}, {}> {} = {}{};\n",
            type_text(of(1)?)?,
            type_text(of(2)?)?,
            field.name(),
            field.number(),
            field_options(field)
        );
        return Some(());
    }
    let label = match field.label() {
        Label::Repeated => "repeated ",
        Label::Required => "required ",
        Label::Optional if syntax == "proto2" || field.proto3_optional() => "optional ",
        Label::Optional => "",
    };
    *out += &format!(
        "{pad}{label}{} {} = {}{};\n",
        type_text(field)?,
        field.name(),
        field.number(),
        field_options(field)
    );
    Some(())
}

fn print_message(
    message: &DescriptorProto,
    syntax: &str,
    depth: usize,
    out: &mut String,
) -> Option<()> {
    if !message.extension.is_empty() || !message.extension_range.is_empty() {
        return None;
    }
    let pad = "  ".repeat(depth);
    let inner = "  ".repeat(depth + 1);
    *out += &format!("{pad}message {} {{\n", message.name());
    let in_real_oneof =
        |field: &FieldDescriptorProto| field.oneof_index.is_some() && !field.proto3_optional();
    let mut written = vec![false; message.oneof_decl.len()];
    for field in &message.field {
        if !in_real_oneof(field) {
            print_field(message, field, syntax, &inner, out)?;
            continue;
        }
        let ix = field.oneof_index? as usize;
        if *written.get(ix)? {
            continue;
        }
        written[ix] = true;
        *out += &format!("{inner}oneof {} {{\n", message.oneof_decl[ix].name());
        let deeper = format!("{inner}  ");
        for member in message
            .field
            .iter()
            .filter(|f| in_real_oneof(f) && f.oneof_index == field.oneof_index)
        {
            // A oneof's fields have no label.
            *out += &format!(
                "{deeper}{} {} = {}{};\n",
                type_text(member)?,
                member.name(),
                member.number(),
                field_options(member)
            );
        }
        *out += &format!("{inner}}}\n");
    }
    for nested in &message.nested_type {
        if nested.options.as_ref().is_some_and(|o| o.map_entry()) {
            continue;
        }
        print_message(nested, syntax, depth + 1, out)?;
    }
    for e in &message.enum_type {
        print_enum(e, depth + 1, out);
    }
    *out += &format!("{pad}}}\n");
    Some(())
}

fn normalize_message(message: &mut DescriptorProto) {
    message.reserved_name.clear();
    message.reserved_range.clear();
    let map_entry = message.options.as_ref().is_some_and(|o| o.map_entry());
    message.options = None;
    if map_entry {
        message.options = Some(prost_types::MessageOptions {
            map_entry: Some(true),
            ..Default::default()
        });
    }
    for field in &mut message.field {
        let packed = field.options.as_ref().and_then(|o| o.packed);
        field.options = packed.map(|p| prost_types::FieldOptions {
            packed: Some(p),
            ..Default::default()
        });
        if field.json_name.as_deref() == Some(default_json_name(field.name()).as_str()) {
            field.json_name = None;
        }
        field.proto3_optional = field.proto3_optional.filter(|o| *o);
        field.label = Some(field.label() as i32);
    }
    for oneof in &mut message.oneof_decl {
        oneof.options = None;
    }
    message.nested_type.iter_mut().for_each(normalize_message);
    message.nested_type.sort_by(|a, b| a.name().cmp(b.name()));
    message.enum_type.iter_mut().for_each(normalize_enum);
    message.enum_type.sort_by(|a, b| a.name().cmp(b.name()));
}

fn normalize_enum(e: &mut EnumDescriptorProto) {
    let alias = e.options.as_ref().and_then(|o| o.allow_alias);
    e.options = alias.map(|a| prost_types::EnumOptions {
        allow_alias: Some(a),
        ..Default::default()
    });
    e.reserved_name.clear();
    e.reserved_range.clear();
    for value in &mut e.value {
        value.options = None;
    }
}

/// What a file says about calls and messages, without what `print` leaves
/// out, so a file and its printed text can be compared.
pub(super) fn normalized(file: &FileDescriptorProto) -> FileDescriptorProto {
    let mut file = file.clone();
    file.name = None;
    file.options = None;
    file.source_code_info = None;
    file.public_dependency.clear();
    file.weak_dependency.clear();
    if file.syntax.as_deref() == Some("proto2") {
        file.syntax = None;
    }
    file.message_type.iter_mut().for_each(normalize_message);
    file.enum_type.iter_mut().for_each(normalize_enum);
    for service in &mut file.service {
        service.options = None;
        for method in &mut service.method {
            method.options = None;
            method.client_streaming = Some(method.client_streaming());
            method.server_streaming = Some(method.server_streaming());
        }
    }
    file
}
