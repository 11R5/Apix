//! The request Settings form: used as the request's Settings tab and in the
//! folder, collection and project Settings dialog. Every field left empty (or
//! on "Inherit") falls back folder → collection → project, then the defaults,
//! like the Auth tab.

use std::collections::HashMap;
use std::path::Path;

use apix_core::{Collection, Project, ProxyChoice, Request, RequestSettings, project, store};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme::Palette;
use crate::tr;

type Picker = SelectState<Vec<SharedString>>;

/// Inherit / On / Off, in picker order.
fn tri() -> Vec<SharedString> {
    vec![tr!("reqset.inherit"), tr!("reqset.on"), tr!("reqset.off")]
}

fn tri_index(value: Option<bool>) -> usize {
    match value {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    }
}

fn tri_value(ix: usize) -> Option<bool> {
    match ix {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// Inherit / Off / A proxy URL, in picker order.
fn proxy_kinds() -> Vec<SharedString> {
    vec![
        tr!("reqset.inherit"),
        tr!("reqset.off"),
        tr!("reqset.proxy_url"),
    ]
}

/// The text fields of the form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum F {
    Timeout,
    MaxRedirects,
    Proxy,
    ClientCert,
    ClientKey,
}

impl F {
    const ALL: [F; 5] = [
        F::Timeout,
        F::MaxRedirects,
        F::Proxy,
        F::ClientCert,
        F::ClientKey,
    ];

    fn placeholder(self) -> SharedString {
        match self {
            F::Timeout | F::MaxRedirects => tr!("reqset.hint.inherit"),
            F::Proxy => "http://proxy.example.com:8080".into(),
            F::ClientCert => tr!("reqset.hint.cert"),
            F::ClientKey => tr!("reqset.hint.key"),
        }
    }
}

pub(super) struct SettingsEditor {
    palette: Palette,
    fields: HashMap<F, Entity<InputState>>,
    follow: Entity<Picker>,
    tls: Entity<Picker>,
    cookies: Entity<Picker>,
    proxy_kind: Entity<Picker>,
    /// What empty fields fall back to here, already merged over the chain.
    inherited: RequestSettings,
    _subscriptions: Vec<Subscription>,
}

impl SettingsEditor {
    pub(super) fn new(palette: Palette, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = |items: Vec<SharedString>, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| SelectState::new(items, Some(IndexPath::new(0)), window, cx))
        };
        let follow = picker(tri(), window, cx);
        let tls = picker(tri(), window, cx);
        let cookies = picker(tri(), window, cx);
        let proxy_kind = picker(proxy_kinds(), window, cx);

        let mut subscriptions = Vec::new();
        for select in [&follow, &tls, &cookies, &proxy_kind] {
            subscriptions.push(cx.subscribe_in(
                select,
                window,
                |_, _, _: &SelectEvent<Vec<SharedString>>, _, cx| cx.notify(),
            ));
        }
        let mut fields = HashMap::new();
        for f in F::ALL {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(f.placeholder()));
            subscriptions.push(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
            fields.insert(f, input);
        }
        Self {
            palette,
            fields,
            follow,
            tls,
            cookies,
            proxy_kind,
            inherited: RequestSettings::default(),
            _subscriptions: subscriptions,
        }
    }

    /// Relabels the pickers and placeholders after a language change.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for select in [&self.follow, &self.tls, &self.cookies] {
            let ix = select.read(cx).selected_index(cx);
            select.update(cx, |state, cx| {
                state.set_items(tri(), window, cx);
                state.set_selected_index(ix, window, cx);
            });
        }
        let ix = self.proxy_kind.read(cx).selected_index(cx);
        self.proxy_kind.update(cx, |state, cx| {
            state.set_items(proxy_kinds(), window, cx);
            state.set_selected_index(ix, window, cx);
        });
        for f in F::ALL {
            self.fields[&f].update(cx, |state, cx| {
                state.set_placeholder(f.placeholder(), window, cx)
            });
        }
        cx.notify();
    }

    fn text(&self, f: F, cx: &App) -> String {
        self.fields[&f].read(cx).value().trim().to_string()
    }

    fn selected(select: &Entity<Picker>, cx: &App) -> usize {
        select
            .read(cx)
            .selected_index(cx)
            .map(|ix| ix.row)
            .unwrap_or(0)
    }

    /// Shows `settings`; everything else is reset to "Inherit".
    pub(super) fn set_settings(
        &mut self,
        settings: &RequestSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pick = |select: &Entity<Picker>, ix: usize, window: &mut Window, cx: &mut App| {
            select.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(ix)), window, cx)
            });
        };
        pick(
            &self.follow,
            tri_index(settings.follow_redirects),
            window,
            cx,
        );
        pick(&self.tls, tri_index(settings.tls_verify), window, cx);
        pick(&self.cookies, tri_index(settings.send_cookies), window, cx);
        let (proxy_ix, proxy_text) = match settings.proxy.as_deref() {
            None => (0, String::new()),
            Some("") => (1, String::new()),
            Some(url) => (2, url.to_owned()),
        };
        pick(&self.proxy_kind, proxy_ix, window, cx);

        let texts = [
            (
                F::Timeout,
                settings.timeout_ms.map(format_seconds).unwrap_or_default(),
            ),
            (
                F::MaxRedirects,
                settings
                    .max_redirects
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            ),
            (F::Proxy, proxy_text),
            (
                F::ClientCert,
                settings.client_cert.clone().unwrap_or_default(),
            ),
            (
                F::ClientKey,
                settings.client_key.clone().unwrap_or_default(),
            ),
        ];
        for (f, text) in texts {
            self.fields[&f].update(cx, |state, cx| state.set_value(text, window, cx));
        }
        cx.notify();
    }

    pub(super) fn set_inherited(&mut self, inherited: RequestSettings, cx: &mut Context<Self>) {
        if self.inherited != inherited {
            self.inherited = inherited;
            cx.notify();
        }
    }

    /// The settings as the form shows them now. Unreadable numbers count
    /// as "inherit".
    pub(super) fn settings(&self, cx: &App) -> RequestSettings {
        let some = |s: String| if s.is_empty() { None } else { Some(s) };
        RequestSettings {
            timeout_ms: parse_seconds(&self.text(F::Timeout, cx)),
            follow_redirects: tri_value(Self::selected(&self.follow, cx)),
            max_redirects: self.text(F::MaxRedirects, cx).parse().ok(),
            tls_verify: tri_value(Self::selected(&self.tls, cx)),
            proxy: match Self::selected(&self.proxy_kind, cx) {
                0 => None,
                1 => Some(String::new()),
                _ => Some(self.text(F::Proxy, cx)),
            },
            client_cert: some(self.text(F::ClientCert, cx)),
            client_key: some(self.text(F::ClientKey, cx)),
            send_cookies: tri_value(Self::selected(&self.cookies, cx)),
        }
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
                    .when(bidi::rtl(), |d| d.flex().flex_row_reverse())
                    .child(child),
            )
    }

    fn field(&self, label: SharedString, f: F) -> AnyElement {
        self.row(label, Input::new(&self.fields[&f]).small())
            .into_any_element()
    }

    fn picker_row(&self, label: SharedString, select: &Entity<Picker>) -> AnyElement {
        self.row(label, div().w(px(220.)).child(Select::new(select).small()))
            .into_any_element()
    }

    /// One line with what this request ends up using.
    fn effective_note(&self, cx: &App) -> SharedString {
        let eff = self.settings(cx).or_inherit(&self.inherited).effective();
        let on_off = |on: bool| {
            if on {
                tr!("reqset.on")
            } else {
                tr!("reqset.off")
            }
        };
        let timeout = match eff.timeout {
            None => tr!("reqset.val.no_timeout"),
            Some(d) => apix_http::human_duration(d).into(),
        };
        let redirects = if eff.follow_redirects {
            tr!("reqset.val.redirects", count = eff.max_redirects)
        } else {
            tr!("reqset.off")
        };
        let proxy = match &eff.proxy {
            ProxyChoice::Environment => tr!("reqset.val.proxy_env"),
            ProxyChoice::Off => tr!("reqset.off"),
            ProxyChoice::Url(url) => url.clone().into(),
        };
        tr!(
            "reqset.effective",
            timeout = timeout,
            redirects = redirects,
            tls = on_off(eff.tls_verify),
            proxy = proxy,
            cookies = on_off(eff.send_cookies)
        )
    }
}

impl Render for SettingsEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let custom_proxy = Self::selected(&self.proxy_kind, cx) == 2;
        v_flex()
            .gap_3()
            .child(self.field(tr!("reqset.timeout"), F::Timeout))
            .child(self.picker_row(tr!("reqset.follow"), &self.follow))
            .child(self.field(tr!("reqset.max_redirects"), F::MaxRedirects))
            .child(self.picker_row(tr!("reqset.tls_verify"), &self.tls))
            .child(self.picker_row(tr!("reqset.proxy"), &self.proxy_kind))
            .children(custom_proxy.then(|| self.field(tr!("reqset.proxy_url"), F::Proxy)))
            .child(self.field(tr!("reqset.client_cert"), F::ClientCert))
            .child(self.field(tr!("reqset.client_key"), F::ClientKey))
            .child(self.picker_row(tr!("reqset.send_cookies"), &self.cookies))
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted.opacity(0.8))
                    .child(bidi::para(self.effective_note(cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted.opacity(0.8))
                    .child(bidi::para(tr!("reqset.note"))),
            )
    }
}

/// `5000` ms → `"5"`, `2500` → `"2.5"`, `0` → `"0"`.
fn format_seconds(ms: u64) -> String {
    let secs = ms as f64 / 1000.0;
    if secs.fract() == 0.0 {
        format!("{}", secs as u64)
    } else {
        format!("{secs}")
    }
}

/// Seconds (a decimal comma works too) → milliseconds.
fn parse_seconds(text: &str) -> Option<u64> {
    let secs: f64 = text.replace(',', ".").parse().ok()?;
    (secs >= 0.0).then_some((secs * 1000.0) as u64)
}

impl super::Workspace {
    /// What a request at `path` inherits for its empty settings. Without a
    /// path (an unsaved request), the active collection's.
    pub(super) fn inherited_settings_for(&self, path: Option<&Path>) -> RequestSettings {
        let collection = path
            .and_then(|p| self.collection_for(p))
            .or(self.collection.as_ref());
        let dir = path
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| collection.map(|c| c.root.clone()));
        let Some(dir) = dir else {
            return RequestSettings::default();
        };
        project::inherited_settings(&dir, collection, self.project.as_ref()).unwrap_or_default()
    }

    /// Updates the Settings tab's effective-values note for the open request.
    pub(super) fn refresh_inherited_settings(&mut self, cx: &mut Context<Self>) {
        let inherited = self.inherited_settings_for(self.selected.as_deref());
        self.settings_editor
            .update(cx, |editor, cx| editor.set_inherited(inherited, cx));
    }

    /// `request` (saved at `path`) with its empty settings filled from what
    /// they inherit.
    pub(super) fn with_inherited_settings(
        &self,
        mut request: Request,
        path: Option<&Path>,
    ) -> Request {
        let inherited = self.inherited_settings_for(path);
        request.settings = request.settings.or_inherit(&inherited);
        request
    }

    /// The dialog that sets a folder's, collection's or project's settings.
    pub(super) fn open_settings_dialog(
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
                let inherited =
                    project::inherited_settings(parent, collection, self.project.as_ref());
                (
                    tr!("reqset.dialog.folder_title", name = name),
                    meta.settings,
                    inherited,
                )
            }),
            AuthOwner::Collection(root) => Collection::open(root).map(|c| {
                let inherited = project::inherited_settings(root, None, self.project.as_ref());
                (
                    tr!("reqset.dialog.collection_title", name = c.meta.name),
                    c.meta.settings,
                    inherited,
                )
            }),
            AuthOwner::Project(root) => Project::open(root).map(|p| {
                (
                    tr!("reqset.dialog.project_title", name = p.meta.name),
                    p.meta.settings,
                    Ok(RequestSettings::default()),
                )
            }),
        };
        let (title, current, inherited) = match loaded {
            Ok(found) => found,
            Err(e) => return self.notify_error(super::describe(&e), window, cx),
        };
        let inherited = inherited.unwrap_or_default();
        let palette = self.palette;
        let editor = cx.new(|cx| {
            let mut editor = SettingsEditor::new(palette, window, cx);
            editor.set_settings(&current, window, cx);
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
                                .child(bidi::para(tr!("reqset.dialog.note"))),
                        )
                        .child(
                            Button::new("reqset-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    super::collections::close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("reqset-save")
                                .primary()
                                .label(tr!("dialog.save"))
                                .on_click(move |_, window, cx| {
                                    let settings = save.read(cx).settings(cx);
                                    let saved = save_view
                                        .update(cx, |this, cx| {
                                            this.save_shared_settings(&owner, settings, window, cx)
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

    fn save_shared_settings(
        &mut self,
        owner: &AuthOwner,
        settings: RequestSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let result = match owner {
            AuthOwner::Folder(dir) => store::load_folder_meta(dir).and_then(|mut meta| {
                meta.settings = settings;
                store::save_folder_meta(dir, &meta)
            }),
            AuthOwner::Collection(root) => Collection::open(root).and_then(|mut c| {
                c.meta.settings = settings;
                store::save_collection_meta(root, &c.meta)
            }),
            AuthOwner::Project(root) => Project::open(root).and_then(|mut p| {
                p.meta.settings = settings;
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
