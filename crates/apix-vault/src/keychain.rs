//! A vault's password, remembered in the operating system's credential
//! store: Credential Manager on Windows, the Keychain on macOS, the Secret
//! Service (GNOME Keyring, KWallet) on Linux. The OS keeps it encrypted
//! and gives it only to programs running as the same user.
//!
//! One entry per vault folder, under the service name "Apix". Nothing here
//! is required: when the store is missing or refuses, Apix asks for the
//! password as before.
//!
//! Any text can be kept the same way ([`save_text`]); Apix uses that for
//! environment secrets.
//!
//! Any text can be kept the same way ([`save_text`]); Apix uses that for
//! environment secrets.

use std::path::Path;

const SERVICE: &str = "Apix";

/// Windows' credential store can bring deleted entries back when one
/// process writes from two threads at once, so every call takes its turn.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _turn = turn();
    save_in(SERVICE, root, password)
}

/// The remembered password of the vault at `root`, if there is one.
pub fn load(root: &Path) -> Option<String> {
    let _turn = turn();
    load_in(SERVICE, root)
}

/// Forgets the vault's password; forgetting one that is not there is fine.
pub fn forget(root: &Path) -> Result<(), String> {
    let _turn = turn();
    forget_in(SERVICE, root)
}

/// Bytes of text per entry. Windows keeps at most 2560 bytes per credential
/// and stores text as UTF-16, which is at most twice the UTF-8 size.
const CHUNK: usize = 1200;

fn text_entry(service: &str, account: &str, part: usize) -> Result<keyring::Entry, String> {
    let account = if part == 0 {
        account.to_owned()
    } else {
        format!("{account}#{part}")
    };
    keyring::Entry::new(service, &account).map_err(|e| e.to_string())
}

/// Splits `text` into pieces of at most `CHUNK` bytes, on character boundaries.
fn chunks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.len() > CHUNK {
        let mut cut = CHUNK;
        while !rest.is_char_boundary(cut) {
            cut -= 1;
        }
        out.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    out.push(rest);
    out
}

fn get(entry: &keyring::Entry) -> Result<Option<String>, String> {
    match entry.get_password() {
        Ok(text) => Ok(Some(text)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn save_text_in(service: &str, account: &str, text: &str) -> Result<(), String> {
    forget_text_in(service, account)?;
    let parts = chunks(text);
    // Later parts first: a half-written value has no first entry and reads as missing.
    for (i, part) in parts.iter().enumerate().skip(1) {
        text_entry(service, account, i)?
            .set_password(part)
            .map_err(|e| e.to_string())?;
    }
    text_entry(service, account, 0)?
        .set_password(&format!("{}:{}", parts.len(), parts[0]))
        .map_err(|e| e.to_string())
}

fn load_text_in(service: &str, account: &str) -> Result<Option<String>, String> {
    let Some(first) = get(&text_entry(service, account, 0)?)? else {
        return Ok(None);
    };
    let (count, head) = first
        .split_once(':')
        .and_then(|(n, head)| Some((n.parse::<usize>().ok()?, head)))
        .ok_or("unreadable keychain entry")?;
    let mut text = head.to_owned();
    for i in 1..count {
        let part = get(&text_entry(service, account, i)?)?.ok_or("incomplete keychain entry")?;
        text.push_str(&part);
    }
    Ok(Some(text))
}

fn forget_text_in(service: &str, account: &str) -> Result<(), String> {
    let first = get(&text_entry(service, account, 0)?)?;
    let count = first
        .as_deref()
        .and_then(|f| f.split_once(':'))
        .and_then(|(n, _)| n.parse::<usize>().ok())
        .unwrap_or(1);
    for i in (0..count.max(1)).rev() {
        match text_entry(service, account, i)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

/// Keeps `text` of any length under `account`, replacing what was there.
/// Long text is split over several entries (`account`, `account#1`, …).
pub fn save_text(account: &str, text: &str) -> Result<(), String> {
    let _turn = turn();
    save_text_in(SERVICE, account, text)
}

/// The text kept under `account`: `None` when there is none, an error when
/// the store cannot be read.
pub fn load_text(account: &str) -> Result<Option<String>, String> {
    let _turn = turn();
    load_text_in(SERVICE, account)
}

/// Removes the text kept under `account`; removing nothing is fine.
pub fn forget_text(account: &str) -> Result<(), String> {
    let _turn = turn();
    forget_text_in(SERVICE, account)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows' credential store can bring deleted entries back when the
    /// same process writes from two threads at once; the store tests take turns.
    static STORE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        let _turn = STORE.lock().unwrap_or_else(|e| e.into_inner());
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

    #[test]
    fn splits_text_on_character_boundaries() {
        let text = "ü".repeat(CHUNK);
        let parts = chunks(&text);
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| p.len() <= CHUNK));
        assert_eq!(parts.concat(), text);
        assert_eq!(chunks(""), [""]);
    }

    #[test]
    fn keeps_long_text_over_several_entries() {
        let _turn = STORE.lock().unwrap_or_else(|e| e.into_inner());
        let service = "Apix-test";
        let account = format!("text-{}", std::process::id());
        let long = format!("[vars]\ntoken = \"{}\"\n", "x".repeat(3 * CHUNK));
        if save_text_in(service, &account, &long).is_err() {
            eprintln!("no credential store here; skipped");
            return;
        }
        assert_eq!(
            load_text_in(service, &account).unwrap().as_deref(),
            Some(long.as_str())
        );
        save_text_in(service, &account, "short").unwrap();
        assert_eq!(
            load_text_in(service, &account).unwrap().as_deref(),
            Some("short")
        );
        assert_eq!(
            get(&text_entry(service, &account, 1).unwrap()).unwrap(),
            None,
            "old parts removed"
        );
        forget_text_in(service, &account).unwrap();
        assert_eq!(load_text_in(service, &account).unwrap(), None);
    }
}
