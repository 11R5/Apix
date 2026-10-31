//! A definition held against what the server says about itself.

use std::collections::HashSet;

use prost_reflect::{Cardinality, DescriptorPool, FieldDescriptor, Kind, MessageDescriptor};

use crate::descriptors::{find_method, kind_name, methods};

const MOST: usize = 40;

/// What differs, in words: each method of `mine` against the server's, then
/// the fields of what it takes and answers with. Fields are matched by name,
/// ignoring case and underscores; message names may differ freely, since they
/// do not go on the wire.
pub fn compare(mine: &DescriptorPool, server: &DescriptorPool) -> Vec<String> {
    let mut out = Vec::new();
    let offered: Vec<String> = methods(server).into_iter().map(|m| m.name).collect();
    for method in methods(mine) {
        let Some(theirs) = find_method(server, &method.name) else {
            let list = if offered.is_empty() {
                "none".to_owned()
            } else {
                offered.join(", ")
            };
            out.push(format!(
                "The server has no method {}; it has: {list}",
                method.name
            ));
            continue;
        };
        let Some(ours) = find_method(mine, &method.name) else {
            continue;
        };
        if ours.is_client_streaming() != theirs.is_client_streaming()
            || ours.is_server_streaming() != theirs.is_server_streaming()
        {
            out.push(format!(
                "{}: streaming differs (server: {})",
                method.name,
                crate::descriptors::kind_of(&theirs)
            ));
        }
        let mut seen = HashSet::new();
        walk(&ours.input(), &theirs.input(), &mut seen, &mut out);
        walk(&ours.output(), &theirs.output(), &mut seen, &mut out);
    }
    if out.len() > MOST {
        let more = out.len() - MOST;
        out.truncate(MOST);
        out.push(format!("… and {more} more"));
    }
    out
}

fn same_name(name: &str) -> String {
    name.replace('_', "").to_ascii_lowercase()
}

fn label(field: &FieldDescriptor) -> String {
    let repeated = field.cardinality() == Cardinality::Repeated && !field.is_map();
    let name = if field.is_map() {
        "map".to_owned()
    } else {
        kind_name(&field.kind())
    };
    if repeated {
        format!("repeated {name}")
    } else {
        name
    }
}

/// Message and enum names are free; their shape is checked instead.
fn same_type(a: &FieldDescriptor, b: &FieldDescriptor) -> bool {
    if a.is_map() != b.is_map() || a.is_list() != b.is_list() {
        return false;
    }
    matches!(
        (a.kind(), b.kind()),
        (Kind::Message(_), Kind::Message(_)) | (Kind::Enum(_), Kind::Enum(_))
    ) || kind_name(&a.kind()) == kind_name(&b.kind())
}

fn walk(
    mine: &MessageDescriptor,
    theirs: &MessageDescriptor,
    seen: &mut HashSet<(String, String)>,
    out: &mut Vec<String>,
) {
    if !seen.insert((mine.full_name().to_owned(), theirs.full_name().to_owned())) {
        return;
    }
    // An empty message says nothing to compare: nothing was known about it.
    if mine.fields().len() == 0 {
        return;
    }
    let owner = mine.full_name();
    for field in mine.fields() {
        let found = theirs
            .fields()
            .find(|f| same_name(f.name()) == same_name(field.name()));
        let Some(other) = found else {
            let taken = theirs
                .get_field(field.number())
                .map(|f| format!(" (its number {} is `{}`)", field.number(), f.name()))
                .unwrap_or_default();
            out.push(format!(
                "{owner}.{}: the server has no such field{taken}",
                field.name()
            ));
            continue;
        };
        if field.number() != other.number() {
            out.push(format!(
                "{owner}.{}: number {} here, {} on the server",
                field.name(),
                field.number(),
                other.number()
            ));
        }
        if !same_type(&field, &other) {
            out.push(format!(
                "{owner}.{}: {} here, {} on the server",
                field.name(),
                label(&field),
                label(&other)
            ));
        }
        if let (Kind::Message(a), Kind::Message(b)) = (field.kind(), other.kind()) {
            walk(&a, &b, seen, out);
        }
    }
    for other in theirs.fields() {
        if !mine
            .fields()
            .any(|f| same_name(f.name()) == same_name(other.name()))
        {
            out.push(format!(
                "{owner}: the server also has `{}` = {} ({})",
                other.name(),
                other.number(),
                label(&other)
            ));
        }
    }
}
