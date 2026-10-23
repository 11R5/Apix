//! Runs pre-request and test scripts with QuickJS. Scripts see a
//! Postman-compatible `pm` object (also as `apix`): variables in every scope,
//! the request (pre-request) or the response (tests), `pm.test` and
//! `pm.expect`, `console`, and Postman's older `tests[...]` / `postman.*` API.
//!
//! Every event runs in a fresh QuickJS runtime, so scripts cannot leak into
//! each other or into the next request. Most of the API is JavaScript
//! (`prelude.js`); the context goes in and the results come out as JSON.

use std::time::{Duration, Instant};

use apix_core::scripts::{Event, ScopeValues, ScriptFile, ScriptVars};
use apix_core::{Body, KeyValue, Method, Request};
use rquickjs::context::EvalOptions;
use rquickjs::{Context, Runtime};
use serde::{Deserialize, Serialize};

const PRELUDE: &str = include_str!("prelude.js");

/// How long the scripts of one event may run before they are stopped.
pub const TIME_LIMIT: Duration = Duration::from_secs(10);
const MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// A response as test scripts see it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ResponseView {
    pub code: u16,
    pub status: String,
    #[serde(serialize_with = "header_rows")]
    pub headers: Vec<(String, String)>,
    /// The body as text (lossy for binary bodies).
    pub body: String,
    /// Milliseconds.
    #[serde(rename = "responseTime")]
    pub time_ms: u64,
    #[serde(rename = "responseSize")]
    pub size: usize,
}

impl ResponseView {
    pub fn new(
        code: u16,
        status: &str,
        headers: &[(String, String)],
        body: &[u8],
        time: Duration,
    ) -> Self {
        Self {
            code,
            status: status.to_owned(),
            headers: headers.to_vec(),
            body: String::from_utf8_lossy(body).into_owned(),
            time_ms: time.as_millis() as u64,
            size: body.len(),
        }
    }
}

/// Headers as the `{ key, value }` rows the prelude's lists expect.
fn header_rows<S: serde::Serializer>(
    headers: &[(String, String)],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(headers.iter().map(|(key, value)| JsRow {
        key: key.clone(),
        value: value.clone(),
        disabled: false,
    }))
}

/// What `pm.info` tells a script.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Info {
    pub request_name: String,
    /// Where the request is, for `pm.info.requestId`.
    pub request_id: String,
    /// 0-based, as in Postman.
    pub iteration: u32,
    pub iteration_count: u32,
}

/// Everything the scripts of one event work with.
#[derive(Debug, Clone)]
pub struct Input<'a> {
    pub event: Event,
    /// The request before variables are filled in; pre-request scripts may change it.
    pub request: &'a Request,
    /// Only for test scripts.
    pub response: Option<&'a ResponseView>,
    pub scopes: &'a ScopeValues,
    /// What earlier scripts set; scripts change it further.
    pub vars: &'a ScriptVars,
    pub info: Info,
}

/// One `pm.test`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    #[serde(default)]
    pub skipped: bool,
    #[serde(default)]
    pub error: Option<String>,
}

/// One `console.*` line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogLine {
    /// `log`, `info`, `warn`, `error` or `debug`.
    pub level: String,
    pub text: String,
}

/// What the scripts of one event did.
#[derive(Debug, Clone, Default)]
pub struct Output {
    /// The request after pre-request scripts (unchanged for tests).
    pub request: Request,
    pub vars: ScriptVars,
    pub tests: Vec<TestResult>,
    pub logs: Vec<LogLine>,
    /// A script threw or ran too long: which file, and why. Scripts after it
    /// did not run.
    pub error: Option<String>,
    /// `pm.execution.setNextRequest`: `Some(None)` stops a run.
    pub next_request: Option<Option<String>>,
    /// `pm.execution.skipRequest()` in a pre-request script.
    pub skip_request: bool,
}

impl Output {
    pub fn failed_tests(&self) -> usize {
        self.tests.iter().filter(|t| !t.passed).count()
    }
}

// ----------------------------------------------------------------- JSON

#[derive(Serialize, Deserialize)]
struct JsRow {
    key: String,
    value: String,
    #[serde(default)]
    disabled: bool,
}

#[derive(Serialize, Deserialize)]
struct JsBody {
    mode: String,
    #[serde(default)]
    raw: String,
    #[serde(default)]
    urlencoded: Vec<JsRow>,
}

#[derive(Serialize, Deserialize)]
struct JsRequest {
    method: String,
    url: String,
    query: Vec<JsRow>,
    headers: Vec<JsRow>,
    body: JsBody,
}

#[derive(Serialize)]
struct JsInput<'a> {
    event: &'static str,
    info: &'a Info,
    request: JsRequest,
    response: Option<&'a ResponseView>,
    scopes: &'a ScopeValues,
    state: &'a ScriptVars,
}

#[derive(Deserialize)]
struct NextRequest {
    set: bool,
    name: Option<String>,
}

#[derive(Deserialize)]
struct JsOutput {
    request: JsRequest,
    state: ScriptVars,
    tests: Vec<TestResult>,
    logs: Vec<LogLine>,
    next_request: NextRequest,
    skip_request: bool,
}

fn rows(list: &[KeyValue]) -> Vec<JsRow> {
    list.iter()
        .map(|kv| JsRow {
            key: kv.key.clone(),
            value: kv.value.clone(),
            disabled: !kv.enabled,
        })
        .collect()
}

fn key_values(rows: Vec<JsRow>) -> Vec<KeyValue> {
    rows.into_iter()
        .map(|r| KeyValue {
            key: r.key,
            value: r.value,
            enabled: !r.disabled,
        })
        .collect()
}

fn to_js(request: &Request) -> JsRequest {
    let (mode, raw, urlencoded) = match &request.body {
        Body::None => ("none", String::new(), Vec::new()),
        Body::Json(t) | Body::Text(t) | Body::Xml(t) | Body::Html(t) => {
            ("raw", t.clone(), Vec::new())
        }
        Body::GraphQl(g) => ("graphql", g.query.clone(), Vec::new()),
        Body::Form(list) => ("urlencoded", String::new(), rows(list)),
        Body::Multipart(_) => ("formdata", String::new(), Vec::new()),
        Body::Binary { .. } => ("file", String::new(), Vec::new()),
    };
    JsRequest {
        method: request.method.as_str().to_owned(),
        url: request.url.clone(),
        query: rows(&request.query),
        headers: rows(&request.headers),
        body: JsBody {
            mode: mode.into(),
            raw,
            urlencoded,
        },
    }
}

/// Puts what a script changed back into `request`. Bodies a script cannot
/// edit (multipart, binary, GraphQL) stay as they were.
fn from_js(mut request: Request, js: JsRequest) -> Request {
    if let Some(method) = Method::ALL
        .iter()
        .find(|m| m.as_str().eq_ignore_ascii_case(&js.method))
    {
        request.method = *method;
    }
    request.url = js.url;
    request.query = key_values(js.query);
    request.headers = key_values(js.headers);
    request.body = match (request.body, js.body.mode.as_str()) {
        (Body::Json(_), "raw") => Body::Json(js.body.raw),
        (Body::Text(_), "raw") => Body::Text(js.body.raw),
        (Body::Xml(_), "raw") => Body::Xml(js.body.raw),
        (Body::Html(_), "raw") => Body::Html(js.body.raw),
        (Body::None, "raw") => Body::Text(js.body.raw),
        (Body::Form(_), "urlencoded") => Body::Form(key_values(js.body.urlencoded)),
        (body, _) => body,
    };
    request
}

// ------------------------------------------------------------------ run

/// Runs `scripts` in order for one event. An error stops the rest.
pub fn run(scripts: &[ScriptFile], input: &Input) -> Output {
    run_within(scripts, input, TIME_LIMIT)
}

fn run_within(scripts: &[ScriptFile], input: &Input, limit: Duration) -> Output {
    let mut output = Output {
        request: input.request.clone(),
        vars: input.vars.clone(),
        ..Output::default()
    };
    if scripts.is_empty() {
        return output;
    }
    let js_input = JsInput {
        event: input.event.suffix(),
        info: &input.info,
        request: to_js(input.request),
        response: input.response,
        scopes: input.scopes,
        state: input.vars,
    };
    let json = match serde_json::to_string(&js_input) {
        Ok(json) => json,
        Err(e) => {
            output.error = Some(e.to_string());
            return output;
        }
    };
    match execute(scripts, &json, limit) {
        Ok((js, error)) => {
            output.request = from_js(output.request, js.request);
            output.vars = js.state;
            output.tests = js.tests;
            output.logs = js.logs;
            output.next_request = js.next_request.set.then_some(js.next_request.name);
            output.skip_request = js.skip_request;
            output.error = error;
        }
        Err(error) => output.error = Some(error),
    }
    output
}

fn file_name(script: &ScriptFile) -> String {
    script
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "script".into())
}

/// Evaluates the prelude and the scripts; returns what the prelude collected
/// and the first script error, if any.
fn execute(
    scripts: &[ScriptFile],
    input: &str,
    limit: Duration,
) -> Result<(JsOutput, Option<String>), String> {
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(MEMORY_LIMIT);
    runtime.set_max_stack_size(1024 * 1024);
    let deadline = Instant::now() + limit;
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;

    context.with(|ctx| {
        let describe = |ctx: &rquickjs::Ctx, error: rquickjs::Error| -> String {
            if !matches!(error, rquickjs::Error::Exception) {
                return error.to_string();
            }
            let thrown = ctx.catch();
            if let Some(exception) = thrown.as_exception() {
                let message = exception.message().unwrap_or_default();
                let at = exception.stack().and_then(|s| {
                    s.lines()
                        .find(|l| l.contains(".js:"))
                        .map(str::trim)
                        .map(str::to_owned)
                });
                return match at {
                    Some(at) => format!("{message} ({at})"),
                    None => message,
                };
            }
            if Instant::now() > deadline {
                return format!("stopped after {} seconds", limit.as_secs_f32());
            }
            thrown
                .as_string()
                .and_then(|s| s.to_string().ok())
                .unwrap_or_else(|| format!("{thrown:?}"))
        };

        ctx.globals()
            .set("__apix_input", input)
            .map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(PRELUDE)
            .map_err(|e| format!("prelude: {}", describe(&ctx, e)))?;

        let mut error = None;
        for script in scripts {
            let name = file_name(script);
            let mut options = EvalOptions::default();
            options.global = true;
            options.filename = Some(name.clone());
            // A function body, so `return` works and names stay per script.
            let wrapped = format!("(function () {{\n{}\n}})();", script.source);
            if let Err(e) = ctx.eval_with_options::<(), _>(wrapped, options) {
                error = Some(format!("{name}: {}", describe(&ctx, e)));
                break;
            }
            // Settled promises (`.then` callbacks) run before the next script.
            while ctx.execute_pending_job() {}
        }
        // The deadline is for scripts; reading the results must not trip it.
        let output: String = ctx
            .eval("__apix_output()")
            .map_err(|e| format!("results: {}", describe(&ctx, e)))?;
        let parsed: JsOutput = serde_json::from_str(&output).map_err(|e| e.to_string())?;
        Ok((parsed, error))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn script(name: &str, source: &str) -> ScriptFile {
        ScriptFile {
            path: PathBuf::from(name),
            source: source.into(),
        }
    }

    fn scopes() -> ScopeValues {
        ScopeValues {
            environment: [("base".to_string(), "https://x.dev".to_string())].into(),
            collection: [("team".to_string(), "core".to_string())].into(),
            environment_name: Some("dev".into()),
            ..ScopeValues::default()
        }
    }

    fn pre(request: &Request, sources: &[ScriptFile]) -> Output {
        let scopes = scopes();
        let vars = ScriptVars::default();
        run(
            sources,
            &Input {
                event: Event::PreRequest,
                request,
                response: None,
                scopes: &scopes,
                vars: &vars,
                info: Info::default(),
            },
        )
    }

    fn test(sources: &[ScriptFile], response: &ResponseView) -> Output {
        let scopes = scopes();
        let vars = ScriptVars::default();
        let request = Request::new(Method::Get, "{{base}}/users");
        run(
            sources,
            &Input {
                event: Event::Test,
                request: &request,
                response: Some(response),
                scopes: &scopes,
                vars: &vars,
                info: Info::default(),
            },
        )
    }

    fn json_response() -> ResponseView {
        ResponseView {
            code: 200,
            status: "OK".into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: r#"{"users":[{"id":1,"name":"Ada"}],"total":1}"#.into(),
            time_ms: 42,
            size: 44,
        }
    }

    #[test]
    fn pre_request_scripts_change_variables_and_the_request() {
        let request = Request::new(Method::Get, "{{base}}/users");
        let out = pre(
            &request,
            &[
                script(
                    "_collection.prerequest.js",
                    "pm.environment.set('token', 'abc' + 1);",
                ),
                script(
                    "get.prerequest.js",
                    r#"
                    pm.request.headers.upsert({ key: 'Authorization', value: 'Bearer ' + pm.environment.get('token') });
                    pm.request.url.query.add({ key: 'page', value: 2 });
                    pm.request.method = 'post';
                    pm.variables.set('local', pm.variables.replaceIn('{{team}}-{{base}}'));
                    pm.collectionVariables.unset('team');
                    console.log('ready', { n: 1 });
                    "#,
                ),
            ],
        );
        assert_eq!(out.error, None);
        assert_eq!(out.vars.environment["token"], Some("abc1".into()));
        assert_eq!(out.vars.local["local"], Some("core-https://x.dev".into()));
        assert_eq!(out.vars.collection["team"], None);
        assert_eq!(out.request.method, Method::Post);
        assert_eq!(out.request.headers[0].value, "Bearer abc1");
        assert_eq!(out.request.query[0].key, "page");
        assert_eq!(out.request.query[0].value, "2");
        assert_eq!(out.logs[0].text, r#"ready {"n":1}"#);
    }

    #[test]
    fn tests_pass_and_fail_with_chai_messages() {
        let out = test(
            &[script(
                "get.test.js",
                r#"
                pm.test('status', () => pm.response.to.have.status(200));
                pm.test('ok', () => { pm.response.to.be.ok; });
                pm.test('json', () => {
                    const body = pm.response.json();
                    pm.expect(body.users).to.be.an('array').that.has.lengthOf(1);
                    pm.expect(body.users[0]).to.have.property('name', 'Ada');
                    pm.expect(body).to.deep.include({ total: 1 });
                    pm.expect(pm.response.responseTime).to.be.below(500);
                    pm.expect(body.users[0].name).to.match(/^A/).and.not.equal('Bob');
                });
                pm.test('fails', () => pm.expect(pm.response.code).to.equal(201));
                pm.test('header', () => pm.response.to.have.header('content-type', 'application/json'));
                pm.test('schema', () => pm.response.to.have.jsonSchema({ type: 'object', required: ['users', 'missing'] }));
                pm.test.skip('later');
                tests['legacy'] = responseCode.code === 200;
                "#,
            )],
            &json_response(),
        );
        assert_eq!(out.error, None);
        let results: Vec<(&str, bool)> = out
            .tests
            .iter()
            .map(|t| (t.name.as_str(), t.passed))
            .collect();
        assert_eq!(
            results,
            [
                ("status", true),
                ("ok", true),
                ("json", true),
                ("fails", false),
                ("header", true),
                ("schema", false),
                ("later", true),
                ("legacy", true),
            ]
        );
        assert_eq!(
            out.tests[3].error.as_deref(),
            Some("expected 200 to equal 201")
        );
        assert_eq!(
            out.tests[5].error.as_deref(),
            Some("expected response body to match the schema: missing is required")
        );
        assert!(out.tests[6].skipped);
        assert_eq!(out.failed_tests(), 2);
    }

    #[test]
    fn an_error_names_the_file_and_stops_the_rest() {
        let request = Request::new(Method::Get, "x");
        let out = pre(
            &request,
            &[
                script("first.js", "pm.environment.set('a', 1);"),
                script("broken.prerequest.js", "\nundefinedThing();"),
                script("never.js", "pm.environment.set('b', 2);"),
            ],
        );
        let error = out.error.unwrap();
        assert!(error.starts_with("broken.prerequest.js: "), "{error}");
        assert!(error.contains("undefinedThing"), "{error}");
        assert_eq!(
            out.vars.environment.get("a"),
            Some(&Some("1".into())),
            "kept"
        );
        assert!(!out.vars.environment.contains_key("b"));
    }

    #[test]
    fn endless_scripts_are_stopped() {
        let scopes = scopes();
        let vars = ScriptVars::default();
        let request = Request::new(Method::Get, "x");
        let started = Instant::now();
        let out = run_within(
            &[script("loop.js", "while (true) {}")],
            &Input {
                event: Event::PreRequest,
                request: &request,
                response: None,
                scopes: &scopes,
                vars: &vars,
                info: Info::default(),
            },
            Duration::from_millis(200),
        );
        assert!(out.error.is_some());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn next_request_skip_and_base64() {
        let request = Request::new(Method::Get, "x");
        let out = pre(
            &request,
            &[script(
                "a.js",
                "pm.execution.setNextRequest(null); pm.execution.skipRequest(); pm.variables.set('b', btoa('hi:there')); pm.variables.set('c', atob('aGk='));",
            )],
        );
        assert_eq!(out.next_request, Some(None));
        assert!(out.skip_request);
        assert_eq!(out.vars.local["b"], Some("aGk6dGhlcmU=".into()));
        assert_eq!(out.vars.local["c"], Some("hi".into()));
    }
}
