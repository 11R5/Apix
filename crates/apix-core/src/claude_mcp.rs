//! Connecting `apix mcp` to Claude: an entry in Claude Desktop's
//! `claude_desktop_config.json` (other servers and settings are kept, the
//! file before the first change is copied to `.bak`), or the `claude mcp
//! add` command for Claude Code.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// Claude Desktop's configuration file on this system. `APIX_CLAUDE_CONFIG`
/// names another one (for tests).
pub fn desktop_config_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("APIX_CLAUDE_CONFIG")
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }
    let base = if cfg!(target_os = "macos") {
        dirs::home_dir()?
            .join("Library")
            .join("Application Support")
    } else {
        // %APPDATA% on Windows, ~/.config elsewhere.
        dirs::config_dir()?
    };
    Some(base.join("Claude").join("claude_desktop_config.json"))
}

/// The server's name in Claude: `apix-<collection>`, lower case with dashes.
pub fn server_name(collection: &str) -> String {
    let mut slug = String::new();
    for c in collection.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "apix".to_owned()
    } else {
        format!("apix-{slug}")
    }
}

/// The `apix` command: next to the running program when it is there (an
/// installed Apix), otherwise whatever `apix` the PATH finds.
pub fn apix_command() -> String {
    let name = if cfg!(windows) { "apix.exe" } else { "apix" };
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(name)))
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "apix".to_owned())
}

/// The arguments that serve `collection`.
pub fn server_args(collection: &Path, list_only: bool) -> Vec<String> {
    let mut args = vec!["mcp".to_owned(), collection.to_string_lossy().into_owned()];
    if list_only {
        args.push("--list-only".to_owned());
    }
    args
}

/// The entry for Claude Desktop, as JSON (to show or copy).
pub fn desktop_entry(name: &str, command: &str, args: &[String]) -> Value {
    json!({ "mcpServers": { name: { "command": command, "args": args } } })
}

/// What adding did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    /// A new entry.
    New,
    /// An entry of that name was replaced.
    Replaced,
}

/// Adds (or replaces) server `name` in the Claude Desktop configuration at
/// `path`, creating the file when there is none.
pub fn add_to_desktop(
    path: &Path,
    name: &str,
    command: &str,
    args: &[String],
) -> Result<Added, String> {
    let mut config = match std::fs::read_to_string(path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?,
        Ok(_) => json!({}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let root = config
        .as_object_mut()
        .ok_or_else(|| format!("{} does not hold a JSON object", path.display()))?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "mcpServers is not an object".to_owned())?;
    let added = if servers.contains_key(name) {
        Added::Replaced
    } else {
        Added::New
    };
    servers.insert(name.to_owned(), json!({ "command": command, "args": args }));
    write(path, &config)?;
    Ok(added)
}

/// Removes server `name`; `false` when it was not there.
pub fn remove_from_desktop(path: &Path, name: &str) -> Result<bool, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(false);
    };
    let mut config: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let removed = config
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .is_some_and(|s| s.remove(name).is_some());
    if removed {
        write(path, &config)?;
    }
    Ok(removed)
}

fn write(path: &Path, config: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    // The file as it was before Apix first touched it.
    let backup = path.with_extension("json.bak");
    if path.is_file() && !backup.exists() {
        let _ = std::fs::copy(path, &backup);
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

/// The command that adds the server to Claude Code for every project.
pub fn claude_code_command(name: &str, command: &str, args: &[String]) -> Vec<String> {
    let mut out: Vec<String> = [
        "claude", "mcp", "add", "--scope", "user", name, "--", command,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    out.extend(args.iter().cloned());
    out
}

/// The same command as one line for a terminal (quoted where needed).
pub fn shell_line(words: &[String]) -> String {
    words
        .iter()
        .map(|w| {
            if w.is_empty() || w.contains([' ', '"', '\'']) {
                format!("\"{}\"", w.replace('"', "\\\""))
            } else {
                w.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_commands() {
        assert_eq!(server_name("JSONPlaceholder"), "apix-jsonplaceholder");
        assert_eq!(server_name("My Shop API!"), "apix-my-shop-api");
        assert_eq!(server_name("Çay"), "apix-ay");
        assert_eq!(server_name("!!"), "apix");
        let args = server_args(Path::new("C:/My APIs/shop"), true);
        assert_eq!(args, ["mcp", "C:/My APIs/shop", "--list-only"]);
        let cmd = claude_code_command("apix-shop", "apix", &args);
        assert_eq!(
            &cmd[..7],
            ["claude", "mcp", "add", "--scope", "user", "apix-shop", "--"]
        );
        assert_eq!(
            shell_line(&cmd),
            "claude mcp add --scope user apix-shop -- apix mcp \"C:/My APIs/shop\" --list-only"
        );
        assert_eq!(
            desktop_entry("apix-shop", "apix", &args)["mcpServers"]["apix-shop"]["args"][2],
            "--list-only"
        );
    }

    #[test]
    fn the_desktop_configuration_keeps_what_was_there() {
        let dir = std::env::temp_dir().join(format!("apix-claude-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("Claude").join("claude_desktop_config.json");
        // No file yet.
        let args = vec!["mcp".to_owned(), "C:/shop".to_owned()];
        assert_eq!(
            add_to_desktop(&path, "apix-shop", "apix", &args).unwrap(),
            Added::New
        );
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["apix-shop"]["command"], "apix");

        // Someone else's servers and settings stay.
        std::fs::write(
            &path,
            r#"{"theme":"dark","mcpServers":{"files":{"command":"npx","args":["fs"]}}}"#,
        )
        .unwrap();
        assert_eq!(
            add_to_desktop(&path, "apix-shop", "apix", &args).unwrap(),
            Added::New
        );
        assert_eq!(
            add_to_desktop(&path, "apix-shop", "apix2", &args).unwrap(),
            Added::Replaced
        );
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["theme"], "dark");
        assert_eq!(config["mcpServers"]["files"]["command"], "npx");
        assert_eq!(config["mcpServers"]["apix-shop"]["command"], "apix2");
        // The first version was kept aside.
        assert!(
            dir.join("Claude")
                .join("claude_desktop_config.json.bak")
                .is_file()
        );

        assert!(remove_from_desktop(&path, "apix-shop").unwrap());
        assert!(!remove_from_desktop(&path, "apix-shop").unwrap());
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config["mcpServers"].get("apix-shop").is_none());
        assert_eq!(config["mcpServers"]["files"]["command"], "npx");

        std::fs::write(&path, "{ not json").unwrap();
        assert!(
            add_to_desktop(&path, "apix-shop", "apix", &args)
                .unwrap_err()
                .contains("not valid JSON")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
