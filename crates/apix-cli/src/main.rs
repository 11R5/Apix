use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use apix_core::store::load_request;
use apix_core::vfs::{self, KdfCost, Vault};
use apix_core::{Collection, Project, Request, Variables};
use apix_http::{HttpClient, Response, human_size};
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
        /// Give up after this many seconds (overrides the request's settings)
        #[arg(long, value_name = "SECONDS")]
        timeout: Option<f64>,
        /// Do not verify the server certificate
        #[arg(short = 'k', long)]
        insecure: bool,
        /// Send through this proxy (overrides the environment)
        #[arg(long, value_name = "URL")]
        proxy: Option<String>,
    },
    /// Send every request in a collection folder; exits non-zero if any fails
    Run {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[command(flatten)]
        vars: VarArgs,
    },
    /// Print a request as a shell command (curl, curl.exe, PowerShell) or as
    /// code (fetch, axios, Python, Go, Rust, C#, Java, PHP, Swift, Kotlin)
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
    /// Import a Postman collection (v2.0/v2.1) or environment export, or a
    /// whole folder / "Export data" zip of them as a project
    Import {
        /// A Postman collection or environment `.json`, a folder of them, or a `.zip`
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

fn parse_var(raw: &str) -> Result<(String, String), String> {
    raw.split_once('=')
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .ok_or_else(|| format!("expected KEY=VALUE, got `{raw}`"))
}

/// The collection, and the project around it, that a file or folder belongs to.
struct Scope {
    collection: Option<Collection>,
    project: Option<Project>,
}

impl Scope {
    fn find(start: &Path) -> Result<Self> {
        let collection = Collection::find_root(start)
            .map(Collection::open)
            .transpose()?;
        let project = collection
            .as_ref()
            .and_then(|c| c.root.parent())
            .and_then(Project::find_root)
            .map(Project::open)
            .transpose()?;
        Ok(Self {
            collection,
            project,
        })
    }

    /// Loads a request and fills in the auth and transport settings it
    /// inherits from its folders, collection or project.
    fn load(&self, path: &Path) -> Result<Request> {
        let mut request = load_request(path)?;
        apix_core::project::apply_inherited_auth(
            &mut request,
            path,
            self.collection.as_ref(),
            self.project.as_ref(),
        )?;
        apix_core::project::apply_inherited_settings(
            &mut request,
            path,
            self.collection.as_ref(),
            self.project.as_ref(),
        )?;
        Ok(request)
    }
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
            timeout,
            insecure,
            proxy,
        } => {
            unlock_around(&file, &var)?;
            let scope = Scope::find(&file)?;
            let vars = vars.build(&scope)?;
            let overrides = apix_core::RequestSettings {
                timeout_ms: timeout.map(|s| (s * 1000.0) as u64),
                tls_verify: insecure.then_some(false),
                proxy,
                ..Default::default()
            };
            let response = send_file(&client, &scope, &file, &vars, &overrides).await?;
            print_status(&style, &response);
            if include_headers {
                for (k, v) in &response.headers {
                    println!("{}{k}{}: {v}", style.dim, style.reset);
                }
                println!();
            }
            if network {
                print_network(&style, &response);
            }
            println!("{}", response.pretty_body());
            Ok(response.is_success())
        }
        Command::Run { dir, vars } => {
            unlock_around(&dir, &var)?;
            let root = Collection::find_root(&dir).with_context(|| {
                format!("no apix.toml found in {} or its parents", dir.display())
            })?;
            let scope = Scope::find(&root)?;
            let collection = Collection::open(&root)?;
            let vars = vars.build(&scope)?;
            let (mut passed, mut failed) = (0, 0);
            for path in collection.request_paths() {
                let label = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                match send_file(&client, &scope, &path, &vars, &Default::default()).await {
                    Ok(response) => {
                        if response.is_success() {
                            passed += 1
                        } else {
                            failed += 1
                        }
                        print!("{label:<40} ");
                        print_status(&style, &response);
                    }
                    Err(err) => {
                        failed += 1;
                        println!("{label:<40} {}error{} {err:#}", style.red, style.reset);
                    }
                }
            }
            println!("\n{passed} passed, {failed} failed");
            Ok(failed == 0)
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
    match postman::detect(&json)? {
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

async fn send_file(
    client: &HttpClient,
    scope: &Scope,
    path: &Path,
    vars: &Variables,
    overrides: &apix_core::RequestSettings,
) -> Result<Response> {
    let mut request = scope.load(path)?;
    request.settings = overrides.clone().or_inherit(&request.settings);
    let resolved = request.resolve(vars);
    if !resolved.missing.is_empty() {
        let names: Vec<_> = resolved.missing.into_iter().collect();
        bail!("undefined variable(s): {}", names.join(", "));
    }
    // Body files are relative to the request file.
    let request = match path.parent() {
        Some(dir) => resolved.request.with_files_relative_to(dir),
        None => resolved.request,
    };
    Ok(client.send(&request).await?)
}

fn print_status(style: &Style, response: &Response) {
    let color = if response.is_success() {
        style.green
    } else {
        style.red
    };
    println!(
        "{color}{} {}{}  {}{:.1} ms · {}{}",
        response.status,
        response.reason.unwrap_or(""),
        style.reset,
        style.dim,
        response.timing.total.as_secs_f64() * 1000.0,
        human_size(response.body.len()),
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
    dim: &'static str,
    reset: &'static str,
}

impl Style {
    fn detect() -> Self {
        if std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none() {
            Self {
                green: "\x1b[32m",
                red: "\x1b[31m",
                dim: "\x1b[2m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                green: "",
                red: "",
                dim: "",
                reset: "",
            }
        }
    }
}
