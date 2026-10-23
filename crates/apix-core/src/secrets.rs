//! Where new environment secrets are written: the gitignored
//! `<name>.local.toml` file (the default) or the system keychain. Reading
//! does not depend on this choice: a `.local.toml` that names a keychain
//! entry is read from the keychain everywhere, the CLI included.
//!
//! Secrets of an encrypted collection always stay in its vault, which is
//! already encrypted.

use std::sync::atomic::{AtomicBool, Ordering};

static KEYCHAIN: AtomicBool = AtomicBool::new(false);

/// Sends secrets written from now on to the keychain (`true`) or to `.local.toml` files.
pub fn use_keychain(on: bool) {
    KEYCHAIN.store(on, Ordering::Relaxed);
}

pub fn keychain_enabled() -> bool {
    KEYCHAIN.load(Ordering::Relaxed)
}

/// A random id for a new keychain entry: 16 hex digits.
pub(crate) fn new_id() -> String {
    use rand_core::{OsRng, RngCore};
    format!("{:016x}", OsRng.next_u64())
}
