//! The Auth form: used as the request's Auth tab and in the folder and
//! collection Auth dialog. "Inherit from parent" shows what would be used
//! instead and where it is set.

use std::path::{Path, PathBuf};

use apix_core::store;
use apix_core::{ApiKeyLocation, Auth, Request};
use apix_core::{Collection, Project, project};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Selectable as _, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::Palette;

/// The picker's entries, in `Kind` order.
const KINDS: [&str; 5] = [
    "Inherit from parent",
    "No auth",
    "Bearer token",
    "Basic auth",
    "API key",
];

type KindSelect = SelectState<Vec<&'static str>>;

fn kind_index(auth: &Auth) -> usize {
    match auth {
        Auth::Inherit => 0,
        Auth::None => 1,
        Auth::Bearer { .. } => 2,
        Auth::Basic { .. } => 3,
        Auth::ApiKey { .. } => 4,
    }
}

pub(super) struct AuthEditor {
    palette: Palette,
    kind: Entity<KindSelect>,
    token: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    key: Entity<InputState>,
    value: Entity<InputState>,
    location: ApiKeyLocation,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let kind =
            cx.new(|cx| SelectState::new(KINDS.to_vec(), Some(IndexPath::new(0)), window, cx));
        let input = |placeholder: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let token = input("Token, e.g. {{token}}", window, cx);
        let username = input("Username", window, cx);
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Password")
                .masked(true)
        });
        let key = input("Name, e.g. X-API-Key", window, cx);
        let value = input("Value, e.g. {{apiKey}}", window, cx);

        let mut subscriptions = vec![cx.subscribe_in(
            &kind,
            window,
            |_, _, _: &SelectEvent<Vec<&'static str>>, _, cx| cx.notify(),
        )];
        for field in [&token, &username, &password, &key, &value] {
            subscriptions.push(cx.subscribe(field, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        Self {
            palette,
            kind,
            token,
            username,
            password,
            key,
            value,
            location: ApiKeyLocation::Header,
            inherited: (Auth::None, None),
            owner,
            _subscriptions: subscriptions,
        }
    }

    /// Shows `auth`; the fields of other kinds are cleared.
    pub(super) fn set_auth(&mut self, auth: &Auth, window: &mut Window, cx: &mut Context<Self>) {
        let ix = kind_index(auth);
        self.kind.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(ix)), window, cx)
        });
        let (mut token, mut username, mut password, mut key, mut value) = Default::default();
        self.location = ApiKeyLocation::Header;
        match auth {
            Auth::Bearer { token: t } => token = t.as_str(),
            Auth::Basic {
                username: u,
                password: p,
            } => (username, password) = (u.as_str(), p.as_str()),
            Auth::ApiKey {
                key: k,
                value: v,
                location,
            } => {
                (key, value) = (k.as_str(), v.as_str());
                self.location = *location;
            }
            Auth::Inherit | Auth::None => {}
        }
        for (field, text) in [
            (&self.token, token),
            (&self.username, username),
            (&self.password, password),
            (&self.key, key),
            (&self.value, value),
        ] {
            let text = text.to_owned();
            field.update(cx, |state, cx| state.set_value(text, window, cx));
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

    fn kind(&self, cx: &App) -> usize {
        self.kind
            .read(cx)
            .selected_value()
            .and_then(|v| KINDS.iter().position(|k| k == v))
            .unwrap_or(0)
    }

    /// The auth as the form shows it now.
    pub(super) fn auth(&self, cx: &App) -> Auth {
        let text = |field: &Entity<InputState>| field.read(cx).value().to_string();
        match self.kind(cx) {
            0 => Auth::Inherit,
            1 => Auth::None,
            2 => Auth::Bearer {
                token: text(&self.token).trim().to_owned(),
            },
            3 => Auth::Basic {
                username: text(&self.username),
                password: text(&self.password),
            },
            _ => Auth::ApiKey {
                key: text(&self.key).trim().to_owned(),
                value: text(&self.value),
                location: self.location,
            },
        }
    }

    fn field(&self, label: &'static str, input: Input) -> Div {
        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(84.))
                    .flex_none()
                    .text_sm()
                    .text_color(self.palette.text_muted)
                    .child(label),
            )
            .child(div().flex_1().min_w_0().max_w(px(520.)).child(input))
    }

    fn note(&self, text: String) -> Div {
        div()
            .text_sm()
            .text_color(self.palette.text_muted)
            .child(text)
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
                let mut out = vec![self.note(text).into_any_element()];
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
            1 => vec![
                self.note(format!(
                    "No auth is sent, even if a folder or collection sets one for this {}.",
                    self.owner
                ))
                .into_any_element(),
            ],
            2 => vec![
                self.field("Token", Input::new(&self.token).small())
                    .into_any_element(),
                self.note("Sent as Authorization: Bearer <token>.".into())
                    .into_any_element(),
            ],
            3 => vec![
                self.field("Username", Input::new(&self.username).small())
                    .into_any_element(),
                self.field("Password", Input::new(&self.password).small().mask_toggle())
                    .into_any_element(),
            ],
            _ => {
                let location = self.location;
                let place = |id: &'static str, label: &'static str, at: ApiKeyLocation| {
                    Button::new(id)
                        .ghost()
                        .small()
                        .label(label)
                        .selected(location == at)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.location = at;
                            cx.notify();
                        }))
                };
                vec![
                    self.field("Key", Input::new(&self.key).small())
                        .into_any_element(),
                    self.field("Value", Input::new(&self.value).small())
                        .into_any_element(),
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            div()
                                .w(px(84.))
                                .flex_none()
                                .text_sm()
                                .text_color(p.text_muted)
                                .child("Add to"),
                        )
                        .child(place("auth-in-header", "Header", ApiKeyLocation::Header))
                        .child(place(
                            "auth-in-query",
                            "Query params",
                            ApiKeyLocation::Query,
                        ))
                        .into_any_element(),
                ]
            }
        };
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .w(px(84.))
                            .flex_none()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child("Type"),
                    )
                    .child(div().w(px(220.)).child(Select::new(&self.kind).small())),
            )
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
        Auth::Basic { username, .. } => Some(format!("Username: {username}")),
        Auth::ApiKey { key, location, .. } => Some(format!(
            "{key} in the {}",
            match location {
                ApiKeyLocation::Header => "headers",
                ApiKeyLocation::Query => "query string",
            }
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
        let editor = cx.new(|cx| {
            let mut editor = AuthEditor::new(palette, label, window, cx);
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
