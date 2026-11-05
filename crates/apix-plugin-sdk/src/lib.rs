//! Write Apix plugins in Rust.
//!
//! Implement [`Plugin`] and call [`export!`]; build the crate as a `cdylib`
//! for `wasm32-unknown-unknown` and copy the `.wasm` into Apix's plugins
//! folder (or `apix plugins install file.wasm`):
//!
//! ```ignore
//! use apix_plugin_sdk::{Plugin, export, serde_json::{Value, json}};
//!
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn manifest() -> Value {
//!         json!({ "name": "hello", "version": "1.0.0",
//!                 "visualize": { "label": "Hello" } })
//!     }
//!     fn call(hook: &str, input: Value) -> Result<Value, String> {
//!         match hook {
//!             "visualize" => Ok(json!({ "text": format!("{} bytes", input["body"].as_str().unwrap_or("").len()) })),
//!             other => Err(format!("no hook {other}")),
//!         }
//!     }
//! }
//!
//! export!(Hello);
//! ```
//!
//! The hooks and their JSON are described in the `apix-plugins` crate and
//! in the user guide. A plugin has no access to files, the network or the
//! clock: it gets JSON and answers JSON.

pub use serde_json;
use serde_json::{Value, json};

/// A plugin: what it offers, and what it does when a hook is called.
pub trait Plugin {
    /// `{"name": …, "version": …, "description": …, "auth"|"import"|"visualize": {…}}`.
    fn manifest() -> Value;
    /// Runs `hook` (`auth`, `import`, `visualize`) on `input`.
    fn call(hook: &str, input: Value) -> Result<Value, String>;
}

/// Room for `len` bytes that the host writes into. Never freed: every call
/// runs in a fresh instance.
#[doc(hidden)]
pub fn alloc(len: i32) -> i32 {
    let mut bytes = Vec::<u8>::with_capacity(len.max(1) as usize);
    let ptr = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    ptr as usize as i32
}

/// Hands text to the host as `ptr << 32 | len`.
#[doc(hidden)]
pub fn answer(text: String) -> i64 {
    let bytes = text.into_bytes().into_boxed_slice();
    let len = bytes.len();
    let ptr = Box::leak(bytes).as_ptr() as usize;
    ((ptr as i64) << 32) | len as i64
}

/// # Safety
/// `ptr..ptr+len` must be memory the host wrote through [`alloc`].
#[doc(hidden)]
pub unsafe fn read(ptr: i32, len: i32) -> String {
    let bytes = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) };
    String::from_utf8_lossy(bytes).into_owned()
}

#[doc(hidden)]
pub fn dispatch<T: Plugin>(hook_ptr: i32, hook_len: i32, input_ptr: i32, input_len: i32) -> i64 {
    // SAFETY: the host wrote both through `alloc`.
    let (hook, input) = unsafe { (read(hook_ptr, hook_len), read(input_ptr, input_len)) };
    let input = serde_json::from_str(&input).unwrap_or(Value::Null);
    let out = match T::call(&hook, input) {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "error": error }),
    };
    answer(out.to_string())
}

/// Exports the plugin's functions for the host.
#[macro_export]
macro_rules! export {
    ($plugin:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn apix_alloc(len: i32) -> i32 {
            $crate::alloc(len)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn apix_manifest() -> i64 {
            $crate::answer(<$plugin as $crate::Plugin>::manifest().to_string())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn apix_call(
            hook_ptr: i32,
            hook_len: i32,
            input_ptr: i32,
            input_len: i32,
        ) -> i64 {
            $crate::dispatch::<$plugin>(hook_ptr, hook_len, input_ptr, input_len)
        }
    };
}
