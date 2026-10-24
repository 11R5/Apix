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
    /// Import a Postman collection or environment, an Insomnia v4 export, an
    /// OpenAPI 3 / Swagger 2 spec (JSON or YAML) or a HAR file; or a whole
    /// folder / "Export data" zip of them as a project
    Import {
        /// An export file, a folder of them, or a `.zip`
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
    /// Override a variable, e.g. --var token=abc (repeatable)
    #[arg(long = "var", value_name = "KEY=VALUE", value_parser = parse_var)]
    vars: Vec<(String, String)>,
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
}

impl NetArgs {
    fn overrides(&self) -> apix_core::RequestSettings {
        apix_core::RequestSettings {
            timeout_ms: self.timeout.map(|s| (s * 1000.0) as u64),
            tls_verify: self.insecure.then_some(false),
            proxy: self.proxy.clone(),
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
    /// environment < --var overrides
    fn build(&self, scope: &Scope) -> Result<Variables> {
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
        for (k, v) in &self.vars {
            vars.set(k, v);
        }
        Ok(vars)
    }

    /// The same values kept apart by scope, for scripts; `--var` values are
    /// the run's data.
    fn scopes(&self, scope: &Scope) -> Result<apix_core::scripts::ScopeValues> {
        let mut scopes = apix_core::project::scope_values(
            scope.project.as_ref(),
            scope.collection.as_ref(),
            self.env.as_deref(),
        )?;
        scopes.data.extend(self.vars.iter().cloned());
        Ok(scopes)
    }
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
            net,
        } => {
            unlock_around(&file, &var)?;
            let scope = Scope::find(&file)?;
            let scopes = vars.scopes(&scope)?;
            let vars = vars.build(&scope)?;
            let mut state = Default::default();
            let info = apix_script::Info {
                request_id: file.display().to_string(),
                iteration_count: 1,
                ..Default::default()
            };
            let done = apix_runner::exchange(
                &client,
                &scope,
                &file,
                &vars,
                &scopes,
                &mut state,
                &net.overrides(),
                info,
                script_sender(),
            )
            .await?;
            if let Some(response) = &done.response {
                print_status(
                    &style,
                    response.status,
                    response.reason.unwrap_or(""),
                    response.timing.total.as_secs_f64() * 1000.0,
                    response.body.len(),
                );
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
            print_scripts(&style, &done.to_item(0, String::new()));
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
            let plan = apix_runner::Plan {
                root: root.clone(),
                requests,
                vars: var_args.build(&scope)?,
                scopes: var_args.scopes(&scope)?,
                environment: var_args.env.clone(),
                pinned: var_args.vars.clone(),
                data,
                iterations: iterations.unwrap_or(0),
                delay: std::time::Duration::from_millis(delay),
                bail: stop_on_failure,
                overrides: net.overrides(),
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
        Command::Import { file, out, into } => import(&style, &file, out, into),
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
                let resolved = request.resolve(&vars.build(&scope)?);
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

fn import(style: &Style, file: &Path, out: Option<PathBuf>, into: Option<PathBuf>) -> Result<bool> {
    use apix_import::postman::{self, Kind};
    use apix_import::postman_dump;

    // A folder of exports or Postman's "Export data" zip: one project.
    let is_zip = file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if file.is_dir() || is_zip {
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

    let json =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let format = apix_import::detect(&json)?.with_context(|| {
        format!(
            "{} is not an export Apix reads (Postman, Insomnia v4, OpenAPI/Swagger, HAR)",
            file.display()
        )
    })?;
    let kind = match format {
        apix_import::Format::Postman(kind) => kind,
        other => {
            let name = postman_dump::project_name(file);
            let found =
                apix_import::convert_named(&json, &name)?.expect("detected formats always convert");
            let written = found.write(&out.unwrap_or_else(|| PathBuf::from(".")), &name)?;
            println!(
                "Imported {} from {}: {} collections, {} requests, {} environments.",
                other.label(),
                file.display(),
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
        println!(
            "  {}skipped by its pre-request script{}",
            style.dim, style.reset
        );
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
        line("Redirect", format!("{} → {}", hop.status, hop.to));
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
