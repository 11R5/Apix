use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use apix_core::vfs::{self, KdfCost, Vault};
use apix_core::{Collection, Variables};
use apix_http::{HttpClient, Response, human_size};
use apix_runner::Scope;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "apix", version, about = "Blazingly fast API client")]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Environment variable holding the password of an encrypted collection
    /// (asked for on the terminal when it is not set)
    #[arg(
        long,
        global = true,
        value_name = "VAR",
        default_value = "APIX_PASSWORD"
    )]
    password_env: String,
}

#[derive(Subcommand)]
enum Command {
    /// Send a single request file and print the response
    Send {
        file: PathBuf,
        #[command(flatten)]
        vars: VarArgs,
        /// Print response headers too
        #[arg(short = 'i', long)]
        include_headers: bool,
        /// Print what happened on the network: DNS, connection, redirects, TLS, local side
        #[arg(short = 'n', long)]
        network: bool,
        /// WebSocket, Socket.IO and MQTT: send this text instead of the saved messages (repeatable)
        #[arg(short = 'm', long = "message", value_name = "TEXT")]
        messages: Vec<String>,
        /// WebSocket, Socket.IO and MQTT: how --message texts are read: text, or bytes as base64 or hex
        #[arg(long = "format", value_name = "FORMAT", default_value = "text")]
        message_format: apix_core::MessageFormat,
        /// Server-sent events: print only events of this type (repeatable). Socket.IO: the
        /// event --message texts are emitted as (default `message`)
        #[arg(long = "event", value_name = "TYPE")]
        event_types: Vec<String>,
        /// Server-sent events: when the stream ends or drops, connect again after the
        /// server's retry time, sending Last-Event-ID (Ctrl+C stops)
        #[arg(long)]
        reconnect: bool,
        /// MQTT: subscribe to this topic too, at --qos (repeatable)
        #[arg(long = "subscribe", value_name = "TOPIC")]
        subscribe: Vec<String>,
        /// MQTT: the topic --message texts are published to
        #[arg(long, value_name = "TOPIC")]
        topic: Option<String>,
        /// MQTT: the quality of service of --message and --subscribe: 0, 1 or 2
        #[arg(long, value_name = "QOS", default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=2))]
        qos: u8,
        /// MQTT: ask the broker to keep --message texts for later subscribers
        #[arg(long)]
        retain: bool,
        /// WebSocket, Socket.IO and MQTT: how long to keep listening after sending, in seconds (0: until the server closes)
        #[arg(long, value_name = "SECONDS", default_value_t = 3.0)]
        listen: f64,
        #[command(flatten)]
        net: NetArgs,
    },
    /// Send every request in a collection (or a folder of it) in sidebar
    /// order; exits non-zero if any fails
    Run {
        /// The collection, or a folder or request inside it
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[command(flatten)]
        vars: VarArgs,
        /// How many times to send the requests (default: once per data row, else once)
        #[arg(short = 'n', long)]
        iterations: Option<u32>,
        /// A CSV or JSON file; each row is one iteration and its columns are variables
        #[arg(short = 'd', long, value_name = "FILE")]
        data: Option<PathBuf>,
        /// Wait this many milliseconds between requests
        #[arg(long, value_name = "MS", default_value_t = 0)]
        delay: u64,
        /// Stop at the first request that fails
        #[arg(long)]
        bail: bool,
        /// Write a JUnit XML report (for CI test views)
        #[arg(long, value_name = "FILE")]
        junit: Option<PathBuf>,
        /// Write everything that happened as JSON
        #[arg(long, value_name = "FILE")]
        json: Option<PathBuf>,
        /// Write a report page that opens in any browser
        #[arg(long, value_name = "FILE")]
        html: Option<PathBuf>,
        #[command(flatten)]
        net: NetArgs,
    },
    /// Print a request as a shell command (curl for bash or fish, curl.exe,
    /// PowerShell, HTTPie, wget) or as code (fetch, axios, Python, Go, Rust, C#, Java, PHP, Swift, Kotlin)
    #[command(visible_alias = "code")]
    Shell {
        file: PathBuf,
        /// What to write
        #[arg(long = "as", value_enum, default_value_t = FormatArg::Bash)]
        format: FormatArg,
        /// Keep {{variables}} instead of filling them in
        #[arg(long)]
        raw: bool,
        #[command(flatten)]
        vars: VarArgs,
    },
    /// Fetch a GraphQL request's schema from its server (with the request's
    /// URL, headers and auth) and print it as SDL
    Schema {
        file: PathBuf,
        /// Print the server's introspection JSON instead of SDL
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        vars: VarArgs,
        #[command(flatten)]
        net: NetArgs,
    },
    /// Make a gRPC definition from what you have: protobuf text, a descriptor
    /// set, generated code, types in a programming language or a JSON example.
    /// Prints it as .proto; what it had to choose goes to stderr
    Proto {
        /// A file, or `-` for standard input
        source: PathBuf,
        /// What to call, as package.Service/Method, when the source does not say
        #[arg(long)]
        method: Option<String>,
        /// An example of the answer, or its type, when the source is the request
        #[arg(long, value_name = "FILE")]
        response: Option<PathBuf>,
        /// Write it to this file instead of printing it
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Keep it in this request's [grpc] table (and its method, if given)
        #[arg(long, value_name = "REQUEST")]
        into: Option<PathBuf>,
    },
    /// Import a Postman collection or environment, an Insomnia v4 export, an
    /// OpenAPI 3 / Swagger 2 spec (JSON or YAML), a HAR file or a WSDL; or a
    /// whole folder / "Export data" zip of them as a project
    Import {
        /// An export file or its http(s) address, a folder of them, or a `.zip`
        file: PathBuf,
        /// Folder to create for a collection (default: ./<collection-name>);
        /// for a folder or zip, the folder to create the project in (default: .)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Collection to add an environment to (default: the collection around the current folder)
        #[arg(long)]
        into: Option<PathBuf>,
    },
    /// Export a collection as Postman v2.1 files (collection and environments)
    Export {
        /// The collection folder (or a folder inside it)
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Where to write the files (default: .)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Make an encrypted copy of a collection; the original is left as it is
    Encrypt {
        /// The plain collection folder
        dir: PathBuf,
        /// Where to create the encrypted collection (default: <dir>-encrypted)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Write a decrypted copy of an encrypted collection
    Decrypt {
        /// The encrypted collection folder
        dir: PathBuf,
        /// Where to write the plain files (must not exist yet)
        #[arg(long)]
        out: PathBuf,
        /// Unlock with the recovery key instead of the password
        #[arg(long)]
        recovery: bool,
    },
}

/// The password from `$var`, or asked for on the terminal.
fn password(var: &str, prompt: &str, confirm: bool) -> Result<String> {
    if let Ok(value) = std::env::var(var) {
        return Ok(value);
    }
    if !std::io::stdin().is_terminal() {
        bail!("this collection is encrypted: set {var} to its password");
    }
    let first = rpassword::prompt_password(prompt)?;
    if confirm && rpassword::prompt_password("Repeat the password: ")? != first {
        bail!("the passwords do not match");
    }
    Ok(first)
}

/// Unlocks the encrypted collection around `path`, if there is one.
fn unlock_around(path: &Path, var: &str) -> Result<()> {
    let Some(root) = vfs::encrypted_root(path) else {
        return Ok(());
    };
    if vfs::is_unlocked(&root) {
        return Ok(());
    }
    // The desktop app may have remembered it; the variable, when set, wins.
    if std::env::var(var).is_err()
        && let Some(saved) = vfs::keychain::load(&root)
        && let Ok(vault) = Vault::unlock(&root, &saved)
    {
        vfs::unlock(vault);
        return Ok(());
    }
    let pw = password(var, &format!("Password for {}: ", root.display()), false)?;
    let vault =
        Vault::unlock(&root, &pw).with_context(|| format!("unlocking {}", root.display()))?;
    vfs::unlock(vault);
    Ok(())
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum FormatArg {
    Bash,
    Cmd,
    Powershell,
    Fish,
    Httpie,
    Wget,
    Fetch,
    Axios,
    Python,
    Go,
    Rust,
    Csharp,
    Java,
    Php,
    Swift,
    Kotlin,
}

impl From<FormatArg> for apix_codegen::Format {
    fn from(arg: FormatArg) -> Self {
        use apix_codegen::{Format, Lang, Shell};
        match arg {
            FormatArg::Bash => Format::Shell(Shell::Bash),
            FormatArg::Cmd => Format::Shell(Shell::Cmd),
            FormatArg::Powershell => Format::Shell(Shell::PowerShell),
            FormatArg::Fish => Format::Shell(Shell::Fish),
            FormatArg::Httpie => Format::Shell(Shell::Httpie),
            FormatArg::Wget => Format::Shell(Shell::Wget),
            FormatArg::Fetch => Format::Code(Lang::Fetch),
            FormatArg::Axios => Format::Code(Lang::Axios),
            FormatArg::Python => Format::Code(Lang::Python),
            FormatArg::Go => Format::Code(Lang::Go),
            FormatArg::Rust => Format::Code(Lang::Rust),
            FormatArg::Csharp => Format::Code(Lang::CSharp),
            FormatArg::Java => Format::Code(Lang::Java),
            FormatArg::Php => Format::Code(Lang::Php),
            FormatArg::Swift => Format::Code(Lang::Swift),
            FormatArg::Kotlin => Format::Code(Lang::Kotlin),
        }
    }
}

#[derive(Args)]
struct VarArgs {
    /// Environment from the collection's environments/ folder
    #[arg(short, long)]
    env: Option<String>,
    /// Override a variable, e.g. --var token=abc (repeatable). Variables
    /// also come from APIX_VAR_<NAME> environment variables (APIX_VAR_TOKEN
    /// fills {{token}}); --var wins over them
    #[arg(long = "var", value_name = "KEY=VALUE", value_parser = parse_var)]
    vars: Vec<(String, String)>,
    /// Read variables from a dotenv file (NAME=value lines), e.g. secrets in CI
    #[arg(long, value_name = "FILE")]
    env_file: Option<PathBuf>,
}

/// The variables of a send or run, worked out once.
struct Resolved {
    vars: Variables,
    scopes: apix_core::scripts::ScopeValues,
    /// What --env-file, APIX_VAR_* and --var set, in that order.
    overrides: Vec<(String, String)>,
}

#[derive(Args)]
struct NetArgs {
    /// Give up after this many seconds (overrides the requests' settings)
    #[arg(long, value_name = "SECONDS")]
    timeout: Option<f64>,
    /// Do not verify the server certificate
    #[arg(short = 'k', long)]
    insecure: bool,
    /// Send through this proxy (overrides the environment)
    #[arg(long, value_name = "URL")]
    proxy: Option<String>,
    /// Also trust the certificate authorities in this PEM file
    #[arg(long, value_name = "FILE")]
    cacert: Option<String>,
}

impl NetArgs {
    fn overrides(&self) -> apix_core::RequestSettings {
        apix_core::RequestSettings {
            timeout_ms: self.timeout.map(|s| (s * 1000.0) as u64),
            tls_verify: self.insecure.then_some(false),
            proxy: self.proxy.clone(),
            // Relative to where the command runs, like any path given to it.
            ca_cert: self.cacert.as_ref().map(|path| {
                std::path::absolute(path)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| path.clone())
            }),
            ..Default::default()
        }
    }
}

fn parse_var(raw: &str) -> Result<(String, String), String> {
    raw.split_once('=')
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .ok_or_else(|| format!("expected KEY=VALUE, got `{raw}`"))
}

impl VarArgs {
    /// project vars < collection vars < project environment < collection
    /// environment < --env-file < APIX_VAR_* < --var
    fn resolve(&self, scope: &Scope) -> Result<Resolved> {
        let (project, collection) = (&scope.project, scope.collection.as_ref());
        if let Some(name) = &self.env {
            let available = match (project, collection) {
                (Some(p), Some(c)) => p.environment_names(std::slice::from_ref(c)),
                (None, Some(c)) => c.environment_names(),
                _ => bail!("--env {name} given, but no apix.toml was found above the request"),
            };
            if !available.contains(name) {
                bail!(
                    "unknown environment `{name}` (available: {})",
                    available.join(", ")
                );
            }
        }
        let mut vars =
            apix_core::project::variables(project.as_ref(), collection, self.env.as_deref())?;
        let mut scopes =
            apix_core::project::scope_values(project.as_ref(), collection, self.env.as_deref())?;
        let overrides = self.overrides(&vars, scope)?;
        for (k, v) in &overrides {
            vars.set(k, v);
        }
        // Values from outside the collection are the run's data for scripts.
        scopes.data.extend(overrides.iter().cloned());
        Ok(Resolved {
            vars,
            scopes,
            overrides,
        })
    }

    fn overrides(&self, known: &Variables, scope: &Scope) -> Result<Vec<(String, String)>> {
        use apix_runner::vars;
        // Names the collection defines or its requests use, to match `APIX_VAR_TOKEN` to `token`.
        let mut names: Vec<String> = known.iter().map(|(k, _)| k.to_owned()).collect();
        if let Some(collection) = &scope.collection {
            for path in collection.request_paths() {
                if let Ok(request) = apix_core::store::load_request(&path) {
                    names.extend(request.resolve(&Variables::new()).missing);
                }
            }
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut out = Vec::new();
        if let Some(path) = &self.env_file {
            let file = vars::match_known(vars::load_env_file(path)?, names.iter().copied());
            note(&format!(
                "{}: {}",
                path.display(),
                file.iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.extend(file);
        }
        let from_env = vars::match_known(
            vars::from_environment(std::env::vars()),
            names.iter().copied(),
        );
        if !from_env.is_empty() {
            note(&format!(
                "from {}*: {}",
                vars::PREFIX,
                from_env
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.extend(from_env);
        out.extend(self.vars.iter().cloned());
        Ok(out)
    }
}

/// A line on stderr about where values came from (names only, never values).
fn note(text: &str) {
    eprintln!("variables {text}");
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

/// Returns whether every request succeeded.
async fn run(cli: Cli) -> Result<bool> {
    let client = HttpClient::new()?;
    let style = Style::detect();
    let var = cli.password_env.clone();
    match cli.command {
        Command::Send {
            file,
            vars,
            include_headers,
            network,
            messages,
            message_format,
            event_types,
            reconnect,
            subscribe,
            topic,
            qos,
            retain,
            listen,
            net,
        } => {
            unlock_around(&file, &var)?;
            let scope = Scope::find(&file)?;
            let Resolved { vars, scopes, .. } = vars.resolve(&scope)?;
            let mut request = scope.load(&file)?;
            request.settings = net.overrides().or_inherit(&request.settings);
            let resolved = request.resolve(&vars);
            if resolved.request.is_grpc() {
                if !resolved.missing.is_empty() {
                    let names: Vec<_> = resolved.missing.into_iter().collect();
                    bail!("undefined variable(s): {}", names.join(", "));
                }
                let request = relative_to_file(resolved.request, &file);
                return grpc(&style, &request, include_headers).await;
            }
            if resolved.request.is_mqtt() {
                if !resolved.missing.is_empty() {
                    let names: Vec<_> = resolved.missing.into_iter().collect();
                    bail!("undefined variable(s): {}", names.join(", "));
                }
                let extra = MqttExtra {
                    messages,
                    format: message_format,
                    subscribe,
                    topic,
                    qos,
                    retain,
                };
                return mqtt(
                    &style,
                    relative_to_file(resolved.request, &file),
                    &vars,
                    extra,
                    listen,
                )
                .await;
            }
            if resolved.request.is_socketio() {
                if !resolved.missing.is_empty() {
                    let names: Vec<_> = resolved.missing.into_iter().collect();
                    bail!("undefined variable(s): {}", names.join(", "));
                }
                let event = event_types.first().cloned();
                return socketio(
                    &style,
                    resolved.request,
                    &vars,
                    (messages, message_format, event),
                    listen,
                    include_headers,
                )
                .await;
            }
            if apix_http::ws::is_websocket_url(&resolved.request.url) {
                if !resolved.missing.is_empty() {
                    let names: Vec<_> = resolved.missing.into_iter().collect();
                    bail!("undefined variable(s): {}", names.join(", "));
                }
                return websocket(
                    &style,
                    relative_to_file(resolved.request, &file),
                    &vars,
                    messages,
                    message_format,
                    listen,
                    include_headers,
                )
                .await;
            }
            let mut state = Default::default();
            let info = apix_script::Info {
                request_id: file.display().to_string(),
                iteration_count: 1,
                ..Default::default()
            };
            // Server-sent events are printed as they arrive, not at the end.
            let started = std::time::Instant::now();
            let mut stream: Option<(apix_http::sse::SseParser, usize)> = None;
            let wanted = |event: &apix_http::sse::SseEvent| {
                event_types.is_empty() || event_types.contains(&event.event)
            };
            // For reconnecting: the request as sent (scripts aside).
            let again = relative_to_file(resolved.request.clone(), &file);
            let sent = apix_runner::exchange(
                &client,
                &scope,
                &file,
                &vars,
                &scopes,
                &mut state,
                &net.overrides(),
                info,
                script_sender(),
                |progress| match progress {
                    apix_http::Progress::Head { status, headers } => {
                        let content_type = headers
                            .iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                            .map(|(_, v)| v.as_str())
                            .unwrap_or_default();
                        if !apix_http::sse::is_event_stream(content_type) {
                            return;
                        }
                        let color = if (200..400).contains(&status) {
                            style.green
                        } else {
                            style.red
                        };
                        println!(
                            "{color}{status}{}  {}event stream, Ctrl+C to stop{}",
                            style.reset, style.dim, style.reset
                        );
                        if include_headers {
                            for (k, v) in headers {
                                println!("{}{k}{}: {v}", style.dim, style.reset);
                            }
                            println!();
                        }
                        stream = Some((apix_http::sse::SseParser::new(), 0));
                    }
                    apix_http::Progress::Chunk(chunk) => {
                        let Some((parser, count)) = stream.as_mut() else {
                            return;
                        };
                        for event in parser.feed(chunk) {
                            *count += 1;
                            if wanted(&event) {
                                print_event(&style, started.elapsed(), &event);
                            }
                        }
                    }
                },
            )
            .await;
            // A stream that drops can be picked up again; anything else fails.
            let done = match sent {
                Ok(done) => done,
                Err(e) if reconnect && stream.is_some() => {
                    println!("{}connection lost: {e:#}{}", style.dim, style.reset);
                    let (parser, count) = stream.take().unwrap_or_default();
                    return follow_stream(&style, &client, &again, parser, count, &event_types)
                        .await;
                }
                Err(e) => return Err(e),
            };
            if reconnect && let Some((parser, count)) = stream.take() {
                println!("{}stream ended: {count} events{}", style.dim, style.reset);
                return follow_stream(&style, &client, &again, parser, count, &event_types).await;
            }
            if let (Some((_, count)), Some(response)) = (&stream, &done.response) {
                println!(
                    "{}stream ended: {count} events in {:.1} s · {}{}",
                    style.dim,
                    response.timing.total.as_secs_f64(),
                    human_size(response.body.len()),
                    style.reset
                );
                if network {
                    print_network(&style, response);
                }
            } else if let Some(response) = &done.response {
                print_status(
                    &style,
                    response.status,
                    response.reason.unwrap_or(""),
                    response.timing.total.as_secs_f64() * 1000.0,
                    response.body.len(),
                );
                if let Some(fault) = response.soap_fault() {
                    println!("{}SOAP Fault: {fault}{}", style.red, style.reset);
                }
                if include_headers {
                    for (k, v) in &response.headers {
                        println!("{}{k}{}: {v}", style.dim, style.reset);
                    }
                    println!();
                }
                if network {
                    print_network(&style, response);
                }
                println!("{}", response.pretty_body());
            }
            // A SOAP fault was already shown, with the response.
            let mut item = done.to_item(0, String::new());
            item.error = None;
            print_scripts(&style, &item);
            Ok(done.passed())
        }
        Command::Run {
            dir,
            vars: var_args,
            iterations,
            data,
            delay,
            bail: stop_on_failure,
            junit,
            json,
            html,
            net,
        } => {
            unlock_around(&dir, &var)?;
            let root = Collection::find_root(&dir).with_context(|| {
                format!("no apix.toml found in {} or its parents", dir.display())
            })?;
            let scope = Scope::find(&root)?;
            let collection = scope.collection.as_ref().context("no collection")?;
            // A folder (or request) inside the collection runs only what is in it.
            let start = std::path::absolute(&dir).unwrap_or_else(|_| dir.clone());
            let mut requests = collection.request_paths();
            if start != root {
                requests.retain(|p| p.starts_with(&start));
            }
            if requests.is_empty() {
                bail!("no requests in {}", dir.display());
            }
            let data = match &data {
                Some(path) => {
                    let rows = apix_runner::data::load(path)?;
                    if rows.is_empty() {
                        bail!("{} has no rows", path.display());
                    }
                    rows
                }
                None => Vec::new(),
            };
            let resolved = var_args.resolve(&scope)?;
            let plan = apix_runner::Plan {
                root: root.clone(),
                requests,
                vars: resolved.vars,
                scopes: resolved.scopes,
                environment: var_args.env.clone(),
                pinned: resolved.overrides,
                data,
                iterations: iterations.unwrap_or(0),
                delay: std::time::Duration::from_millis(delay),
                bail: stop_on_failure,
                overrides: net.overrides(),
                state: Default::default(),
            };
            let total = plan.iteration_count();
            let control = apix_runner::Control::default();
            let report = apix_runner::run(
                &scope,
                &plan,
                &client,
                script_sender(),
                &control,
                |progress| match progress {
                    apix_runner::Progress::Iteration(n, row) if total > 1 => {
                        let row = row
                            .map(|r| {
                                let pairs: Vec<String> =
                                    r.iter().map(|(k, v)| format!("{k}={v}")).collect();
                                format!(" · {}", pairs.join(", "))
                            })
                            .unwrap_or_default();
                        println!(
                            "{}Iteration {}/{total}{row}{}",
                            style.dim,
                            n + 1,
                            style.reset
                        );
                    }
                    apix_runner::Progress::Item(item) => {
                        print!("{:<40} ", item.path);
                        match item.status {
                            Some(status) => print_status(
                                &style,
                                status,
                                &item.reason,
                                item.time_ms,
                                item.size as usize,
                            ),
                            None => println!(),
                        }
                        print_scripts(&style, item);
                    }
                    _ => {}
                },
            )
            .await;
            let summary = report.summary();
            let tests = if summary.tests_failed > 0 {
                format!(" ({} failed tests)", summary.tests_failed)
            } else {
                String::new()
            };
            let stopped = if report.stopped {
                format!(
                    " {}(stopped at the first failure){}",
                    style.dim, style.reset
                )
            } else {
                String::new()
            };
            println!(
                "\n{} passed, {} failed{tests}{stopped}",
                summary.passed, summary.failed
            );
            for (path, text) in [
                (junit, report.to_junit()),
                (json, report.to_json()),
                (html, report.to_html()),
            ]
            .into_iter()
            .filter_map(|(path, text)| Some((path?, text)))
            {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, text)
                    .with_context(|| format!("writing {}", path.display()))?;
                println!("{}Wrote {}{}", style.dim, path.display(), style.reset);
            }
            Ok(summary.failed == 0)
        }
        Command::Schema {
            file,
            json,
            vars,
            net,
        } => {
            unlock_around(&file, &var)?;
            let scope = Scope::find(&file)?;
            let mut request = scope.load(&file)?;
            request.settings = net.overrides().or_inherit(&request.settings);
            let resolved = request.resolve(&vars.resolve(&scope)?.vars);
            if !resolved.missing.is_empty() {
                let names: Vec<_> = resolved.missing.into_iter().collect();
                bail!("undefined variable(s): {}", names.join(", "));
            }
            if resolved.request.is_grpc() {
                if json {
                    bail!("--json is for GraphQL; a gRPC service is printed as .proto text");
                }
                let request = relative_to_file(resolved.request, &file);
                let described = apix_grpc::load_description(&request).await?;
                print_notes(&style, &described.notes);
                print!("{}", apix_grpc::describe(&described.pool));
                return Ok(true);
            }
            let fetched = apix_http::graphql::fetch_schema(&client, &resolved.request).await?;
            if json {
                println!("{}", fetched.json);
            } else {
                print!("{}", fetched.schema.to_sdl());
            }
            Ok(true)
        }
        Command::Proto {
            source,
            method,
            response,
            out,
            into,
        } => {
            if let Some(request) = &into {
                unlock_around(request, &var)?;
            }
            proto(&style, &source, method, response, out, into)
        }
        Command::Import { file, out, into } => import(&style, &client, &file, out, into).await,
        Command::Export { dir, out } => {
            unlock_around(&dir, &var)?;
            let scope = Scope::find(&dir)?;
            let collection = scope.collection.as_ref().with_context(|| {
                format!("no apix.toml found in {} or its parents", dir.display())
            })?;
            let export = apix_import::postman_export::export_collection(
                &collection.root,
                scope.project.as_ref(),
            )?;
            let written = export.write(&out.unwrap_or_else(|| PathBuf::from(".")))?;
            for note in &export.notes {
                println!("  ! {note}");
            }
            for path in written {
                println!("{}Created {}{}", style.green, path.display(), style.reset);
            }
            Ok(true)
        }
        Command::Encrypt { dir, out } => encrypt(&style, &dir, out, &var),
        Command::Decrypt { dir, out, recovery } => decrypt(&style, &dir, &out, recovery, &var),
        Command::Shell {
            file,
            format,
            raw,
            vars,
        } => {
            unlock_around(&file, &var)?;
            let scope = Scope::find(&file)?;
            let request = scope.load(&file)?;
            let request = if raw {
                request
            } else {
                let resolved = request.resolve(&vars.resolve(&scope)?.vars);
                if !resolved.missing.is_empty() {
                    let names: Vec<_> = resolved.missing.into_iter().collect();
                    eprintln!("note: left as-is, not defined: {}", names.join(", "));
                }
                resolved.request
            };
            let format: apix_codegen::Format = format.into();
            println!("{}", format.render(&request));
            Ok(true)
        }
    }
}

fn encrypt(style: &Style, dir: &Path, out: Option<PathBuf>, var: &str) -> Result<bool> {
    if !dir.join(apix_core::store::COLLECTION_FILE).is_file() {
        bail!("{} is not an Apix collection (no apix.toml)", dir.display());
    }
    let out = out.unwrap_or_else(|| {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "collection".into());
        dir.with_file_name(format!("{name}-encrypted"))
    });
    let pw = password(var, "New password: ", true)?;
    let (vault, recovery) = Vault::encrypt_folder(dir, &out, &pw, KdfCost::default())?;
    println!(
        "{}Encrypted {} files into {}{}",
        style.green,
        vault.files().len(),
        out.display(),
        style.reset
    );
    println!("\nRecovery key (shown only now, keep it somewhere safe):\n\n    {recovery}\n");
    println!(
        "With neither the password nor this key, the collection cannot be opened.\n\
         The original folder {} was left as it is; delete it when you are ready.",
        dir.display()
    );
    Ok(true)
}

fn decrypt(style: &Style, dir: &Path, out: &Path, recovery: bool, var: &str) -> Result<bool> {
    if !vfs::is_encrypted(dir) {
        bail!("{} is not an encrypted Apix collection", dir.display());
    }
    let vault = if recovery {
        let key = password(var, "Recovery key: ", false)?;
        Vault::unlock_with_recovery(dir, &key)?
    } else {
        Vault::unlock(dir, &password(var, "Password: ", false)?)?
    };
    vault.export(out)?;
    println!(
        "{}Wrote {} files to {}{}",
        style.green,
        vault.files().len(),
        out.display(),
        style.reset
    );
    Ok(true)
}

/// A document from the web, or a local file.
async fn fetch_text(client: &HttpClient, location: &str) -> Result<String, String> {
    if !is_web_address(location) {
        return std::fs::read_to_string(location).map_err(|e| e.to_string());
    }
    let request = apix_core::Request::new(apix_core::Method::Get, location);
    let response = client.send(&request).await.map_err(|e| e.to_string())?;
    if !response.is_success() {
        return Err(format!("the server answered {}", response.status));
    }
    Ok(String::from_utf8_lossy(&response.body).into_owned())
}

fn is_web_address(location: &str) -> bool {
    let lower = location.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// What a gRPC description or answer gave reason to say, for stderr.
fn print_notes(style: &Style, notes: &[String]) {
    for note in notes {
        eprintln!("{}{note}{}", style.yellow, style.reset);
    }
}

/// `apix proto`: a definition from whatever describes the service.
fn proto(
    style: &Style,
    source: &Path,
    method: Option<String>,
    response: Option<PathBuf>,
    out: Option<PathBuf>,
    into: Option<PathBuf>,
) -> Result<bool> {
    let bytes = if source == Path::new("-") {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin(), &mut bytes)
            .context("cannot read standard input")?;
        bytes
    } else {
        std::fs::read(source).with_context(|| format!("cannot read {}", source.display()))?
    };
    let response = response
        .map(|path| {
            std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))
        })
        .transpose()?;
    let scope_and_request = into
        .as_ref()
        .map(|path| {
            let request = apix_core::store::load_request(path)?;
            Ok::<_, anyhow::Error>((path, request))
        })
        .transpose()?;
    // A request's own method says what to call, unless told otherwise.
    let known_method = method.clone().or_else(|| {
        scope_and_request
            .as_ref()
            .and_then(|(_, request)| request.grpc.as_ref())
            .map(|call| call.method.clone())
            .filter(|m| !m.trim().is_empty())
    });
    let conversion =
        apix_grpc::convert_bytes(&bytes, response.as_deref(), known_method.as_deref())?;
    let how = match conversion.certainty {
        apix_grpc::Certainty::Exact => "exact",
        apix_grpc::Certainty::NumbersFromSource => {
            "field numbers from the source; check that the calls are right"
        }
        apix_grpc::Certainty::Guessed => {
            "a guess: field numbers must match the server's, and wrong ones fill the wrong fields without an error"
        }
    };
    eprintln!(
        "{}From {}: {how}{}",
        style.dim, conversion.format, style.reset
    );
    print_notes(style, &conversion.notes);
    if let Some((path, mut request)) = scope_and_request {
        let mut call = request.grpc.take().unwrap_or_default();
        call.definition = conversion.definition.clone();
        call.guessed = conversion.is_guess();
        if let Some(method) = method.or_else(|| {
            call.method
                .trim()
                .is_empty()
                .then(|| {
                    apix_grpc::methods(&conversion.pool)
                        .first()
                        .map(|m| m.name.clone())
                })
                .flatten()
        }) {
            call.method = method;
        }
        request.grpc = Some(call);
        apix_core::store::save_request(path, &request)
            .with_context(|| format!("cannot write {}", path.display()))?;
        eprintln!("Kept in {}", path.display());
    }
    match out {
        Some(path) => {
            std::fs::write(&path, &conversion.definition)
                .with_context(|| format!("cannot write {}", path.display()))?;
            eprintln!("Written to {}", path.display());
        }
        None if into.is_none() => print!("{}", conversion.definition),
        None => {}
    }
    Ok(true)
}

async fn import(
    style: &Style,
    client: &HttpClient,
    file: &Path,
    out: Option<PathBuf>,
    into: Option<PathBuf>,
) -> Result<bool> {
    use apix_import::postman::{self, Kind};
    use apix_import::postman_dump;

    let location = file.to_string_lossy().into_owned();
    let remote = is_web_address(&location);
    // A folder of exports or Postman's "Export data" zip: one project.
    let is_zip = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !remote && (file.is_dir() || is_zip) {
        let found = postman_dump::scan(file)?;
        if found.is_empty() {
            bail!(
                "no Postman collections or environments found in {}",
                file.display()
            );
        }
        let name = postman_dump::project_name(file);
        let parent = out.unwrap_or_else(|| PathBuf::from("."));
        let written = postman_dump::write_project(&found, &parent, &name)?;
        println!(
            "Imported {} collections ({} requests), {} environments and {} global variables.",
            written.collections, written.requests, written.environments, written.global_vars
        );
        for note in &written.notes {
            println!("  ! {note}");
        }
        println!(
            "{}Created project {}{}",
            style.green,
            written.root.display(),
            style.reset
        );
        return Ok(true);
    }

    let shown = apix_core::display_url(&location);
    let json = fetch_text(client, &location)
        .await
        .map_err(|e| anyhow::anyhow!("reading {shown}: {e}"))?;
    let format = apix_import::detect(&json)?.with_context(|| {
        format!(
            "{shown} is not an export Apix reads (Postman, Insomnia v4, OpenAPI/Swagger, HAR, WSDL)"
        )
    })?;
    let kind = match format {
        apix_import::Format::Postman(kind) => kind,
        other => {
            let name = if remote {
                apix_import::location_name(&location)
            } else {
                postman_dump::project_name(file)
            };
            let found = if other == apix_import::Format::Wsdl {
                // A WSDL's imports are read too, from where it points.
                let mut loader = apix_import::wsdl::Loader::new(&location, &json);
                while let Some(next) = loader.next_location() {
                    let read = fetch_text(client, &next).await;
                    loader.add(next, read);
                }
                apix_import::wsdl::convert(&json, &loader)?
            } else {
                apix_import::convert_named(&json, &name)?.expect("detected formats always convert")
            };
            let written = found.write(&out.unwrap_or_else(|| PathBuf::from(".")), &name)?;
            println!(
                "Imported {} from {}: {} collections, {} requests, {} environments.",
                other.label(),
                shown,
                written.collections,
                written.requests,
                written.environments
            );
            for note in &written.notes {
                println!("  ! {note}");
            }
            println!(
                "{}Created {}{}",
                style.green,
                written.root.display(),
                style.reset
            );
            return Ok(true);
        }
    };
    match kind {
        Kind::Collection => {
            let plan = postman::convert_collection(&json)?;
            let root = out
                .unwrap_or_else(|| PathBuf::from(apix_import::slug(&plan.meta.name, "collection")));
            plan.write(&root)?;
            print!("{}", plan.report);
            println!("{}Created {}{}", style.green, root.display(), style.reset);
            println!("Try: apix run {}", root.display());
        }
        Kind::Environment | Kind::Globals => {
            let start = into.unwrap_or_else(|| PathBuf::from("."));
            let root = Collection::find_root(&start).with_context(|| {
                format!(
                    "no apix.toml found in {} or its parents; pass --into <collection>",
                    start.display()
                )
            })?;
            let env = postman::convert_environment(&json)?;
            let written = env.write(&root)?;
            println!(
                "Imported environment \"{}\" as `{}`: {} variables, {} secrets ({} disabled skipped).",
                env.title,
                env.name,
                env.vars.len(),
                env.secrets.len(),
                env.skipped_disabled
            );
            for path in written {
                println!("{}Created {}{}", style.green, path.display(), style.reset);
            }
            if !env.secrets.is_empty() {
                println!("Secrets are in the .local.toml file, which is gitignored.");
            }
        }
    }
    Ok(true)
}

/// `pm.sendRequest` from the command line: a fresh client per request.
fn script_sender() -> apix_script::Sender {
    apix_script::Sender::new(|request| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let client = HttpClient::new().map_err(|e| e.to_string())?;
        let response = runtime
            .block_on(client.send(request))
            .map_err(|e| format!("{e:#}"))?;
        Ok(apix_script::ResponseView::new(
            response.status,
            response.reason.unwrap_or(""),
            &response.headers,
            &response.body,
            response.timing.total,
        ))
    })
}

/// Console lines, script errors and test results, indented under a request.
fn print_scripts(style: &Style, item: &apix_runner::ItemResult) {
    for log in &item.logs {
        let color = match log.level.as_str() {
            "error" => style.red,
            "warn" => style.yellow,
            _ => style.dim,
        };
        println!("  {color}│ {}{}", log.text, style.reset);
    }
    if let Some(error) = &item.pre_error {
        println!(
            "  {}pre-request script error: {error}{}",
            style.red, style.reset
        );
    }
    if let Some(error) = &item.error {
        println!("  {}error: {error}{}", style.red, style.reset);
    }
    if item.skipped {
        println!("  {}skipped{}", style.dim, style.reset);
    }
    for test in &item.tests {
        if test.skipped {
            println!("  {}- {} (skipped){}", style.dim, test.name, style.reset);
        } else if test.passed {
            println!("  {}✓{} {}", style.green, style.reset, test.name);
        } else {
            let why = test.error.as_deref().unwrap_or("failed");
            println!(
                "  {}✗ {}{} {}{why}{}",
                style.red, test.name, style.reset, style.dim, style.reset
            );
        }
    }
    if let Some(error) = &item.test_error {
        println!("  {}test script error: {error}{}", style.red, style.reset);
    }
}

/// `apix send` on a `ws://` or `wss://` request: connect, send the saved
/// messages (or `--message` texts), print what goes both ways, and close
/// after `listen` seconds (0 waits for the server). Fails on errors.
async fn websocket(
    style: &Style,
    request: apix_core::Request,
    vars: &Variables,
    extra: Vec<String>,
    extra_format: apix_core::MessageFormat,
    listen: f64,
    include_headers: bool,
) -> Result<bool> {
    use apix_http::ws::{WsCommand, WsData, WsEvent};

    let outgoing: Vec<(String, apix_core::MessageFormat)> = if extra.is_empty() {
        let mut missing = std::collections::BTreeSet::new();
        let bodies = request
            .messages
            .iter()
            .map(|m| (vars.interpolate(&m.body, &mut missing), m.format))
            .collect();
        if !missing.is_empty() {
            let names: Vec<_> = missing.into_iter().collect();
            bail!(
                "undefined variable(s) in the messages: {}",
                names.join(", ")
            );
        }
        bodies
    } else {
        extra.into_iter().map(|text| (text, extra_format)).collect()
    };
    // Base64 and hex become bytes before anything is sent.
    let outgoing = outgoing
        .into_iter()
        .map(|(text, format)| WsData::from_text(text, format))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("cannot send a message: {e}"))?;
    let started = std::time::Instant::now();
    let connection = apix_http::ws::connect(&request).await?;
    println!(
        "{}{} Switching Protocols{}  {}connected to {}{}",
        style.green,
        connection.status,
        style.reset,
        style.dim,
        apix_core::display_url(&request.url),
        style.reset
    );
    if let Some(protocol) = &connection.protocol {
        println!("{}subprotocol {protocol}{}", style.dim, style.reset);
    }
    if include_headers {
        for (k, v) in &connection.headers {
            println!("{}{k}{}: {v}", style.dim, style.reset);
        }
        println!();
    }
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    for data in outgoing {
        let _ = tx.send(WsCommand::Send(data));
    }
    if listen > 0.0 {
        let closer = tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f64(listen)).await;
            let _ = closer.send(WsCommand::Close);
        });
    }
    let mut ok = true;
    connection
        .run(rx, |event| {
            let at = format!("{:>8.2}s", started.elapsed().as_secs_f64());
            let show = |data: &WsData| match data {
                WsData::Text(text) => text.clone(),
                WsData::Binary(bytes) => apix_http::ws::hex_preview(bytes),
            };
            match event {
                WsEvent::Sent(data) => println!(
                    "{}{at}{} {}↑{} {}",
                    style.dim,
                    style.reset,
                    style.yellow,
                    style.reset,
                    show(&data)
                ),
                WsEvent::Received(data) => println!(
                    "{}{at}{} {}↓{} {}",
                    style.dim,
                    style.reset,
                    style.green,
                    style.reset,
                    show(&data)
                ),
                WsEvent::Closed { code, reason } => {
                    let code = code.map(|c| format!(" {c}")).unwrap_or_default();
                    let reason = if reason.is_empty() {
                        String::new()
                    } else {
                        format!(": {reason}")
                    };
                    println!("{}closed{code}{reason}{}", style.dim, style.reset);
                }
                WsEvent::Error(message) => {
                    ok = false;
                    println!("{}error: {message}{}", style.red, style.reset);
                }
            }
        })
        .await;
    drop(tx);
    Ok(ok)
}

/// `apix send` on a Socket.IO request: join the namespace, emit the saved
/// messages (or `--message` texts as one event), print the events both ways
/// and the acknowledgements, and close after `listen` seconds (0 waits for
/// the server). Fails on errors.
async fn socketio(
    style: &Style,
    request: apix_core::Request,
    vars: &Variables,
    (extra, extra_format, extra_event): (Vec<String>, apix_core::MessageFormat, Option<String>),
    listen: f64,
    include_headers: bool,
) -> Result<bool> {
    use apix_http::socketio::{SioCommand, SioEvent, args_from_text, show_args};

    let event_or_default = |event: &str| match event.trim() {
        "" => "message".to_owned(),
        event => event.to_owned(),
    };
    let outgoing: Vec<(String, String, apix_core::MessageFormat, bool)> = if extra.is_empty() {
        let mut missing = std::collections::BTreeSet::new();
        let messages = request
            .messages
            .iter()
            .map(|m| {
                (
                    event_or_default(&vars.interpolate(&m.event, &mut missing)),
                    vars.interpolate(&m.body, &mut missing),
                    m.format,
                    m.ack,
                )
            })
            .collect();
        if !missing.is_empty() {
            let names: Vec<_> = missing.into_iter().collect();
            bail!(
                "undefined variable(s) in the messages: {}",
                names.join(", ")
            );
        }
        messages
    } else {
        let event = event_or_default(extra_event.as_deref().unwrap_or_default());
        extra
            .into_iter()
            .map(|text| (event.clone(), text, extra_format, false))
            .collect()
    };
    let commands = outgoing
        .into_iter()
        .map(|(event, text, format, ack)| {
            args_from_text(&text, format).map(|args| SioCommand::Emit { event, args, ack })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("cannot send a message: {e}"))?;
    let started = std::time::Instant::now();
    let connection = apix_http::socketio::connect(&request).await?;
    println!(
        "{}{} Switching Protocols{}  {}connected to {}, namespace {} (sid {}){}",
        style.green,
        connection.status,
        style.reset,
        style.dim,
        connection.url,
        connection.namespace,
        connection.sid,
        style.reset
    );
    if include_headers {
        for (k, v) in &connection.headers {
            println!("{}{k}{}: {v}", style.dim, style.reset);
        }
        println!();
    }
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    for command in commands {
        let _ = tx.send(command);
    }
    if listen > 0.0 {
        let closer = tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f64(listen)).await;
            let _ = closer.send(SioCommand::Close);
        });
    }
    let mut ok = true;
    connection
        .run(rx, |event| {
            let at = format!("{:>8.2}s", started.elapsed().as_secs_f64());
            let line = |arrow: &str, color: &str, text: String| {
                let text = text.trim_end();
                println!(
                    "{}{at}{} {color}{arrow}{} {text}",
                    style.dim, style.reset, style.reset
                )
            };
            match event {
                SioEvent::Sent(m) => {
                    let ack = m.ack.map(|id| format!("  (ack {id})")).unwrap_or_default();
                    line(
                        "↑",
                        style.yellow,
                        format!("{} {}{ack}", m.event, show_args(&m.args)),
                    )
                }
                SioEvent::Received(m) => {
                    let ack = m
                        .ack
                        .map(|id| format!("  (wants ack {id})"))
                        .unwrap_or_default();
                    line(
                        "↓",
                        style.green,
                        format!("{} {}{ack}", m.event, show_args(&m.args)),
                    )
                }
                SioEvent::Acked { id, args } => {
                    line("✓", style.green, format!("ack {id} {}", show_args(&args)))
                }
                SioEvent::Disconnected => {
                    println!(
                        "{}the server made us leave the namespace{}",
                        style.dim, style.reset
                    )
                }
                SioEvent::Unreadable(text) => line("?", style.dim, text),
                SioEvent::Closed { code, reason } => {
                    let code = code.map(|c| format!(" {c}")).unwrap_or_default();
                    let reason = if reason.is_empty() {
                        String::new()
                    } else {
                        format!(": {reason}")
                    };
                    println!("{}closed{code}{reason}{}", style.dim, style.reset);
                }
                SioEvent::Error(message) => {
                    ok = false;
                    println!("{}error: {message}{}", style.red, style.reset);
                }
            }
        })
        .await;
    drop(tx);
    Ok(ok)
}

/// What the command line adds to a MQTT request.
struct MqttExtra {
    messages: Vec<String>,
    format: apix_core::MessageFormat,
    subscribe: Vec<String>,
    topic: Option<String>,
    qos: u8,
    retain: bool,
}

/// `apix send` on a `mqtt://` or `mqtts://` request: connect, subscribe,
/// publish the saved messages (or `--message` texts to `--topic`), print what
/// goes both ways and close after `listen` seconds (0 waits for the broker).
/// Fails on errors and on anything the broker refuses.
async fn mqtt(
    style: &Style,
    mut request: apix_core::Request,
    vars: &Variables,
    extra: MqttExtra,
    listen: f64,
) -> Result<bool> {
    use apix_proto::mqtt::{MqttCommand, MqttEvent, MqttMessage, payload_from_text, show_payload};

    let settings = request.mqtt.get_or_insert_default();
    settings.subscribe.extend(
        extra
            .subscribe
            .iter()
            .map(|topic| apix_core::MqttSubscription {
                topic: topic.clone(),
                qos: extra.qos,
            }),
    );
    let outgoing: Vec<(String, String, apix_core::MessageFormat, u8, bool)> =
        if extra.messages.is_empty() {
            let mut missing = std::collections::BTreeSet::new();
            let messages = request
                .messages
                .iter()
                .map(|m| {
                    (
                        vars.interpolate(&m.topic, &mut missing),
                        vars.interpolate(&m.body, &mut missing),
                        m.format,
                        m.qos,
                        m.retain,
                    )
                })
                .collect();
            if !missing.is_empty() {
                let names: Vec<_> = missing.into_iter().collect();
                bail!(
                    "undefined variable(s) in the messages: {}",
                    names.join(", ")
                );
            }
            messages
        } else {
            let Some(topic) = extra.topic.clone() else {
                bail!("--message needs --topic for a MQTT request");
            };
            extra
                .messages
                .into_iter()
                .map(|text| (topic.clone(), text, extra.format, extra.qos, extra.retain))
                .collect()
        };
    let publishes = outgoing
        .into_iter()
        .map(|(topic, text, format, qos, retain)| {
            if topic.trim().is_empty() {
                return Err("a message has no topic".to_owned());
            }
            payload_from_text(&text, format).map(|payload| {
                MqttCommand::Publish(MqttMessage {
                    topic,
                    payload,
                    qos,
                    retain,
                })
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("cannot send a message: {e}"))?;
    let started = std::time::Instant::now();
    let connection = apix_proto::mqtt::connect(&request).await?;
    let session = if connection.session_present {
        ", session kept"
    } else {
        ""
    };
    println!(
        "{}connected{}  {}{} as {}{session}{}",
        style.green,
        style.reset,
        style.dim,
        apix_core::display_url(&request.url),
        connection.client_id,
        style.reset
    );
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    for command in publishes {
        let _ = tx.send(command);
    }
    if listen > 0.0 {
        let closer = tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f64(listen)).await;
            let _ = closer.send(MqttCommand::Close);
        });
    }
    let mut ok = true;
    connection
        .run(rx, |event| {
            let at = format!("{:>8.2}s", started.elapsed().as_secs_f64());
            let line = |arrow: &str, color: &str, text: String| {
                println!(
                    "{}{at}{} {color}{arrow}{} {text}",
                    style.dim, style.reset, style.reset
                )
            };
            let message = |m: &MqttMessage| {
                let qos = if m.qos > 0 {
                    format!("  (QoS {})", m.qos)
                } else {
                    String::new()
                };
                let retain = if m.retain { "  (retain)" } else { "" };
                format!("{} {}{qos}{retain}", m.topic, show_payload(&m.payload))
            };
            match event {
                MqttEvent::Sent(m) => line("↑", style.yellow, message(&m)),
                MqttEvent::Received(m) => line("↓", style.green, message(&m)),
                MqttEvent::Delivered { topic } => {
                    line("✓", style.green, format!("delivered {topic}"))
                }
                MqttEvent::Subscribed { topic, qos } => {
                    line("•", style.dim, format!("subscribed to {topic} (QoS {qos})"))
                }
                MqttEvent::Unsubscribed { topic } => {
                    line("•", style.dim, format!("unsubscribed from {topic}"))
                }
                MqttEvent::SubscribeRefused { topic } => {
                    ok = false;
                    println!(
                        "{}the broker refused the subscription to {topic}{}",
                        style.red, style.reset
                    );
                }
                MqttEvent::Rejected(message) => {
                    ok = false;
                    println!("{}error: {message}{}", style.red, style.reset);
                }
                MqttEvent::Closed { by_broker } => {
                    let who = if by_broker { " by the broker" } else { "" };
                    println!("{}closed{who}{}", style.dim, style.reset);
                }
                MqttEvent::Error(message) => {
                    ok = false;
                    println!("{}error: {message}{}", style.red, style.reset);
                }
            }
        })
        .await;
    drop(tx);
    Ok(ok)
}

/// `.proto` files and other paths in the request, made absolute against the
/// request file's folder.
fn relative_to_file(request: apix_core::Request, file: &Path) -> apix_core::Request {
    match file.parent() {
        Some(dir) => request.with_files_relative_to(dir),
        None => request,
    }
}

/// `apix send` for a `grpc://` request: a unary answer is printed whole,
/// streams message by message as they come; the exit code follows the status.
async fn grpc(style: &Style, request: &apix_core::Request, include_headers: bool) -> Result<bool> {
    use apix_grpc::CallEvent;

    let name = request
        .grpc
        .as_ref()
        .map(|g| g.method.trim().to_owned())
        .filter(|m| !m.is_empty())
        .ok_or(apix_grpc::GrpcError::NoMethod)?;
    let metadata = apix_grpc::metadata(request).await?;
    let channel = apix_grpc::connect(request).await?;
    let described = apix_grpc::described(request, &channel, &metadata).await?;
    print_notes(style, &described.notes);
    let method = apix_grpc::find_method(&described.pool, &name)
        .ok_or(apix_grpc::GrpcError::UnknownMethod(name))?;
    let messages = apix_grpc::messages(request, &method)?;
    let streaming = method.is_client_streaming() || method.is_server_streaming();
    let started = std::time::Instant::now();
    // One line per message in a stream.
    let compact = |json: &str| {
        serde_json::from_str::<serde_json::Value>(json)
            .map(|v| v.to_string())
            .unwrap_or_else(|_| json.to_owned())
    };
    let outcome = apix_grpc::call(channel, &method, messages, metadata, |event| {
        let at = format!("{:>8.2}s", started.elapsed().as_secs_f64());
        match event {
            CallEvent::Sent(json) if streaming => println!(
                "{}{at}{} {}↑{} {}",
                style.dim,
                style.reset,
                style.yellow,
                style.reset,
                compact(&json)
            ),
            CallEvent::Received(json) if streaming => println!(
                "{}{at}{} {}↓{} {}",
                style.dim,
                style.reset,
                style.green,
                style.reset,
                compact(&json)
            ),
            CallEvent::Headers(headers) if include_headers => {
                for (k, v) in headers {
                    println!("{}{k}{}: {v}", style.dim, style.reset);
                }
                println!();
            }
            CallEvent::Note(note) => {
                eprintln!("{}{note}{}", style.yellow, style.reset);
            }
            _ => {}
        }
    })
    .await?;
    let color = if outcome.is_ok() {
        style.green
    } else {
        style.red
    };
    let detail = if outcome.status_message.is_empty() {
        String::new()
    } else {
        format!(": {}", outcome.status_message)
    };
    let count = match outcome.received.len() {
        1 => "1 message".to_owned(),
        n => format!("{n} messages"),
    };
    println!(
        "{color}{} ({}){detail}{}  {}{:.1} ms · {count} · {}/{}{}",
        outcome.code_name(),
        outcome.code,
        style.reset,
        style.dim,
        outcome.elapsed.as_secs_f64() * 1000.0,
        method.parent_service().full_name(),
        method.name(),
        style.reset
    );
    if !streaming && let Some(message) = outcome.received.first() {
        println!("{message}");
    }
    if include_headers && !outcome.trailers.is_empty() {
        println!();
        for (k, v) in &outcome.trailers {
            println!("{}{k}{}: {v}", style.dim, style.reset);
        }
    }
    Ok(outcome.is_ok())
}

/// `apix send --reconnect` once a stream ended or dropped: wait the server's
/// retry time, connect again with `Last-Event-ID`, print the events, and so
/// on until Ctrl+C, or until the server answers with something other than
/// an event stream.
async fn follow_stream(
    style: &Style,
    client: &HttpClient,
    request: &apix_core::Request,
    mut parser: apix_http::sse::SseParser,
    mut count: usize,
    event_types: &[String],
) -> Result<bool> {
    let started = std::time::Instant::now();
    loop {
        let wait = parser.reconnection_time();
        let id = parser
            .last_id()
            .map(|id| format!(" with Last-Event-ID {id}"))
            .unwrap_or_default();
        println!(
            "{}reconnecting in {:.1} s{id}{}",
            style.dim,
            wait.as_secs_f64(),
            style.reset
        );
        tokio::time::sleep(wait).await;
        let next = apix_http::sse::reconnect_request(request, parser.last_id());
        parser = parser.reconnected();
        let mut stream_ok = false;
        let result = client
            .send_with(&next, |progress| match progress {
                apix_http::Progress::Head { status, headers } => {
                    stream_ok = status == 200
                        && headers.iter().any(|(k, v)| {
                            k.eq_ignore_ascii_case("content-type")
                                && apix_http::sse::is_event_stream(v)
                        });
                    if stream_ok {
                        println!("{}{status} reconnected{}", style.green, style.reset);
                    }
                }
                apix_http::Progress::Chunk(chunk) if stream_ok => {
                    for event in parser.feed(chunk) {
                        count += 1;
                        if event_types.is_empty() || event_types.contains(&event.event) {
                            print_event(style, started.elapsed(), &event);
                        }
                    }
                }
                apix_http::Progress::Chunk(_) => {}
            })
            .await;
        match result {
            // As a browser does: an answer that is not a stream ends it all.
            Ok(response) if !stream_ok => {
                print_status(
                    style,
                    response.status,
                    response.reason.unwrap_or(""),
                    response.timing.total.as_secs_f64() * 1000.0,
                    response.body.len(),
                );
                println!(
                    "{}not an event stream any more; stopped after {count} events{}",
                    style.dim, style.reset
                );
                return Ok(false);
            }
            Ok(_) => println!(
                "{}stream ended: {count} events so far{}",
                style.dim, style.reset
            ),
            Err(e) => {
                let e = anyhow::Error::from(e);
                println!("{}connection failed: {e:#}{}", style.dim, style.reset)
            }
        }
    }
}

/// One server-sent event: when it came, its type and id, then its data.
fn print_event(style: &Style, at: std::time::Duration, event: &apix_http::sse::SseEvent) {
    let id = event
        .id
        .as_deref()
        .map(|id| format!(" #{id}"))
        .unwrap_or_default();
    println!(
        "{}{:>8.2}s{} {}{}{}{}{}{}",
        style.dim,
        at.as_secs_f64(),
        style.reset,
        style.green,
        event.event,
        style.reset,
        style.dim,
        id,
        style.reset
    );
    for line in event.data.lines() {
        println!("          {line}");
    }
}

fn print_status(style: &Style, status: u16, reason: &str, ms: f64, size: usize) {
    let color = if (200..400).contains(&status) {
        style.green
    } else {
        style.red
    };
    println!(
        "{color}{status} {reason}{}  {}{ms:.1} ms · {}{}",
        style.reset,
        style.dim,
        human_size(size),
        style.reset,
    );
}

/// `apix send --network`: where the time went and what the connection was.
fn print_network(style: &Style, response: &Response) {
    let trace = &response.trace;
    let ms = |d: std::time::Duration| format!("{:.1} ms", d.as_secs_f64() * 1000.0);
    let line =
        |label: &str, value: String| println!("{}{label:<12}{} {value}", style.dim, style.reset);
    let p = trace.phases(response.timing.ttfb, response.timing.total);
    line(
        "Timing",
        format!(
            "DNS {} · connect {} · waiting {} · download {}",
            ms(p.dns),
            ms(p.connect),
            ms(p.waiting),
            ms(p.download)
        ),
    );
    for lookup in &trace.dns {
        let addrs: Vec<String> = lookup.addrs.iter().map(ToString::to_string).collect();
        line(
            "DNS",
            format!(
                "{} → {} ({})",
                lookup.host,
                addrs.join(", "),
                ms(lookup.duration)
            ),
        );
    }
    if let Some(remote) = trace.remote_addr {
        line("Remote", format!("{remote} · {}", response.version));
    }
    for hop in &trace.redirects {
        line(
            "Redirect",
            format!("{} → {}", hop.status, apix_core::display_url(&hop.to)),
        );
    }
    if let Some(cert) = &trace.certificate {
        line("Certificate", cert.subject.clone());
        line("  issued by", cert.issuer.clone());
        line(
            "  valid",
            format!(
                "{} to {} ({} days left)",
                cert.not_before, cert.not_after, cert.days_left
            ),
        );
        if !cert.names.is_empty() {
            line("  names", cert.names.join(", "));
        }
    }
    let local = apix_http::local_network(trace.local_addr.map(|a| a.ip()));
    let via = match (&local.interface, &local.kind) {
        (Some(name), Some(kind)) => format!("{name} ({kind})"),
        (Some(name), None) => name.clone(),
        _ => "unknown interface".into(),
    };
    let address = trace
        .local_addr
        .map(|a| a.to_string())
        .or_else(|| local.local_ip.map(|ip| ip.to_string()))
        .unwrap_or_default();
    line(
        "Local",
        format!(
            "{address} via {via}{}",
            if local.vpn { " · VPN" } else { "" }
        ),
    );
    if let Some(gateway) = local.gateway {
        line("Gateway", gateway.to_string());
    }
    if !local.dns_servers.is_empty() {
        let servers: Vec<String> = local.dns_servers.iter().map(ToString::to_string).collect();
        line("DNS servers", servers.join(", "));
    }
    match &local.proxy {
        Some((var, value)) => line("Proxy", format!("{value} (from {var})")),
        None => line("Proxy", "none in the environment".into()),
    }
    println!();
}

struct Style {
    green: &'static str,
    red: &'static str,
    yellow: &'static str,
    dim: &'static str,
    reset: &'static str,
}

impl Style {
    fn detect() -> Self {
        if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
            Self {
                green: "\x1b[32m",
                red: "\x1b[31m",
                yellow: "\x1b[33m",
                dim: "\x1b[2m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                green: "",
                red: "",
                yellow: "",
                dim: "",
                reset: "",
            }
        }
    }
}
