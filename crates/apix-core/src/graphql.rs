//! GraphQL schemas: the standard introspection query, the schema it returns
//! (types, fields, arguments, enums, input types), its SDL, and what fits at
//! a place in a query — for completion and hover docs in the editor.
//!
//! The query reader is deliberately small: it follows selections, field
//! arguments, input objects, fragments and variable definitions well enough
//! to know which type the cursor is in, and does not validate anything.

use std::collections::BTreeMap;

use serde_json::Value;

/// The introspection query every GraphQL server answers.
pub const INTROSPECTION_QUERY: &str = r#"query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types { ...FullType }
  }
}
fragment FullType on __Type {
  kind name description
  fields(includeDeprecated: true) {
    name description
    args { ...InputValue }
    type { ...TypeRef }
    isDeprecated deprecationReason
  }
  inputFields { ...InputValue }
  interfaces { ...TypeRef }
  enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason }
  possibleTypes { ...TypeRef }
}
fragment InputValue on __InputValue {
  name description
  type { ...TypeRef }
  defaultValue
}
fragment TypeRef on __Type {
  kind name
  ofType { kind name ofType { kind name ofType { kind name ofType { kind name
    ofType { kind name ofType { kind name ofType { kind name } } } } } } }
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Scalar,
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
}

/// A type as fields and arguments name it: `User`, `[User!]!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    /// The type underneath the lists and `!`s.
    pub fn named(&self) -> &str {
        match self {
            TypeRef::Named(name) => name,
            TypeRef::List(inner) | TypeRef::NonNull(inner) => inner.named(),
        }
    }

    fn from_json(value: &Value) -> Option<Self> {
        let kind = value.get("kind")?.as_str()?;
        match kind {
            "NON_NULL" => Some(TypeRef::NonNull(Box::new(Self::from_json(
                value.get("ofType")?,
            )?))),
            "LIST" => Some(TypeRef::List(Box::new(Self::from_json(
                value.get("ofType")?,
            )?))),
            _ => Some(TypeRef::Named(value.get("name")?.as_str()?.to_owned())),
        }
    }
}

impl std::fmt::Display for TypeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeRef::Named(name) => f.write_str(name),
            TypeRef::List(inner) => write!(f, "[{inner}]"),
            TypeRef::NonNull(inner) => write!(f, "{inner}!"),
        }
    }
}

/// An argument or an input object's field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValue {
    pub name: String,
    pub description: String,
    pub ty: TypeRef,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub description: String,
    pub args: Vec<InputValue>,
    pub ty: TypeRef,
    /// Why it is deprecated, when it is.
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumValue {
    pub name: String,
    pub description: String,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub name: String,
    pub kind: TypeKind,
    pub description: String,
    pub fields: Vec<Field>,
    pub input_fields: Vec<InputValue>,
    pub enum_values: Vec<EnumValue>,
    pub interfaces: Vec<String>,
    /// The members of a union, or the types that implement an interface.
    pub possible_types: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Schema {
    pub query: Option<String>,
    pub mutation: Option<String>,
    pub subscription: Option<String>,
    pub types: BTreeMap<String, TypeDef>,
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn deprecation(value: &Value) -> Option<String> {
    value
        .get("isDeprecated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| text(value, "deprecationReason"))
}

fn input_values(value: Option<&Value>) -> Vec<InputValue> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some(InputValue {
                name: text(v, "name"),
                description: text(v, "description"),
                ty: TypeRef::from_json(v.get("type")?)?,
                default: v
                    .get("defaultValue")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

fn names(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|v| text(v, "name"))
        .collect()
}

impl Schema {
    /// Reads an introspection result: the server's whole answer
    /// (`{"data": {"__schema": …}}`), or just `data` or `__schema`.
    pub fn from_introspection(json: &str) -> Result<Self, String> {
        let root: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(message) = root
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .filter(|_| root.get("data").is_none_or(Value::is_null))
        {
            return Err(format!("the server answered with an error: {message}"));
        }
        let schema = root
            .pointer("/data/__schema")
            .or_else(|| root.get("__schema"))
            .or_else(|| root.get("types").map(|_| &root))
            .ok_or("not an introspection result (no __schema)")?;
        let root_name = |key: &str| {
            schema
                .get(key)
                .and_then(|t| t.get("name"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let mut types = BTreeMap::new();
        for t in schema
            .get("types")
            .and_then(Value::as_array)
            .ok_or("the schema lists no types")?
        {
            let kind = match t.get("kind").and_then(Value::as_str) {
                Some("SCALAR") => TypeKind::Scalar,
                Some("OBJECT") => TypeKind::Object,
                Some("INTERFACE") => TypeKind::Interface,
                Some("UNION") => TypeKind::Union,
                Some("ENUM") => TypeKind::Enum,
                Some("INPUT_OBJECT") => TypeKind::InputObject,
                _ => continue,
            };
            let name = text(t, "name");
            let fields = t
                .get("fields")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|f| {
                    Some(Field {
                        name: text(f, "name"),
                        description: text(f, "description"),
                        args: input_values(f.get("args")),
                        ty: TypeRef::from_json(f.get("type")?)?,
                        deprecated: deprecation(f),
                    })
                })
                .collect();
            let enum_values = t
                .get("enumValues")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|v| EnumValue {
                    name: text(v, "name"),
                    description: text(v, "description"),
                    deprecated: deprecation(v),
                })
                .collect();
            types.insert(
                name.clone(),
                TypeDef {
                    name,
                    kind,
                    description: text(t, "description"),
                    fields,
                    input_fields: input_values(t.get("inputFields")),
                    enum_values,
                    interfaces: names(t.get("interfaces")),
                    possible_types: names(t.get("possibleTypes")),
                },
            );
        }
        Ok(Self {
            query: root_name("queryType"),
            mutation: root_name("mutationType"),
            subscription: root_name("subscriptionType"),
            types,
        })
    }

    pub fn get(&self, name: &str) -> Option<&TypeDef> {
        self.types.get(name)
    }

    /// The types written by the API (not `__Schema` and the like).
    pub fn own_types(&self) -> impl Iterator<Item = &TypeDef> {
        self.types.values().filter(|t| !t.name.starts_with("__"))
    }

    fn field(&self, ty: &str, name: &str) -> Option<&Field> {
        self.get(ty)?.fields.iter().find(|f| f.name == name)
    }

    /// The schema in SDL, the way people write it.
    pub fn to_sdl(&self) -> String {
        let builtin = ["String", "Int", "Float", "Boolean", "ID"];
        let mut out = String::new();
        let roots = [
            ("query", &self.query, "Query"),
            ("mutation", &self.mutation, "Mutation"),
            ("subscription", &self.subscription, "Subscription"),
        ];
        if roots
            .iter()
            .any(|(_, name, usual)| name.as_deref().is_some_and(|n| n != *usual))
        {
            out += "schema {\n";
            for (key, name, _) in roots {
                if let Some(name) = name {
                    out += &format!("  {key}: {name}\n");
                }
            }
            out += "}\n\n";
        }
        // The root types first, then the rest by name.
        let root_names: Vec<&str> = roots.iter().filter_map(|(_, n, _)| n.as_deref()).collect();
        let ordered = root_names.iter().filter_map(|n| self.get(n)).chain(
            self.own_types()
                .filter(|t| !root_names.contains(&t.name.as_str())),
        );
        for t in ordered {
            if t.kind == TypeKind::Scalar && builtin.contains(&t.name.as_str()) {
                continue;
            }
            out += &description_block(&t.description, "");
            match t.kind {
                TypeKind::Scalar => out += &format!("scalar {}\n\n", t.name),
                TypeKind::Union => {
                    out += &format!("union {} = {}\n\n", t.name, t.possible_types.join(" | "))
                }
                TypeKind::Enum => {
                    out += &format!("enum {} {{\n", t.name);
                    for v in &t.enum_values {
                        out += &description_block(&v.description, "  ");
                        out += &format!("  {}{}\n", v.name, deprecated_sdl(&v.deprecated));
                    }
                    out += "}\n\n";
                }
                TypeKind::InputObject => {
                    out += &format!("input {} {{\n", t.name);
                    for f in &t.input_fields {
                        out += &description_block(&f.description, "  ");
                        out += &format!("  {}\n", input_value_sdl(f));
                    }
                    out += "}\n\n";
                }
                TypeKind::Object | TypeKind::Interface => {
                    let keyword = if t.kind == TypeKind::Object {
                        "type"
                    } else {
                        "interface"
                    };
                    let implements = if t.interfaces.is_empty() {
                        String::new()
                    } else {
                        format!(" implements {}", t.interfaces.join(" & "))
                    };
                    out += &format!("{keyword} {}{implements} {{\n", t.name);
                    for f in &t.fields {
                        out += &description_block(&f.description, "  ");
                        let args = if f.args.is_empty() {
                            String::new()
                        } else {
                            let args: Vec<String> = f.args.iter().map(input_value_sdl).collect();
                            format!("({})", args.join(", "))
                        };
                        out += &format!(
                            "  {}{args}: {}{}\n",
                            f.name,
                            f.ty,
                            deprecated_sdl(&f.deprecated)
                        );
                    }
                    out += "}\n\n";
                }
            }
        }
        out.trim_end().to_owned() + "\n"
    }
}

fn description_block(description: &str, indent: &str) -> String {
    if description.is_empty() {
        return String::new();
    }
    if description.contains('\n') || description.contains('"') {
        let body: Vec<String> = description
            .lines()
            .map(|l| format!("{indent}{l}"))
            .collect();
        format!("{indent}\"\"\"\n{}\n{indent}\"\"\"\n", body.join("\n"))
    } else {
        format!("{indent}\"{description}\"\n")
    }
}

fn deprecated_sdl(reason: &Option<String>) -> String {
    match reason {
        None => String::new(),
        Some(r) if r.is_empty() || r == "No longer supported" => " @deprecated".into(),
        Some(r) => format!(
            " @deprecated(reason: {})",
            serde_json::to_string(r).unwrap_or_default()
        ),
    }
}

fn input_value_sdl(v: &InputValue) -> String {
    match &v.default {
        Some(default) => format!("{}: {} = {default}", v.name, v.ty),
        None => format!("{}: {}", v.name, v.ty),
    }
}

// ----------------------------------------------------------- reading queries

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Name(String),
    Punct(char),
    Spread,
    /// Strings, numbers, `$variables`: values with no further meaning here.
    Value,
}

/// The tokens of `text` (comments, commas and whitespace dropped).
fn tokens(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' | b',' => i += 1,
            b'#' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'"' => {
                if text[i..].starts_with("\"\"\"") {
                    i += 3;
                    match text[i..].find("\"\"\"") {
                        Some(end) => i += end + 3,
                        None => i = bytes.len(),
                    }
                } else {
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' && bytes[i] != b'\n' {
                        if bytes[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    i += 1;
                }
                out.push(Token::Value);
            }
            b'.' if text[i..].starts_with("...") => {
                out.push(Token::Spread);
                i += 3;
            }
            b'$' => {
                i += 1;
                while i < bytes.len() && is_name_byte(bytes[i]) {
                    i += 1;
                }
                out.push(Token::Value);
            }
            c if c.is_ascii_digit() || c == b'-' => {
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'.' | b'-' | b'+'))
                {
                    i += 1;
                }
                out.push(Token::Value);
            }
            c if is_name_byte(c) => {
                let start = i;
                while i < bytes.len() && is_name_byte(bytes[i]) {
                    i += 1;
                }
                out.push(Token::Name(text[start..i].to_owned()));
            }
            c if c.is_ascii() => {
                out.push(Token::Punct(c as char));
                i += 1;
            }
            // Anything else (non-ASCII outside strings) is skipped.
            _ => i += text[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
    out
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[derive(Debug, Clone)]
enum Frame {
    /// Inside `{ … }` choosing fields of this type.
    Selection(Option<String>),
    /// Inside a field's `( … )`.
    Args {
        owner: Option<String>,
        field: String,
    },
    /// Inside a directive's `( … )`.
    DirectiveArgs,
    /// Inside an operation's `( $a: Type … )`.
    VarDefs,
    /// Inside an input object value `{ … }` of this input type.
    InputObject(Option<String>),
    /// Inside a list value `[ … ]` of this type (the item type).
    List(Option<TypeRef>),
}

/// What fits at a place in a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spot {
    /// Outside any operation: `query`, `mutation`, …
    TopLevel,
    /// Choosing fields of this type.
    Fields(String),
    /// Naming an argument of `owner.field` (those already given are listed).
    Args {
        owner: String,
        field: String,
        given: Vec<String>,
    },
    /// Naming a field of an input object (those already given are listed).
    InputFields { ty: String, given: Vec<String> },
    /// A value of this type.
    Value(TypeRef),
    /// A type name: after `on` (types with fields) or in a variable definition (input types).
    TypeName { input: bool },
    /// Nothing to suggest.
    Nothing,
}

/// The input type of argument or input field `name` in `frame`.
fn value_type(schema: &Schema, frame: &Frame, name: &str) -> Option<TypeRef> {
    match frame {
        Frame::Args { owner, field } => schema
            .field(owner.as_deref()?, field)?
            .args
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.ty.clone()),
        Frame::InputObject(ty) => schema
            .get(ty.as_deref()?)?
            .input_fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.ty.clone()),
        _ => None,
    }
}

/// The item type of a list type (or the type itself).
fn item_type(ty: &TypeRef) -> TypeRef {
    match ty {
        TypeRef::NonNull(inner) => item_type(inner),
        TypeRef::List(inner) => (**inner).clone(),
        named => named.clone(),
    }
}

/// Reads `text` (everything before the cursor) and says what fits next.
pub fn spot_at(schema: &Schema, text: &str) -> Spot {
    let tokens = tokens(text);
    let mut stack: Vec<Frame> = Vec::new();
    // At the top level: the root type the next `{` opens.
    let mut pending: Option<Option<String>> = None;
    // In a selection: the last field named, for its `(` or `{`.
    let mut last_field: Option<(Option<String>, String)> = None;
    // In arguments or an input object: the name before `:`.
    let mut current: Option<String> = None;
    let mut ix = 0;
    while ix < tokens.len() {
        let token = &tokens[ix];
        let next = tokens.get(ix + 1);
        match (stack.last().cloned(), token) {
            (None, Token::Name(word)) => match word.as_str() {
                "query" => pending = Some(schema.query.clone()),
                "mutation" => pending = Some(schema.mutation.clone()),
                "subscription" => pending = Some(schema.subscription.clone()),
                "on" => {
                    if let Some(Token::Name(ty)) = next {
                        pending = Some(Some(ty.clone()));
                        ix += 1;
                    }
                }
                _ => {}
            },
            (None, Token::Punct('{')) => {
                let root = pending.take().unwrap_or_else(|| schema.query.clone());
                stack.push(Frame::Selection(root));
                last_field = None;
            }
            (None, Token::Punct('(')) => stack.push(Frame::VarDefs),
            (Some(Frame::Selection(ty)), Token::Name(name)) => {
                if next == Some(&Token::Punct(':')) {
                    // An alias; the field comes after the colon.
                    ix += 1;
                } else {
                    last_field = Some((ty.clone(), name.clone()));
                }
            }
            (Some(Frame::Selection(_)), Token::Spread) => {
                if let (Some(Token::Name(on)), Some(Token::Name(ty))) = (next, tokens.get(ix + 2))
                    && on == "on"
                {
                    last_field = None;
                    pending = Some(Some(ty.clone()));
                    ix += 2;
                } else if let Some(Token::Name(_)) = next {
                    ix += 1;
                    last_field = None;
                }
            }
            (Some(Frame::Selection(_)), Token::Punct('@')) => {
                // A directive: skip its name; its `(` is not a field's.
                if let Some(Token::Name(_)) = next {
                    ix += 1;
                }
                if tokens.get(ix + 1) == Some(&Token::Punct('(')) {
                    stack.push(Frame::DirectiveArgs);
                    ix += 1;
                }
            }
            (Some(Frame::Selection(_)), Token::Punct('(')) => {
                let (owner, field) = last_field.clone().unwrap_or_default();
                stack.push(Frame::Args { owner, field });
                current = None;
            }
            (Some(Frame::Selection(_)), Token::Punct('{')) => {
                let child = match pending.take() {
                    Some(ty) => ty,
                    None => last_field.as_ref().and_then(|(owner, field)| {
                        schema
                            .field(owner.as_deref()?, field)
                            .map(|f| f.ty.named().to_owned())
                    }),
                };
                stack.push(Frame::Selection(child));
                last_field = None;
            }
            (Some(Frame::Selection(_)), Token::Punct('}')) => {
                stack.pop();
                last_field = None;
            }
            (Some(Frame::Args { .. } | Frame::InputObject(_)), Token::Name(name))
                if next == Some(&Token::Punct(':')) =>
            {
                current = Some(name.clone());
                ix += 1;
            }
            (Some(frame @ (Frame::Args { .. } | Frame::InputObject(_))), Token::Punct('{')) => {
                let ty = current
                    .as_deref()
                    .and_then(|name| value_type(schema, &frame, name))
                    .map(|t| item_type(&t).named().to_owned());
                stack.push(Frame::InputObject(ty));
                current = None;
            }
            (Some(frame @ (Frame::Args { .. } | Frame::InputObject(_))), Token::Punct('[')) => {
                let ty = current
                    .as_deref()
                    .and_then(|name| value_type(schema, &frame, name))
                    .map(|t| item_type(&t));
                stack.push(Frame::List(ty));
            }
            (Some(Frame::List(ty)), Token::Punct('{')) => {
                stack.push(Frame::InputObject(ty.map(|t| t.named().to_owned())));
                current = None;
            }
            (Some(Frame::List(_)), Token::Punct('[')) => stack.push(Frame::List(None)),
            (Some(Frame::List(_)), Token::Punct(']')) => {
                stack.pop();
            }
            (Some(Frame::InputObject(_)), Token::Punct('}')) => {
                stack.pop();
                current = None;
            }
            (
                Some(Frame::Args { .. } | Frame::DirectiveArgs | Frame::VarDefs),
                Token::Punct(')'),
            ) => {
                stack.pop();
                current = None;
            }
            _ => {}
        }
        ix += 1;
    }

    let last = tokens.last();
    match stack.last() {
        None => match last {
            Some(Token::Name(on)) if on == "on" => Spot::TypeName { input: false },
            Some(Token::Name(w))
                if matches!(
                    w.as_str(),
                    "query" | "mutation" | "subscription" | "fragment"
                ) =>
            {
                Spot::Nothing
            }
            Some(Token::Name(_)) if pending.is_some() => Spot::Nothing,
            _ => Spot::TopLevel,
        },
        Some(Frame::Selection(ty)) => match (last, ty) {
            (Some(Token::Name(on)), _)
                if on == "on" && tokens.len() >= 2 && tokens[tokens.len() - 2] == Token::Spread =>
            {
                Spot::TypeName { input: false }
            }
            (Some(Token::Punct('@')), _) => Spot::Nothing,
            (_, Some(ty)) => Spot::Fields(ty.clone()),
            (_, None) => Spot::Nothing,
        },
        Some(frame @ (Frame::Args { .. } | Frame::InputObject(_))) => {
            if last == Some(&Token::Punct(':')) {
                return current
                    .as_deref()
                    .and_then(|name| value_type(schema, frame, name))
                    .map(Spot::Value)
                    .unwrap_or(Spot::Nothing);
            }
            // The names already given at this level.
            let given = given_names(&tokens);
            match frame {
                Frame::Args {
                    owner: Some(owner),
                    field,
                } => Spot::Args {
                    owner: owner.clone(),
                    field: field.clone(),
                    given,
                },
                Frame::InputObject(Some(ty)) => Spot::InputFields {
                    ty: ty.clone(),
                    given,
                },
                _ => Spot::Nothing,
            }
        }
        Some(Frame::List(Some(ty))) => Spot::Value(ty.clone()),
        Some(Frame::VarDefs) => match last {
            Some(Token::Punct(':' | '[')) => Spot::TypeName { input: true },
            _ => Spot::Nothing,
        },
        _ => Spot::Nothing,
    }
}

/// The `name:`s at the innermost open `(` or `{` (to leave them out).
fn given_names(tokens: &[Token]) -> Vec<String> {
    let mut depth = 0i32;
    let mut out = Vec::new();
    for (ix, token) in tokens.iter().enumerate().rev() {
        match token {
            Token::Punct(')' | '}' | ']') => depth += 1,
            Token::Punct('(' | '{' | '[') => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Token::Name(name) if depth == 0 && tokens.get(ix + 1) == Some(&Token::Punct(':')) => {
                out.push(name.clone())
            }
            _ => {}
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    Keyword,
    Field,
    Argument,
    Value,
    Type,
}

/// One completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    /// What to insert (`id: ` for an argument).
    pub insert: String,
    /// Its type, or what it is.
    pub detail: String,
    pub documentation: String,
    pub kind: SuggestionKind,
    pub deprecated: bool,
}

impl Suggestion {
    fn new(label: &str, detail: String, documentation: &str, kind: SuggestionKind) -> Self {
        Self {
            label: label.to_owned(),
            insert: label.to_owned(),
            detail,
            documentation: documentation.to_owned(),
            kind,
            deprecated: false,
        }
    }
}

/// Where the word being typed at byte `cursor` starts.
pub fn word_start(text: &str, cursor: usize) -> usize {
    let cursor = cursor.min(text.len());
    text[..cursor]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map_or(cursor, |(ix, _)| ix)
}

/// What can be typed at byte `cursor` of `text`, best first, matching the
/// word already begun there. Also returns where that word starts.
pub fn suggestions(schema: &Schema, text: &str, cursor: usize) -> (usize, Vec<Suggestion>) {
    let start = word_start(text, cursor);
    let typed = text[start..cursor.min(text.len())].to_ascii_lowercase();
    let mut all = Vec::new();
    match spot_at(schema, &text[..start]) {
        Spot::TopLevel => {
            for keyword in ["query", "mutation", "subscription", "fragment"] {
                all.push(Suggestion::new(
                    keyword,
                    "keyword".into(),
                    "",
                    SuggestionKind::Keyword,
                ));
            }
        }
        Spot::Fields(ty) => {
            if let Some(def) = schema.get(&ty) {
                for f in &def.fields {
                    let mut s = Suggestion::new(
                        &f.name,
                        f.ty.to_string(),
                        &f.description,
                        SuggestionKind::Field,
                    );
                    s.deprecated = f.deprecated.is_some();
                    all.push(s);
                }
                if matches!(def.kind, TypeKind::Union | TypeKind::Interface) {
                    for member in &def.possible_types {
                        let mut s = Suggestion::new(
                            &format!("... on {member}"),
                            "fragment".into(),
                            "",
                            SuggestionKind::Type,
                        );
                        s.insert = format!("... on {member} {{\n  \n}}");
                        all.push(s);
                    }
                }
            }
            all.push(Suggestion::new(
                "__typename",
                "String!".into(),
                "The name of the object's type.",
                SuggestionKind::Field,
            ));
        }
        Spot::Args {
            owner,
            field,
            given,
        } => {
            if let Some(f) = schema.field(&owner, &field) {
                for a in f.args.iter().filter(|a| !given.contains(&a.name)) {
                    let mut s = Suggestion::new(
                        &a.name,
                        a.ty.to_string(),
                        &a.description,
                        SuggestionKind::Argument,
                    );
                    s.insert = format!("{}: ", a.name);
                    all.push(s);
                }
            }
        }
        Spot::InputFields { ty, given } => {
            if let Some(def) = schema.get(&ty) {
                for f in def.input_fields.iter().filter(|f| !given.contains(&f.name)) {
                    let mut s = Suggestion::new(
                        &f.name,
                        f.ty.to_string(),
                        &f.description,
                        SuggestionKind::Argument,
                    );
                    s.insert = format!("{}: ", f.name);
                    all.push(s);
                }
            }
        }
        Spot::Value(ty) => match schema.get(ty.named()) {
            Some(def) if def.kind == TypeKind::Enum => {
                for v in &def.enum_values {
                    let mut s = Suggestion::new(
                        &v.name,
                        def.name.clone(),
                        &v.description,
                        SuggestionKind::Value,
                    );
                    s.deprecated = v.deprecated.is_some();
                    all.push(s);
                }
            }
            _ if ty.named() == "Boolean" => {
                for b in ["true", "false"] {
                    all.push(Suggestion::new(
                        b,
                        "Boolean".into(),
                        "",
                        SuggestionKind::Value,
                    ));
                }
            }
            _ => {}
        },
        Spot::TypeName { input } => {
            for t in schema.own_types().filter(|t| {
                if input {
                    matches!(
                        t.kind,
                        TypeKind::Scalar | TypeKind::Enum | TypeKind::InputObject
                    )
                } else {
                    matches!(
                        t.kind,
                        TypeKind::Object | TypeKind::Interface | TypeKind::Union
                    )
                }
            }) {
                all.push(Suggestion::new(
                    &t.name,
                    format!("{:?}", t.kind).to_lowercase(),
                    &t.description,
                    SuggestionKind::Type,
                ));
            }
        }
        Spot::Nothing => {}
    }
    // Words that start with what was typed first, then ones that contain it;
    // deprecated ones last.
    let rank = |s: &Suggestion| {
        let label = s.label.to_ascii_lowercase();
        let place = if label.starts_with(&typed) {
            0
        } else if label.contains(&typed) {
            1
        } else {
            2
        };
        (place, s.deprecated)
    };
    all.retain(|s| rank(s).0 < 2);
    all.sort_by_key(rank);
    (start, all)
}

/// Docs for the word at byte `offset`: a field, argument, enum value or
/// type, in Markdown.
pub fn describe_at(schema: &Schema, text: &str, offset: usize) -> Option<String> {
    let offset = offset.min(text.len());
    let start = word_start(text, offset);
    let end = text[offset..]
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(text.len(), |(ix, _)| offset + ix);
    let word = &text[start..end];
    if word.is_empty() {
        return None;
    }
    let with_doc = |head: String, doc: &str| {
        if doc.is_empty() {
            head
        } else {
            format!("{head}\n\n{doc}")
        }
    };
    match spot_at(schema, &text[..start]) {
        Spot::Fields(ty) => {
            let f = schema.field(&ty, word)?;
            let args: Vec<String> = f.args.iter().map(input_value_sdl).collect();
            let args = if args.is_empty() {
                String::new()
            } else {
                format!("({})", args.join(", "))
            };
            let mut doc = f.description.clone();
            if let Some(reason) = &f.deprecated {
                doc = format!("**Deprecated** {reason}\n\n{doc}");
            }
            Some(with_doc(
                format!("`{ty}.{}{args}: {}`", f.name, f.ty),
                doc.trim(),
            ))
        }
        Spot::Args { owner, field, .. } => {
            let a = schema
                .field(&owner, &field)?
                .args
                .iter()
                .find(|a| a.name == word)?;
            Some(with_doc(
                format!("`{}`", input_value_sdl(a)),
                &a.description,
            ))
        }
        Spot::InputFields { ty, .. } => {
            let f = schema
                .get(&ty)?
                .input_fields
                .iter()
                .find(|f| f.name == word)?;
            Some(with_doc(
                format!("`{}`", input_value_sdl(f)),
                &f.description,
            ))
        }
        Spot::Value(ty) => {
            let def = schema.get(ty.named())?;
            let v = def.enum_values.iter().find(|v| v.name == word)?;
            Some(with_doc(
                format!("`{}.{}`", def.name, v.name),
                &v.description,
            ))
        }
        Spot::TypeName { .. } => {
            let def = schema.get(word)?;
            Some(with_doc(format!("`{}`", def.name), &def.description))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small schema, as a server would answer the introspection query.
    const FIXTURE: &str = include_str!("../tests/fixtures/graphql-schema.json");

    fn schema() -> Schema {
        Schema::from_introspection(FIXTURE).unwrap()
    }

    fn labels(text: &str) -> Vec<String> {
        let schema = schema();
        suggestions(&schema, text, text.len())
            .1
            .into_iter()
            .map(|s| s.label)
            .collect()
    }

    #[test]
    fn reads_the_schema() {
        let s = schema();
        assert_eq!(s.query.as_deref(), Some("Query"));
        assert_eq!(s.subscription, None);
        let users = s.field("Query", "users").unwrap();
        assert_eq!(users.ty.to_string(), "[User!]!");
        assert_eq!(users.ty.named(), "User");
        assert_eq!(users.args[0].default.as_deref(), Some("10"));
        assert_eq!(
            s.field("Query", "oldUsers").unwrap().deprecated.as_deref(),
            Some("Use users.")
        );
        assert_eq!(s.get("Node").unwrap().possible_types, ["User", "Post"]);
        assert!(
            Schema::from_introspection(
                r#"{"errors":[{"message":"introspection is off"}],"data":null}"#
            )
            .unwrap_err()
            .contains("introspection is off")
        );
        assert!(Schema::from_introspection("{}").is_err());
    }

    #[test]
    fn writes_sdl() {
        let sdl = schema().to_sdl();
        assert!(
            sdl.contains("type Query {\n  \"One user by id.\"\n  user(id: ID!): User\n"),
            "{sdl}"
        );
        assert!(
            sdl.contains("  users(first: Int = 10, role: Role, filter: UserFilter): [User!]!\n")
        );
        assert!(sdl.contains("  oldUsers: [User] @deprecated(reason: \"Use users.\")\n"));
        assert!(sdl.contains("type User implements Node {"));
        assert!(sdl.contains("enum Role {\n  \"Can do anything.\"\n  ADMIN\n  USER\n}"));
        assert!(sdl.contains("input NewUser {\n  name: String!\n  role: Role = USER\n}"));
        assert!(sdl.contains("\"An ISO date.\"\nscalar DateTime"));
        assert!(!sdl.contains("scalar String"));
        assert!(!sdl.contains("__Schema"));
        assert!(
            !sdl.contains("schema {"),
            "standard root names need no schema block"
        );
        assert!(sdl.starts_with("type Query {"), "{sdl}");
        assert!(sdl.find("type Mutation").unwrap() < sdl.find("scalar DateTime").unwrap());
    }

    #[test]
    fn suggests_fields_by_where_the_cursor_is() {
        assert_eq!(
            labels(""),
            ["query", "mutation", "subscription", "fragment"]
        );
        assert_eq!(
            labels("{ "),
            ["user", "users", "node", "__typename", "oldUsers"]
        );
        assert_eq!(labels("query Q { us"), ["user", "users", "oldUsers"]);
        assert_eq!(
            labels("{ user(id: 1) { "),
            ["id", "name", "role", "posts", "__typename"]
        );
        assert_eq!(
            labels("{ me: user(id: 1) { posts { t"),
            ["title", "__typename"]
        );
        assert_eq!(
            labels("mutation { createUser(input: {name: \"x\"}) { na"),
            ["name", "__typename"]
        );
        assert_eq!(labels("{ user(id: 1) { id } users { r"), ["role"]);
        // Comments, strings and closed braces do not confuse it.
        assert_eq!(
            labels("# { not this\n{ user(id: \"}\") { posts { id } na"),
            ["name", "__typename"]
        );
    }

    #[test]
    fn suggests_arguments_values_and_input_fields() {
        assert_eq!(labels("{ users("), ["first", "role", "filter"]);
        assert_eq!(labels("{ users(first: 2, "), ["role", "filter"]);
        assert_eq!(labels("{ users(role: "), ["ADMIN", "USER"]);
        assert_eq!(labels("{ users(role: A"), ["ADMIN"]);
        assert_eq!(labels("{ users(filter: {"), ["roles", "active"]);
        assert_eq!(labels("{ users(filter: {active: "), ["true", "false"]);
        assert_eq!(labels("{ users(filter: {roles: ["), ["ADMIN", "USER"]);
        assert_eq!(
            labels("mutation { createUser(input: { role: "),
            ["ADMIN", "USER"]
        );
        let (_, args) = suggestions(&schema(), "{ users(", 8);
        assert_eq!(args[0].insert, "first: ");
        assert_eq!(args[0].detail, "Int");
    }

    #[test]
    fn suggests_fragments_and_types() {
        assert_eq!(
            labels("{ node { "),
            ["id", "... on User", "... on Post", "__typename"]
        );
        assert_eq!(
            labels("{ node { ... on "),
            ["Mutation", "Node", "Post", "Query", "User"]
        );
        assert_eq!(labels("{ node { ... on User { n"), ["name", "__typename"]);
        assert_eq!(labels("fragment F on User { po"), ["posts"]);
        assert!(labels("query Q($r: ").contains(&"Role".to_string()));
        assert!(!labels("query Q($r: ").contains(&"User".to_string()));
        assert!(labels("query Q($r: Role) { users(role: $r) { ").contains(&"name".to_string()));
    }

    #[test]
    fn describes_what_is_under_the_cursor() {
        let s = schema();
        let text = "{ user(id: 1) { name role } users(role: ADMIN) { id } }";
        let at = |word: &str| describe_at(&s, text, text.find(word).unwrap() + 1);
        assert_eq!(
            at("user(").unwrap(),
            "`Query.user(id: ID!): User`\n\nOne user by id."
        );
        assert_eq!(at("name").unwrap(), "`User.name: String`\n\nFull name.");
        assert_eq!(at("id:").unwrap(), "`id: ID!`\n\nTheir id.");
        assert_eq!(at("ADMIN").unwrap(), "`Role.ADMIN`\n\nCan do anything.");
        assert_eq!(describe_at(&s, "{ nope }", 3), None);
    }
}
