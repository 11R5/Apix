//! Filtering a JSON body down to what matters: JSONPath (RFC 9535) for an
//! expression that starts with `$`, jq for anything else. The app's response
//! filter and `apix send --filter` share it.

use jaq_core::load::{Arena, File, Loader};
use jaq_core::{Compiler, Ctx, Vars, data, unwrap_valr};
use jaq_json::Val;
use serde_json::Value;

/// Which language an expression is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    JsonPath,
    Jq,
}

impl Language {
    pub fn of(expression: &str) -> Self {
        if expression.trim_start().starts_with('$') {
            Language::JsonPath
        } else {
            Language::Jq
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Language::JsonPath => "JSONPath",
            Language::Jq => "jq",
        }
    }
}

/// What a filter gave: its values, one per match (JSONPath) or output (jq).
#[derive(Debug, Clone, PartialEq)]
pub struct Filtered {
    pub language: Language,
    pub values: Vec<Value>,
}

impl Filtered {
    /// The values as pretty JSON: JSONPath's matches as a list, as JSONPath
    /// tools show them; jq's one output alone, several one after another.
    pub fn to_pretty(&self) -> String {
        let pretty = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_default();
        match self.language {
            Language::JsonPath => pretty(&Value::Array(self.values.clone())),
            Language::Jq => self
                .values
                .iter()
                .map(pretty)
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// At most this many jq outputs; `.[]` over a huge list, or `repeat`, stops here.
const MOST_OUTPUTS: usize = 10_000;

/// Runs `expression` on `json`. Errors say what is wrong, in a sentence.
pub fn filter(json: &str, expression: &str) -> Result<Filtered, String> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Err("the filter is empty".into());
    }
    let language = Language::of(expression);
    let values = match language {
        Language::JsonPath => json_path(json, expression)?,
        Language::Jq => jq(json, expression)?,
    };
    Ok(Filtered { language, values })
}

fn parse(json: &str) -> Result<Value, String> {
    serde_json::from_str(json).map_err(|e| format!("the body is not JSON: {e}"))
}

fn json_path(json: &str, expression: &str) -> Result<Vec<Value>, String> {
    let path = serde_json_path::JsonPath::parse(expression)
        .map_err(|e| format!("not a JSONPath expression: {e}"))?;
    let value = parse(json)?;
    Ok(path.query(&value).all().into_iter().cloned().collect())
}

fn jq(json: &str, expression: &str) -> Result<Vec<Value>, String> {
    let input = jaq_json::read::parse_single(json.as_bytes())
        .map_err(|e| format!("the body is not JSON: {e}"))?;
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());
    let loader = Loader::new(defs);
    let arena = Arena::default();
    let program = File {
        code: expression,
        path: (),
    };
    let modules = loader.load(&arena, program).map_err(|errors| {
        let mut said = Vec::new();
        for (_, error) in errors {
            match error {
                jaq_core::load::Error::Io(items) => {
                    said.extend(items.into_iter().map(|(_, e)| e));
                }
                jaq_core::load::Error::Lex(items) => said.extend(
                    items
                        .into_iter()
                        .map(|(expect, at)| expected(expect.as_str(), at)),
                ),
                jaq_core::load::Error::Parse(items) => said.extend(
                    items
                        .into_iter()
                        .map(|(expect, at)| expected(expect.as_str(), at)),
                ),
            }
        }
        format!("not a jq filter: {}", said.join("; "))
    })?;
    let filter = Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|errors| {
            let said: Vec<String> = errors
                .into_iter()
                .flat_map(|(_, items)| items)
                .map(|(name, undefined)| format!("no {} `{name}`", undefined.as_str()))
                .collect();
            format!("not a jq filter: {}", said.join("; "))
        })?;
    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let mut values = Vec::new();
    for out in filter.id.run((ctx, input)).map(unwrap_valr) {
        if values.len() == MOST_OUTPUTS {
            break;
        }
        let out = out.map_err(|e| format!("the jq filter failed: {e}"))?;
        // jaq writes JSON; what JSON cannot hold (NaN, byte strings) stays text.
        let text = out.to_string();
        values.push(serde_json::from_str(&text).unwrap_or(Value::String(text)));
    }
    Ok(values)
}

/// "expected term at `)`" for a filter that stops making sense there.
fn expected(what: &str, at: &str) -> String {
    let at: String = at.chars().take(20).collect();
    if at.is_empty() {
        format!("expected {what} at the end")
    } else {
        format!("expected {what} at `{at}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"users": [{"id": 1, "name": "Ada", "tags": ["a"]}, {"id": 2, "name": "Lin", "tags": []}], "total": 2}"#;

    #[test]
    fn json_path_lists_its_matches() {
        let f = filter(BODY, "$.users[*].name").unwrap();
        assert_eq!(f.language, Language::JsonPath);
        assert_eq!(f.values, [Value::from("Ada"), Value::from("Lin")]);
        assert_eq!(f.to_pretty(), "[\n  \"Ada\",\n  \"Lin\"\n]");
        let f = filter(BODY, "$.users[?@.id > 1].name").unwrap();
        assert_eq!(f.values, [Value::from("Lin")]);
        assert_eq!(filter(BODY, "$.nothing").unwrap().to_pretty(), "[]");
    }

    #[test]
    fn jq_runs_with_its_standard_library() {
        let f = filter(BODY, ".users | map(.name)").unwrap();
        assert_eq!(f.language, Language::Jq);
        assert_eq!(f.to_pretty(), "[\n  \"Ada\",\n  \"Lin\"\n]");
        let f = filter(BODY, ".users[] | select(.tags | length > 0) | .id").unwrap();
        assert_eq!(f.values, [Value::from(1)]);
        let f = filter(BODY, ".users[].id").unwrap();
        assert_eq!(f.to_pretty(), "1\n2");
        // Keys keep their order.
        let f = filter(BODY, ".users[0]").unwrap();
        assert!(f.to_pretty().starts_with("{\n  \"id\": 1,\n  \"name\""));
    }

    #[test]
    fn mistakes_say_what_is_wrong() {
        assert!(
            filter(BODY, "$.users[")
                .unwrap_err()
                .contains("not a JSONPath")
        );
        let error = filter(BODY, ".users | map(").unwrap_err();
        assert!(error.starts_with("not a jq filter: expected"), "{error}");
        let error = filter(BODY, "nosuchthing").unwrap_err();
        assert!(error.contains("no filter `nosuchthing`"), "{error}");
        let error = filter(BODY, ".total | keys").unwrap_err();
        assert!(error.starts_with("the jq filter failed"), "{error}");
        assert!(filter("not json", ".").unwrap_err().contains("not JSON"));
        assert!(filter(BODY, "  ").is_err());
    }

    #[test]
    fn endless_filters_stop() {
        let f = filter("1", "repeat(.)").unwrap();
        assert_eq!(f.values.len(), MOST_OUTPUTS);
    }
}
