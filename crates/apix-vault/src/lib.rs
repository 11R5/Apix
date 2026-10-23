//! Password-encrypted Apix collections.
//!
//! An encrypted collection is a folder that reveals only that it is one:
//!
//! ```text
//! my-api/
//! ├── apix-vault.toml        plaintext header: format, Argon2id cost and salts,
//! │                          the data key wrapped by the password and by the recovery key
//! ├── .gitignore             keeps objects/local/ (this machine's secrets) out of git
//! └── objects/
//!     ├── index.apixe        the real paths of every file, encrypted
//!     ├── 3f9c…e1.apixe      one encrypted file each; names are random
//!     └── local/             `*.local.toml` secrets, same format, never committed
//! ```
//!
//! Every file is sealed with XChaCha20-Poly1305 under a random 256-bit data
//! key, with a fresh random nonce per write. The object id is the associated
//! data, so blobs cannot be swapped or renamed without detection. The data key
//! is stored twice, wrapped by a key derived from the password (Argon2id) and
//! by one derived from the recovery key, so changing the password never
//! re-encrypts the files. Git still sees which object changed, never what is
//! in it. Security rests on the cipher and the password, not on secrecy of
//! the format: Apix is open source.

pub mod keychain;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const HEADER_FILE: &str = "apix-vault.toml";
pub const OBJECTS_DIR: &str = "objects";
const LOCAL_DIR: &str = "local";
const INDEX_ID: &str = "index";
const OBJECT_EXT: &str = "apixe";
/// `APXE` and a format version byte, at the start of every blob.
const MAGIC: &[u8; 5] = b"APXE\x01";
const NONCE_LEN: usize = 24;
/// Files ending in this stay on this machine (`objects/local/`).
const LOCAL_SUFFIX: &str = ".local.toml";
/// Crockford base32: no I, L, O or U, so the recovery key reads unambiguously.
const BASE32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

type Key = Zeroizing<[u8; 32]>;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("wrong password")]
    WrongPassword,
    #[error("wrong recovery key")]
    WrongRecoveryKey,
    #[error("{0} is not an encrypted Apix collection")]
    NotAVault(PathBuf),
    #[error("the encrypted collection is damaged: {0}")]
    Corrupt(String),
    #[error("{0} already exists and is not empty")]
    NotEmpty(PathBuf),
    #[error("no such file in the encrypted collection: {0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> VaultError + '_ {
    move |source| VaultError::Io {
        path: path.to_owned(),
        source,
    }
}

/// How expensive deriving a key from the password is. The default follows
/// OWASP's Argon2id advice and takes a fraction of a second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfCost {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfCost {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl KdfCost {
    /// Cheap enough for unit tests. Never use it for real data.
    #[doc(hidden)]
    pub const INSECURE_FAST: Self = Self {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    apix_vault: u32,
    kdf: KdfCost,
    /// Salt for the password key.
    salt: String,
    /// Salt for the recovery key.
    recovery_salt: String,
    /// The data key sealed under the password key.
    password_key: String,
    /// The data key sealed under the recovery key.
    recovery_key: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    /// Logical path (`users/get-user.apix`) → object id.
    #[serde(default)]
    files: BTreeMap<String, String>,
    /// Folders, so empty ones survive.
    #[serde(default)]
    dirs: BTreeSet<String>,
}

/// The one-time recovery key, shown once when a vault is created, e.g.
/// `7K2M-9QXA-…`. With it the vault opens even if the password is lost.
pub struct RecoveryKey(Zeroizing<String>);

impl RecoveryKey {
    fn random() -> (Self, Zeroizing<[u8; 20]>) {
        let mut bytes = Zeroizing::new([0u8; 20]);
        OsRng.fill_bytes(bytes.as_mut());
        (Self(Zeroizing::new(encode_recovery(bytes.as_ref()))), bytes)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey(…)")
    }
}

/// 20 bytes → 32 base32 characters in groups of four.
fn encode_recovery(bytes: &[u8]) -> String {
    let mut bits = 0u32;
    let mut held = 0u32;
    let mut chars = Vec::new();
    for &byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        held += 8;
        while held >= 5 {
            held -= 5;
            chars.push(BASE32[((bits >> held) & 31) as usize] as char);
        }
    }
    chars
        .chunks(4)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Reads a recovery key leniently: case, dashes and spaces do not matter,
/// and the look-alikes I, L and O count as 1, 1 and 0.
fn decode_recovery(text: &str) -> Option<Zeroizing<[u8; 20]>> {
    let mut out = Zeroizing::new([0u8; 20]);
    let (mut bits, mut held, mut at) = (0u32, 0u32, 0usize);
    let mut count = 0;
    for c in text.chars().filter(|c| !matches!(c, '-' | ' ')) {
        let c = match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        };
        let value = BASE32.iter().position(|&b| b as char == c)? as u32;
        count += 1;
        bits = (bits << 5) | value;
        held += 5;
        if held >= 8 {
            held -= 8;
            *out.get_mut(at)? = (bits >> held) as u8;
            at += 1;
        }
    }
    (count == 32 && at == 20).then_some(out)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

fn derive(secret: &[u8], salt_b64: &str, cost: KdfCost) -> Result<Key, VaultError> {
    let salt = B64
        .decode(salt_b64)
        .map_err(|_| VaultError::Corrupt("bad salt".into()))?;
    let params = Params::new(cost.memory_kib, cost.iterations, cost.parallelism, Some(32))
        .map_err(|e| VaultError::Corrupt(format!("bad key derivation settings: {e}")))?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(secret, &salt, key.as_mut())
        .map_err(|e| VaultError::Corrupt(format!("key derivation failed: {e}")))?;
    Ok(key)
}

/// `nonce || ciphertext` of `plain` under `key`, bound to `aad`.
fn seal(key: &[u8; 32], aad: &[u8], plain: &[u8]) -> Vec<u8> {
    let nonce: [u8; NONCE_LEN] = random_bytes();
    let cipher = XChaCha20Poly1305::new(key.into());
    let sealed = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plain, aad })
        .expect("encrypting in memory cannot fail");
    let mut out = nonce.to_vec();
    out.extend(sealed);
    out
}

fn open(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    if sealed.len() < NONCE_LEN {
        return None;
    }
    let (nonce, body) = sealed.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(key.into())
        .decrypt(XNonce::from_slice(nonce), Payload { msg: body, aad })
        .ok()
        .map(Zeroizing::new)
}

fn wrap(wrapping: &[u8; 32], slot: &str, data_key: &[u8; 32]) -> String {
    B64.encode(seal(wrapping, slot.as_bytes(), data_key))
}

fn unwrap(wrapping: &[u8; 32], slot: &str, wrapped: &str) -> Option<Key> {
    let sealed = B64.decode(wrapped).ok()?;
    let plain = open(wrapping, slot.as_bytes(), &sealed)?;
    let key: [u8; 32] = plain.as_slice().try_into().ok()?;
    Some(Zeroizing::new(key))
}

/// Rejects absolute paths, `..` and empty names; returns `a/b/c`.
fn logical(path: &str) -> Result<String, VaultError> {
    let mut parts = Vec::new();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(VaultError::Invalid(format!("not a relative path: {path}"))),
        }
    }
    if parts.is_empty() {
        return Err(VaultError::Invalid("empty path".into()));
    }
    Ok(parts.join("/"))
}

fn is_local(path: &str) -> bool {
    path.ends_with(LOCAL_SUFFIX)
}

fn under(path: &str, dir: &str) -> bool {
    path.strip_prefix(dir)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `root` holds an encrypted collection.
pub fn is_vault(root: &Path) -> bool {
    root.join(HEADER_FILE).is_file()
}

/// An unlocked encrypted collection. The data key lives only in memory and
/// is wiped when the vault is dropped (locked).
pub struct Vault {
    root: PathBuf,
    key: Key,
    index: Index,
    local: Index,
}

impl fmt::Debug for Vault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl Vault {
    /// Creates an empty encrypted collection in `root` (missing or empty).
    /// Returns it unlocked, with the recovery key to show the user once.
    pub fn create(
        root: &Path,
        password: &str,
        cost: KdfCost,
    ) -> Result<(Self, RecoveryKey), VaultError> {
        if password.is_empty() {
            return Err(VaultError::Invalid("the password cannot be empty".into()));
        }
        if root.exists() && fs::read_dir(root).map_err(io(root))?.next().is_some() {
            return Err(VaultError::NotEmpty(root.to_owned()));
        }
        let local = root.join(OBJECTS_DIR).join(LOCAL_DIR);
        fs::create_dir_all(&local).map_err(io(&local))?;
        let gitignore = root.join(".gitignore");
        fs::write(&gitignore, "# This machine's secrets\nobjects/local/\n")
            .map_err(io(&gitignore))?;

        let data_key: Key = Zeroizing::new(random_bytes());
        let (recovery, recovery_bytes) = RecoveryKey::random();
        let salt = B64.encode(random_bytes::<16>());
        let recovery_salt = B64.encode(random_bytes::<16>());
        let password_wrap = derive(password.as_bytes(), &salt, cost)?;
        let recovery_wrap = derive(recovery_bytes.as_ref(), &recovery_salt, cost)?;
        let header = Header {
            apix_vault: 1,
            kdf: cost,
            password_key: wrap(&password_wrap, "password", &data_key),
            recovery_key: wrap(&recovery_wrap, "recovery", &data_key),
            salt,
            recovery_salt,
        };
        write_header(root, &header)?;
        let vault = Self {
            root: root.to_owned(),
            key: data_key,
            index: Index::default(),
            local: Index::default(),
        };
        vault.save_index(false)?;
        vault.save_index(true)?;
        Ok((vault, recovery))
    }

    /// Unlocks with the password.
    pub fn unlock(root: &Path, password: &str) -> Result<Self, VaultError> {
        let header = read_header(root)?;
        let wrapping = derive(password.as_bytes(), &header.salt, header.kdf)?;
        let key =
            unwrap(&wrapping, "password", &header.password_key).ok_or(VaultError::WrongPassword)?;
        Self::load(root, key)
    }

    /// Unlocks with the recovery key, e.g. after the password was forgotten.
    pub fn unlock_with_recovery(root: &Path, recovery: &str) -> Result<Self, VaultError> {
        let header = read_header(root)?;
        let bytes = decode_recovery(recovery).ok_or(VaultError::WrongRecoveryKey)?;
        let wrapping = derive(bytes.as_ref(), &header.recovery_salt, header.kdf)?;
        let key = unwrap(&wrapping, "recovery", &header.recovery_key)
            .ok_or(VaultError::WrongRecoveryKey)?;
        Self::load(root, key)
    }

    fn load(root: &Path, key: Key) -> Result<Self, VaultError> {
        let mut vault = Self {
            root: root.to_owned(),
            key,
            index: Index::default(),
            local: Index::default(),
        };
        vault.index = vault.load_index(false)?;
        // This machine may have no secrets yet (a fresh clone).
        vault.local = vault.load_index(true).unwrap_or_default();
        Ok(vault)
    }

    /// Re-reads the indexes from disk, e.g. after a `git pull` changed them.
    pub fn reload(&mut self) -> Result<(), VaultError> {
        self.index = self.load_index(false)?;
        self.local = self.load_index(true).unwrap_or_default();
        Ok(())
    }

    /// Sets a new password. Files are not re-encrypted; only the wrapped key changes.
    pub fn change_password(&mut self, password: &str) -> Result<(), VaultError> {
        if password.is_empty() {
            return Err(VaultError::Invalid("the password cannot be empty".into()));
        }
        let mut header = read_header(&self.root)?;
        header.salt = B64.encode(random_bytes::<16>());
        let wrapping = derive(password.as_bytes(), &header.salt, header.kdf)?;
        header.password_key = wrap(&wrapping, "password", &self.key);
        write_header(&self.root, &header)
    }

    /// Replaces the recovery key; the old one stops working.
    pub fn new_recovery_key(&mut self) -> Result<RecoveryKey, VaultError> {
        let mut header = read_header(&self.root)?;
        let (recovery, bytes) = RecoveryKey::random();
        header.recovery_salt = B64.encode(random_bytes::<16>());
        let wrapping = derive(bytes.as_ref(), &header.recovery_salt, header.kdf)?;
        header.recovery_key = wrap(&wrapping, "recovery", &self.key);
        write_header(&self.root, &header)?;
        Ok(recovery)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Points the unlocked vault at its folder's new place after the folder was moved.
    pub fn relocate(&mut self, root: &Path) {
        self.root = root.to_owned();
    }

    /// Every file, as logical paths (`users/get-user.apix`), sorted.
    pub fn files(&self) -> Vec<String> {
        let mut all: Vec<String> = self
            .index
            .files
            .keys()
            .chain(self.local.files.keys())
            .cloned()
            .collect();
        all.sort();
        all
    }

    /// Every folder that exists, including empty ones and parents of files.
    pub fn dirs(&self) -> BTreeSet<String> {
        let mut dirs = self.index.dirs.clone();
        for file in self.files() {
            let mut path = file.as_str();
            while let Some((parent, _)) = path.rsplit_once('/') {
                dirs.insert(parent.to_owned());
                path = parent;
            }
        }
        dirs
    }

    pub fn is_file(&self, path: &str) -> bool {
        logical(path).is_ok_and(|p| self.side(&p).files.contains_key(&p))
    }

    pub fn is_dir(&self, path: &str) -> bool {
        logical(path).is_ok_and(|p| self.dirs().contains(&p))
    }

    fn side(&self, path: &str) -> &Index {
        if is_local(path) {
            &self.local
        } else {
            &self.index
        }
    }

    fn side_mut(&mut self, path: &str) -> &mut Index {
        if is_local(path) {
            &mut self.local
        } else {
            &mut self.index
        }
    }

    fn object_path(&self, id: &str, local: bool) -> PathBuf {
        let dir = self.root.join(OBJECTS_DIR);
        let dir = if local { dir.join(LOCAL_DIR) } else { dir };
        dir.join(format!("{id}.{OBJECT_EXT}"))
    }

    /// The associated data that ties a blob to its id (and side).
    fn aad(id: &str, local: bool) -> String {
        if local {
            format!("{LOCAL_DIR}/{id}")
        } else {
            id.to_owned()
        }
    }

    fn write_object(&self, id: &str, local: bool, plain: &[u8]) -> Result<(), VaultError> {
        let path = self.object_path(id, local);
        let mut blob = MAGIC.to_vec();
        blob.extend(seal(&self.key, Self::aad(id, local).as_bytes(), plain));
        // Write beside, then rename over, so a crash never leaves half a file.
        let temp = path.with_extension("tmp");
        fs::write(&temp, blob).map_err(io(&temp))?;
        fs::rename(&temp, &path).map_err(io(&path))
    }

    fn read_object(&self, id: &str, local: bool) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let path = self.object_path(id, local);
        let blob = fs::read(&path).map_err(io(&path))?;
        let body = blob.strip_prefix(MAGIC.as_slice()).ok_or_else(|| {
            VaultError::Corrupt(format!("{} is not an Apix object", path.display()))
        })?;
        open(&self.key, Self::aad(id, local).as_bytes(), body).ok_or_else(|| {
            VaultError::Corrupt(format!("{} failed its integrity check", path.display()))
        })
    }

    fn load_index(&self, local: bool) -> Result<Index, VaultError> {
        let plain = self.read_object(INDEX_ID, local)?;
        let text = std::str::from_utf8(&plain).map_err(|_| VaultError::Corrupt("index".into()))?;
        toml::from_str(text).map_err(|e| VaultError::Corrupt(format!("index: {e}")))
    }

    fn save_index(&self, local: bool) -> Result<(), VaultError> {
        let index = if local { &self.local } else { &self.index };
        let text = Zeroizing::new(
            toml::to_string(index).map_err(|e| VaultError::Corrupt(format!("index: {e}")))?,
        );
        self.write_object(INDEX_ID, local, text.as_bytes())
    }

    /// The decrypted contents of a file.
    pub fn read(&self, path: &str) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let path = logical(path)?;
        let id = self
            .side(&path)
            .files
            .get(&path)
            .ok_or_else(|| VaultError::NotFound(path.clone()))?;
        self.read_object(id, is_local(&path))
    }

    pub fn read_to_string(&self, path: &str) -> Result<Zeroizing<String>, VaultError> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes.to_vec())
            .map(Zeroizing::new)
            .map_err(|_| VaultError::Corrupt(format!("{path} is not text")))
    }

    /// Creates or replaces a file.
    pub fn write(&mut self, path: &str, contents: &[u8]) -> Result<(), VaultError> {
        let path = logical(path)?;
        let local = is_local(&path);
        let existing = self.side(&path).files.get(&path).cloned();
        let id = existing.clone().unwrap_or_else(|| {
            random_bytes::<16>()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        });
        self.write_object(&id, local, contents)?;
        if existing.is_none() {
            self.side_mut(&path).files.insert(path, id);
            self.save_index(local)?;
        }
        Ok(())
    }

    /// Records a folder (and its parents), so it exists even while empty.
    pub fn create_dir(&mut self, path: &str) -> Result<(), VaultError> {
        let path = logical(path)?;
        let mut changed = false;
        let mut dir = path.as_str();
        loop {
            changed |= self.index.dirs.insert(dir.to_owned());
            match dir.rsplit_once('/') {
                Some((parent, _)) => dir = parent,
                None => break,
            }
        }
        if changed {
            self.save_index(false)?;
        }
        Ok(())
    }

    /// The names directly inside `dir` ("" for the top), each with whether it is a folder.
    pub fn children(&self, dir: &str) -> Vec<(String, bool)> {
        let dir = if dir.is_empty() {
            String::new()
        } else {
            match logical(dir) {
                Ok(d) => d,
                Err(_) => return Vec::new(),
            }
        };
        let direct = |path: &str| -> Option<String> {
            let rest = if dir.is_empty() {
                path
            } else {
                path.strip_prefix(dir.as_str())?.strip_prefix('/')?
            };
            (!rest.is_empty() && !rest.contains('/')).then(|| rest.to_owned())
        };
        let mut out: Vec<(String, bool)> = self
            .dirs()
            .iter()
            .filter_map(|d| direct(d).map(|name| (name, true)))
            .collect();
        out.extend(
            self.files()
                .iter()
                .filter_map(|f| direct(f).map(|name| (name, false))),
        );
        out.sort();
        out
    }

    /// Deletes a file.
    pub fn remove(&mut self, path: &str) -> Result<(), VaultError> {
        let path = logical(path)?;
        let local = is_local(&path);
        let id = self
            .side_mut(&path)
            .files
            .remove(&path)
            .ok_or_else(|| VaultError::NotFound(path.clone()))?;
        self.save_index(local)?;
        let object = self.object_path(&id, local);
        fs::remove_file(&object).map_err(io(&object))
    }

    /// Deletes a folder and everything in it.
    pub fn remove_dir(&mut self, path: &str) -> Result<(), VaultError> {
        let path = logical(path)?;
        let doomed: Vec<String> = self
            .files()
            .into_iter()
            .filter(|f| under(f, &path))
            .collect();
        for file in doomed {
            self.remove(&file)?;
        }
        let before = self.index.dirs.len();
        self.index.dirs.retain(|d| *d != path && !under(d, &path));
        if self.index.dirs.len() != before {
            self.save_index(false)?;
        }
        Ok(())
    }

    /// Renames a file or a folder. Nothing is re-encrypted: only the index changes.
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), VaultError> {
        let (from, to) = (logical(from)?, logical(to)?);
        if self.is_file(&to) || self.is_dir(&to) {
            return Err(VaultError::Invalid(format!("{to} already exists")));
        }
        if self.is_file(&from) {
            if is_local(&from) != is_local(&to) {
                // Moving between shared and local means re-encrypting under the other side.
                let contents = self.read(&from)?;
                self.write(&to, &contents)?;
                return self.remove(&from);
            }
            let local = is_local(&from);
            let side = self.side_mut(&from);
            let id = side.files.remove(&from).expect("checked above");
            side.files.insert(to, id);
            return self.save_index(local);
        }
        if !self.is_dir(&from) {
            return Err(VaultError::NotFound(from));
        }
        if under(&to, &from) {
            return Err(VaultError::Invalid(
                "a folder cannot move into itself".into(),
            ));
        }
        let moved = |p: &str| format!("{to}{}", &p[from.len()..]);
        for index in [&mut self.index, &mut self.local] {
            let keys: Vec<String> = index
                .files
                .keys()
                .filter(|k| under(k, &from))
                .cloned()
                .collect();
            for key in keys {
                let id = index.files.remove(&key).expect("listed above");
                index.files.insert(moved(&key), id);
            }
        }
        let dirs: Vec<String> = self
            .index
            .dirs
            .iter()
            .filter(|d| **d == from || under(d, &from))
            .cloned()
            .collect();
        for dir in dirs {
            self.index.dirs.remove(&dir);
            self.index.dirs.insert(moved(&dir));
        }
        self.save_index(false)?;
        self.save_index(true)
    }

    /// Encrypts the plain collection in `plain` into a new vault at `root`.
    /// The plain files are left alone; the caller decides what to do with them.
    pub fn encrypt_folder(
        plain: &Path,
        root: &Path,
        password: &str,
        cost: KdfCost,
    ) -> Result<(Self, RecoveryKey), VaultError> {
        let (mut vault, recovery) = Self::create(root, password, cost)?;
        let mut stack = vec![plain.to_owned()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).map_err(io(&dir))? {
                let path = entry.map_err(io(&dir))?.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                // Version control and editor folders are not part of the collection.
                if name.starts_with('.') {
                    continue;
                }
                let rel = path.strip_prefix(plain).expect("walked from plain");
                let rel = rel.to_string_lossy().replace('\\', "/");
                if path.is_dir() {
                    vault.create_dir(&rel)?;
                    stack.push(path);
                } else {
                    let contents = Zeroizing::new(fs::read(&path).map_err(io(&path))?);
                    vault.write(&rel, &contents)?;
                }
            }
        }
        Ok((vault, recovery))
    }

    /// Writes every file, decrypted, into `dest` (missing or empty).
    pub fn export(&self, dest: &Path) -> Result<(), VaultError> {
        if dest.exists() && fs::read_dir(dest).map_err(io(dest))?.next().is_some() {
            return Err(VaultError::NotEmpty(dest.to_owned()));
        }
        for dir in self.dirs() {
            let path = dest.join(&dir);
            fs::create_dir_all(&path).map_err(io(&path))?;
        }
        fs::create_dir_all(dest).map_err(io(dest))?;
        for file in self.files() {
            let path = dest.join(&file);
            fs::write(&path, self.read(&file)?.as_slice()).map_err(io(&path))?;
        }
        Ok(())
    }
}

fn read_header(root: &Path) -> Result<Header, VaultError> {
    let path = root.join(HEADER_FILE);
    if !path.is_file() {
        return Err(VaultError::NotAVault(root.to_owned()));
    }
    let text = fs::read_to_string(&path).map_err(io(&path))?;
    let header: Header =
        toml::from_str(&text).map_err(|e| VaultError::Corrupt(format!("{HEADER_FILE}: {e}")))?;
    if header.apix_vault != 1 {
        return Err(VaultError::Invalid(format!(
            "this collection uses vault format {}; update Apix to open it",
            header.apix_vault
        )));
    }
    Ok(header)
}

fn write_header(root: &Path, header: &Header) -> Result<(), VaultError> {
    let path = root.join(HEADER_FILE);
    let body = toml::to_string_pretty(header)
        .map_err(|e| VaultError::Corrupt(format!("{HEADER_FILE}: {e}")))?;
    let text = format!(
        "# An encrypted Apix collection. Open it in Apix with its password.\n\
         # Everything below is safe to share: it holds no secret in the clear.\n{body}"
    );
    let temp = path.with_extension("tmp");
    fs::write(&temp, text).map_err(io(&temp))?;
    fs::rename(&temp, &path).map_err(io(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAST: KdfCost = KdfCost::INSECURE_FAST;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("apix-vault-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// Every byte the vault put on disk, for "no plaintext leaks" checks.
    fn disk_bytes(root: &Path) -> Vec<u8> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_owned()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                out.extend(path.to_string_lossy().as_bytes());
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.extend(fs::read(&path).unwrap());
                }
            }
        }
        out
    }

    fn contains(haystack: &[u8], needle: &str) -> bool {
        haystack
            .windows(needle.len())
            .any(|w| w == needle.as_bytes())
    }

    #[test]
    fn round_trips_and_hides_names_and_contents() {
        let root = temp("roundtrip");
        let (mut vault, recovery) = Vault::create(&root, "correct horse", FAST).unwrap();
        vault
            .write("apix.toml", b"name = \"Secret API\"\n")
            .unwrap();
        vault
            .write(
                "payments/charge-card.apix",
                b"url = \"https://pay.internal/charge\"\n",
            )
            .unwrap();
        vault.create_dir("empty-folder").unwrap();
        drop(vault);

        let on_disk = disk_bytes(&root);
        for secret in [
            "Secret API",
            "payments",
            "charge-card",
            "pay.internal",
            "empty-folder",
        ] {
            assert!(!contains(&on_disk, secret), "{secret} leaked to disk");
        }

        let vault = Vault::unlock(&root, "correct horse").unwrap();
        assert_eq!(vault.files(), ["apix.toml", "payments/charge-card.apix"]);
        assert!(vault.is_dir("payments") && vault.is_dir("empty-folder"));
        assert_eq!(
            vault.read_to_string("apix.toml").unwrap().as_str(),
            "name = \"Secret API\"\n"
        );
        assert!(matches!(
            Vault::unlock(&root, "wrong").unwrap_err(),
            VaultError::WrongPassword
        ));
        // The recovery key opens it too, typed sloppily.
        let sloppy = recovery.as_str().to_lowercase().replace('-', " ");
        assert!(Vault::unlock_with_recovery(&root, &sloppy).is_ok());
        assert!(matches!(
            Vault::unlock_with_recovery(&root, "0000-0000").unwrap_err(),
            VaultError::WrongRecoveryKey
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changes_password_and_recovery_key_without_reencrypting() {
        let root = temp("password");
        let (mut vault, old_recovery) = Vault::create(&root, "one", FAST).unwrap();
        vault.write("a.apix", b"x").unwrap();
        let object = fs::read_dir(root.join(OBJECTS_DIR))
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.is_file() && !p.ends_with("index.apixe"))
            .unwrap();
        let before = fs::read(&object).unwrap();

        vault.change_password("two").unwrap();
        let new_recovery = vault.new_recovery_key().unwrap();
        assert_eq!(fs::read(&object).unwrap(), before, "files are untouched");
        assert!(Vault::unlock(&root, "one").is_err());
        assert_eq!(Vault::unlock(&root, "two").unwrap().files(), ["a.apix"]);
        assert!(Vault::unlock_with_recovery(&root, old_recovery.as_str()).is_err());
        assert!(Vault::unlock_with_recovery(&root, new_recovery.as_str()).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn renames_removes_and_keeps_local_secrets_apart() {
        let root = temp("ops");
        let (mut vault, _) = Vault::create(&root, "pw", FAST).unwrap();
        vault.write("users/admin/list.apix", b"1").unwrap();
        vault.write("users/get.apix", b"2").unwrap();
        vault.write("environments/dev.toml", b"[vars]").unwrap();
        vault
            .write("environments/dev.local.toml", b"token = 's'")
            .unwrap();
        assert_eq!(
            fs::read_dir(root.join(OBJECTS_DIR).join(LOCAL_DIR))
                .unwrap()
                .count(),
            2,
            "the secret and the local index"
        );
        assert!(
            fs::read_to_string(root.join(".gitignore"))
                .unwrap()
                .contains("objects/local/")
        );

        vault.rename("users", "people").unwrap();
        assert!(vault.is_file("people/admin/list.apix") && !vault.is_dir("users"));
        assert!(vault.rename("people", "people/inner").is_err());
        vault.rename("people/get.apix", "get.apix").unwrap();
        vault.remove_dir("people").unwrap();
        vault.remove("environments/dev.toml").unwrap();
        drop(vault);

        let vault = Vault::unlock(&root, "pw").unwrap();
        assert_eq!(vault.files(), ["environments/dev.local.toml", "get.apix"]);
        assert_eq!(vault.read("get.apix").unwrap().as_slice(), b"2");
        // A fresh clone has no local secrets, and still opens.
        fs::remove_dir_all(root.join(OBJECTS_DIR).join(LOCAL_DIR)).unwrap();
        fs::create_dir(root.join(OBJECTS_DIR).join(LOCAL_DIR)).unwrap();
        assert_eq!(Vault::unlock(&root, "pw").unwrap().files(), ["get.apix"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_swapped_or_edited_objects() {
        let root = temp("tamper");
        let (mut vault, _) = Vault::create(&root, "pw", FAST).unwrap();
        vault.write("a.apix", b"alpha").unwrap();
        vault.write("b.apix", b"bravo").unwrap();
        let id = |name: &str| vault.index.files[name].clone();
        let (a, b) = (
            vault.object_path(&id("a.apix"), false),
            vault.object_path(&id("b.apix"), false),
        );
        let (bytes_a, bytes_b) = (fs::read(&a).unwrap(), fs::read(&b).unwrap());
        fs::write(&a, &bytes_b).unwrap();
        fs::write(&b, &bytes_a).unwrap();
        assert!(matches!(
            vault.read("a.apix").unwrap_err(),
            VaultError::Corrupt(_)
        ));

        let mut flipped = bytes_a.clone();
        *flipped.last_mut().unwrap() ^= 1;
        fs::write(&a, flipped).unwrap();
        assert!(matches!(
            vault.read("a.apix").unwrap_err(),
            VaultError::Corrupt(_)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn encrypts_a_plain_collection_and_exports_it_back() {
        let plain = temp("plain");
        fs::create_dir_all(plain.join("users/empty")).unwrap();
        fs::create_dir_all(plain.join(".git")).unwrap();
        fs::write(plain.join(".git/HEAD"), "ref").unwrap();
        fs::write(plain.join("apix.toml"), "name = \"Shop\"\n").unwrap();
        fs::write(plain.join("users/get.apix"), "url = \"x\"\n").unwrap();

        let root = temp("encrypted");
        let (vault, _) = Vault::encrypt_folder(&plain, &root, "pw", FAST).unwrap();
        assert_eq!(
            vault.files(),
            ["apix.toml", "users/get.apix"],
            ".git is skipped"
        );
        assert!(vault.is_dir("users/empty"));

        let back = temp("exported");
        vault.export(&back).unwrap();
        assert_eq!(
            fs::read_to_string(back.join("users/get.apix")).unwrap(),
            "url = \"x\"\n"
        );
        assert!(back.join("users/empty").is_dir());
        assert!(matches!(
            vault.export(&back).unwrap_err(),
            VaultError::NotEmpty(_)
        ));
        for dir in [plain, root, back] {
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn recovery_keys_encode_and_decode() {
        let bytes = [0xA5u8; 20];
        let text = encode_recovery(&bytes);
        assert_eq!(text.len(), 32 + 7, "8 groups of 4");
        assert_eq!(decode_recovery(&text).unwrap().as_ref(), &bytes);
        assert!(decode_recovery("too-short").is_none());
        assert!(
            decode_recovery(&text.replace('5', "U")).is_none(),
            "U is not base32"
        );
    }

    #[test]
    fn rejects_paths_outside_the_vault() {
        assert!(logical("../x").is_err());
        assert!(logical("/etc/passwd").is_err());
        assert_eq!(logical("./a/b").unwrap(), "a/b");
    }
}
