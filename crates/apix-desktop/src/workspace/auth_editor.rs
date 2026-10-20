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
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme::Palette;
use crate::tr;

/// The picker's entries, in `kind_index` order.
fn kinds() -> Vec<SharedString> {
    vec![
        tr!("auth.kind.inherit"),
        tr!("auth.kind.none"),
        tr!("auth.kind.bearer"),
        tr!("auth.kind.basic"),
        tr!("auth.kind.api_key"),
        tr!("auth.kind.digest"),
        "OAuth 1.0".into(),
        "OAuth 2.0".into(),
        tr!("auth.kind.aws"),
    ]
}

fn grant_label(grant: OAuth2Grant) -> SharedString {
    match grant {
        OAuth2Grant::ClientCredentials => tr!("auth.grant.client_credentials"),
        OAuth2Grant::Password => tr!("auth.grant.password"),
        OAuth2Grant::AuthorizationCode => tr!("auth.grant.authorization_code"),
    }
}

fn grants() -> Vec<SharedString> {
    OAuth2Grant::ALL.into_iter().map(grant_label).collect()
}

type Picker = SelectState<Vec<SharedString>>;

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

    fn label(self) -> SharedString {
        match self {
            F::Token => tr!("auth.field.token"),
            F::Username => tr!("auth.field.username"),
            F::Password => tr!("auth.field.password"),
            F::Key => tr!("auth.field.key"),
            F::Value => tr!("auth.field.value"),
            F::ConsumerKey => tr!("auth.field.consumer_key"),
            F::ConsumerSecret => tr!("auth.field.consumer_secret"),
            F::AccessToken => tr!("auth.field.access_token"),
            F::TokenSecret => tr!("auth.field.token_secret"),
            F::Realm => tr!("auth.field.realm"),
            F::TokenUrl => tr!("auth.field.token_url"),
            F::AuthUrl => tr!("auth.field.auth_url"),
            F::ClientId => tr!("auth.field.client_id"),
            F::ClientSecret => tr!("auth.field.client_secret"),
            F::Scope => tr!("auth.field.scope"),
            F::RedirectUri => tr!("auth.field.redirect_url"),
            F::AccessKey => tr!("auth.field.access_key"),
            F::SecretKey => tr!("auth.field.secret_key"),
            F::SessionToken => tr!("auth.field.session_token"),
            F::Region => tr!("auth.field.region"),
            F::Service => tr!("auth.field.service"),
        }
    }

    fn placeholder(self) -> SharedString {
        match self {
            F::Token => tr!("auth.hint.token"),
            F::Key => tr!("auth.hint.key"),
            F::Value => tr!("auth.hint.value"),
            F::AccessToken | F::TokenSecret | F::Realm => tr!("auth.hint.optional"),
            F::TokenUrl => "https://auth.example.com/oauth/token".into(),
            F::AuthUrl => "https://auth.example.com/authorize".into(),
            F::ClientSecret => tr!("auth.hint.client_secret"),
            F::Scope => tr!("auth.hint.scope"),
            F::RedirectUri => tr!("auth.hint.redirect_url"),
            F::AccessKey => tr!("auth.hint.access_key"),
            F::SessionToken => tr!("auth.hint.session_token"),
            F::Region => tr!("auth.hint.region"),
            F::Service => tr!("auth.hint.service"),
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
        let picker = |items: Vec<SharedString>, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| SelectState::new(items, Some(IndexPath::new(0)), window, cx))
        };
        let kind = picker(kinds(), window, cx);
        let signature = picker(
            OAuth1Method::ALL
                .iter()
                .map(|m| SharedString::from(m.as_str()))
                .collect(),
            window,
            cx,
        );
        let grant = picker(grants(), window, cx);

        let mut subscriptions = Vec::new();
        for select in [&kind, &signature, &grant] {
            subscriptions.push(cx.subscribe_in(
                select,
                window,
                |_, _, _: &SelectEvent<Vec<SharedString>>, _, cx| cx.notify(),
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

    /// Relabels the pickers and placeholders after a language change.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // `set_items` keeps the selection by position, which is all that changes here.
        let keep = |select: &Entity<Picker>,
                    items: Vec<SharedString>,
                    window: &mut Window,
                    cx: &mut App| {
            let ix = select.read(cx).selected_index(cx);
            select.update(cx, |state, cx| {
                state.set_items(items, window, cx);
                state.set_selected_index(ix, window, cx);
            });
        };
        keep(&self.kind, kinds(), window, cx);
        keep(&self.grant, grants(), window, cx);
        for f in F::ALL {
            self.fields[&f].update(cx, |state, cx| {
                state.set_placeholder(f.placeholder(), window, cx)
            });
        }
        cx.notify();
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

    fn row(&self, label: impl Into<SharedString>, child: impl IntoElement) -> Div {
        let label: SharedString = label.into();
        h_flex()
            .dir_row()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(150.))
                    .flex_none()
                    .text_sm()
                    .text_color(self.palette.text_muted)
                    .child(label),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(520.))
                    // A row in RTL, so a narrower control (a picker) sits next to the label.
                    .when(bidi::rtl(), |d| d.flex().flex_row_reverse())
                    .child(child),
            )
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
            .child(bidi::para(text))
            .into_any_element()
    }

    /// Two or more toggle buttons in a labelled row.
    fn choice<T: Copy + PartialEq + 'static>(
        &self,
        label: SharedString,
        current: T,
        options: Vec<(&'static str, SharedString, T)>,
        set: fn(&mut Self, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let buttons = options.into_iter().map(|(id, text, value)| {
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
            .dir_row()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(150.))
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
            tr!("auth.add_to"),
            self.location,
            vec![
                (
                    "auth-in-header",
                    tr!("auth.in_header"),
                    ApiKeyLocation::Header,
                ),
                ("auth-in-query", tr!("auth.in_query"), ApiKeyLocation::Query),
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
                tr!("auth.grant_type"),
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
                        .dir_row()
                        .checked(self.pkce)
                        .label(tr!("auth.use_pkce"))
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.pkce = *checked;
                            cx.notify();
                        })),
                )
                .into_any_element(),
            );
        }
        out.push(self.choice(
            tr!("auth.client_auth"),
            self.client_auth,
            vec![
                (
                    "auth-client-basic",
                    tr!("auth.client_basic"),
                    ClientAuth::Basic,
                ),
                (
                    "auth-client-body",
                    tr!("auth.client_body"),
                    ClientAuth::Body,
                ),
            ],
            |this, value| this.client_auth = value,
            cx,
        ));

        // The token: kept in memory only, fetched on the first send.
        let (status, color) = match &self.fetch {
            Fetch::Running if grant == OAuth2Grant::AuthorizationCode => {
                (tr!("auth.token.waiting_browser"), p.text_muted)
            }
            Fetch::Running => (tr!("auth.token.getting"), p.text_muted),
            Fetch::Failed(error) => (error.clone().into(), p.err),
            Fetch::Idle => match self
                .resolved_oauth2(cx)
                .and_then(|c| apix_http::oauth2::cached(&c))
            {
                None => (tr!("auth.token.none"), p.text_muted),
                Some(token) => match token.expires_in() {
                    Some(left) if left.as_secs() < 30 => (tr!("auth.token.expired"), p.text_muted),
                    Some(left) => (
                        tr!("auth.token.valid_for", time = human_minutes(left.as_secs())),
                        p.ok,
                    ),
                    None => (tr!("auth.token.ready"), p.ok),
                },
            },
        };
        let running = matches!(self.fetch, Fetch::Running);
        out.push(
            self.row(
                tr!("auth.field.token"),
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(color)
                            .child(bidi::para(status)),
                    )
                    .child(
                        Button::new("auth-get-token")
                            .small()
                            .label(tr!("auth.token.get_new"))
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
                            .label(tr!("auth.token.forget"))
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
                .child(bidi::para(tr!("auth.note.oauth2")))
                .into_any_element(),
        );
        out
    }
}

/// `54 min`, `2 h 5 min`.
fn human_minutes(secs: u64) -> SharedString {
    let minutes = secs.div_ceil(60);
    if minutes < 60 {
        tr!("time.minutes", count = minutes)
    } else {
        tr!(
            "time.hours_minutes",
            hours = minutes / 60,
            minutes = minutes % 60
        )
    }
}

/// "Uses â€¦ from the folder "orders"." The source comes from apix-core as
/// `folder "orders"`, `collection "Shop"` or `project "Shop"`.
fn inherited_note(what: SharedString, from: &str) -> SharedString {
    let (kind, name) = from.split_once(' ').unwrap_or((from, ""));
    let name = name.trim_matches('"');
    match kind {
        "folder" => tr!("auth.inherit.from_folder", what = what, name = name),
        "collection" => tr!("auth.inherit.from_collection", what = what, name = name),
        _ => tr!("auth.inherit.from_project", what = what, name = name),
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
                    Auth::Bearer { .. } => tr!("auth.what.bearer"),
                    Auth::Basic { .. } => tr!("auth.what.basic"),
                    Auth::ApiKey { .. } => tr!("auth.what.api_key"),
                    Auth::Digest { .. } => tr!("auth.what.digest"),
                    Auth::OAuth1(_) => "OAuth 1.0".into(),
                    Auth::OAuth2(_) => "OAuth 2.0".into(),
                    Auth::AwsSigV4(_) => tr!("auth.what.aws"),
                    Auth::Inherit | Auth::None => tr!("auth.what.none"),
                };
                let text = match (from, self.owner) {
                    (Some(from), _) => inherited_note(what, from),
                    (None, "project") => tr!("auth.inherit.project_none"),
                    (None, "collection") => tr!("auth.inherit.collection_none"),
                    (None, _) => tr!("auth.inherit.request_none"),
                };
                let mut out = vec![self.note(text)];
                if let Some(detail) = describe(auth) {
                    out.push(
                        div()
                            .text_xs()
                            .text_color(p.text_muted.opacity(0.8))
                            .child(bidi::para(detail))
                            .into_any_element(),
                    );
                }
                out
            }
            1 => vec![self.note(match self.owner {
                "folder" => tr!("auth.no_auth.folder"),
                "collection" => tr!("auth.no_auth.collection"),
                "project" => tr!("auth.no_auth.project"),
                _ => tr!("auth.no_auth.request"),
            })],
            2 => vec![self.field(F::Token), self.note(tr!("auth.note.bearer"))],
            3 => vec![self.field(F::Username), self.field(F::Password)],
            4 => vec![
                self.field(F::Key),
                self.field(F::Value),
                self.location_choice(cx),
            ],
            5 => vec![
                self.field(F::Username),
                self.field(F::Password),
                self.note(tr!("auth.note.digest")),
            ],
            6 => vec![
                self.field(F::ConsumerKey),
                self.field(F::ConsumerSecret),
                self.field(F::AccessToken),
                self.field(F::TokenSecret),
                self.row(
                    tr!("auth.signature"),
                    div()
                        .w(px(220.))
                        .child(Select::new(&self.signature).small()),
                )
                .into_any_element(),
                self.field(F::Realm),
                self.location_choice(cx),
                self.note(tr!("auth.note.oauth1")),
            ],
            7 => self.render_oauth2(cx),
            _ => vec![
                self.field(F::AccessKey),
                self.field(F::SecretKey),
                self.field(F::SessionToken),
                self.field(F::Region),
                self.field(F::Service),
                self.note(tr!("auth.note.aws")),
            ],
        };
        v_flex()
            .gap_3()
            .child(self.row(
                tr!("auth.type"),
                div().w(px(220.)).child(Select::new(&self.kind).small()),
            ))
            .children(body)
            .when(kind >= 2, |col| {
                col.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted.opacity(0.8))
                        .child(bidi::para(tr!("auth.note.variables"))),
                )
            })
    }
}

/// One line about an inherited auth, without revealing secrets.
fn describe(auth: &Auth) -> Option<SharedString> {
    match auth {
        Auth::Bearer { token } if token.starts_with("{{") => {
            Some(tr!("auth.describe.token", token = token))
        }
        Auth::Bearer { .. } => Some(tr!("auth.describe.token_set")),
        Auth::Basic { username, .. } | Auth::Digest { username, .. } => {
            Some(tr!("auth.describe.username", name = username))
        }
        Auth::ApiKey { key, location, .. } => Some(match location {
            ApiKeyLocation::Header => tr!("auth.describe.key_in_header", key = key),
            ApiKeyLocation::Query => tr!("auth.describe.key_in_query", key = key),
        }),
        Auth::OAuth1(o) => Some(tr!("auth.describe.consumer_key", key = o.consumer_key)),
        Auth::OAuth2(o) => Some(tr!(
            "auth.describe.oauth2",
            grant = grant_label(o.grant),
            url = o.token_url
        )),
        Auth::AwsSigV4(a) => Some(tr!(
            "auth.describe.aws",
            service = a.service,
            region = a.region
        )),
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
                    tr!("auth.dialog.folder_title", name = name),
                    "folder",
                    meta.auth,
                    inherited,
                )
            }),
            AuthOwner::Collection(root) => Collection::open(root).map(|c| {
                let inherited = project::inherited_auth(root, None, self.project.as_ref());
                (
                    tr!("auth.dialog.collection_title", name = c.meta.name),
                    "collection",
                    c.meta.auth,
                    inherited,
                )
            }),
            AuthOwner::Project(root) => Project::open(root).map(|p| {
                (
                    tr!("auth.dialog.project_title", name = p.meta.name),
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
                .bidi_title(title.clone())
                .width(px(620.))
                .child(div().pb_2().child(editor.clone()))
                .on_ok(|_, _, _| false)
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(palette.text_muted)
                                .child(bidi::para(match label {
                                    "folder" => tr!("auth.dialog.folder_note"),
                                    "collection" => tr!("auth.dialog.collection_note"),
                                    _ => tr!("auth.dialog.project_note"),
                                })),
                        )
                        .child(
                            Button::new("auth-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    super::collections::close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("auth-save")
                                .primary()
                                .label(tr!("dialog.save"))
                                .on_click(move |_, window, cx| {
                                    let auth = save.read(cx).auth(cx);
                                    let saved = save_view
                                        .update(cx, |this, cx| {
                                            this.save_shared_auth(&owner, auth, window, cx)
                                        })
                                        .unwrap_or(false);
                                    if saved {
                                        super::collections::close_dialog(&save_view, window, cx);
                                    }
                                }),
                        ),
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
