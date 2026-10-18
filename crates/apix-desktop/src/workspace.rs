//! The main window: collection sidebar, request editor and response viewer.

mod auth_editor;
mod body_editor;
mod collections;
mod drag;
mod env_editor;
mod menu_bar;
mod network;
mod palette;
mod shell_copy;
mod tabs;
mod var_chips;
mod vault_ui;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use apix_core::store::TreeNode;
use apix_core::{Auth, Collection, KeyValue, Method, Request, Variables};
use apix_http::{Response, group_digits, human_duration, human_size};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    IconName, IndexPath, Sizable, TitleBar, WindowExt as _, h_flex, h_resizable, resizable_panel,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::large_view::{Document, LARGE_BODY_BYTES, LargeView};
use crate::net;
use crate::state::AppState;
use crate::theme::{self, Palette};

actions!(
    apix,
    [
        SendRequest,
        CancelRequest,
        SaveRequest,
        NewRequest,
        OpenCollection,
        ImportFile,
        EditVariables,
        OpenPalette,
        CloseTab,
        NextTab,
        PrevTab,
        MethodGet,
        MethodPost,
        MethodPut,
        MethodPatch,
        MethodDelete,
        MethodHead,
        MethodOptions
    ]
);

pub use body_editor::register_graphql;

const CONTEXT: &str = "ApixWorkspace";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-enter", SendRequest, Some(CONTEXT)),
        KeyBinding::new("escape", CancelRequest, Some(CONTEXT)),
        KeyBinding::new("secondary-s", SaveRequest, Some(CONTEXT)),
        KeyBinding::new("secondary-n", NewRequest, Some(CONTEXT)),
        KeyBinding::new("secondary-o", OpenCollection, Some(CONTEXT)),
        KeyBinding::new("secondary-e", EditVariables, Some(CONTEXT)),
        KeyBinding::new("secondary-k", OpenPalette, Some(CONTEXT)),
        KeyBinding::new("secondary-p", OpenPalette, Some(CONTEXT)),
        KeyBinding::new("secondary-w", CloseTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-tab", NextTab, Some(CONTEXT)),
        KeyBinding::new("ctrl-shift-tab", PrevTab, Some(CONTEXT)),
        // Alt+1…7 pick the method, in the order of the method picker.
        KeyBinding::new("alt-1", MethodGet, Some(CONTEXT)),
        KeyBinding::new("alt-2", MethodPost, Some(CONTEXT)),
        KeyBinding::new("alt-3", MethodPut, Some(CONTEXT)),
        KeyBinding::new("alt-4", MethodPatch, Some(CONTEXT)),
        KeyBinding::new("alt-5", MethodDelete, Some(CONTEXT)),
        KeyBinding::new("alt-6", MethodHead, Some(CONTEXT)),
        KeyBinding::new("alt-7", MethodOptions, Some(CONTEXT)),
    ]);
}

type MethodSelect = SelectState<Vec<&'static str>>;
type EnvSelect = SelectState<Vec<SharedString>>;

/// The first entry of the environment picker.
const NO_ENVIRONMENT: &str = "No environment";

/// One editable row of the Params or Headers table.
struct KvRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
}

impl KvRow {
    fn new(kv: &KeyValue, window: &mut Window, cx: &mut App) -> Self {
        let key = kv.key.clone();
        let value = kv.value.clone();
        Self {
            key: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Key")
                    .default_value(key)
            }),
            value: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Value")
                    .default_value(value)
            }),
            enabled: kv.enabled,
        }
    }

    /// Rows without a key are ignored, like Postman's trailing empty row.
    fn to_key_value(&self, cx: &App) -> Option<KeyValue> {
        let key = self.key.read(cx).value().trim().to_string();
        (!key.is_empty()).then(|| KeyValue {
            key,
            value: self.value.read(cx).value().to_string(),
            enabled: self.enabled,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Table {
    Params,
    Headers,
}

impl Table {
    fn ids(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Table::Params => ("param-on", "param-del", "param-add"),
            Table::Headers => ("header-on", "header-del", "header-add"),
        }
    }
}

/// A response with its body already formatted and indexed for viewing.
struct Finished {
    response: Response,
    pretty: Arc<Document>,
    /// The unformatted body. For large bodies it is only built when the Raw
    /// tab is opened, so a 100 MB response is not held three times over.
    raw: Option<Arc<Document>>,
    /// Too big for the code editor; shown in the virtualized viewer.
    large: bool,
    json: bool,
    /// This machine's side of the connection, read once per response.
    local: apix_http::LocalNetwork,
}

impl Finished {
    /// Linear in the body size. Runs on a background thread.
    fn prepare(response: Response) -> Self {
        let json = response.is_json();
        let large = response.body.len() >= LARGE_BODY_BYTES;
        let formatted = json
            .then(|| apix_core::text::pretty_json(&response.body))
            .flatten();
        let (pretty, raw) = match formatted {
            Some(text) => {
                let raw = (!large).then(|| raw_document(&response, json));
                (Arc::new(Document::new(text, true)), raw)
            }
            // Nothing to format: Pretty and Raw are the same text.
            None => {
                let doc = raw_document(&response, json);
                (doc.clone(), Some(doc))
            }
        };
        let local = apix_http::local_network(response.trace.local_addr.map(|a| a.ip()));
        Self {
            response,
            pretty,
            raw,
            large,
            json,
            local,
        }
    }
}

fn raw_document(response: &Response, json: bool) -> Arc<Document> {
    Arc::new(Document::new(response.raw_body(), json))
}

enum ResponseState {
    Idle,
    Loading,
    Done(Rc<Finished>),
    Failed(String),
    Cancelled,
}

pub struct Workspace {
    focus: FocusHandle,
    palette: Palette,
    /// The open project, if the folder belongs to one.
    project: Option<apix_core::Project>,
    /// Every open collection: the project's, or the single one opened.
    collections: Vec<Collection>,
    /// The collection of the open request; its variables apply when sending.
    collection: Option<Collection>,
    /// Each collection's folders and request files, by collection root.
    trees: Vec<(PathBuf, Vec<TreeNode>)>,
    /// Every request as saved on disk, by path.
    requests: BTreeMap<PathBuf, Request>,
    collapsed: HashSet<PathBuf>,
    load_errors: Vec<String>,
    /// Open requests, in tab order; the one at `selected` is in front.
    tabs: Vec<tabs::Tab>,
    /// The request file open in the editor.
    selected: Option<PathBuf>,
    watcher: Option<collections::Watcher>,
    /// The active environment of the open collection, by file name.
    environment: Option<String>,
    env_select: Entity<EnvSelect>,
    app_state: AppState,
    /// The request as last loaded or saved; edits are compared to it.
    base: Request,
    /// The Auth tab.
    auth_editor: Entity<auth_editor::AuthEditor>,
    /// The Body tab.
    body_editor: Entity<body_editor::BodyEditor>,
    /// URL length before the last change, to tell a paste from typing.
    last_url_len: usize,
    /// The URL text whose references were last turned into chips.
    last_chipped: Option<String>,
    /// The `{{variable}}` chip under the pointer, for the hover card.
    hovered_var: Option<var_chips::HoveredVar>,
    vars: Variables,
    method: Entity<MethodSelect>,
    url: Entity<InputState>,
    params: Vec<KvRow>,
    headers: Vec<KvRow>,
    request_tab: usize,
    response_tab: usize,
    response: ResponseState,
    response_body: Entity<EditorState>,
    large_view: Option<Entity<LargeView>>,
    /// Raw view of a large body, built on first use.
    raw_doc: Option<Arc<Document>>,
    raw_task: Option<Task<()>>,
    /// When the user last clicked or typed, for auto-lock.
    last_input: std::cell::Cell<std::time::Instant>,
    /// Locks idle encrypted collections.
    _auto_lock: Option<Task<()>>,
    /// The in-flight request. Dropping it cancels the request.
    task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(
        dir: Option<PathBuf>,
        palette: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let methods: Vec<&'static str> = Method::ALL.iter().map(|m| m.as_str()).collect();
        let method = cx.new(|cx| SelectState::new(methods, Some(IndexPath::new(0)), window, cx));
        let url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("https://api.example.com/users   or   {{base_url}}/users")
        });
        let workspace = cx.weak_entity();
        let body_editor = cx.new(|cx| body_editor::BodyEditor::new(palette, workspace, window, cx));
        let env_select = cx.new(|cx| {
            SelectState::new(
                vec![SharedString::from(NO_ENVIRONMENT)],
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let response_body = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language("json");
            state.set_readonly(true, cx);
            state
        });

        let weak = cx.weak_entity();
        let auth_editor =
            cx.new(|cx| auth_editor::AuthEditor::new(palette, "request", weak, window, cx));
        let subscriptions = vec![
            // Typing in the Auth tab changes the unsaved marker.
            cx.observe(&auth_editor, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &url,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => this.send(window, cx),
                    InputEvent::Change => {
                        this.on_url_changed(window, cx);
                        this.chip_url(window, cx);
                    }
                    _ => {}
                },
            ),
            cx.observe(&body_editor, |_, _, cx| cx.notify()),
            // The body editors keep Ctrl+Enter for themselves and pass it on.
            cx.subscribe_in(
                &body_editor,
                window,
                |this, _, _: &body_editor::SendRequested, window, cx| this.send(window, cx),
            ),
            cx.subscribe_in(
                &method,
                window,
                |_, _, _: &SelectEvent<Vec<&'static str>>, _, cx| cx.notify(),
            ),
            cx.subscribe_in(
                &env_select,
                window,
                |this, state, _: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let name = state
                        .read(cx)
                        .selected_value()
                        .filter(|v| v.as_ref() != NO_ENVIRONMENT)
                        .map(|v| v.to_string());
                    this.set_environment(name, window, cx);
                },
            ),
        ];

        let mut this = Self {
            focus: cx.focus_handle(),
            palette,
            project: None,
            collections: Vec::new(),
            collection: None,
            trees: Vec::new(),
            requests: BTreeMap::new(),
            collapsed: HashSet::new(),
            load_errors: Vec::new(),
            selected: None,
            tabs: Vec::new(),
            watcher: None,
            environment: None,
            env_select,
            app_state: AppState::load(),
            base: Request::default(),
            auth_editor,
            body_editor,
            last_url_len: 0,
            last_chipped: None,
            hovered_var: None,
            vars: Variables::new(),
            method,
            url,
            params: Vec::new(),
            headers: Vec::new(),
            request_tab: 0,
            response_tab: 0,
            response: ResponseState::Idle,
            response_body,
            large_view: None,
            raw_doc: None,
            raw_task: None,
            task: None,
            last_input: std::cell::Cell::new(std::time::Instant::now()),
            _auto_lock: None,
            _subscriptions: subscriptions,
        };
        this._auto_lock = Some(Self::auto_lock_task(window, cx));
        // After the window exists: an encrypted folder opens with a dialog.
        if let Some(dir) = dir {
            cx.defer_in(window, move |this, window, cx| {
                this.open_or_create(dir, window, cx)
            });
        }
        this.url.update(cx, |state, cx| state.focus(window, cx));
        this
    }

    /// Opens `request` in the editor as the saved state (no unsaved changes).
    fn show_request(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        self.fill_editor(&request, window, cx);
        self.base = request;
        self.task = None;
        self.raw_task = None;
        self.raw_doc = None;
        self.large_view = None;
        self.response = ResponseState::Idle;
        cx.notify();
    }

    /// Alt+1…7: sets the method to the `ix`-th entry of `Method::ALL`.
    fn pick_method(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.method.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(ix)), window, cx)
        });
        cx.notify();
    }

    /// Puts `request`'s fields into the editor without touching the saved
    /// state, so the change shows as unsaved (used when pasting a curl command).
    fn fill_editor(&mut self, request: &Request, window: &mut Window, cx: &mut Context<Self>) {
        let method_ix = Method::ALL
            .iter()
            .position(|m| *m == request.method)
            .unwrap_or(0);
        self.method.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(method_ix)), window, cx)
        });
        let url = var_chips::chip_content(&request.url);
        self.last_url_len = request.url.chars().count();
        self.last_chipped = None;
        self.hovered_var = None;
        self.url
            .update(cx, |state, cx| state.set_value(url, window, cx));
        self.params = request
            .query
            .iter()
            .map(|kv| KvRow::new(kv, window, cx))
            .collect();
        self.headers = request
            .headers
            .iter()
            .map(|kv| KvRow::new(kv, window, cx))
            .collect();

        self.body_editor
            .update(cx, |editor, cx| editor.set_body(&request.body, window, cx));
        self.request_tab = if request.body.is_none() { 0 } else { 3 };
        let auth = request.auth.clone();
        self.auth_editor
            .update(cx, |editor, cx| editor.set_auth(&auth, window, cx));
        self.refresh_inherited_auth(cx);
        cx.notify();
    }

    /// The open request's folder and its collection's root, which body
    /// file paths are relative to.
    fn body_location(&self) -> (Option<PathBuf>, Option<PathBuf>) {
        let dir = self
            .selected
            .as_deref()
            .and_then(|p| p.parent())
            .map(PathBuf::from);
        let root = self.collection.as_ref().map(|c| c.root.clone());
        (dir, root)
    }

    fn current_method(&self, cx: &App) -> Method {
        self.method
            .read(cx)
            .selected_value()
            .and_then(|m| m.parse().ok())
            .unwrap_or_default()
    }

    fn build_request(&self, cx: &App) -> Request {
        let mut request = self.base.clone();
        request.auth = self.auth_editor.read(cx).auth(cx);
        request.method = self.current_method(cx);
        request.url = self.url.read(cx).value().trim().to_string();
        request.query = self
            .params
            .iter()
            .filter_map(|r| r.to_key_value(cx))
            .collect();
        request.headers = self
            .headers
            .iter()
            .filter_map(|r| r.to_key_value(cx))
            .collect();
        request.body = self.body_editor.read(cx).body(cx);
        request
    }

    pub(super) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let request = self.with_inherited_auth(self.build_request(cx), self.selected.as_deref());
        if request.url.is_empty() {
            self.fail("Enter a URL to send a request.", cx);
            return;
        }
        let resolved = request.resolve(&self.vars);
        if !resolved.missing.is_empty() {
            let names: Vec<_> = resolved.missing.into_iter().collect();
            self.fail(
                format!(
                    "Undefined variable(s): {}. Define them with {} (Variables).",
                    names.join(", "),
                    key_hint("E")
                ),
                cx,
            );
            return;
        }

        // Body files are relative to the request file.
        let request = match self.body_location().0 {
            Some(dir) => resolved.request.with_files_relative_to(&dir),
            None => resolved.request,
        };
        self.response = ResponseState::Loading;
        let pending = net::send(request);
        // The tab may be in the back by the time the response arrives.
        let tab = self.selected.clone();
        // Replacing an older task drops it, which cancels that request.
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = pending.await;
            // Formatting and indexing a big body must not stall the UI thread.
            let prepared = cx
                .background_spawn(async move { result.map(Finished::prepare) })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.finish_for(tab, prepared, window, cx)
            })
            .ok();
        }));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.task.take().is_some() {
            self.response = ResponseState::Cancelled;
            cx.notify();
        }
    }

    fn fail(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.task = None;
        self.response = ResponseState::Failed(message.into());
        cx.notify();
    }

    fn finish(
        &mut self,
        result: Result<Finished, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.task = None;
        self.raw_doc = None;
        self.raw_task = None;
        match result {
            Ok(finished) => {
                self.response = ResponseState::Done(Rc::new(finished));
                self.set_response_tab(0, window, cx);
            }
            Err(message) => self.fail(message, cx),
        }
        cx.notify();
    }

    fn set_response_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.response_tab = ix;
        self.large_view = None;
        let ResponseState::Done(done) = &self.response else {
            cx.notify();
            return;
        };
        let done = done.clone();
        let doc = match ix {
            0 => Some(done.pretty.clone()),
            1 => done.raw.clone().or_else(|| self.raw_doc.clone()),
            _ => {
                cx.notify();
                return;
            }
        };

        match doc {
            Some(doc) if done.large => {
                let palette = self.palette;
                self.large_view = Some(cx.new(|cx| LargeView::new(doc, palette, cx)));
            }
            Some(doc) => {
                let lang = if ix == 0 && done.json { "json" } else { "text" };
                let text = doc.text().to_owned();
                self.response_body.update(cx, |state, cx| {
                    state.set_highlighter(lang, cx);
                    state.set_value(text, window, cx);
                });
            }
            // Raw view of a large body: build it in the background, then show it.
            None if self.raw_task.is_none() => {
                let body = done.response.body.clone();
                let json = done.json;
                self.raw_task = Some(cx.spawn_in(window, async move |this, cx| {
                    let doc = cx
                        .background_spawn(async move {
                            let text = String::from_utf8_lossy(&body).into_owned();
                            Arc::new(Document::new(text, json))
                        })
                        .await;
                    this.update_in(cx, |this, window, cx| {
                        this.raw_doc = Some(doc);
                        this.raw_task = None;
                        if this.response_tab == 1 {
                            this.set_response_tab(1, window, cx);
                        }
                    })
                    .ok();
                }));
            }
            None => {}
        }
        cx.notify();
    }

    fn rows_mut(&mut self, table: Table) -> &mut Vec<KvRow> {
        match table {
            Table::Params => &mut self.params,
            Table::Headers => &mut self.headers,
        }
    }

    fn rows(&self, table: Table) -> &[KvRow] {
        match table {
            Table::Params => &self.params,
            Table::Headers => &self.headers,
        }
    }

    fn active_count(&self, table: Table, cx: &App) -> usize {
        self.rows(table)
            .iter()
            .filter(|r| r.enabled && r.to_key_value(cx).is_some())
            .count()
    }
}

/// `error: cause`, for messages shown to the user.
fn describe(err: &dyn std::error::Error) -> String {
    match err.source() {
        Some(source) => format!("{err}: {source}"),
        None => err.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl Workspace {
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let name = match (&self.project, &self.collection) {
            (Some(project), Some(c)) => format!("{}  ›  {}", project.meta.name, c.meta.name),
            (Some(project), None) => project.meta.name.clone(),
            (None, Some(c)) => c.meta.name.clone(),
            (None, None) => String::new(),
        };
        TitleBar::new()
            .bg(transparent_black())
            .border_color(p.hairline)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(img(theme::app_icon()).size(px(16.)).flex_none())
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.accent)
                            .child("apix"),
                    )
                    .child(div().w(px(6.)))
                    .child(self.render_menu_bar(cx))
                    .child(div().w(px(10.)))
                    .child(div().text_xs().text_color(p.text_muted).child(name)),
            )
    }

    fn render_url_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let method = self.current_method(cx);
        let busy = matches!(self.response, ResponseState::Loading);
        let action = if busy {
            Button::new("cancel")
                .small()
                .icon(IconName::Close)
                .label("Cancel")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx)))
        } else {
            Button::new("send")
                .primary()
                .small()
                .label("Send")
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.send(window, cx)))
        };

        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .w(px(4.))
                    .h(px(20.))
                    .rounded_full()
                    .bg(theme::method_color(method)),
            )
            .child(
                div()
                    .w(px(112.))
                    .flex_none()
                    .child(Select::new(&self.method).small()),
            )
            .child(div().flex_1().min_w_0().child(self.url_input(cx)))
            .child(
                Button::new("copy-as")
                    .ghost()
                    .small()
                    .icon(IconName::Copy)
                    .tooltip("Copy as cURL or PowerShell")
                    .dropdown_menu({
                        let view = cx.entity().downgrade();
                        move |menu, _, _| {
                            shell_copy::copy_items(menu, view.clone(), Workspace::copy_current_as)
                        }
                    }),
            )
            .child(div().w(px(92.)).flex_none().child(action))
    }

    fn render_request_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let params = self.active_count(Table::Params, cx);
        let headers = self.active_count(Table::Headers, cx);
        let label = |name: &str, n: usize| -> SharedString {
            if n == 0 {
                name.to_string().into()
            } else {
                format!("{name} {n}").into()
            }
        };
        let body_label: SharedString = match self.body_editor.read(cx).kind() {
            body_editor::BodyKind::None => "Body".into(),
            kind => format!("Body · {}", kind.label()).into(),
        };

        let auth_label: SharedString = match self.auth_editor.read(cx).auth(cx) {
            Auth::Inherit => "Auth".into(),
            Auth::None => "Auth · none".into(),
            Auth::Bearer { .. } => "Auth · Bearer".into(),
            Auth::Basic { .. } => "Auth · Basic".into(),
            Auth::ApiKey { .. } => "Auth · API key".into(),
            Auth::Digest { .. } => "Auth · Digest".into(),
            Auth::OAuth1(_) => "Auth · OAuth 1.0".into(),
            Auth::OAuth2(_) => "Auth · OAuth 2.0".into(),
            Auth::AwsSigV4(_) => "Auth · AWS".into(),
        };

        let content = match self.request_tab {
            0 => self.render_table(Table::Params, cx).into_any_element(),
            1 => self.render_table(Table::Headers, cx).into_any_element(),
            2 => div()
                .p_2()
                .child(self.auth_editor.clone())
                .into_any_element(),
            _ => self.body_editor.clone().into_any_element(),
        };

        v_flex()
            .size_full()
            .rounded_lg()
            .bg(p.surface)
            .border_1()
            .border_color(p.hairline)
            .overflow_hidden()
            .child(
                TabBar::new("request-tabs")
                    .underline()
                    .selected_index(self.request_tab)
                    .child(Tab::new().label(label("Params", params)))
                    .child(Tab::new().label(label("Headers", headers)))
                    .child(Tab::new().label(auth_label))
                    .child(Tab::new().label(body_label))
                    .on_click(cx.listener(|this, ix: &usize, _, cx| {
                        this.request_tab = *ix;
                        cx.notify();
                    })),
            )
            .child(div().flex_1().min_h_0().p_2().child(content))
    }

    fn render_table(&self, table: Table, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let (on_id, del_id, add_id) = table.ids();
        let rows = self.rows(table).iter().enumerate().map(|(ix, row)| {
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    Checkbox::new((on_id, ix))
                        .checked(row.enabled)
                        .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(row) = this.rows_mut(table).get_mut(ix) {
                                row.enabled = *checked;
                            }
                            cx.notify();
                        })),
                )
                .child(div().flex_1().min_w_0().child(Input::new(&row.key).small()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&row.value).small()),
                )
                .child(
                    Button::new((del_id, ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let rows = this.rows_mut(table);
                            if ix < rows.len() {
                                rows.remove(ix);
                            }
                            cx.notify();
                        })),
                )
        });

        v_flex()
            .id(add_id)
            .gap_1()
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .child(
                h_flex().pt_1().child(
                    Button::new(add_id)
                        .ghost()
                        .small()
                        .icon(IconName::Plus)
                        .label("Add")
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let row = KvRow::new(&KeyValue::new("", ""), window, cx);
                            this.rows_mut(table).push(row);
                            cx.notify();
                        })),
                ),
            )
            .when(self.rows(table).is_empty(), |d| {
                d.child(
                    div()
                        .pt_2()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(match table {
                            Table::Params => "Query parameters are appended to the URL.",
                            Table::Headers => {
                                "Content-Type is set from the body type unless you add one here."
                            }
                        }),
                )
            })
    }

    fn render_response_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let shell = v_flex()
            .size_full()
            .rounded_lg()
            .bg(p.surface)
            .border_1()
            .border_color(p.hairline)
            .overflow_hidden();

        let centered = |child: AnyElement| {
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .p_4()
                .child(child)
        };

        match &self.response {
            ResponseState::Cancelled => shell.child(centered(
                div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child("Request cancelled.")
                    .into_any_element(),
            )),
            ResponseState::Idle => shell.child(centered(
                v_flex()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child("Send a request to see the response")
                    .child(div().text_xs().child(send_hint()))
                    .into_any_element(),
            )),
            ResponseState::Loading => shell.child(centered(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(Spinner::new().small().color(p.accent))
                    .child("Sending…  Esc to cancel")
                    .into_any_element(),
            )),
            ResponseState::Failed(message) => shell.child(centered(
                v_flex()
                    .w_full()
                    .gap_1()
                    .text_sm()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(p.err)
                            .child("Request failed"),
                    )
                    .child(div().text_color(p.text_muted).child(message.clone()))
                    .into_any_element(),
            )),
            ResponseState::Done(done) => {
                let response = &done.response;
                let status = p.status(response.status);
                let headers_label: SharedString =
                    format!("Headers {}", response.headers.len()).into();
                let content = match (&self.large_view, self.response_tab) {
                    (_, 2) => self.render_response_headers(response).into_any_element(),
                    (_, 3) => self.render_network(done).into_any_element(),
                    (Some(view), _) => view.clone().into_any_element(),
                    (None, _) if done.large => centered(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child(Spinner::new().small().color(p.accent))
                            .child("Preparing the raw view…")
                            .into_any_element(),
                    )
                    .into_any_element(),
                    (None, _) => Editor::new(&self.response_body).h_full().into_any_element(),
                };
                let viewer_note = done
                    .large
                    .then(|| format!("{} lines", group_digits(done.pretty.lines().into())));
                shell
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_full()
                                    .bg(status.opacity(0.15))
                                    .text_color(status)
                                    .font_weight(FontWeight::BOLD)
                                    .child(format!(
                                        "{} {}",
                                        response.status,
                                        response.reason.unwrap_or("")
                                    )),
                            )
                            .child(human_duration(response.timing.total))
                            .child(format!("TTFB {}", human_duration(response.timing.ttfb)))
                            .child(human_size(response.body.len()))
                            .child(response.version.clone())
                            .children(viewer_note),
                    )
                    .child(
                        TabBar::new("response-tabs")
                            .underline()
                            .selected_index(self.response_tab)
                            .child(Tab::new().label("Pretty"))
                            .child(Tab::new().label("Raw"))
                            .child(Tab::new().label(headers_label))
                            .child(Tab::new().label("Network"))
                            .on_click(cx.listener(|this, ix: &usize, window, cx| {
                                this.set_response_tab(*ix, window, cx)
                            })),
                    )
                    .child(div().flex_1().min_h_0().p_2().child(content))
            }
        }
    }

    fn render_response_headers(&self, response: &Response) -> impl IntoElement {
        let p = self.palette;
        v_flex()
            .id("response-headers")
            .size_full()
            .overflow_y_scroll()
            .gap_1()
            .text_xs()
            .children(response.headers.iter().map(|(key, value)| {
                h_flex()
                    .gap_3()
                    .child(
                        div()
                            .w(px(200.))
                            .flex_none()
                            .text_color(p.text_muted)
                            .child(key.clone()),
                    )
                    .child(div().flex_1().min_w_0().child(value.clone()))
            }))
    }

    fn render_status_bar(&self, cx: &App) -> impl IntoElement {
        let p = self.palette;
        let location = match &self.collection {
            Some(c) => {
                let file = self
                    .selected
                    .as_ref()
                    .and_then(|path| path.strip_prefix(&c.root).ok())
                    .map(|rel| format!("  ·  {}", rel.display()))
                    .unwrap_or_default();
                let scope = match &self.project {
                    Some(project) => format!(
                        "{}  ·  {} collections",
                        project.meta.name,
                        self.collections.len()
                    ),
                    None => c.meta.name.clone(),
                };
                format!("{scope}  ·  {} requests{file}", self.requests.len())
            }
            None => match &self.project {
                Some(project) => format!("{}  ·  no collections yet", project.meta.name),
                None => "No collection".to_string(),
            },
        };
        let environment = self.environment.as_ref().map(|name| {
            div()
                .text_color(if self.is_prod() { p.err } else { p.ok })
                .child(format!("env: {name}"))
        });
        let unsaved = self.is_dirty(cx).then(|| {
            div()
                .text_color(p.warn)
                .child(format!("Unsaved changes · {} save", key_hint("S")))
        });
        h_flex()
            .justify_between()
            .gap_3()
            .px_3()
            .h(px(26.))
            .flex_none()
            .border_t_1()
            .border_color(p.hairline)
            .text_xs()
            .text_color(p.text_muted)
            .child(div().flex_1().min_w_0().truncate().child(location))
            .children(environment)
            .children(unsaved)
            .child(format!(
                "{} commands  ·  {} send  ·  Esc cancel",
                key_hint("K"),
                key_hint("Enter")
            ))
    }
}

/// `Ctrl+S`, or `⌘S` on macOS.
fn key_hint(key: &str) -> String {
    if cfg!(target_os = "macos") {
        let key = if key == "Enter" { "↩" } else { key };
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

fn send_hint() -> String {
    format!("{} send", key_hint("Enter"))
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        // Shortcuts dispatch along the focus path. When a dialog closes, focus
        // is left nowhere, so take it back or Ctrl+S / Ctrl+N would stop working.
        if window.focused(cx).is_none() && !window.has_active_dialog(cx) {
            self.focus.focus(window, cx);
        }
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| this.send(window, cx)))
            .on_action(cx.listener(|this, _: &CancelRequest, _, cx| this.cancel(cx)))
            .on_action(cx.listener(|this, _: &SaveRequest, window, cx| {
                this.save(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewRequest, window, cx| {
                this.new_request(None, Method::Get, window, cx)
            }))
            .on_action(cx.listener(|this, _: &MethodGet, w, cx| this.pick_method(0, w, cx)))
            .on_action(cx.listener(|this, _: &MethodPost, w, cx| this.pick_method(1, w, cx)))
            .on_action(cx.listener(|this, _: &MethodPut, w, cx| this.pick_method(2, w, cx)))
            .on_action(cx.listener(|this, _: &MethodPatch, w, cx| this.pick_method(3, w, cx)))
            .on_action(cx.listener(|this, _: &MethodDelete, w, cx| this.pick_method(4, w, cx)))
            .on_action(cx.listener(|this, _: &MethodHead, w, cx| this.pick_method(5, w, cx)))
            .on_action(cx.listener(|this, _: &MethodOptions, w, cx| this.pick_method(6, w, cx)))
            .on_action(cx.listener(|this, _: &OpenCollection, window, cx| {
                this.prompt_open_collection(window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ImportFile, window, cx| this.prompt_import(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &EditVariables, window, cx| this.open_env_editor(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &CloseTab, window, cx| this.close_front_tab(window, cx)),
            )
            .on_action(cx.listener(|this, _: &NextTab, window, cx| this.cycle_tab(1, window, cx)))
            .on_action(cx.listener(|this, _: &PrevTab, window, cx| this.cycle_tab(-1, window, cx)))
            .on_action(
                cx.listener(|this, _: &OpenPalette, window, cx| this.open_palette(window, cx)),
            )
            // A click anywhere brings focus back inside the workspace (inputs
            // focus themselves first, so they keep it), so shortcuts keep working.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.touch();
                    if !this.focus.contains_focused(window, cx) {
                        this.focus.focus(window, cx);
                    }
                }),
            )
            .capture_key_down(cx.listener(|this, _: &KeyDownEvent, _, _| this.touch()))
            // Drop a Postman export to import it, or a folder to open it.
            .id("workspace")
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(p.accent.opacity(0.06)))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.handle_dropped(paths.paths().to_vec(), window, cx)
            }))
            .size_full()
            .bg(p.background())
            .text_color(p.text)
            .child(self.render_title_bar(cx))
            // Hard to miss: requests in a production environment touch real data.
            .when(self.is_prod(), |d| {
                d.child(div().h(px(2.)).flex_none().bg(p.err))
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.render_sidebar(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .p_3()
                            .gap_3()
                            .children(self.render_tabs(cx))
                            .child(self.render_url_bar(cx))
                            .child(
                                div().flex_1().min_h_0().child(
                                    h_resizable("request-response")
                                        .child(
                                            resizable_panel()
                                                .size(px(520.))
                                                .size_range(px(300.)..px(1400.))
                                                .child(
                                                    div()
                                                        .size_full()
                                                        .pr_1()
                                                        .child(self.render_request_pane(cx)),
                                                ),
                                        )
                                        .child(
                                            resizable_panel().child(
                                                div()
                                                    .size_full()
                                                    .pl_1()
                                                    .child(self.render_response_pane(cx)),
                                            ),
                                        ),
                                ),
                            ),
                    ),
            )
            .child(self.render_status_bar(cx))
            .children(self.render_var_card(cx))
    }
}
