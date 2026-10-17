use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use apix_core::store::load_request;
use apix_core::{Collection, Project, Request, Variables};
use apix_http::{HttpClient, Response, human_size};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "apix", version, about = "Blazingly fast API client")]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    },
    /// Send every request in a collection folder; exits non-zero if any fails
    Run {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[command(flatten)]
        vars: VarArgs,
    },
    /// Print a request as a shell command (curl for bash, curl.exe for cmd, or PowerShell)
    Shell {
        file: PathBuf,
        /// Which shell to write for
        #[arg(long = "as", value_enum, default_value_t = ShellArg::Bash)]
        shell: ShellArg,
        /// Keep {{variables}} instead of filling them in
        #[arg(long)]
        raw: bool,
        #[command(flatten)]
        vars: VarArgs,
    },
    /// Import a Postman collection (v2.0/v2.1) or environment export
    Import {
        /// A Postman `.postman_collection.json` or `.postman_environment.json` file
        file: PathBuf,
        /// Folder to create for a collection (default: ./<collection-name>)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Collection to add an environment to (default: the collection around the current folder)
        #[arg(long)]
        into: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ShellArg {
    Bash,
    Cmd,
    Powershell,
}

impl From<ShellArg> for apix_codegen::Shell {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => apix_codegen::Shell::Bash,
            ShellArg::Cmd => apix_codegen::Shell::Cmd,
            ShellArg::Powershell => apix_codegen::Shell::PowerShell,
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

    /// Loads a request and fills in the auth it inherits from its folders,
    /// collection or project.
    fn load(&self, path: &Path) -> Result<Request> {
        let mut request = load_request(path)?;
        apix_core::project::apply_inherited_auth(
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
    match cli.command {
        Command::Send {
            file,
            vars,
            include_headers,
        } => {
            let scope = Scope::find(&file)?;
            let vars = vars.build(&scope)?;
            let response = send_file(&client, &scope, &file, &vars).await?;
            print_status(&style, &response);
            if include_headers {
                for (k, v) in &response.headers {
                    println!("{}{k}{}: {v}", style.dim, style.reset);
                }
                println!();
            }
            println!("{}", response.pretty_body());
            Ok(response.is_success())
        }
        Command::Run { dir, vars } => {
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
                match send_file(&client, &scope, &path, &vars).await {
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
        Command::Shell {
            file,
            shell,
            raw,
            vars,
        } => {
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
            println!("{}", apix_codegen::to_shell(&request, shell.into()));
            Ok(true)
        }
    }
}

fn import(style: &Style, file: &Path, out: Option<PathBuf>, into: Option<PathBuf>) -> Result<bool> {
    use apix_import::postman::{self, Kind};

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
) -> Result<Response> {
    let request = scope.load(path)?;
    let resolved = request.resolve(vars);
    if !resolved.missing.is_empty() {
        let names: Vec<_> = resolved.missing.into_iter().collect();
        bail!("undefined variable(s): {}", names.join(", "));
    }
    Ok(client.send(&resolved.request).await?)
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
