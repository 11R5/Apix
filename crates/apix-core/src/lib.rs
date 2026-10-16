//! Apix core: the request model, `{{variable}}` interpolation and the
//! plain-text collection format. No networking and no UI live here, so every
//! frontend (desktop, CLI, future TUI) shares exactly the same behaviour.

pub mod model;
pub mod store;
pub mod vars;

pub use model::{ApiKeyLocation, Auth, Body, KeyValue, Method, Request};
pub use store::{Collection, Environment, StoreError};
pub use vars::{Resolved, Variables};
