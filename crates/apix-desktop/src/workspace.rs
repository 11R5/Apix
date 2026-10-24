//! The main window: collection sidebar, request editor and response viewer.

mod auth_editor;
mod body_editor;
mod collections;
mod cookies_ui;
mod drag;
mod env_editor;
mod extract_ui;
mod history_ui;
mod menu_bar;
mod network;
mod palette;
mod runner_ui;
mod script_editor;
mod script_run;
mod settings_editor;
mod settings_ui;
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
use apix_http::{BodyKind, ImageKind, Response, group_digits, human_duration, human_size};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::AlertDialog;
use gpui_kit::component::input::{Editor, EditorState, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::Tab;
use gpui_kit::component::{
    IconName, IndexPath, Sizable, TitleBar, WindowExt as _, h_flex, h_resizable, resizable_panel,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::bidi::{self, Bidi as _};
use crate::large_view::{Document, LARGE_BODY_BYTES, LargeView};
use crate::net;
use crate::settings::Settings;
use crate::state::AppState;
use crate::theme;
use crate::tr;

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
        OpenSettings,
        OpenHistory,
        OpenRunner,
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
        KeyBinding::new("secondary-,", OpenSettings, Some(CONTEXT)),
        KeyBinding::new("secondary-h", OpenHistory, Some(CONTEXT)),
        KeyBinding::new("secondary-shift-r", OpenRunner, Some(CONTEXT)),
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

/// One editable row of the Params or Headers table (or a form body).
/// `{{variables}}` in either field show as chips.
struct KvRow {
    key: Entity<InputState>,
    value: Entity<InputState>,
    enabled: bool,
    _chips: [Subscription; 2],
}

impl KvRow {
    fn new(kv: &KeyValue, window: &mut Window, cx: &mut App) -> Self {
        let (key, key_chips) = var_chips::chipped_input(&kv.key, tr!("table.key"), window, cx);
        let (value, value_chips) =
            var_chips::chipped_input(&kv.value, tr!("table.value"), window, cx);
        Self {
            key,
            value,
            enabled: kv.enabled,
            _chips: [key_chips, value_chips],
        }
    }

    fn retranslate(&self, window: &mut Window, cx: &mut App) {
        self.key.update(cx, |state, cx| {
            state.set_placeholder(tr!("table.key"), window, cx)
        });
        self.value.update(cx, |state, cx| {
            state.set_placeholder(tr!("table.value"), window, cx)
        });
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
    kind: BodyKind,
    /// An image body, decoded by gpui when it is first drawn.
    image: Option<Arc<gpui_kit::Image>>,
    /// This machine's side of the connection, read once per response.
    local: apix_http::LocalNetwork,
    /// Test results and console lines, when the request has scripts.
    scripts: Option<script_run::ScriptReport>,
}

/// Images bigger than this are not previewed (save them instead).
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;
/// Bytes shown in the Hex tab.
const MAX_HEX_BYTES: usize = 4 * 1024 * 1024;

impl Finished {
    /// Linear in the body size. Runs on a background thread.
    fn prepare(response: Response) -> Self {
        let kind = response.body_kind();
        let json = kind == BodyKind::Json;
        let large = response.body.len() >= LARGE_BODY_BYTES;
        let formatted = match kind {
            BodyKind::Json => apix_core::text::pretty_json(&response.body),
            BodyKind::Xml => apix_core::text::pretty_xml(&response.body),
            BodyKind::Html => apix_core::text::pretty_html(&response.body),
            _ => None,
        };
        let image = match kind {
            BodyKind::Image(image) if response.body.len() <= MAX_PREVIEW_BYTES => {
                let format = match image {
                    ImageKind::Png => gpui_kit::ImageFormat::Png,
                    ImageKind::Jpeg => gpui_kit::ImageFormat::Jpeg,
                    ImageKind::Gif => gpui_kit::ImageFormat::Gif,
                    ImageKind::Webp => gpui_kit::ImageFormat::Webp,
                    ImageKind::Bmp => gpui_kit::ImageFormat::Bmp,
                    ImageKind::Svg => gpui_kit::ImageFormat::Svg,
                };
                Some(Arc::new(gpui_kit::Image::from_bytes(
                    format,
                    response.body.to_vec(),
                )))
            }
            _ => None,
        };
        let (pretty, raw) = match formatted {
            Some(text) => {
                let raw = (!large).then(|| raw_document(&response, json));
                (Arc::new(Document::new(text, json)), raw)
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
            kind,
            image,
            local,
            scripts: None,
        }
    }
}

/// A file name for saving `response`: the server's `Content-Disposition`
/// name, else the URL's last path segment, with an extension that fits the body.
fn suggested_file_name(response: &Response, url: &str) -> String {
    let from_header = response.header("content-disposition").and_then(|v| {
        let at = v.to_ascii_lowercase().find("filename=")?;
        let name = v[at + "filename=".len()..].split(';').next()?;
        Some(name.trim().trim_matches('"').to_owned())
    });
    let extension = match response.body_kind() {
        BodyKind::Json => "json",
        BodyKind::Xml => "xml",
        BodyKind::Html => "html",
        BodyKind::Text => "txt",
        BodyKind::Image(ImageKind::Png) => "png",
        BodyKind::Image(ImageKind::Jpeg) => "jpg",
        BodyKind::Image(ImageKind::Gif) => "gif",
        BodyKind::Image(ImageKind::Webp) => "webp",
        BodyKind::Image(ImageKind::Bmp) => "bmp",
        BodyKind::Image(ImageKind::Svg) => "svg",
        BodyKind::Binary => "bin",
    };
    let from_url = || {
        let path = url.split(['?', '#']).next().unwrap_or_default();
        let last = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let last = if last.contains("://") || last.contains("{{") {
            ""
        } else {
            last
        };
        match last {
            "" => format!("response.{extension}"),
            name if name.contains('.') => name.to_owned(),
            name => format!("{name}.{extension}"),
        }
    };
    let name = from_header
        .filter(|n| !n.is_empty())
        .unwrap_or_else(from_url);
    name.chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

/// "Variable … is not defined", pointing at the Variables dialog.
fn undefined_message(missing: std::collections::BTreeSet<String>) -> SharedString {
    let names: Vec<_> = missing.into_iter().collect();
    let key = key_hint("E");
    if names.len() == 1 {
        tr!(
            "request.error.undefined_variable",
            name = names[0],
            key = key
        )
    } else {
        tr!(
            "request.error.undefined_variables",
            names = names.join(", "),
            key = key
        )
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
    settings: Settings,
    /// The request as last loaded or saved; edits are compared to it.
    base: Request,
    /// The Auth tab.
    auth_editor: Entity<auth_editor::AuthEditor>,
    /// The Body tab.
    body_editor: Entity<body_editor::BodyEditor>,
    /// The Settings tab (timeout, redirects, TLS, proxy, certificates, cookies).
    settings_editor: Entity<settings_editor::SettingsEditor>,
    /// The Scripts tab (pre-request and test scripts).
    script_editor: Entity<script_editor::ScriptEditor>,
    /// The request's scripts as last loaded or saved.
    base_scripts: script_editor::Scripts,
    /// What scripts set or unset since the app started; never written to disk.
    script_vars: apix_core::scripts::ScriptVars,
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
    /// The Hex tab's dump, with the response it was made for.
    hex_doc: Option<(usize, Arc<Document>)>,
    hex_task: Option<Task<()>>,
    /// When the user last clicked or typed, for auto-lock.
    last_input: std::cell::Cell<std::time::Instant>,
    /// When the `APIX_BENCH=json` request was sent.
    bench_sent: Option<std::time::Instant>,
    /// Locks idle encrypted collections.
    _auto_lock: Option<Task<()>>,
    /// The in-flight request. Dropping it cancels the request.
    task: Option<Task<()>>,
    /// The open tabs as last written to the app state.
    remembered_tabs: Option<(PathBuf, crate::state::OpenTabs)>,
    /// The sidebar row and zone a drag would drop into.
    drop_hint: Option<(PathBuf, drag::Zone)>,
    /// The Runner, kept so a run goes on with its dialog closed.
    runner: Option<Entity<runner_ui::RunnerView>>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(dir: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let methods: Vec<&'static str> = Method::ALL.iter().map(|m| m.as_str()).collect();
        let method = cx.new(|cx| SelectState::new(methods, Some(IndexPath::new(0)), window, cx));
        let url =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("request.url_placeholder")));
        let workspace = cx.weak_entity();
        let body_editor = cx.new(|cx| body_editor::BodyEditor::new(workspace, window, cx));
        let env_select = cx
            .new(|cx| SelectState::new(vec![tr!("env.none")], Some(IndexPath::new(0)), window, cx));
        let response_body = cx.new(|cx| {
            let mut state = EditorState::new(window, cx).language("json");
            state.set_readonly(true, cx);
            state
        });

        let weak = cx.weak_entity();
        let auth_editor = cx.new(|cx| auth_editor::AuthEditor::new("request", weak, window, cx));
        let settings_editor = cx.new(|cx| settings_editor::SettingsEditor::new(window, cx));
        let script_editor = cx.new(|cx| script_editor::ScriptEditor::new(window, cx));
        let subscriptions = vec![
            cx.observe_self(|this, _| this.remember_tabs()),
            // "Follow the system": repaint when the OS switches between light and dark.
            cx.observe_window_appearance(window, |this, _, cx| {
                if theme::Appearance::from_id(&this.settings.appearance)
                    == theme::Appearance::System
                {
                    theme::set(theme::from_settings(&this.settings, cx), cx);
                }
            }),
            // Typing in the Auth tab changes the unsaved marker.
            cx.observe(&auth_editor, |_, _, cx| cx.notify()),
            cx.observe(&settings_editor, |_, _, cx| cx.notify()),
            cx.observe(&script_editor, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &script_editor,
                window,
                |this, _, _: &body_editor::SendRequested, window, cx| this.send(window, cx),
            ),
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
                    // The first entry is "No environment", in any language.
                    let state = state.read(cx);
                    let name = state
                        .selected_index(cx)
                        .filter(|ix| ix.row > 0)
                        .and_then(|_| state.selected_value())
                        .map(|v| v.to_string());
                    this.set_environment(name, window, cx);
                },
            ),
        ];

        let mut this = Self {
            focus: cx.focus_handle(),
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
            settings: Settings::load(),
            base: Request::default(),
            auth_editor,
            body_editor,
            settings_editor,
            script_editor,
            base_scripts: Default::default(),
            script_vars: Default::default(),
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
            hex_doc: None,
            hex_task: None,
            task: None,
            last_input: std::cell::Cell::new(std::time::Instant::now()),
            bench_sent: None,
            _auto_lock: None,
            remembered_tabs: None,
            drop_hint: None,
            runner: None,
            _subscriptions: subscriptions,
        };
        this._auto_lock = Some(Self::auto_lock_task(window, cx));
        match crate::bench::mode() {
            Some(crate::bench::Bench::Startup) => window.on_next_frame(|_, cx| {
                crate::bench::report("startup_first_frame_ms", crate::bench::since_start_ms());
                cx.quit();
            }),
            Some(crate::bench::Bench::Json(target)) => {
                cx.defer_in(window, move |this, window, cx| {
                    this.url
                        .update(cx, |state, cx| state.set_value(target, window, cx));
                    this.bench_sent = Some(std::time::Instant::now());
                    this.send(window, cx);
                })
            }
            None => {}
        }
        // After the window exists: an encrypted folder opens with a dialog.
        if let Some(dir) = dir {
            cx.defer_in(window, move |this, window, cx| {
                this.open_or_create(dir, window, cx)
            });
        }
        this.url.update(cx, |state, cx| state.focus(window, cx));
        this
    }

    /// After a language change: texts kept inside widgets (placeholders,
    /// picker entries) are set again; everything else follows on the next frame.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.url.update(cx, |state, cx| {
            state.set_placeholder(tr!("request.url_placeholder"), window, cx)
        });
        for row in self.params.iter().chain(&self.headers) {
            row.retranslate(window, cx);
        }
        self.auth_editor
            .update(cx, |editor, cx| editor.retranslate(window, cx));
        self.body_editor
            .update(cx, |editor, cx| editor.retranslate(window, cx));
        self.settings_editor
            .update(cx, |editor, cx| editor.retranslate(window, cx));
        self.refresh_environments(window, cx);
        cx.notify();
    }

    /// Opens `request` in the editor as the saved state (no unsaved changes).
    fn show_request(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        self.fill_editor(&request, window, cx);
        self.base = request;
        let scripts = self
            .selected
            .as_deref()
            .map(script_editor::Scripts::read)
            .unwrap_or_default();
        self.show_scripts(&scripts, window, cx);
        self.base_scripts = scripts;
        self.task = None;
        self.raw_task = None;
        self.hex_doc = None;
        self.hex_task = None;
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
        let settings = request.settings.clone();
        self.settings_editor
            .update(cx, |editor, cx| editor.set_settings(&settings, window, cx));
        self.refresh_inherited_settings(cx);
        let asserts = request.asserts.clone();
        self.script_editor
            .update(cx, |editor, cx| editor.set_asserts(&asserts, window, cx));
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
        request.settings = self.settings_editor.read(cx).settings(cx);
        request.asserts = self.script_editor.read(cx).asserts(cx);
        request
    }

    /// Sends the request, after asking first when it would change something
    /// in a production environment (DELETE, PUT, PATCH).
    pub(super) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let method = self.build_request(cx).method;
        if self.settings.confirm_prod
            && self.is_prod()
            && matches!(method, Method::Delete | Method::Put | Method::Patch)
        {
            self.confirm_prod_send(method, window, cx);
        } else {
            self.send_now(window, cx);
        }
    }

    fn confirm_prod_send(&mut self, method: Method, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let environment = self.environment.clone().unwrap_or_default();
        let url = self.build_request(cx).resolve(&self.vars).request.url;
        window.open_alert_dialog(cx, move |alert: AlertDialog, _, _| {
            let view = view.clone();
            alert
                .on_close(collections::refocus(view.clone()))
                .confirm()
                .cancel_text(tr!("dialog.cancel"))
                .text_start()
                .title(tr!("prod.confirm_title", method = method.as_str()))
                .description(bidi::para(tr!(
                    "prod.confirm_detail",
                    method = method.as_str(),
                    url = url,
                    environment = environment
                )))
                .ok_text(tr!("prod.confirm_send", method = method.as_str()))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| this.send_now(window, cx)).ok();
                    true
                })
        });
    }

    fn send_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let draft = self.build_request(cx);
        let record = self.pending_record(&draft);
        let request = self.with_inherited_auth(draft, self.selected.as_deref());
        let request = self.with_inherited_settings(request, self.selected.as_deref());
        if request.url.is_empty() {
            self.fail(tr!("request.error.no_url"), cx);
            return;
        }
        let (pre_scripts, test_scripts) = self.script_chains(cx);
        if !pre_scripts.is_empty() || !test_scripts.is_empty() {
            self.send_scripted(request, record, pre_scripts, test_scripts, window, cx);
            return;
        }
        let resolved = request.resolve(&self.vars);
        if !resolved.missing.is_empty() {
            self.fail(undefined_message(resolved.missing), cx);
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
            // Formatting and indexing a big body must not stall the UI thread,
            // and neither must writing the history.
            let prepared = cx
                .background_spawn(async move {
                    if let Some(record) = record {
                        record.save(&result);
                    }
                    result.map(Finished::prepare)
                })
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
        if let Some(sent) = self.bench_sent.take() {
            crate::bench::report("json_failed_ms", sent.elapsed().as_secs_f64() * 1000.0);
            cx.quit();
        }
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
        self.hex_doc = None;
        self.hex_task = None;
        match result {
            Ok(finished) => {
                self.response = ResponseState::Done(Rc::new(finished));
                self.set_response_tab(0, window, cx);
            }
            Err(message) => self.fail(message, cx),
        }
        // `APIX_BENCH=json:<url>`: time until this response is on screen.
        if let Some(sent) = self.bench_sent.take() {
            let ok = matches!(self.response, ResponseState::Done(_));
            window.on_next_frame(move |_, cx| {
                let name = if ok {
                    "json_on_screen_ms"
                } else {
                    "json_failed_ms"
                };
                crate::bench::report(name, sent.elapsed().as_secs_f64() * 1000.0);
                cx.quit();
            });
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
        let binary = matches!(done.kind, BodyKind::Binary | BodyKind::Image(_));
        let doc = match ix {
            // Pretty for an image or binary body is drawn by `render_binary`.
            0 if binary => {
                cx.notify();
                return;
            }
            0 => Some(done.pretty.clone()),
            1 => done.raw.clone().or_else(|| self.raw_doc.clone()),
            4 => return self.show_hex(&done, window, cx),
            _ => {
                cx.notify();
                return;
            }
        };

        match doc {
            Some(doc) if done.large => {
                self.large_view = Some(cx.new(|cx| LargeView::new(doc, cx)));
            }
            Some(doc) => {
                let lang = match done.kind {
                    BodyKind::Json if ix == 0 => "json",
                    BodyKind::Xml | BodyKind::Html => "html",
                    _ => "text",
                };
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

    /// The Hex tab: a dump of the first `MAX_HEX_BYTES`, built in the
    /// background once per response and shown in the virtualized viewer.
    fn show_hex(&mut self, done: &Rc<Finished>, window: &mut Window, cx: &mut Context<Self>) {
        let key = Rc::as_ptr(done) as usize;
        if let Some((for_response, doc)) = &self.hex_doc
            && *for_response == key
        {
            let doc = doc.clone();
            self.large_view = Some(cx.new(|cx| LargeView::new(doc, cx)));
            cx.notify();
            return;
        }
        let body = done.response.body.clone();
        self.hex_task = Some(cx.spawn_in(window, async move |this, cx| {
            let doc = cx
                .background_spawn(async move {
                    Arc::new(Document::new(
                        apix_core::text::hex_dump(&body, MAX_HEX_BYTES),
                        false,
                    ))
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.hex_doc = Some((key, doc));
                this.hex_task = None;
                if this.response_tab == 4 {
                    this.set_response_tab(4, window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Writes the response body, byte for byte, to a file the user picks.
    fn save_response_body(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ResponseState::Done(done) = &self.response else {
            return;
        };
        let body = done.response.body.clone();
        let name = suggested_file_name(&done.response, &self.build_request(cx).url);
        let dir = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let picked = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let result = std::fs::write(&path, &body);
            this.update_in(cx, |this, window, cx| match result {
                Ok(()) => window.push_notification(
                    gpui_kit::component::notification::Notification::success(tr!(
                        "response.saved",
                        size = human_size(body.len()),
                        path = path.display()
                    )),
                    cx,
                ),
                Err(e) => this.notify_error(format!("{}: {e}", path.display()), window, cx),
            })
            .ok();
        })
        .detach();
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
        Some(source) => tr!("app.error_with_cause", error = err, cause = source).into(),
        None => err.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl Workspace {
    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let name = match (&self.project, &self.collection) {
            (Some(project), Some(c)) => format!("{}  ›  {}", project.meta.name, c.meta.name),
            (Some(project), None) => project.meta.name.clone(),
            (None, Some(c)) => c.meta.name.clone(),
            (None, None) => String::new(),
        };
        // In RTL the window controls move to the left, as in native RTL apps.
        // macOS draws its own controls on the left, so there only the content mirrors.
        let rtl = bidi::rtl();
        TitleBar::new()
            .bg(transparent_black())
            .border_color(p.hairline)
            .when(rtl && !cfg!(target_os = "macos"), |bar| {
                bar.flex_row_reverse().pl_0().pr_3()
            })
            .child(
                h_flex()
                    .when(rtl, |row| row.flex_1())
                    .dir_row()
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
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(crate::i18n::display(&name)),
                    ),
            )
    }

    fn render_url_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let method = self.current_method(cx);
        let busy = matches!(self.response, ResponseState::Loading);
        let action = if busy {
            Button::new("cancel")
                .small()
                .icon(IconName::Close)
                .label(tr!("menu.cancel"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx)))
        } else {
            Button::new("send")
                .primary()
                .small()
                .label(tr!("menu.send"))
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
                    .tooltip(tr!("request.copy_as_tooltip"))
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
        let p = crate::theme::current();
        let params = self.active_count(Table::Params, cx);
        let headers = self.active_count(Table::Headers, cx);
        let params_label = match params {
            0 => tr!("request.tab.params"),
            n => tr!("request.tab.params_count", count = n),
        };
        let headers_label = match headers {
            0 => tr!("request.tab.headers"),
            n => tr!("request.tab.headers_count", count = n),
        };
        let body_label = match self.body_editor.read(cx).kind() {
            body_editor::BodyKind::None => tr!("request.tab.body"),
            kind => tr!("request.tab.body_kind", kind = kind.label()),
        };

        // Technical names stay as they are; only the frame is translated.
        let auth_kind = |kind: &str| tr!("request.tab.auth_kind", kind = kind);
        let auth_label = match self.auth_editor.read(cx).auth(cx) {
            Auth::Inherit => tr!("request.tab.auth"),
            Auth::None => tr!("request.tab.auth_none"),
            Auth::Bearer { .. } => auth_kind("Bearer"),
            Auth::Basic { .. } => auth_kind("Basic"),
            Auth::ApiKey { .. } => tr!("request.tab.auth_api_key"),
            Auth::Digest { .. } => auth_kind("Digest"),
            Auth::OAuth1(_) => auth_kind("OAuth 1.0"),
            Auth::OAuth2(_) => auth_kind("OAuth 2.0"),
            Auth::AwsSigV4(_) => auth_kind("AWS"),
        };

        // The Settings tab is labelled with a dot when the request sets anything.
        let settings_label = if self.settings_editor.read(cx).settings(cx).is_inherit() {
            tr!("request.tab.settings")
        } else {
            tr!("request.tab.settings_set")
        };

        let editor = self.script_editor.read(cx);
        let scripts_label = if editor.scripts(cx).is_empty() && editor.asserts(cx).is_empty() {
            tr!("request.tab.scripts")
        } else {
            tr!("request.tab.scripts_set")
        };

        let content = match self.request_tab {
            0 => self.render_table(Table::Params, cx).into_any_element(),
            1 => self.render_table(Table::Headers, cx).into_any_element(),
            2 => div()
                .p_2()
                .child(self.auth_editor.clone())
                .into_any_element(),
            4 => self.script_editor.clone().into_any_element(),
            5 => div()
                .id("request-settings")
                .p_2()
                .size_full()
                .overflow_y_scroll()
                .child(self.settings_editor.clone())
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
            .child(bidi::tab_bar(
                "request-tabs",
                self.request_tab,
                vec![
                    Tab::new().label(params_label),
                    Tab::new().label(headers_label),
                    Tab::new().label(auth_label),
                    Tab::new().label(body_label),
                    Tab::new().label(scripts_label),
                    Tab::new().label(settings_label),
                ],
                cx.listener(|this, ix: &usize, _, cx| {
                    this.request_tab = *ix;
                    cx.notify();
                }),
            ))
            .child(div().flex_1().min_h_0().p_2().child(content))
    }

    fn render_table(&self, table: Table, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let (on_id, del_id, add_id) = table.ids();
        let view = cx.entity().downgrade();
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
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(self.chip_input(&row.key, view.clone(), cx)),
                )
                .child(div().flex_1().min_w_0().child(self.chip_input(
                    &row.value,
                    view.clone(),
                    cx,
                )))
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
                        .label(tr!("table.add"))
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
                        .child(bidi::para(match table {
                            Table::Params => tr!("request.params_hint"),
                            Table::Headers => tr!("request.headers_hint"),
                        })),
                )
            })
    }

    fn render_response_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
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
                    .child(tr!("response.cancelled"))
                    .into_any_element(),
            )),
            ResponseState::Idle => shell.child(centered(
                v_flex()
                    .items_center()
                    .gap_1()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("response.empty")))
                    .child(div().text_xs().child(bidi::para(send_hint())))
                    .into_any_element(),
            )),
            ResponseState::Loading => shell.child(centered(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(Spinner::new().small().color(p.accent))
                    .child(tr!("response.sending"))
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
                            .child(tr!("response.failed")),
                    )
                    .child(
                        div()
                            .text_color(p.text_muted)
                            .child(bidi::para(message.clone())),
                    )
                    .into_any_element(),
            )),
            ResponseState::Done(done) => {
                let response = &done.response;
                let status = p.status(response.status);
                let headers_label =
                    tr!("request.tab.headers_count", count = response.headers.len());
                let binary = matches!(done.kind, BodyKind::Binary | BodyKind::Image(_));
                let content = match (&self.large_view, self.response_tab) {
                    (_, 5) => self.render_tests(done).into_any_element(),
                    (_, 6) => self.render_console(done, cx).into_any_element(),
                    (_, 2) => self.render_response_headers(response).into_any_element(),
                    (_, 3) => self.render_network(done).into_any_element(),
                    (None, 0) if binary => self.render_binary(done, cx).into_any_element(),
                    (Some(view), _) => view.clone().into_any_element(),
                    (None, 4) => centered(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child(Spinner::new().small().color(p.accent))
                            .child(tr!("response.preparing_hex"))
                            .into_any_element(),
                    )
                    .into_any_element(),
                    (None, _) if done.large => centered(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child(Spinner::new().small().color(p.accent))
                            .child(tr!("response.preparing_raw"))
                            .into_any_element(),
                    )
                    .into_any_element(),
                    (None, _) => Editor::new(&self.response_body).h_full().into_any_element(),
                };
                let viewer_note = (done.large && !binary).then(|| {
                    tr!(
                        "response.lines",
                        count = group_digits(done.pretty.lines().into())
                    )
                });
                shell
                    .child(
                        h_flex()
                            .dir_row()
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
                            .children(viewer_note)
                            .child(div().flex_1())
                            .child(
                                Button::new("extract-value")
                                    .ghost()
                                    .xsmall()
                                    .label("{x}")
                                    .tooltip(tr!("extract.title"))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.prompt_extract(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("save-body")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowDown)
                                    .tooltip(tr!("response.save"))
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.save_response_body(window, cx)
                                    })),
                            ),
                    )
                    .child(bidi::tab_bar(
                        "response-tabs",
                        self.response_tab,
                        [
                            Tab::new().label(tr!("response.tab.pretty")),
                            Tab::new().label(tr!("response.tab.raw")),
                            Tab::new().label(headers_label),
                            Tab::new().label(tr!("response.tab.network")),
                            Tab::new().label("Hex"),
                        ]
                        .into_iter()
                        .chain(
                            done.scripts
                                .as_ref()
                                .map(script_run::tests_tab)
                                .into_iter()
                                .flatten(),
                        )
                        .collect(),
                        cx.listener(|this, ix: &usize, window, cx| {
                            this.set_response_tab(*ix, window, cx)
                        }),
                    ))
                    .child(div().flex_1().min_h_0().p_2().child(content))
            }
        }
    }

    /// Pretty for an image or other binary body: what it is, the image when
    /// there is one, and how to see or keep the bytes.
    fn render_binary(&self, done: &Finished, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let response = &done.response;
        let content_type = response
            .header("content-type")
            .unwrap_or("application/octet-stream")
            .to_owned();
        let title = match (&done.image, done.kind) {
            (Some(_), _) => tr!("response.image_title"),
            (None, BodyKind::Image(_)) => tr!("response.image_too_big"),
            _ => tr!("response.binary_title"),
        };
        v_flex()
            .id("binary-body")
            .size_full()
            .gap_3()
            .p_2()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .child(div().font_weight(FontWeight::BOLD).child(title))
                    .child(div().text_color(p.text_muted).child(format!(
                        "{content_type} · {}",
                        human_size(response.body.len())
                    ))),
            )
            .children(done.image.clone().map(|image| {
                div()
                    .flex()
                    .justify_center()
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(p.hairline)
                    .child(
                        gpui_kit::img(image)
                            .max_w_full()
                            .max_h(px(480.))
                            .object_fit(gpui_kit::ObjectFit::Contain),
                    )
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("response.binary_hint"))),
            )
            .child(
                h_flex().dir_row().child(
                    Button::new("save-binary")
                        .small()
                        .label(tr!("response.save"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.save_response_body(window, cx)
                        })),
                ),
            )
    }

    fn render_response_headers(&self, response: &Response) -> impl IntoElement {
        let p = crate::theme::current();
        // Header names and values are data: left to right in any language.
        v_flex()
            .id("response-headers")
            .ltr()
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

    /// The request pane beside the response pane, request on the reading side.
    fn render_split(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let request = resizable_panel()
            .size(px(520.))
            .size_range(px(300.)..px(1400.))
            .child(
                div()
                    .size_full()
                    .pe(px(4.))
                    .child(self.render_request_pane(cx)),
            );
        let response = resizable_panel().child(
            div()
                .size_full()
                .ps(px(4.))
                .child(self.render_response_pane(cx)),
        );
        // The group remembers panel sizes by position, so each direction keeps its own.
        if bidi::rtl() {
            h_resizable("response-request")
                .child(response)
                .child(request)
        } else {
            h_resizable("request-response")
                .child(request)
                .child(response)
        }
    }

    fn render_status_bar(&self, cx: &App) -> impl IntoElement {
        let p = crate::theme::current();
        let location = match &self.collection {
            Some(c) => {
                // Separate facts joined by dots, so each is translated whole.
                let mut parts: Vec<String> = Vec::new();
                match &self.project {
                    Some(project) => {
                        parts.push(project.meta.name.clone());
                        parts.push(match self.collections.len() {
                            1 => tr!("app.status.one_collection").into(),
                            n => tr!("app.status.collections", count = n).into(),
                        });
                    }
                    None => parts.push(c.meta.name.clone()),
                }
                parts.push(match self.requests.len() {
                    1 => tr!("app.status.one_request").into(),
                    n => tr!("app.status.requests", count = n).into(),
                });
                if let Some(rel) = self
                    .selected
                    .as_ref()
                    .and_then(|path| path.strip_prefix(&c.root).ok())
                {
                    parts.push(rel.display().to_string());
                }
                parts.join("  ·  ")
            }
            None => match &self.project {
                Some(project) => format!(
                    "{}  ·  {}",
                    project.meta.name,
                    tr!("app.status.no_collections")
                ),
                None => tr!("app.status.no_collection").into(),
            },
        };
        let environment = self.environment.as_ref().map(|name| {
            div()
                .text_color(if self.is_prod() { p.err } else { p.ok })
                .child(tr!("app.status.env", name = name))
        });
        let unsaved = self.is_dirty(cx).then(|| {
            div()
                .text_color(p.warn)
                .child(tr!("app.status.unsaved", key = key_hint("S")))
        });
        h_flex()
            .dir_row()
            .justify_between()
            .gap_3()
            .px_3()
            .h(px(26.))
            .flex_none()
            .border_t_1()
            .border_color(p.hairline)
            .text_xs()
            .text_color(p.text_muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(crate::i18n::display(&location)),
            )
            .children(environment)
            .children(unsaved)
            .child(tr!(
                "app.status.shortcuts",
                palette = key_hint("K"),
                send = key_hint("Enter")
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

fn send_hint() -> SharedString {
    tr!("response.send_hint", key = key_hint("Enter"))
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
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
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &OpenHistory, window, cx| this.open_history(window, cx)),
            )
            .on_action(cx.listener(|this, _: &OpenRunner, window, cx| this.open_runner(window, cx)))
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
            .text_start()
            .child(self.render_title_bar(cx))
            // Hard to miss: requests in a production environment touch real data.
            .when(self.is_prod(), |d| {
                d.child(div().h(px(2.)).flex_none().bg(p.err))
            })
            .child(
                h_flex()
                    .dir_row()
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
                            .child(div().flex_1().min_h_0().child(self.render_split(cx))),
                    ),
            )
            .child(self.render_status_bar(cx))
            .children(self.render_var_card(cx))
    }
}
