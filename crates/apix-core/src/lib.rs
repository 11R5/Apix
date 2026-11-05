//! Apix core: the request model, `{{variable}}` interpolation and the
//! plain-text collection format. No networking and no UI live here, so every
//! frontend (desktop, CLI, future TUI) shares exactly the same behaviour.

pub mod diff;
pub mod dynamic;
pub mod filter;
pub mod git;
pub mod graphql;
pub mod headers;
pub mod history;
pub mod model;
pub mod monitor;
pub mod project;
pub mod scripts;
pub mod secrets;
pub mod sign;
pub mod soap;
pub mod store;
pub mod table;
pub mod text;
pub mod update;
pub mod urls;
pub mod vars;
pub mod vfs;

pub use model::{
    ApiKeyLocation, AssertOp, Assertion, Auth, AwsSigV4, Body, ClientAuth, EffectiveSettings,
    Example, ExampleRequest, FormPart, GraphQl, GrpcCall, KeyValue, MessageFormat, Method,
    MqttSettings, MqttSubscription, OAuth1, OAuth1Method, OAuth2, OAuth2Grant, ProxyChoice,
    Request, RequestSettings, SocketIoOptions, WebSocketOptions, WsMessage, display_url,
};
pub use project::{GroupBy, Project};
pub use store::{Collection, Environment, StoreError};
pub use vars::{Resolved, Variables};
