use std::collections::{BTreeSet, HashMap};

use crate::model::{Auth, Body, KeyValue, Request};

/// How deep `{{a}}` -> `{{b}}` -> ... chains are followed before giving up,
/// which also stops self-referencing variables from looping forever.
const MAX_DEPTH: usize = 8;

/// Variables merged from several scopes. Layers added later win, so callers
/// add them from the broadest scope to the narrowest:
/// globals -> collection -> environment -> command-line overrides.
#[derive(Debug, Clone, Default)]
pub struct Variables {
    map: HashMap<String, String>,
    /// Where each value came from, for the UI. Absent for unnamed layers.
    origins: HashMap<String, Origin>,
}

/// The layer that set a variable's current value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    /// Shown to the user, e.g. `Environment "test"`.
    pub scope: String,
    /// The value came from a gitignored `.local.toml` file.
    pub secret: bool,
}

impl Variables {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_layer<'a>(
        mut self,
        layer: impl IntoIterator<Item = (&'a String, &'a String)>,
    ) -> Self {
        for (key, value) in layer {
            self.origins.remove(key);
            self.map.insert(key.clone(), value.clone());
        }
        self
    }

    /// Like [`Self::with_layer`], remembering `scope` as the origin of each value.
    pub fn with_scope<'a>(
        mut self,
        layer: impl IntoIterator<Item = (&'a String, &'a String)>,
        scope: &str,
        secret: bool,
    ) -> Self {
        for (key, value) in layer {
            self.map.insert(key.clone(), value.clone());
            let origin = Origin {
                scope: scope.to_owned(),
                secret,
            };
            self.origins.insert(key.clone(), origin);
        }
        self
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.origins.remove(&key);
        self.map.insert(key, value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(String::as_str)
    }

    pub fn origin(&self, key: &str) -> Option<&Origin> {
        self.origins.get(key)
    }

    /// Replaces every `{{name}}` in `input`. Unknown names are left in place
    /// verbatim and recorded in `missing`, so the UI can highlight them.
    pub fn interpolate(&self, input: &str, missing: &mut BTreeSet<String>) -> String {
        self.interpolate_at(input, missing, 0)
    }

    fn interpolate_at(&self, input: &str, missing: &mut BTreeSet<String>, depth: usize) -> String {
        if !input.contains("{{") {
            return input.to_owned();
        }
        let mut out = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                out.push_str(&rest[start..]);
                return out;
            };
            let name = after[..end].trim();
            // A defined variable named `$x` wins; otherwise `$x` is a dynamic variable.
            let dynamic = || name.strip_prefix('$').and_then(crate::dynamic::generate);
            match self.get(name) {
                Some(value) if depth < MAX_DEPTH => {
                    out.push_str(&self.interpolate_at(value, missing, depth + 1))
                }
                Some(value) => out.push_str(value),
                None => match dynamic() {
                    Some(value) => out.push_str(&value),
                    None => {
                        missing.insert(name.to_owned());
                        out.push_str(&rest[start..start + 2 + end + 2]);
                    }
                },
            }
            rest = &after[end + 2..];
        }
        out.push_str(rest);
        out
    }
}

/// A request with every variable substituted, ready to send.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub request: Request,
    /// Variable names that were referenced but not defined anywhere.
    pub missing: BTreeSet<String>,
}

impl Request {
    pub fn resolve(&self, vars: &Variables) -> Resolved {
        let mut missing = BTreeSet::new();
        let mut s = |input: &str| vars.interpolate(input, &mut missing);

        let request = Request {
            name: self.name.clone(),
            description: self.description.clone(),
            method: self.method,
            url: s(&self.url),
            query: pairs(&self.query, &mut s),
            headers: pairs(&self.headers, &mut s),
            body: match &self.body {
                Body::None => Body::None,
                Body::Json(text) => Body::Json(s(text)),
                Body::Text(text) => Body::Text(s(text)),
                Body::Xml(text) => Body::Xml(s(text)),
                Body::Form(rows) => Body::Form(pairs(rows, &mut s)),
            },
            auth: match &self.auth {
                Auth::None => Auth::None,
                Auth::Bearer { token } => Auth::Bearer { token: s(token) },
                Auth::Basic { username, password } => Auth::Basic {
                    username: s(username),
                    password: s(password),
                },
                Auth::ApiKey {
                    key,
                    value,
                    location,
                } => Auth::ApiKey {
                    key: s(key),
                    value: s(value),
                    location: *location,
                },
            },
        };
        Resolved { request, missing }
    }
}

fn pairs(rows: &[KeyValue], s: &mut impl FnMut(&str) -> String) -> Vec<KeyValue> {
    rows.iter()
        .map(|row| KeyValue {
            key: s(&row.key),
            value: s(&row.value),
            enabled: row.enabled,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Method;

    fn vars(pairs: &[(&str, &str)]) -> Variables {
        let mut v = Variables::new();
        for (k, val) in pairs {
            v.set(*k, *val);
        }
        v
    }

    #[test]
    fn substitutes_and_reports_missing() {
        let v = vars(&[("host", "api.dev"), ("id", "42")]);
        let mut missing = BTreeSet::new();
        let out = v.interpolate("https://{{host}}/users/{{ id }}?t={{token}}", &mut missing);
        assert_eq!(out, "https://api.dev/users/42?t={{token}}");
        assert_eq!(missing.into_iter().collect::<Vec<_>>(), ["token"]);
    }

    #[test]
    fn follows_nested_variables_and_stops_on_cycles() {
        let v = vars(&[
            ("base", "https://{{host}}"),
            ("host", "api.dev"),
            ("loop", "{{loop}}"),
        ]);
        let mut missing = BTreeSet::new();
        assert_eq!(
            v.interpolate("{{base}}/v1", &mut missing),
            "https://api.dev/v1"
        );
        assert_eq!(v.interpolate("{{loop}}", &mut missing), "{{loop}}");
        assert!(missing.is_empty());
    }

    #[test]
    fn fills_dynamic_variables_freshly_each_time() {
        let mut missing = BTreeSet::new();
        let v = vars(&[("$fixed", "kept")]);
        let out = v.interpolate("{{$guid}} {{$guid}} {{$fixed}} {{$nope}}", &mut missing);
        let parts: Vec<&str> = out.split(' ').collect();
        assert_eq!(parts[0].len(), 36);
        assert_ne!(parts[0], parts[1], "every occurrence is new");
        assert_eq!(parts[2], "kept", "a defined variable wins");
        assert_eq!(parts[3], "{{$nope}}");
        assert_eq!(missing.into_iter().collect::<Vec<_>>(), ["$nope"]);
    }

    #[test]
    fn leaves_unterminated_braces_alone() {
        let mut missing = BTreeSet::new();
        assert_eq!(Variables::new().interpolate("a {{b", &mut missing), "a {{b");
    }

    #[test]
    fn later_layers_win() {
        let collection = HashMap::from([("host".to_string(), "prod".to_string())]);
        let env = HashMap::from([("host".to_string(), "staging".to_string())]);
        let v = Variables::new().with_layer(&collection).with_layer(&env);
        assert_eq!(v.get("host"), Some("staging"));
    }

    #[test]
    fn remembers_where_values_came_from() {
        let shared = HashMap::from([("host".to_string(), "a".to_string())]);
        let secrets = HashMap::from([("token".to_string(), "t".to_string())]);
        let mut v = Variables::new()
            .with_scope(&shared, "Collection", false)
            .with_scope(&secrets, "Environment \"dev\"", true);
        assert_eq!(v.origin("host").unwrap().scope, "Collection");
        assert!(v.origin("token").unwrap().secret);
        v.set("host", "override");
        assert!(v.origin("host").is_none(), "an override has no named scope");
    }

    #[test]
    fn resolves_whole_request() {
        let mut req = Request::new(Method::Post, "{{base}}/login");
        req.headers.push(KeyValue::new("X-Env", "{{env}}"));
        req.body = Body::Json(r#"{"user":"{{user}}"}"#.into());
        req.auth = Auth::Bearer {
            token: "{{token}}".into(),
        };

        let v = vars(&[("base", "http://x"), ("env", "dev"), ("user", "ada")]);
        let resolved = req.resolve(&v);
        assert_eq!(resolved.request.url, "http://x/login");
        assert_eq!(resolved.request.headers[0].value, "dev");
        assert_eq!(
            resolved.request.body,
            Body::Json(r#"{"user":"ada"}"#.into())
        );
        assert!(resolved.missing.contains("token"));
    }
}
