//! Apix core: the request model, `{{variable}}` interpolation and the
//! plain-text collection format. No networking and no UI live here, so every
//! frontend (desktop, CLI, future TUI) shares exactly the same behaviour.

pub mod dynamic;
pub mod model;
pub mod project;
pub mod store;
pub mod text;
pub mod vars;
pub mod vfs;

pub use model::{ApiKeyLocation, Auth, Body, FormPart, GraphQl, KeyValue, Method, Request};
pub use project::{GroupBy, Project};
pub use store::{Collection, Environment, StoreError};
pub use vars::{Resolved, Variables};
