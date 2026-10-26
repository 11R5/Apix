//! The Message tab of a gRPC request (`grpc://` or `grpcs://`): the method
//! to call, picked from what the server offers (reflection) or what the
//! request's `.proto` files describe, and the message as JSON.

use apix_core::{Body, GrpcCall};
use apix_grpc::MethodInfo;
use apix_grpc::prost_reflect::DescriptorPool;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable as _, IconName, IndexPath, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use super::body_editor::{SendRequested, stored_path};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// "Load methods": the workspace asks the server (or reads the .proto files).
pub(super) struct LoadMethods;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum MethodsState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(SharedString),
}

type MethodSelect = SelectState<Vec<SharedString>>;

pub(super) struct GrpcEditor {
    workspace: WeakEntity<Workspace>,
    method: Entity<MethodSelect>,
    /// What the service offers, once loaded.
    methods: Vec<MethodInfo>,
    pool: Option<DescriptorPool>,
    state: MethodsState,
    /// As written in the request file (relative to it).
    protos: Vec<String>,
    import_paths: Vec<String>,
    message: Entity<EditorState>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<LoadMethods> for GrpcEditor {}
impl EventEmitter<SendRequested> for GrpcEditor {}

impl GrpcEditor {
    pub(super) fn new(
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let method = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx).searchable(true));
        let message = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let subscriptions = vec![
            cx.subscribe(&method, |_, _, _: &SelectEvent<Vec<SharedString>>, cx| {
                cx.notify()
            }),
            cx.subscribe(&message, |_, _, event: &InputEvent, cx| match event {
                InputEvent::PressEnter {
                    secondary: true, ..
                } => cx.emit(SendRequested),
                InputEvent::Change => cx.notify(),
                _ => {}
            }),
        ];
        Self {
            workspace,
            method,
            methods: Vec::new(),
            pool: None,
            state: MethodsState::NotLoaded,
            protos: Vec::new(),
            import_paths: Vec::new(),
            message,
            _subscriptions: subscriptions,
        }
    }

    /// Shows a request's call and message; what was loaded for another
    /// request is forgotten.
    pub(super) fn set_call(
        &mut self,
        call: Option<&GrpcCall>,
        body: &Body,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let call = call.cloned().unwrap_or_default();
        self.protos = call.protos;
        self.import_paths = call.import_paths;
        self.methods.clear();
        self.pool = None;
        self.state = MethodsState::NotLoaded;
        self.set_method_items(Some(call.method), window, cx);
        let text = match body {
            Body::Json(t) | Body::Text(t) => t.clone(),
            _ => String::new(),
        };
        self.message
            .update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }

    /// The method list: the loaded methods, plus `keep` when it is not one
    /// of them (a saved method stays visible before anything is loaded).
    fn set_method_items(
        &mut self,
        keep: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut items: Vec<SharedString> = self
            .methods
            .iter()
            .map(|m| SharedString::from(m.name.clone()))
            .collect();
        let keep = keep.filter(|m| !m.trim().is_empty());
        if let Some(keep) = &keep
            && !items.iter().any(|i| i.as_ref() == keep)
        {
            items.insert(0, keep.clone().into());
        }
        let selected = keep
            .and_then(|k| items.iter().position(|i| i.as_ref() == k))
            .map(IndexPath::new);
        self.method.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_index(selected, window, cx);
        });
    }

    pub(super) fn call(&self, cx: &App) -> GrpcCall {
        GrpcCall {
            method: self.selected_method(cx),
            protos: self.protos.clone(),
            import_paths: self.import_paths.clone(),
        }
    }

    pub(super) fn message(&self, cx: &App) -> String {
        self.message.read(cx).value().to_string()
    }

    fn selected_method(&self, cx: &App) -> String {
        self.method
            .read(cx)
            .selected_value()
            .map(|m| m.to_string())
            .unwrap_or_default()
    }

    pub(super) fn set_loading(&mut self, cx: &mut Context<Self>) {
        self.state = MethodsState::Loading;
        cx.notify();
    }

    /// What loading the methods came to.
    pub(super) fn set_methods(
        &mut self,
        result: Result<DescriptorPool, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keep = Some(self.selected_method(cx));
        match result {
            Ok(pool) => {
                self.methods = apix_grpc::methods(&pool);
                self.pool = Some(pool);
                self.state = MethodsState::Loaded;
            }
            Err(message) => {
                self.methods.clear();
                self.pool = None;
                self.state = MethodsState::Failed(message.into());
            }
        }
        self.set_method_items(keep, window, cx);
        cx.notify();
    }

    fn method_info(&self, cx: &App) -> Option<&MethodInfo> {
        let name = self.selected_method(cx);
        self.methods.iter().find(|m| m.name == name)
    }

    /// Fills the message with every field of the method's input.
    fn fill_example(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.selected_method(cx);
        let Some(method) = self
            .pool
            .as_ref()
            .and_then(|pool| apix_grpc::find_method(pool, &name))
        else {
            return;
        };
        let mut example = apix_grpc::example_message(&method.input());
        if method.is_client_streaming() {
            example = format!("[\n{}\n]", indent(&example));
        }
        self.message
            .update(cx, |state, cx| state.set_value(example, window, cx));
        cx.notify();
    }

    fn pick_protos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr!("grpc.choose_protos")),
        });
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let (dir, root) = workspace
                .read_with(cx, |ws, _| ws.body_location())
                .unwrap_or_default();
            this.update(cx, |this, cx| {
                for path in paths {
                    let stored = stored_path(&path, dir.as_deref(), root.as_deref());
                    if !this.protos.contains(&stored) {
                        this.protos.push(stored);
                    }
                }
                // What described the service before no longer does.
                this.state = MethodsState::NotLoaded;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn source_note(&self) -> (SharedString, bool) {
        match &self.state {
            MethodsState::Failed(message) => (message.clone(), true),
            MethodsState::Loading => (tr!("grpc.loading"), false),
            MethodsState::Loaded if self.protos.is_empty() => (
                tr!("grpc.loaded_reflection", count = self.methods.len()),
                false,
            ),
            MethodsState::Loaded => (tr!("grpc.loaded_protos", count = self.methods.len()), false),
            MethodsState::NotLoaded if self.protos.is_empty() => {
                (tr!("grpc.hint_reflection"), false)
            }
            MethodsState::NotLoaded => (tr!("grpc.hint_protos"), false),
        }
    }
}

/// `text` indented by two spaces, for an example inside a list.
fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Render for GrpcEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let info = self.method_info(cx).cloned();
        let loading = self.state == MethodsState::Loading;
        let (note, failed) = self.source_note();
        let label = |text: SharedString| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_muted)
                .child(text)
        };
        let protos = self.protos.iter().enumerate().map(|(ix, proto)| {
            h_flex()
                .id(("grpc-proto", ix))
                .gap_1()
                .items_center()
                .pl_2()
                .rounded_md()
                .bg(p.hover)
                .text_xs()
                .ltr()
                .child(proto.clone())
                .child(
                    Button::new(("grpc-proto-del", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if ix < this.protos.len() {
                                this.protos.remove(ix);
                                this.state = MethodsState::NotLoaded;
                            }
                            cx.notify();
                        })),
                )
        });
        v_flex()
            .size_full()
            .gap_2()
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(label(tr!("grpc.method")))
                    .child(
                        div().flex_1().min_w_0().ltr().child(
                            Select::new(&self.method)
                                .small()
                                .placeholder(tr!("grpc.pick_method"))
                                .search_placeholder(tr!("grpc.search_methods")),
                        ),
                    )
                    .children(info.as_ref().map(|m| {
                        div()
                            .flex_none()
                            .px_2()
                            .py_0p5()
                            .rounded_full()
                            .bg(p.accent.opacity(0.15))
                            .text_xs()
                            .text_color(p.accent)
                            .child(kind_label(m))
                    }))
                    .child(
                        Button::new("grpc-load")
                            .ghost()
                            .small()
                            .label(if self.methods.is_empty() {
                                tr!("grpc.load_methods")
                            } else {
                                tr!("grpc.reload_methods")
                            })
                            .disabled(loading)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if this.state != MethodsState::Loading {
                                    cx.emit(LoadMethods);
                                }
                            })),
                    ),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_1()
                    .items_center()
                    .when(loading, |d| {
                        d.child(Spinner::new().xsmall().color(p.accent))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(if failed { p.err } else { p.text_muted })
                            .child(bidi::para(note)),
                    ),
            )
            .child(
                h_flex()
                    .dir_row()
                    .flex_wrap()
                    .gap_1()
                    .items_center()
                    .child(label(tr!("grpc.protos")))
                    .when(self.protos.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(p.text_muted)
                                .child(tr!("grpc.protos_none")),
                        )
                    })
                    .children(protos)
                    .child(
                        Button::new("grpc-add-proto")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .label(tr!("grpc.add_protos"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.pick_protos(window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .pt_1()
                    .child(label(tr!("grpc.message")))
                    .when(info.as_ref().is_some_and(|m| m.client_streaming), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(p.text_muted)
                                .child(tr!("grpc.stream_hint")),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        Button::new("grpc-example")
                            .ghost()
                            .xsmall()
                            .label(tr!("grpc.example"))
                            .disabled(info.is_none())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.fill_example(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(Editor::new(&self.message).h_full()),
            )
    }
}

/// "unary", "server streaming", …, in the interface language.
fn kind_label(method: &MethodInfo) -> SharedString {
    match (method.client_streaming, method.server_streaming) {
        (false, false) => tr!("grpc.kind_unary"),
        (false, true) => tr!("grpc.kind_server"),
        (true, false) => tr!("grpc.kind_client"),
        (true, true) => tr!("grpc.kind_bidi"),
    }
}
