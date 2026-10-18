//! The Auth form: used as the request's Auth tab and in the folder and
//! collection Auth dialog. "Inherit from parent" shows what would be used
//! instead and where it is set.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use apix_core::store;
use apix_core::{
    ApiKeyLocation, Auth, AwsSigV4, ClientAuth, OAuth1, OAuth1Method, OAuth2, OAuth2Grant, Request,
};
use apix_core::{Collection, Project, project};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{
    Disableable as _, IndexPath, Selectable as _, Sizable, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::theme::Palette;

/// The picker's entries, in `kind_index` order.
const KINDS: [&str; 9] = [
    "Inherit from parent",
    "No auth",
    "Bearer token",
    "Basic auth",
    "API key",
    "Digest auth",
    "OAuth 1.0",
    "OAuth 2.0",
    "AWS Signature",
];

type Picker = SelectState<Vec<&'static str>>;

fn kind_index(auth: &Auth) -> usize {
    match auth {
        Auth::Inherit => 0,
        Auth::None => 1,
        Auth::Bearer { .. } => 2,
        Auth::Basic { .. } => 3,
        Auth::ApiKey { .. } => 4,
        Auth::Digest { .. } => 5,
        Auth::OAuth1(_) => 6,
        Auth::OAuth2(_) => 7,
        Auth::AwsSigV4(_) => 8,
    }
}

/// The text fields of every auth type. Types that share a meaning share a
/// field (Basic and Digest both use Username and Password), so switching
/// between them keeps what was typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum F {
    Token,
    Username,
    Password,
    Key,
    Value,
    ConsumerKey,
    ConsumerSecret,
    AccessToken,
    TokenSecret,
    Realm,
    TokenUrl,
    AuthUrl,
    ClientId,
    ClientSecret,
    Scope,
    RedirectUri,
    AccessKey,
    SecretKey,
    SessionToken,
    Region,
    Service,
}

impl F {
    const ALL: [F; 21] = [
        F::Token,
        F::Username,
        F::Password,
        F::Key,
        F::Value,
        F::ConsumerKey,
        F::ConsumerSecret,
        F::AccessToken,
        F::TokenSecret,
        F::Realm,
        F::TokenUrl,
        F::AuthUrl,
        F::ClientId,
        F::ClientSecret,
        F::Scope,
        F::RedirectUri,
        F::AccessKey,
        F::SecretKey,
        F::SessionToken,
        F::Region,
        F::Service,
    ];

    fn label(self) -> &'static str {
        match self {
            F::Token => "Token",
            F::Username => "Username",
            F::Password => "Password",
            F::Key => "Key",
            F::Value => "Value",
            F::ConsumerKey => "Consumer key",
            F::ConsumerSecret => "Consumer secret",
            F::AccessToken => "Access token",
            F::TokenSecret => "Token secret",
            F::Realm => "Realm",
            F::TokenUrl => "Token URL",
            F::AuthUrl => "Auth URL",
            F::ClientId => "Client ID",
            F::ClientSecret => "Client secret",
            F::Scope => "Scope",
            F::RedirectUri => "Redirect URL",
            F::AccessKey => "Access key",
            F::SecretKey => "Secret key",
            F::SessionToken => "Session token",
            F::Region => "Region",
            F::Service => "Service",
        }
    }

    fn placeholder(self) -> &'static str {
        match self {
            F::Token => "Token, e.g. {{token}}",
            F::Key => "Name, e.g. X-API-Key",
            F::Value => "Value, e.g. {{apiKey}}",
            F::AccessToken | F::TokenSecret => "Optional",
            F::Realm => "Optional",
            F::TokenUrl => "https://auth.example.com/oauth/token",
            F::AuthUrl => "https://auth.example.com/authorize",
            F::ClientSecret => "Empty for a public client",
            F::Scope => "Space-separated, e.g. read write",
            F::RedirectUri => "Empty: a free port on http://127.0.0.1",
            F::AccessKey => "e.g. {{aws_access_key}}",
            F::SessionToken => "Only for temporary credentials",
            F::Region => "e.g. eu-central-1",
            F::Service => "e.g. execute-api, s3, lambda",
            other => other.label(),
        }
    }

    fn secret(self) -> bool {
        matches!(
            self,
            F::Password | F::ConsumerSecret | F::TokenSecret | F::ClientSecret | F::SecretKey
        )
    }
}

/// The last "Get new token" click.
enum Fetch {
    Idle,
    Running,
    Failed(String),
}

pub(super) struct AuthEditor {
    palette: Palette,
    workspace: WeakEntity<Workspace>,
    kind: Entity<Picker>,
    fields: HashMap<F, Entity<InputState>>,
    /// API key and OAuth 1.0: header or query string.
    location: ApiKeyLocation,
    signature: Entity<Picker>,
    grant: Entity<Picker>,
    pkce: bool,
    client_auth: ClientAuth,
    fetch: Fetch,
    fetch_task: Option<Task<()>>,
    /// What "Inherit" resolves to here, and where that is set.
    inherited: (Auth, Option<String>),
    /// Where this auth lives, for the hint under the form ("request", "folder" ...).
    owner: &'static str,
    _subscriptions: Vec<Subscription>,
}

impl AuthEditor {
    pub(super) fn new(
        palette: Palette,
        owner: &'static str,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let picker = |items: Vec<&'static str>, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| SelectState::new(items, Some(IndexPath::new(0)), window, cx))
        };
        let kind = picker(KINDS.to_vec(), window, cx);
        let signature = picker(
            OAuth1Method::ALL.iter().map(|m| m.as_str()).collect(),
            window,
            cx,
        );
        let grant = picker(
            OAuth2Grant::ALL.iter().map(|g| g.label()).collect(),
            window,
            cx,
        );

        let mut subscriptions = Vec::new();
        for select in [&kind, &signature, &grant] {
            subscriptions.push(cx.subscribe_in(
                select,
                window,
                |_, _, _: &SelectEvent<Vec<&'static str>>, _, cx| cx.notify(),
            ));
        }
        let mut fields = HashMap::new();
        for f in F::ALL {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(f.placeholder())
                    .masked(f.secret())
            });
            subscriptions.push(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
            fields.insert(f, input);
        }
        Self {
            palette,
            workspace,
            kind,
            fields,
            location: ApiKeyLocation::Header,
            signature,
            grant,
            pkce: true,
            client_auth: ClientAuth::Basic,
            fetch: Fetch::Idle,
            fetch_task: None,
            inherited: (Auth::None, None),
            owner,
            _subscriptions: subscriptions,
        }
    }

    fn input(&self, f: F) -> &Entity<InputState> {
        &self.fields[&f]
    }

    fn text(&self, f: F, cx: &App) -> String {
        self.input(f).read(cx).value().to_string()
    }

    /// Shows `auth`; the fields of other kinds are cleared.
    pub(super) fn set_auth(&mut self, auth: &Auth, window: &mut Window, cx: &mut Context<Self>) {
        let pick = |select: &Entity<Picker>, ix: usize, window: &mut Window, cx: &mut App| {
            select.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(ix)), window, cx)
            });
        };
        pick(&self.kind, kind_index(auth), window, cx);
        self.location = ApiKeyLocation::Header;
        self.pkce = true;
        self.client_auth = ClientAuth::Basic;
        self.fetch = Fetch::Idle;
        self.fetch_task = None;

        let values: Vec<(F, &str)> = match auth {
            Auth::Inherit | Auth::None => Vec::new(),
            Auth::Bearer { token } => vec![(F::Token, token)],
            Auth::Basic { username, password } | Auth::Digest { username, password } => {
                vec![(F::Username, username), (F::Password, password)]
            }
            Auth::ApiKey {
                key,
                value,
                location,
            } => {
                self.location = *location;
                vec![(F::Key, key), (F::Value, value)]
            }
            Auth::OAuth1(o) => {
                self.location = o.location;
                let ix = OAuth1Method::ALL
                    .iter()
                    .position(|m| *m == o.signature_method)
                    .unwrap_or(0);
                pick(&self.signature, ix, window, cx);
                vec![
                    (F::ConsumerKey, &o.consumer_key),
                    (F::ConsumerSecret, &o.consumer_secret),
                    (F::AccessToken, &o.token),
                    (F::TokenSecret, &o.token_secret),
                    (F::Realm, &o.realm),
                ]
            }
            Auth::OAuth2(o) => {
                self.pkce = o.pkce;
                self.client_auth = o.client_auth;
                let ix = OAuth2Grant::ALL
                    .iter()
                    .position(|g| *g == o.grant)
                    .unwrap_or(0);
                pick(&self.grant, ix, window, cx);
                vec![
                    (F::TokenUrl, &o.token_url),
                    (F::AuthUrl, &o.auth_url),
                    (F::ClientId, &o.client_id),
                    (F::ClientSecret, &o.client_secret),
                    (F::Scope, &o.scope),
                    (F::Username, &o.username),
                    (F::Password, &o.password),
                    (F::RedirectUri, &o.redirect_uri),
                ]
            }
            Auth::AwsSigV4(a) => vec![
                (F::AccessKey, &a.access_key),
                (F::SecretKey, &a.secret_key),
                (F::SessionToken, &a.session_token),
                (F::Region, &a.region),
                (F::Service, &a.service),
            ],
        };
        for f in F::ALL {
            let text = values
                .iter()
                .find(|(field, _)| *field == f)
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
            self.fields[&f].update(cx, |state, cx| state.set_value(text, window, cx));
        }
        cx.notify();
    }

    pub(super) fn set_inherited(
        &mut self,
        inherited: (Auth, Option<String>),
        cx: &mut Context<Self>,
    ) {
        if self.inherited != inherited {
            self.inherited = inherited;
            cx.notify();
        }
    }

    fn selected(select: &Entity<Picker>, cx: &App) -> usize {
        select
            .read(cx)
            .selected_index(cx)
            .map(|ix| ix.row)
            .unwrap_or(0)
    }

    fn kind(&self, cx: &App) -> usize {
        Self::selected(&self.kind, cx)
    }

    fn grant(&self, cx: &App) -> OAuth2Grant {
        OAuth2Grant::ALL[Self::selected(&self.grant, cx).min(OAuth2Grant::ALL.len() - 1)]
    }

    /// The auth as the form shows it now.
    pub(super) fn auth(&self, cx: &App) -> Auth {
        let text = |f: F| self.text(f, cx);
        let trimmed = |f: F| self.text(f, cx).trim().to_owned();
        match self.kind(cx) {
            0 => Auth::Inherit,
            1 => Auth::None,
            2 => Auth::Bearer {
                token: trimmed(F::Token),
            },
            3 => Auth::Basic {
                username: text(F::Username),
                password: text(F::Password),
            },
            4 => Auth::ApiKey {
                key: trimmed(F::Key),
                value: text(F::Value),
                location: self.location,
            },
            5 => Auth::Digest {
                username: text(F::Username),
                password: text(F::Password),
            },
            6 => Auth::OAuth1(OAuth1 {
                consumer_key: trimmed(F::ConsumerKey),
                consumer_secret: text(F::ConsumerSecret),
                token: trimmed(F::AccessToken),
                token_secret: text(F::TokenSecret),
                signature_method: OAuth1Method::ALL
                    [Self::selected(&self.signature, cx).min(OAuth1Method::ALL.len() - 1)],
                realm: trimmed(F::Realm),
                location: self.location,
            }),
            7 => {
                let grant = self.grant(cx);
                let code = grant == OAuth2Grant::AuthorizationCode;
                let password = grant == OAuth2Grant::Password;
                // Fields of other grants are not saved.
                let only = |on: bool, f: F| if on { text(f) } else { String::new() };
                Auth::OAuth2(OAuth2 {
                    grant,
                    token_url: trimmed(F::TokenUrl),
                    auth_url: if code {
                        trimmed(F::AuthUrl)
                    } else {
                        String::new()
                    },
                    client_id: trimmed(F::ClientId),
                    client_secret: text(F::ClientSecret),
                    scope: trimmed(F::Scope),
                    username: only(password, F::Username),
                    password: only(password, F::Password),
                    redirect_uri: if code {
                        trimmed(F::RedirectUri)
                    } else {
                        String::new()
                    },
                    pkce: self.pkce,
                    client_auth: self.client_auth,
                })
            }
            _ => Auth::AwsSigV4(AwsSigV4 {
                access_key: trimmed(F::AccessKey),
                secret_key: text(F::SecretKey),
                session_token: trimmed(F::SessionToken),
                region: trimmed(F::Region),
                service: trimmed(F::Service),
            }),
        }
    }

    /// This form's OAuth 2.0 settings with the workspace's variables filled in.
    fn resolved_oauth2(&self, cx: &App) -> Option<OAuth2> {
        let Auth::OAuth2(config) = self.auth(cx) else {
            return None;
        };
        let vars = self
            .workspace
            .upgrade()
            .map(|ws| ws.read(cx).vars.clone())
            .unwrap_or_default();
        let request = Request {
            auth: Auth::OAuth2(config),
            ..Request::default()
        };
        match request.resolve(&vars).request.auth {
            Auth::OAuth2(resolved) => Some(resolved),
            _ => None,
        }
    }

    fn get_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(config) = self.resolved_oauth2(cx) else {
            return;
        };
        self.fetch = Fetch::Running;
        let pending = crate::net::fetch_token(config);
        self.fetch_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = pending.await;
            this.update(cx, |this, cx| {
                this.fetch = match result {
                    Ok(_) => Fetch::Idle,
                    Err(e) => Fetch::Failed(e),
                };
                this.fetch_task = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn forget_token(&mut self, cx: &mut Context<Self>) {
        if let Some(config) = self.resolved_oauth2(cx) {
            apix_http::oauth2::forget(&config);
        }
        self.fetch = Fetch::Idle;
        cx.notify();
    }

    fn row(&self, label: &'static str, child: impl IntoElement) -> Div {
        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(124.))
                    .flex_none()
                    .text_sm()
                    .text_color(self.palette.text_muted)
                    .child(label),
            )
            .child(div().flex_1().min_w_0().max_w(px(520.)).child(child))
    }

    fn field(&self, f: F) -> AnyElement {
        let input = Input::new(self.input(f)).small();
        let input = if f.secret() {
            input.mask_toggle()
        } else {
            input
        };
        self.row(f.label(), input).into_any_element()
    }

    fn note(&self, text: impl Into<SharedString>) -> AnyElement {
        div()
            .text_sm()
            .text_color(self.palette.text_muted)
            .child(text.into())
            .into_any_element()
    }

    /// Two or more toggle buttons in a labelled row.
    fn choice<T: Copy + PartialEq + 'static>(
        &self,
        label: &'static str,
        current: T,
        options: &[(&'static str, &'static str, T)],
        set: fn(&mut Self, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let buttons = options.iter().map(|&(id, text, value)| {
            Button::new(id)
                .ghost()
                .small()
                .label(text)
                .selected(current == value)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    set(this, value);
                    cx.notify();
                }))
        });
        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(124.))
                    .flex_none()
                    .text_sm()
                    .text_color(self.palette.text_muted)
                    .child(label),
            )
            .children(buttons)
            .into_any_element()
    }

    fn location_choice(&self, cx: &mut Context<Self>) -> AnyElement {
        self.choice(
            "Add to",
            self.location,
            &[
                ("auth-in-header", "Header", ApiKeyLocation::Header),
                ("auth-in-query", "Query params", ApiKeyLocation::Query),
            ],
            |this, at| this.location = at,
            cx,
        )
    }

    fn render_oauth2(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = self.palette;
        let grant = self.grant(cx);
        let mut out = vec![
            self.row(
                "Grant type",
                div().w(px(220.)).child(Select::new(&self.grant).small()),
            )
            .into_any_element(),
        ];
        if grant == OAuth2Grant::AuthorizationCode {
            out.push(self.field(F::AuthUrl));
        }
        out.push(self.field(F::TokenUrl));
        out.push(self.field(F::ClientId));
        out.push(self.field(F::ClientSecret));
        out.push(self.field(F::Scope));
        if grant == OAuth2Grant::Password {
            out.push(self.field(F::Username));
            out.push(self.field(F::Password));
        }
        if grant == OAuth2Grant::AuthorizationCode {
            out.push(self.field(F::RedirectUri));
            out.push(
                self.row(
                    "",
                    Checkbox::new("auth-pkce")
                        .checked(self.pkce)
                        .label("Use PKCE (recommended)")
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.pkce = *checked;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            );
        }
        out.push(self.choice(
            "Client auth",
            self.client_auth,
            &[
                ("auth-client-basic", "Basic auth header", ClientAuth::Basic),
                ("auth-client-body", "In the body", ClientAuth::Body),
            ],
            |this, value| this.client_auth = value,
            cx,
        ));

        // The token: kept in memory only, fetched on the first send.
        let (status, color) = match &self.fetch {
            Fetch::Running if grant == OAuth2Grant::AuthorizationCode => (
                "Waiting for the sign-in in your browser…".to_owned(),
                p.text_muted,
            ),
            Fetch::Running => ("Getting a token…".to_owned(), p.text_muted),
            Fetch::Failed(error) => (error.clone(), p.err),
            Fetch::Idle => match self
                .resolved_oauth2(cx)
                .and_then(|c| apix_http::oauth2::cached(&c))
            {
                None => (
                    "No token yet. One is fetched on the first send.".to_owned(),
                    p.text_muted,
                ),
                Some(token) => match token.expires_in() {
                    Some(left) if left.as_secs() < 30 => (
                        "The token has expired; the next send gets a new one.".to_owned(),
                        p.text_muted,
                    ),
                    Some(left) => (
                        format!("Token ready, valid for {}.", human_minutes(left.as_secs())),
                        p.ok,
                    ),
                    None => ("Token ready.".to_owned(), p.ok),
                },
            },
        };
        let running = matches!(self.fetch, Fetch::Running);
        out.push(
            self.row(
                "Token",
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(color)
                            .child(status),
                    )
                    .child(
                        Button::new("auth-get-token")
                            .small()
                            .label("Get new token")
                            .loading(running)
                            .disabled(running)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.get_token(window, cx)
                            })),
                    )
                    .child(
                        Button::new("auth-forget-token")
                            .ghost()
                            .small()
                            .label("Forget")
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.forget_token(cx)),
                            ),
                    ),
            )
            .into_any_element(),
        );
        out.push(
            div()
                .text_xs()
                .text_color(p.text_muted.opacity(0.8))
                .child(
                    "Apix gets the token itself and sends it as a bearer token. Tokens stay \
                     in memory and are never written to disk.",
                )
                .into_any_element(),
        );
        out
    }
}

/// `54 min`, `2 h 5 min`.
fn human_minutes(secs: u64) -> String {
    let minutes = secs.div_ceil(60);
    if minutes < 60 {
        format!("{minutes} min")
    } else {
        format!("{} h {} min", minutes / 60, minutes % 60)
    }
}

impl Render for AuthEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let kind = self.kind(cx);
        let body: Vec<AnyElement> = match kind {
            0 => {
                let (auth, from) = &self.inherited;
                let what = match auth {
                    Auth::Bearer { .. } => "a bearer token",
                    Auth::Basic { .. } => "basic auth",
                    Auth::ApiKey { .. } => "an API key",
                    Auth::Digest { .. } => "digest auth",
                    Auth::OAuth1(_) => "OAuth 1.0",
                    Auth::OAuth2(_) => "OAuth 2.0",
                    Auth::AwsSigV4(_) => "an AWS signature",
                    Auth::Inherit | Auth::None => "no auth",
                };
                let text = match (from, self.owner) {
                    (Some(from), _) => format!("Uses {what} from the {from}."),
                    (None, "project") => "Requests that inherit get no auth.".to_owned(),
                    (None, "collection") => "Nothing above sets auth, so none is sent. \
                         Set it for the whole project from the project menu (Project auth…)."
                        .to_owned(),
                    (None, _) => "Nothing above sets auth, so none is sent. Set it on a folder \
                         or collection from its right-click menu (Auth…)."
                        .to_owned(),
                };
                let mut out = vec![self.note(text)];
                if let Some(detail) = describe(auth) {
                    out.push(
                        div()
                            .text_xs()
                            .text_color(p.text_muted.opacity(0.8))
                            .child(detail)
                            .into_any_element(),
                    );
                }
                out
            }
            1 => vec![self.note(format!(
                "No auth is sent, even if a folder or collection sets one for this {}.",
                self.owner
            ))],
            2 => vec![
                self.field(F::Token),
                self.note("Sent as Authorization: Bearer <token>."),
            ],
            3 => vec![self.field(F::Username), self.field(F::Password)],
            4 => vec![
                self.field(F::Key),
                self.field(F::Value),
                self.location_choice(cx),
            ],
            5 => vec![
                self.field(F::Username),
                self.field(F::Password),
                self.note("Apix answers the server's Digest challenge (MD5 or SHA-256)."),
            ],
            6 => vec![
                self.field(F::ConsumerKey),
                self.field(F::ConsumerSecret),
                self.field(F::AccessToken),
                self.field(F::TokenSecret),
                self.row(
                    "Signature",
                    div()
                        .w(px(220.))
                        .child(Select::new(&self.signature).small()),
                )
                .into_any_element(),
                self.field(F::Realm),
                self.location_choice(cx),
                self.note("Signed with a fresh nonce and timestamp on every send."),
            ],
            7 => self.render_oauth2(cx),
            _ => vec![
                self.field(F::AccessKey),
                self.field(F::SecretKey),
                self.field(F::SessionToken),
                self.field(F::Region),
                self.field(F::Service),
                self.note("Every send is signed with AWS Signature Version 4."),
            ],
        };
        v_flex()
            .gap_3()
            .child(self.row(
                "Type",
                div().w(px(220.)).child(Select::new(&self.kind).small()),
            ))
            .children(body)
            .when(kind >= 2, |col| {
                col.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted.opacity(0.8))
                        .child("Values can use {{variables}}."),
                )
            })
    }
}

/// One line about an inherited auth, without revealing secrets.
fn describe(auth: &Auth) -> Option<String> {
    match auth {
        Auth::Bearer { token } if token.starts_with("{{") => Some(format!("Token: {token}")),
        Auth::Bearer { .. } => Some("Token: set".into()),
        Auth::Basic { username, .. } | Auth::Digest { username, .. } => {
            Some(format!("Username: {username}"))
        }
        Auth::ApiKey { key, location, .. } => Some(format!(
            "{key} in the {}",
            match location {
                ApiKeyLocation::Header => "headers",
                ApiKeyLocation::Query => "query string",
            }
        )),
        Auth::OAuth1(o) => Some(format!("Consumer key: {}", o.consumer_key)),
        Auth::OAuth2(o) => Some(format!("{}, token from {}", o.grant.label(), o.token_url)),
        Auth::AwsSigV4(a) => Some(format!("{} in {}", a.service, a.region)),
        Auth::Inherit | Auth::None => None,
    }
}

impl super::Workspace {
    /// What a request at `path` gets from "Inherit": the nearest folder,
    /// collection or project auth, and where it is set. Without a path (an
    /// unsaved request), the active collection's.
    pub(super) fn inherited_auth_for(&self, path: Option<&Path>) -> (Auth, Option<String>) {
        let collection = path
            .and_then(|p| self.collection_for(p))
            .or(self.collection.as_ref());
        let dir = path
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| collection.map(|c| c.root.clone()));
        let Some(dir) = dir else {
            return (Auth::None, None);
        };
        project::inherited_auth(&dir, collection, self.project.as_ref())
            .unwrap_or((Auth::None, None))
    }

    /// Updates the Auth tab's "Inherit" note for the open request.
    pub(super) fn refresh_inherited_auth(&mut self, cx: &mut Context<Self>) {
        let inherited = self.inherited_auth_for(self.selected.as_deref());
        self.auth_editor
            .update(cx, |editor, cx| editor.set_inherited(inherited, cx));
    }

    /// `request` (saved at `path`) with "Inherit" replaced by what it inherits.
    pub(super) fn with_inherited_auth(&self, mut request: Request, path: Option<&Path>) -> Request {
        if request.auth.is_inherit() {
            request.auth = self.inherited_auth_for(path).0;
        }
        request
    }
}

/// Where a shared auth is set.
#[derive(Debug, Clone)]
pub(super) enum AuthOwner {
    /// `<folder>/folder.toml`
    Folder(PathBuf),
    /// The collection's `apix.toml`
    Collection(PathBuf),
    /// `apix-project.toml`
    Project(PathBuf),
}

impl super::Workspace {
    /// The dialog that sets a folder's, collection's or project's auth.
    pub(super) fn open_auth_dialog(
        &mut self,
        owner: AuthOwner,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let loaded = match &owner {
            AuthOwner::Folder(dir) => store::load_folder_meta(dir).map(|meta| {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                let collection = self.collection_for(dir);
                let parent = dir.parent().unwrap_or(dir);
                let inherited = project::inherited_auth(parent, collection, self.project.as_ref());
                (
                    format!("Auth for folder \"{name}\""),
                    "folder",
                    meta.auth,
                    inherited,
                )
            }),
            AuthOwner::Collection(root) => Collection::open(root).map(|c| {
                let inherited = project::inherited_auth(root, None, self.project.as_ref());
                (
                    format!("Auth for collection \"{}\"", c.meta.name),
                    "collection",
                    c.meta.auth,
                    inherited,
                )
            }),
            AuthOwner::Project(root) => Project::open(root).map(|p| {
                (
                    format!("Auth for project \"{}\"", p.meta.name),
                    "project",
                    p.meta.auth,
                    Ok((Auth::None, None)),
                )
            }),
        };
        let (title, label, current, inherited) = match loaded {
            Ok(found) => found,
            Err(e) => return self.notify_error(super::describe(&e), window, cx),
        };
        let inherited = inherited.unwrap_or((Auth::None, None));
        let palette = self.palette;
        let workspace = cx.entity().downgrade();
        let editor = cx.new(|cx| {
            let mut editor = AuthEditor::new(palette, label, workspace, window, cx);
            editor.set_auth(&current, window, cx);
            editor.set_inherited(inherited, cx);
            editor
        });
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let (save, cancel_view, save_view, owner) =
                (editor.clone(), view.clone(), view.clone(), owner.clone());
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .title(title.clone())
                .width(px(620.))
                .child(div().pb_2().child(editor.clone()))
                .on_ok(|_, _, _| false)
                .footer(
                    h_flex()
                        .w_full()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(palette.text_muted)
                                .child(format!(
                                    "Requests in this {label} without their own auth use this."
                                )),
                        )
                        .child(Button::new("auth-cancel").ghost().label("Cancel").on_click(
                            move |_, window, cx| {
                                super::collections::close_dialog(&cancel_view, window, cx)
                            },
                        ))
                        .child(Button::new("auth-save").primary().label("Save").on_click(
                            move |_, window, cx| {
                                let auth = save.read(cx).auth(cx);
                                let saved = save_view
                                    .update(cx, |this, cx| {
                                        this.save_shared_auth(&owner, auth, window, cx)
                                    })
                                    .unwrap_or(false);
                                if saved {
                                    super::collections::close_dialog(&save_view, window, cx);
                                }
                            },
                        )),
                )
        });
    }

    fn save_shared_auth(
        &mut self,
        owner: &AuthOwner,
        auth: Auth,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let result = match owner {
            AuthOwner::Folder(dir) => store::save_folder_meta(dir, &store::FolderMeta { auth }),
            AuthOwner::Collection(root) => Collection::open(root).and_then(|mut c| {
                c.meta.auth = auth;
                store::save_collection_meta(root, &c.meta)
            }),
            AuthOwner::Project(root) => Project::open(root).and_then(|mut p| {
                p.meta.auth = auth;
                p.save()
            }),
        };
        match result {
            Ok(()) => {
                self.reload(window, cx);
                true
            }
            Err(e) => {
                self.notify_error(super::describe(&e), window, cx);
                false
            }
        }
    }
}
