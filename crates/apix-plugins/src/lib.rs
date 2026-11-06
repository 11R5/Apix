//! WebAssembly plugins: custom auth types, importers and visualizers that
//! anyone can write in any language that compiles to WebAssembly.
//!
//! A plugin is one `.wasm` file in `<config>/Apix/plugins`. It runs in the
//! `wasmi` interpreter with no imports at all: no files, no network, no
//! clock, only the JSON it is given. Each call gets a fresh instance, a fuel
//! budget (a runaway loop stops) and a memory cap.
//!
//! # The interface
//!
//! The module exports:
//!
//! - `memory`
//! - `apix_alloc(len: i32) -> i32`: room for `len` bytes the host writes into
//! - `apix_manifest() -> i64`: the manifest as JSON
//! - `apix_call(hook_ptr: i32, hook_len: i32, input_ptr: i32, input_len: i32) -> i64`:
//!   runs a hook on JSON input and returns JSON
//!
//! An `i64` result is `ptr << 32 | len` of UTF-8 JSON in memory. A call
//! answers `{"ok": …}` or `{"error": "…"}`. The manifest says what the
//! plugin offers (see [`Manifest`]); `apix-plugin-sdk` does all of this for
//! plugins written in Rust.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wasmi::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Fuel for one call: enough for real work on large bodies, a few seconds
/// at most.
const FUEL: u64 = 2_000_000_000;
/// Memory one instance may grow to.
const MEMORY: usize = 256 << 20;
/// Largest plugin file read.
const MAX_FILE: u64 = 32 << 20;

/// What a plugin says about itself.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Unique among plugins: `[a-z0-9-]`, how requests name it.
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// An auth type it adds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthSpec>,
    /// A file format it imports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<ImportSpec>,
    /// A view of response bodies it offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visualize: Option<VisualizeSpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuthSpec {
    /// The name in the auth type list.
    pub label: String,
    /// What the user fills in.
    #[serde(default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(default)]
    pub label: String,
    /// Shown masked and kept out of exports like other secrets.
    #[serde(default)]
    pub secret: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportSpec {
    pub label: String,
    /// File extensions it reads, without the dot.
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VisualizeSpec {
    pub label: String,
}

/// The request an auth plugin signs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuthInput {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub config: BTreeMap<String, String>,
}

/// What an auth plugin adds to the request.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuthOutput {
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub query: Vec<(String, String)>,
}

/// What an importer read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Imported {
    #[serde(default)]
    pub name: String,
    pub requests: Vec<ImportedRequest>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportedRequest {
    /// A folder path inside the collection, `/` between parts.
    #[serde(default)]
    pub folder: String,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Option<String>,
}

/// A visualizer's view of a response.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct View {
    #[serde(default)]
    pub title: String,
    /// Plain text (shown when there is no table).
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
}

/// A loaded plugin.
#[derive(Clone)]
pub struct Plugin {
    pub manifest: Manifest,
    pub path: PathBuf,
    engine: Engine,
    module: Module,
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin")
            .field("manifest", &self.manifest)
            .field("path", &self.path)
            .finish()
    }
}

struct Limits(StoreLimits);

impl Plugin {
    /// Reads, checks and asks a plugin for its manifest.
    pub fn load(path: &Path) -> Result<Self, String> {
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        if size > MAX_FILE {
            return Err(format!("{size} bytes; plugins may be at most {MAX_FILE}"));
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::from_bytes(&bytes, path)
    }

    pub fn from_bytes(bytes: &[u8], path: &Path) -> Result<Self, String> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module =
            Module::new(&engine, bytes).map_err(|e| format!("not a WebAssembly module: {e}"))?;
        if let Some(import) = module.imports().next() {
            return Err(format!(
                "imports {}::{}; plugins may not import anything",
                import.module(),
                import.name()
            ));
        }
        let mut plugin = Self {
            manifest: Manifest::default(),
            path: path.to_path_buf(),
            engine,
            module,
        };
        let raw = plugin.run(|store, instance| {
            let manifest = instance
                .get_typed_func::<(), i64>(&*store, "apix_manifest")
                .map_err(|_| "no apix_manifest export".to_owned())?;
            manifest.call(&mut *store, ()).map_err(|e| e.to_string())
        })?;
        let manifest: Manifest =
            serde_json::from_str(&raw).map_err(|e| format!("its manifest: {e}"))?;
        if manifest.name.is_empty()
            || !manifest
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("its name {:?} must be [a-z0-9-]", manifest.name));
        }
        plugin.manifest = manifest;
        Ok(plugin)
    }

    /// A fresh instance, `work` run on it, and the JSON it pointed at. It
    /// runs on a thread of its own with a large stack: the interpreter's
    /// needs do not depend on the caller (a UI thread, a test thread).
    fn run(
        &self,
        work: impl FnOnce(&mut Store<Limits>, &wasmi::Instance) -> Result<i64, String> + Send,
    ) -> Result<String, String> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("apix-plugin".into())
                .stack_size(64 << 20)
                .spawn_scoped(scope, || self.run_here(work))
                .map_err(|e| e.to_string())?
                .join()
                .map_err(|_| "the plugin crashed the interpreter".to_owned())?
        })
    }

    fn run_here(
        &self,
        work: impl FnOnce(&mut Store<Limits>, &wasmi::Instance) -> Result<i64, String>,
    ) -> Result<String, String> {
        let mut store = Store::new(
            &self.engine,
            Limits(StoreLimitsBuilder::new().memory_size(MEMORY).build()),
        );
        store.limiter(|l| &mut l.0);
        store.set_fuel(FUEL).map_err(|e| e.to_string())?;
        let instance = Linker::<Limits>::new(&self.engine)
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|e| e.to_string())?;
        let packed = work(&mut store, &instance).map_err(|e| {
            if e.contains("fuel") {
                "it ran too long and was stopped".to_owned()
            } else {
                e
            }
        })?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or("no memory export")?;
        let (ptr, len) = ((packed >> 32) as u32 as usize, packed as u32 as usize);
        let data = memory.data(&store);
        let bytes = data
            .get(ptr..ptr.saturating_add(len))
            .ok_or("it answered outside its memory")?;
        String::from_utf8(bytes.to_vec()).map_err(|_| "its answer is not UTF-8".to_owned())
    }

    /// Runs `hook` on `input` and returns what it answered.
    pub fn call(&self, hook: &str, input: &Value) -> Result<Value, String> {
        let input = input.to_string();
        let raw = self.run(|store, instance| {
            let alloc = instance
                .get_typed_func::<i32, i32>(&*store, "apix_alloc")
                .map_err(|_| "no apix_alloc export".to_owned())?;
            let call = instance
                .get_typed_func::<(i32, i32, i32, i32), i64>(&*store, "apix_call")
                .map_err(|_| "no apix_call export".to_owned())?;
            let memory = instance
                .get_memory(&*store, "memory")
                .ok_or("no memory export")?;
            let put = |store: &mut Store<Limits>, bytes: &[u8]| -> Result<i32, String> {
                let ptr = alloc
                    .call(&mut *store, bytes.len() as i32)
                    .map_err(|e| e.to_string())?;
                memory
                    .write(&mut *store, ptr as u32 as usize, bytes)
                    .map_err(|e| e.to_string())?;
                Ok(ptr)
            };
            let hook_ptr = put(store, hook.as_bytes())?;
            let input_ptr = put(store, input.as_bytes())?;
            call.call(
                &mut *store,
                (hook_ptr, hook.len() as i32, input_ptr, input.len() as i32),
            )
            .map_err(|e| e.to_string())
        })?;
        let answer: Value =
            serde_json::from_str(&raw).map_err(|e| format!("its answer is not JSON: {e}"))?;
        if let Some(error) = answer.get("error") {
            return Err(error
                .as_str()
                .map_or_else(|| error.to_string(), str::to_owned));
        }
        answer
            .get("ok")
            .cloned()
            .ok_or_else(|| "its answer has neither ok nor error".to_owned())
    }

    fn typed<I: Serialize, O: for<'de> Deserialize<'de>>(
        &self,
        hook: &str,
        input: &I,
    ) -> Result<O, String> {
        let value = self.call(hook, &json!(input))?;
        serde_json::from_value(value).map_err(|e| format!("{}: {hook}: {e}", self.manifest.name))
    }

    pub fn auth(&self, input: &AuthInput) -> Result<AuthOutput, String> {
        self.typed("auth", input)
    }

    pub fn import(&self, file_name: &str, content: &str) -> Result<Imported, String> {
        self.typed(
            "import",
            &json!({ "file_name": file_name, "content": content }),
        )
    }

    pub fn visualize(
        &self,
        status: u16,
        headers: &[(String, String)],
        body: &str,
    ) -> Result<View, String> {
        self.typed(
            "visualize",
            &json!({ "status": status, "headers": headers, "body": body }),
        )
    }
}

/// Where plugins are kept: `<config>/Apix/plugins`.
pub fn plugins_dir() -> Option<PathBuf> {
    Some(
        match std::env::var_os("APIX_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => dirs::config_dir()?.join("Apix"),
        }
        .join("plugins"),
    )
}

/// A plugin file: loaded, or its path and why it could not be.
pub type Loaded = Result<Plugin, (PathBuf, String)>;

/// Every `.wasm` in `dir`, by file name: loaded, or why not.
pub fn load_dir(dir: &Path) -> Vec<Loaded> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wasm"))
        })
        .collect();
    paths.sort();
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .map(|path| match Plugin::load(&path) {
            Ok(p) if !seen.insert(p.manifest.name.clone()) => Err((
                path,
                format!("another plugin is already named {}", p.manifest.name),
            )),
            Ok(p) => Ok(p),
            Err(e) => Err((path, e)),
        })
        .collect()
}

/// The plugins of this run of the program, loaded once and again after
/// [`reload`].
fn registry() -> &'static Mutex<Option<Arc<Vec<Loaded>>>> {
    static REGISTRY: OnceLock<Mutex<Option<Arc<Vec<Loaded>>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(None))
}

/// The folder plugins are read from: [`plugins_dir`], or `APIX_PLUGINS_DIR`.
pub fn folder() -> Option<PathBuf> {
    std::env::var_os("APIX_PLUGINS_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(plugins_dir)
}

/// The installed plugins, and the files that failed to load.
pub fn installed() -> Arc<Vec<Loaded>> {
    let mut slot = registry().lock().unwrap_or_else(|e| e.into_inner());
    slot.get_or_insert_with(|| Arc::new(folder().map(|d| load_dir(&d)).unwrap_or_default()))
        .clone()
}

/// Forgets the loaded plugins; the next use reads the folder again.
pub fn reload() {
    *registry().lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// The installed plugin called `name`.
pub fn find(name: &str) -> Option<Plugin> {
    installed()
        .iter()
        .filter_map(|p| p.as_ref().ok())
        .find(|p| p.manifest.name == name)
        .cloned()
}

/// Copies a plugin file into the plugins folder after checking that it loads.
pub fn install(file: &Path) -> Result<PathBuf, String> {
    let plugin = Plugin::load(file)?;
    let dir = folder().ok_or("no configuration folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(format!("{}.wasm", plugin.manifest.name));
    std::fs::copy(file, &target).map_err(|e| e.to_string())?;
    reload();
    Ok(target)
}
