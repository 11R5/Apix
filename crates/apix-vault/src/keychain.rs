//! A vault's password, remembered in the operating system's credential
//! store: Credential Manager on Windows, the Keychain on macOS, the Secret
//! Service (GNOME Keyring, KWallet) on Linux. The OS keeps it encrypted
//! and gives it only to programs running as the same user.
//!
//! One entry per vault folder, under the service name "Apix". Nothing here
//! is required: when the store is missing or refuses, Apix asks for the
//! password as before.

use std::path::Path;

const SERVICE: &str = "Apix";

/// The entry name for the vault at `root`: its absolute path (Windows
/// paths in lower case, as the file system ignores case there).
fn account(root: &Path) -> String {
    let path = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    let mut text = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    format!("vault:{}", text.trim_end_matches(['/', '\\']))
}

fn entry(service: &str, root: &Path) -> Result<keyring::Entry, String> {
    keyring::Entry::new(service, &account(root)).map_err(|e| e.to_string())
}

fn save_in(service: &str, root: &Path, password: &str) -> Result<(), String> {
    entry(service, root)?
        .set_password(password)
        .map_err(|e| e.to_string())
}

fn load_in(service: &str, root: &Path) -> Option<String> {
    entry(service, root).ok()?.get_password().ok()
}

fn forget_in(service: &str, root: &Path) -> Result<(), String> {
    match entry(service, root)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Remembers `password` for the vault at `root`, replacing an older one.
pub fn save(root: &Path, password: &str) -> Result<(), String> {
    save_in(SERVICE, root, password)
}

/// The remembered password of the vault at `root`, if there is one.
pub fn load(root: &Path) -> Option<String> {
    load_in(SERVICE, root)
}

/// Forgets the vault's password; forgetting one that is not there is fine.
pub fn forget(root: &Path) -> Result<(), String> {
    forget_in(SERVICE, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_entries_by_absolute_folder() {
        let a = account(Path::new("some/vault/"));
        let b = account(&std::env::current_dir().unwrap().join("some").join("vault"));
        assert_eq!(a, b);
        assert!(a.starts_with("vault:"));
        if cfg!(windows) {
            assert_eq!(
                account(Path::new("C:\\Vault")),
                account(Path::new("c:\\vault"))
            );
        }
    }

    /// Talks to the real credential store, under a test service name, and
    /// cleans up after itself. Skipped where no store is available (CI on
    /// Linux without a Secret Service).
    #[test]
    fn saves_loads_and_forgets() {
        let service = "Apix-test";
        let root = std::env::temp_dir().join(format!("apix-keychain-{}", std::process::id()));
        if save_in(service, &root, "first").is_err() {
            eprintln!("no credential store here; skipped");
            return;
        }
        assert_eq!(load_in(service, &root).as_deref(), Some("first"));
        save_in(service, &root, "second").unwrap();
        assert_eq!(
            load_in(service, &root).as_deref(),
            Some("second"),
            "replaced"
        );
        forget_in(service, &root).unwrap();
        assert_eq!(load_in(service, &root), None);
        forget_in(service, &root).unwrap();
    }
}
