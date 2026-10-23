//! Every file the store touches goes through here. A path under the root of
//! an unlocked encrypted collection is served from its vault, decrypted in
//! memory; every other path goes to the disk. So the rest of Apix keeps
//! using ordinary paths (`…/my-api/users/get-user.apix`) whether or not the
//! collection is encrypted, and nothing decrypted is ever written to disk.

use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

pub use apix_vault::{KdfCost, RecoveryKey, Vault, VaultError, is_vault as is_encrypted, keychain};

/// Unlocked vaults, by absolute root. Locking drops the vault, which wipes its key.
static VAULTS: Mutex<Vec<(PathBuf, Vault)>> = Mutex::new(Vec::new());

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Makes the collection at `vault.root()` readable and writable through its paths.
pub fn unlock(vault: Vault) {
    let root = absolute(vault.root());
    let mut vaults = VAULTS.lock().expect("vault registry");
    vaults.retain(|(r, _)| *r != root);
    vaults.push((root, vault));
}

/// Forgets the key of the collection at `root`. Returns whether it was unlocked.
pub fn lock(root: &Path) -> bool {
    let root = absolute(root);
    let mut vaults = VAULTS.lock().expect("vault registry");
    let before = vaults.len();
    vaults.retain(|(r, _)| *r != root);
    vaults.len() != before
}

/// Locks every encrypted collection (on idle, or before quitting).
pub fn lock_all() {
    VAULTS.lock().expect("vault registry").clear();
}

pub fn is_unlocked(root: &Path) -> bool {
    let root = absolute(root);
    VAULTS
        .lock()
        .expect("vault registry")
        .iter()
        .any(|(r, _)| *r == root)
}

/// The roots of every unlocked collection.
pub fn unlocked_roots() -> Vec<PathBuf> {
    VAULTS
        .lock()
        .expect("vault registry")
        .iter()
        .map(|(r, _)| r.clone())
        .collect()
}

/// The encrypted collection `path` is in, if any, whether locked or not.
pub fn encrypted_root(path: &Path) -> Option<PathBuf> {
    absolute(path)
        .ancestors()
        .find(|dir| is_encrypted(dir))
        .map(Path::to_path_buf)
}

/// Re-reads an unlocked vault's index from disk (after a `git pull`).
pub fn refresh(root: &Path) -> io::Result<()> {
    with_vault(root, |vault, _| vault.reload().map_err(to_io)).unwrap_or(Ok(()))
}

/// Runs `f` with the vault that holds `path` and the path inside it ("" for
/// the root), or returns `None` when `path` is not in an unlocked vault.
pub fn with_vault<R>(path: &Path, f: impl FnOnce(&mut Vault, &str) -> R) -> Option<R> {
    let path = absolute(path);
    let mut vaults = VAULTS.lock().expect("vault registry");
    let (root, vault) = vaults.iter_mut().find(|(r, _)| path.starts_with(r))?;
    let rel: Vec<String> = path
        .strip_prefix(&*root)
        .ok()?
        .components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    Some(f(vault, &rel.join("/")))
}

fn to_io(e: VaultError) -> io::Error {
    let kind = match e {
        VaultError::NotFound(_) => io::ErrorKind::NotFound,
        VaultError::Io { ref source, .. } => source.kind(),
        _ => io::ErrorKind::Other,
    };
    io::Error::new(kind, e.to_string())
}

pub fn read_to_string(path: &Path) -> io::Result<String> {
    match with_vault(path, |v, rel| v.read_to_string(rel).map(|s| s.to_string())) {
        Some(result) => result.map_err(to_io),
        None => std::fs::read_to_string(path),
    }
}

pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    match with_vault(path, |v, rel| v.write(rel, contents.as_ref())) {
        Some(result) => result.map_err(to_io),
        None => std::fs::write(path, contents),
    }
}

pub fn is_file(path: &Path) -> bool {
    with_vault(path, |v, rel| !rel.is_empty() && v.is_file(rel)).unwrap_or_else(|| path.is_file())
}

pub fn is_dir(path: &Path) -> bool {
    with_vault(path, |v, rel| rel.is_empty() || v.is_dir(rel)).unwrap_or_else(|| path.is_dir())
}

pub fn exists(path: &Path) -> bool {
    is_file(path) || is_dir(path)
}

/// The entries directly inside `dir`, as full paths.
pub fn read_dir(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if let Some(children) = with_vault(dir, |v, rel| v.children(rel)) {
        return Ok(children
            .into_iter()
            .map(|(name, _)| dir.join(name))
            .collect());
    }
    std::fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect()
}

pub fn create_dir_all(path: &Path) -> io::Result<()> {
    match with_vault(path, |v, rel| {
        if rel.is_empty() {
            Ok(())
        } else {
            v.create_dir(rel)
        }
    }) {
        Some(result) => result.map_err(to_io),
        None => std::fs::create_dir_all(path),
    }
}

/// Renames a file or folder. Both paths must be in the same vault, or both outside one.
pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let to_rel = with_vault(to, |_, rel| rel.to_owned());
    match with_vault(from, |v, from_rel| {
        (v.root().to_path_buf(), from_rel.to_owned())
    }) {
        Some((root, from_rel)) => {
            let same_vault = to.starts_with(&root) || absolute(to).starts_with(absolute(&root));
            let to_rel = to_rel
                .filter(|_| same_vault)
                .ok_or_else(|| io::Error::other("cannot move out of an encrypted collection"))?;
            with_vault(from, |v, _| v.rename(&from_rel, &to_rel))
                .unwrap_or(Err(VaultError::NotFound(from_rel)))
                .map_err(to_io)
        }
        None if to_rel.is_some() => {
            Err(io::Error::other("cannot move into an encrypted collection"))
        }
        None => std::fs::rename(from, to),
    }
}

pub fn remove_file(path: &Path) -> io::Result<()> {
    match with_vault(path, |v, rel| v.remove(rel)) {
        Some(result) => result.map_err(to_io),
        None => std::fs::remove_file(path),
    }
}

pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    match with_vault(path, |v, rel| v.remove_dir(rel)) {
        Some(result) => result.map_err(to_io),
        None => std::fs::remove_dir_all(path),
    }
}

pub fn copy(from: &Path, to: &Path) -> io::Result<()> {
    let contents = match with_vault(from, |v, rel| v.read(rel).map(|b| b.to_vec())) {
        Some(result) => result.map_err(to_io)?,
        None => std::fs::read(from)?,
    };
    write(to, contents)
}

/// Whether `path` lives in an unlocked encrypted collection (and so is not on disk).
pub fn is_virtual(path: &Path) -> bool {
    with_vault(path, |_, _| ()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_paths_into_an_unlocked_vault_only() {
        let root = std::env::temp_dir().join(format!("apix-vfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (vault, _) = Vault::create(&root, "pw", KdfCost::INSECURE_FAST).unwrap();
        let file = root.join("users").join("get.apix");

        unlock(vault);
        assert!(is_unlocked(&root) && is_virtual(&file));
        write(&file, "url = \"x\"").unwrap();
        create_dir_all(&root.join("empty")).unwrap();
        assert!(is_file(&file) && is_dir(&root.join("users")) && is_dir(&root.join("empty")));
        assert!(!file.exists(), "nothing decrypted reaches the disk");
        assert_eq!(read_to_string(&file).unwrap(), "url = \"x\"");
        let mut names: Vec<String> = read_dir(&root)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["empty", "users"]);
        rename(&root.join("users"), &root.join("people")).unwrap();
        assert!(is_file(&root.join("people").join("get.apix")));
        assert!(rename(&root.join("people"), &std::env::temp_dir().join("out")).is_err());

        assert!(lock(&root));
        assert!(
            !is_file(&root.join("people").join("get.apix")),
            "locked: gone"
        );
        assert_eq!(encrypted_root(&root.join("people")), Some(absolute(&root)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_store_works_unchanged_on_an_encrypted_collection() {
        use crate::store::{self, CollectionMeta, EnvVar, EnvironmentParts, TreeNode};
        use crate::{Collection, Method, Request};

        let root = std::env::temp_dir().join(format!("apix-vfs-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (vault, _) = Vault::create(&root, "pw", KdfCost::INSECURE_FAST).unwrap();
        unlock(vault);

        let meta = CollectionMeta {
            name: "Secret".into(),
            ..CollectionMeta::default()
        };
        store::save_collection_meta(&root, &meta).unwrap();
        let collection = Collection::open(&root).unwrap();
        let folder = store::create_folder(&root, "Admin").unwrap();
        let mut request = Request::new(Method::Get, "https://internal/me");
        request.name = "Who am I".into();
        let path = store::create_request(&folder, &request).unwrap();
        let renamed = store::rename_request(&path, "Me").unwrap();
        assert_eq!(
            Collection::find_root(&renamed).as_deref(),
            Some(root.as_path())
        );
        let rows = [EnvVar {
            key: "token".into(),
            value: "s3cr3t".into(),
            secret: true,
        }];
        EnvironmentParts::default()
            .with_rows(&rows)
            .write(&root, "dev")
            .unwrap();

        let tree = collection.tree();
        let TreeNode::Folder { children, .. } = &tree[0] else {
            panic!("admin folder first: {tree:?}")
        };
        assert_eq!(
            children,
            &[TreeNode::Request {
                path: renamed.clone()
            }]
        );
        assert_eq!(
            collection.environment("dev").unwrap().vars["token"],
            "s3cr3t"
        );
        assert_eq!(store::load_request(&renamed).unwrap().name, "Me");

        lock(&root);
        assert!(Collection::open(&root).is_err(), "locked: unreadable");
        let reopened = Vault::unlock(&root, "pw").unwrap();
        assert!(reopened.is_file("admin/me.apix"));
        assert!(reopened.is_file("environments/dev.local.toml"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
