//! Apix core: the request model, `{{variable}}` interpolation and the
//! plain-text collection format. No networking and no UI live here, so every
//! frontend (desktop, CLI, future TUI) shares exactly the same behaviour.

pub mod dynamic;
pub mod history;
pub mod model;
pub mod project;
pub mod scripts;
pub mod secrets;
pub mod sign;
pub mod store;
pub mod text;
pub mod vars;
pub mod vfs;

pub use model::{
    ApiKeyLocation, AssertOp, Assertion, Auth, AwsSigV4, Body, ClientAuth, EffectiveSettings,
    FormPart, GraphQl, KeyValue, Method, OAuth1, OAuth1Method, OAuth2, OAuth2Grant, ProxyChoice,
    Request, RequestSettings,
};
pub use project::{GroupBy, Project};
pub use store::{Collection, Environment, StoreError};
pub use vars::{Resolved, Variables};
