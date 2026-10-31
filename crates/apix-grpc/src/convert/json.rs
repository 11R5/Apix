//! A guess at messages from an example of them, in JSON. Keys keep their
//! order, so the first field gets the first number.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::schema::{Field, Msg, Schema, Ty, names_from_key, to_pascal};

/// JSON with the order of its keys (`serde_json::Value` sorts them).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Json {
    Null,
    Bool,
    Int(i128),
    Float,
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Json;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_bool<E>(self, _: bool) -> Result<Json, E> {
                Ok(Json::Bool)
            }
            fn visit_i64<E>(self, v: i64) -> Result<Json, E> {
                Ok(Json::Int(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Json, E> {
                Ok(Json::Int(v.into()))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Json, E> {
                Ok(Json::Float)
            }
            fn visit_str<E>(self, v: &str) -> Result<Json, E> {
                Ok(Json::Str(v.to_owned()))
            }
            fn visit_unit<E>(self) -> Result<Json, E> {
                Ok(Json::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Json::Arr(items))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
                let mut items: Vec<(String, Json)> = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, Json>()? {
                    match items.iter_mut().find(|(k, _)| *k == key) {
                        Some(found) => found.1 = value,
                        None => items.push((key, value)),
                    }
                }
                Ok(Json::Obj(items))
            }
        }
        deserializer.deserialize_any(V)
    }
}

pub(super) fn parse(text: &str) -> Result<Json, serde_json::Error> {
    serde_json::from_str(text)
}

/// Adds the message `root` for an example, or for a list of examples of it
/// (a stream). Says whether it was a list.
pub(super) fn infer(json: &Json, root: &str, schema: &mut Schema, notes: &mut Vec<String>) -> bool {
    let (items, list): (Vec<&Json>, bool) = match json {
        Json::Arr(items) => (items.iter().collect(), true),
        other => (vec![other], false),
    };
    let objects: Vec<&Json> = items
        .iter()
        .copied()
        .filter(|j| matches!(j, Json::Obj(_)))
        .collect();
    if objects.len() != items.len() || objects.is_empty() {
        notes.push(format!(
            "A gRPC message is a JSON object; {root} is empty because the example holds something else"
        ));
    }
    let name = schema.free_name(root);
    add_message(&name, &objects, schema, notes);
    list
}

fn add_message(name: &str, objects: &[&Json], schema: &mut Schema, notes: &mut Vec<String>) {
    // Taken first, so what is inside cannot have the same name.
    schema.messages.push(Msg {
        name: name.to_owned(),
        fields: Vec::new(),
    });
    let mut keys: Vec<&str> = Vec::new();
    for object in objects {
        if let Json::Obj(items) = object {
            for (key, _) in items {
                if !keys.contains(&key.as_str()) {
                    keys.push(key);
                }
            }
        }
    }
    let mut fields = Vec::new();
    for key in keys {
        let mut values = Vec::new();
        let mut absent = false;
        for object in objects {
            if let Json::Obj(items) = object {
                match items.iter().find(|(k, _)| k == key) {
                    Some((_, value)) => values.push(value),
                    None => absent = true,
                }
            }
        }
        let (field_name, json_name) = names_from_key(key);
        let shape = infer_values(
            &format!("{name}.{field_name}"),
            &field_name,
            &values,
            schema,
            notes,
        );
        fields.push(Field {
            name: field_name,
            json_name,
            ty: shape.ty,
            repeated: shape.repeated,
            optional: shape.optional || absent,
            number: None,
        });
    }
    if let Some(message) = schema.messages.iter_mut().find(|m| m.name == name) {
        message.fields = fields;
    }
}

struct Shape {
    ty: Ty,
    repeated: bool,
    optional: bool,
}

fn scalar(name: &str) -> Ty {
    Ty::Scalar(name.to_owned())
}

/// `tags` → `Tag`, for the message of a list's items.
fn singular(name: &str) -> String {
    match name.strip_suffix('s') {
        Some(rest) if rest.len() > 2 && !rest.ends_with('s') => rest.to_owned(),
        _ => name.to_owned(),
    }
}

/// Keys that are data rather than names: ids, numbers, addresses.
fn looks_like_map(items: &[(String, Json)]) -> bool {
    !items.is_empty()
        && items.iter().all(|(key, _)| {
            key.starts_with(|c: char| c.is_ascii_digit()) || key.contains(['-', '.', ':', '/', ' '])
        })
}

fn looks_like_time(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() >= 20
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && (text.ends_with('Z') || text[19..].contains(['+', '-']))
}

fn infer_values(
    place: &str,
    field: &str,
    values: &[&Json],
    schema: &mut Schema,
    notes: &mut Vec<String>,
) -> Shape {
    let optional = values.iter().any(|v| matches!(v, Json::Null));
    let values: Vec<&Json> = values
        .iter()
        .copied()
        .filter(|v| !matches!(v, Json::Null))
        .collect();
    if values.is_empty() {
        notes.push(format!(
            "{place} is only ever null in the example, so it was taken for a string"
        ));
        return Shape {
            ty: scalar("string"),
            repeated: false,
            optional: true,
        };
    }
    let lists = values.iter().filter(|v| matches!(v, Json::Arr(_))).count();
    if lists == values.len() {
        let items: Vec<&Json> = values
            .iter()
            .flat_map(|v| match v {
                Json::Arr(items) => items.iter().collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect();
        if items.is_empty() {
            notes.push(format!(
                "{place} is an empty list in the example, so it was taken for a list of strings"
            ));
            return Shape {
                ty: scalar("string"),
                repeated: true,
                optional: false,
            };
        }
        let inner = infer_values(place, &singular(field), &items, schema, notes);
        if inner.repeated {
            // Protobuf has no list of lists: each inner list becomes a message.
            let name = schema.free_name(&format!("{}List", to_pascal(&singular(field))));
            notes.push(format!(
                "{place} is a list of lists, which protobuf does not have, so its items are {name} messages"
            ));
            schema.messages.push(Msg {
                name: name.clone(),
                fields: vec![Field {
                    name: "items".into(),
                    json_name: None,
                    ty: inner.ty,
                    repeated: true,
                    optional: false,
                    number: None,
                }],
            });
            return Shape {
                ty: Ty::Named(name),
                repeated: true,
                optional: false,
            };
        }
        return Shape {
            ty: inner.ty,
            repeated: true,
            optional: false,
        };
    }
    let objects: Vec<&Json> = values
        .iter()
        .copied()
        .filter(|v| matches!(v, Json::Obj(_)))
        .collect();
    if objects.len() == values.len() {
        let all_map_like = objects.iter().all(|o| match o {
            Json::Obj(items) => looks_like_map(items),
            _ => false,
        });
        if all_map_like {
            let inner: Vec<&Json> = objects
                .iter()
                .flat_map(|o| match o {
                    Json::Obj(items) => items.iter().map(|(_, v)| v).collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect();
            let value = infer_values(place, &singular(field), &inner, schema, notes);
            if !value.repeated {
                notes.push(format!(
                    "{place} has data for keys, so it was taken for a map"
                ));
                return Shape {
                    ty: Ty::Map(Box::new(scalar("string")), Box::new(value.ty)),
                    repeated: false,
                    optional: false,
                };
            }
        }
        let name = schema.free_name(&to_pascal(field));
        add_message(&name, &objects, schema, notes);
        return Shape {
            ty: Ty::Named(name),
            repeated: false,
            optional,
        };
    }
    let ints: Vec<i128> = values
        .iter()
        .filter_map(|v| match v {
            Json::Int(n) => Some(*n),
            _ => None,
        })
        .collect();
    let floats = values.iter().filter(|v| matches!(v, Json::Float)).count();
    let bools = values.iter().filter(|v| matches!(v, Json::Bool)).count();
    let strings: Vec<&str> = values
        .iter()
        .filter_map(|v| match v {
            Json::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    let ty = if ints.len() + floats == values.len() {
        if floats > 0 {
            notes.push(format!(
                "{place} has a fraction, so it was taken for a double; a float goes differently on the wire"
            ));
            scalar("double")
        } else if ints.iter().all(|n| i32::try_from(*n).is_ok()) {
            scalar("int32")
        } else if ints.iter().all(|n| i64::try_from(*n).is_ok()) {
            scalar("int64")
        } else {
            scalar("uint64")
        }
    } else if bools == values.len() {
        scalar("bool")
    } else if strings.len() == values.len() {
        if strings.iter().all(|s| looks_like_time(s)) {
            notes.push(format!(
                "{place} looks like a time; it was taken for a string, change it to google.protobuf.Timestamp if the server's is one"
            ));
        } else if strings
            .iter()
            .all(|s| s.len() > 9 && s.chars().all(|c| c.is_ascii_digit()))
        {
            notes.push(format!(
                "{place} holds digits in a string, as protobuf's JSON writes 64-bit numbers; it was taken for a string, change it to int64 if the server's is one"
            ));
        }
        scalar("string")
    } else {
        notes.push(format!(
            "{place} has values of different types, so it was taken for a string"
        ));
        scalar("string")
    };
    Shape {
        ty,
        repeated: false,
        optional,
    }
}
