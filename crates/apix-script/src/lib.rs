//! Runs pre-request and test scripts with QuickJS. Scripts see a
//! Postman-compatible `pm` object (also as `apix`): variables in every scope,
//! the request (pre-request) or the response (tests), `pm.test` and
//! `pm.expect`, `console`, and Postman's older `tests[...]` / `postman.*` API.
//!
//! Every event runs in a fresh QuickJS runtime, so scripts cannot leak into
//! each other or into the next request. Most of the API is JavaScript
//! (`prelude.js`); the context goes in and the results come out as JSON.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use apix_core::scripts::{Event, ScopeValues, ScriptFile, ScriptVars};
use apix_core::{Body, KeyValue, Method, Request};
use rquickjs::context::EvalOptions;
use rquickjs::{Context, Function, Runtime};
use serde::{Deserialize, Serialize};

const PRELUDE: &str = include_str!("prelude.js");
/// What `require(...)` offers: subsets of crypto-js, lodash, moment, uuid,
/// ajv, tv4 and chai, written for Apix.
const LIBRARIES: &str = include_str!("libs.js");

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

/// Sends a request for `pm.sendRequest`, blocking until the response is in.
/// The caller decides how (its HTTP client, its runtime); scripts only wait.
#[derive(Clone)]
pub struct Sender(pub Arc<SendFn>);

type SendFn = dyn Fn(&Request) -> Result<ResponseView, String> + Send + Sync;

impl Sender {
    pub fn new(
        send: impl Fn(&Request) -> Result<ResponseView, String> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(send))
    }
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sender")
    }
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
    /// For `pm.sendRequest`; without one it fails with an error.
    pub sender: Option<Sender>,
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

// ------------------------------------------------------------- natives

fn from_hex(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .filter_map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `crypto-js` digests, done in Rust: `alg` over hex-encoded `data`; with a
/// `key`, the HMAC. Unknown algorithms give an empty string.
fn digest(alg: &str, data: &[u8], key: Option<&[u8]>) -> Option<Vec<u8>> {
    use hmac::{Hmac, Mac};
    use sha2::Digest;
    macro_rules! hash {
        ($h:ty) => {
            match key {
                None => <$h>::digest(data).to_vec(),
                Some(key) => {
                    let mut mac = <Hmac<$h> as Mac>::new_from_slice(key).ok()?;
                    mac.update(data);
                    mac.finalize().into_bytes().to_vec()
                }
            }
        };
    }
    Some(match alg {
        "MD5" => hash!(md5::Md5),
        "SHA1" => hash!(sha1::Sha1),
        "SHA224" => hash!(sha2::Sha224),
        "SHA256" => hash!(sha2::Sha256),
        "SHA384" => hash!(sha2::Sha384),
        "SHA512" => hash!(sha2::Sha512),
        _ => return None,
    })
}

/// `__apix_hash`, `__apix_hmac` and `__apix_random` for the libraries.
fn add_natives(ctx: &rquickjs::Ctx) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    globals.set(
        "__apix_hash",
        Function::new(ctx.clone(), |alg: String, data: String| {
            digest(&alg, &from_hex(&data), None)
                .map(|d| to_hex(&d))
                .unwrap_or_default()
        })?,
    )?;
    globals.set(
        "__apix_hmac",
        Function::new(ctx.clone(), |alg: String, data: String, key: String| {
            digest(&alg, &from_hex(&data), Some(&from_hex(&key)))
                .map(|d| to_hex(&d))
                .unwrap_or_default()
        })?,
    )?;
    globals.set(
        "__apix_random",
        Function::new(ctx.clone(), |count: u32| {
            use rand_core::RngCore;
            let mut bytes = vec![0u8; count.min(1024) as usize];
            rand_core::OsRng.fill_bytes(&mut bytes);
            to_hex(&bytes)
        })?,
    )?;
    Ok(())
}

/// A request a script built for `pm.sendRequest`.
fn request_from_js(js: JsRequest) -> Request {
    let body = match js.body.mode.as_str() {
        "raw" => Body::Text(js.body.raw.clone()),
        "urlencoded" => Body::Form(Vec::new()),
        _ => Body::None,
    };
    let base = Request {
        body,
        ..Request::new(Method::Get, "")
    };
    from_js(base, js)
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
    match execute(scripts, &json, limit, input.sender.clone()) {
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
    sender: Option<Sender>,
) -> Result<(JsOutput, Option<String>), String> {
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    runtime.set_memory_limit(MEMORY_LIMIT);
    runtime.set_max_stack_size(1024 * 1024);
    let started = Instant::now();
    // Time spent waiting for `pm.sendRequest` does not count against the limit.
    let waited_ms = Arc::new(AtomicU64::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let over = {
        let (waited_ms, finished) = (waited_ms.clone(), finished.clone());
        move || {
            !finished.load(Ordering::Relaxed)
                && started.elapsed()
                    > limit + Duration::from_millis(waited_ms.load(Ordering::Relaxed))
        }
    };
    let timed_out = over.clone();
    runtime.set_interrupt_handler(Some(Box::new(over)));
    let context = Context::full(&runtime).map_err(|e| e.to_string())?;

    context.with(|ctx| {
        let send = Function::new(ctx.clone(), move |spec: String| -> String {
            let Some(sender) = &sender else {
                return r#"{"error":"pm.sendRequest is not available here"}"#.into();
            };
            let began = Instant::now();
            let result = serde_json::from_str::<JsRequest>(&spec)
                .map_err(|e| e.to_string())
                .and_then(|js| (sender.0)(&request_from_js(js)));
            waited_ms.fetch_add(began.elapsed().as_millis() as u64, Ordering::Relaxed);
            let reply = match result {
                Ok(response) => serde_json::json!({ "response": response }),
                Err(error) => serde_json::json!({ "error": error }),
            };
            reply.to_string()
        })
        .map_err(|e| e.to_string())?;
        ctx.globals()
            .set("__apix_send", send)
            .map_err(|e| e.to_string())?;
        let timed_out = &timed_out;
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
            if timed_out() {
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
        add_natives(&ctx).map_err(|e| e.to_string())?;
        ctx.eval::<(), _>(PRELUDE)
            .map_err(|e| format!("prelude: {}", describe(&ctx, e)))?;
        ctx.eval::<(), _>(LIBRARIES)
            .map_err(|e| format!("libraries: {}", describe(&ctx, e)))?;

        let mut error = None;
        for script in scripts {
            let name = file_name(script);
            let mut options = EvalOptions::default();
            options.global = true;
            options.filename = Some(name.clone());
            // An async function body: `return` and `await` work, names stay per script.
            let wrapped = format!("__apix_run(async function () {{\n{}\n}});", script.source);
            if let Err(e) = ctx.eval_with_options::<(), _>(wrapped, options) {
                error = Some(format!("{name}: {}", describe(&ctx, e)));
                break;
            }
            // Settled promises (`.then` callbacks, `await`) run before the next script.
            while ctx.execute_pending_job() {}
            let thrown: Option<String> = ctx
                .eval("__apix_error()")
                .map_err(|e| format!("results: {}", describe(&ctx, e)))?;
            if let Some(thrown) = thrown {
                let thrown = if timed_out() {
                    format!("stopped after {} seconds", limit.as_secs_f32())
                } else {
                    thrown
                };
                error = Some(format!("{name}: {thrown}"));
                break;
            }
        }
        // The limit is for scripts; reading the results must not trip it.
        finished.store(true, Ordering::Relaxed);
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
                sender: None,
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
                sender: None,
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
                sender: None,
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

    #[test]
    fn send_request_calls_back_and_awaits() {
        let scopes = scopes();
        let vars = ScriptVars::default();
        let request = Request::new(Method::Get, "x");
        let sender = Sender::new(|req: &Request| {
            // Slower than the limit below: waiting must not count.
            std::thread::sleep(Duration::from_millis(300));
            let header = req
                .headers
                .first()
                .map(|h| h.value.clone())
                .unwrap_or_default();
            Ok(ResponseView {
                code: 200,
                status: "OK".into(),
                headers: vec![("X-Echo".into(), header)],
                body: format!(
                    r#"{{"method":"{}","url":"{}","q":"{}"}}"#,
                    req.method.as_str(),
                    req.url,
                    req.query
                        .first()
                        .map(|q| q.value.clone())
                        .unwrap_or_default()
                ),
                time_ms: 1,
                size: 0,
            })
        });
        let out = run_within(
            &[script(
                "token.prerequest.js",
                r#"
                pm.sendRequest({ url: 'https://auth.dev/token?scope=all', method: 'post', header: { 'X-Key': 'k1' }, body: { mode: 'raw', raw: '{}' } }, (err, res) => {
                    pm.environment.set('first', res.json().method + ' ' + res.json().url + ' ' + res.json().q + ' ' + res.headers.get('x-echo'));
                });
                const res = await pm.sendRequest('https://auth.dev/me');
                pm.environment.set('second', res.code);
                await pm.sendRequest('https://auth.dev/again');
                throw new Error('after await');
                "#,
            )],
            &Input {
                event: Event::PreRequest,
                request: &request,
                response: None,
                scopes: &scopes,
                vars: &vars,
                info: Info::default(),
                sender: Some(sender),
            },
            Duration::from_millis(500),
        );
        assert_eq!(
            out.vars.environment["first"],
            Some("POST https://auth.dev/token all k1".into())
        );
        assert_eq!(out.vars.environment["second"], Some("200".into()));
        let error = out.error.unwrap();
        assert!(
            error.starts_with("token.prerequest.js: after await"),
            "{error}"
        );
    }

    #[test]
    fn without_a_sender_send_request_reports_an_error() {
        let request = Request::new(Method::Get, "x");
        let out = pre(
            &request,
            &[script(
                "a.js",
                "pm.sendRequest('https://x.dev', (err) => pm.variables.set('e', err.message));",
            )],
        );
        assert_eq!(
            out.vars.local["e"],
            Some("pm.sendRequest is not available here".into())
        );
    }

    #[test]
    fn built_in_libraries_work() {
        let request = Request::new(Method::Get, "x");
        let out = pre(
            &request,
            &[script(
                "libs.js",
                r#"
                const CryptoJS = require('crypto-js');
                const set = (k, v) => pm.variables.set(k, v);
                set('md5', CryptoJS.MD5('abc').toString());
                set('sha256', CryptoJS.SHA256('abc').toString(CryptoJS.enc.Hex));
                const mac = CryptoJS.HmacSHA256('The quick brown fox jumps over the lazy dog', 'key');
                set('hmac', mac.toString());
                set('hmac64', CryptoJS.enc.Base64.stringify(mac));
                set('utf8', CryptoJS.enc.Utf8.stringify(CryptoJS.enc.Base64.parse(CryptoJS.enc.Base64.stringify(CryptoJS.enc.Utf8.parse('çay ☕')))));
                set('uuid', require('uuid').validate(require('uuid').v4()));
                const _ = require('lodash');
                set('lodash', _.get({ a: [{ b: 7 }] }, 'a[0].b') + ',' + _.uniq([1, 1, 2]).length + ',' + _.sortBy([{ n: 2 }, { n: 1 }], 'n')[0].n + ',' + _.camelCase('hello big world'));
                const moment = require('moment');
                set('moment', moment.utc('2026-10-23T08:09:10Z').add(1, 'day').format('YYYY-MM-DD HH:mm [at] ddd'));
                set('diff', moment.utc('2026-10-23T00:00:00Z').diff(moment.utc('2026-10-20T12:00:00Z'), 'days'));
                const Ajv = require('ajv');
                const check = new Ajv().compile({ type: 'object', required: ['id'], properties: { id: { type: 'integer', minimum: 1 }, email: { type: 'string', format: 'email' } }, additionalProperties: false });
                set('ajv', check({ id: 1, email: 'a@b.co' }) + ',' + check({ id: 0, extra: true }) + ':' + check.errors.length);
                set('tv4', tv4.validate([1, 2], { type: 'array', minItems: 3 }) + ':' + tv4.error.message);
                const { assert } = require('chai');
                assert.deepEqual({ a: 1 }, { a: 1 });
                set('legacy', CryptoJS === require('crypto-js') && _.isEqual(_.pick({ a: 1, b: 2 }, 'a'), { a: 1 }));
                try { require('fs'); } catch (e) { set('missing', e.message.slice(0, 30)); }
                "#,
            )],
        );
        assert_eq!(out.error, None);
        let get = |k: &str| out.vars.local[k].clone().unwrap();
        assert_eq!(get("md5"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            get("sha256"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            get("hmac"),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
        assert_eq!(
            get("hmac64"),
            "97yD9DBThCSxMpjmqm+xQ+9NWaFJRhdZl0edvC0aPNg="
        );
        assert_eq!(get("utf8"), "çay ☕");
        assert_eq!(get("uuid"), "true");
        assert_eq!(get("lodash"), "7,2,1,helloBigWorld");
        assert_eq!(get("moment"), "2026-10-24 08:09 at Sat");
        assert_eq!(get("diff"), "2");
        assert_eq!(get("ajv"), "true,false:2");
        assert_eq!(get("tv4"), "false:the value should have at least 3 items");
        assert_eq!(get("legacy"), "true");
        assert_eq!(get("missing"), "require('fs'): not built into ");
    }
}
