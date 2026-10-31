//! The Message tab of a gRPC request (`grpc://` or `grpcs://`): the method
//! to call, picked from what the server offers (reflection) or what the
//! request's files or definition describe, the message as JSON, and the
//! definition: made from protobuf, generated code, types or a JSON example.

use apix_core::{Body, GrpcCall};
use apix_grpc::prost_reflect::DescriptorPool;
use apix_grpc::{Certainty, Conversion, ConvertInput, Described, MethodInfo};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    Disableable as _, IconName, IndexPath, Selectable as _, Sizable, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use super::body_editor::{SendRequested, stored_path};
use crate::bidi::{self, Bidi as _};
use crate::tr;

/// "Load methods": the workspace asks the server (or reads the description).
pub(super) struct LoadMethods;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum MethodsState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(SharedString),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Message,
    Definition,
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
    view: View,
    /// What to convert: anything that describes the service.
    source: Entity<EditorState>,
    /// An example of the answer, when the source is only the request.
    response: Entity<EditorState>,
    method_path: Entity<InputState>,
    /// The result, which can be edited; what goes in `[grpc] definition`.
    definition: Entity<EditorState>,
    guessed: bool,
    /// How the last conversion went, while its result is on screen.
    converted: Option<(Certainty, String)>,
    convert_error: Option<SharedString>,
    converting: bool,
    /// What was said about the description: choices made, differences found.
    notes: Vec<String>,
    _convert_task: Option<Task<()>>,
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
        let source = cx.new(|cx| EditorState::new(window, cx));
        let response = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let definition = cx.new(|cx| EditorState::new(window, cx));
        let method_path =
            cx.new(|cx| InputState::new(window, cx).placeholder("package.Service/Method"));
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
            cx.subscribe(&definition, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    // Whatever was loaded from it no longer holds.
                    if this.pool.is_some() && this.state == MethodsState::Loaded {
                        this.state = MethodsState::NotLoaded;
                    }
                    cx.notify();
                }
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
            view: View::Message,
            source,
            response,
            method_path,
            definition,
            guessed: false,
            converted: None,
            convert_error: None,
            converting: false,
            notes: Vec::new(),
            _convert_task: None,
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
        self.guessed = call.guessed;
        self.converted = None;
        self.convert_error = None;
        self.notes.clear();
        self.methods.clear();
        self.pool = None;
        self.state = MethodsState::NotLoaded;
        self.method_path.update(cx, |state, cx| {
            state.set_value(call.method.clone(), window, cx)
        });
        self.set_method_items(Some(call.method), window, cx);
        let text = match body {
            Body::Json(t) | Body::Text(t) => t.clone(),
            _ => String::new(),
        };
        self.message
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.definition
            .update(cx, |state, cx| state.set_value(call.definition, window, cx));
        // Nothing has changed yet: the set above is not an edit.
        self.state = MethodsState::NotLoaded;
        for editor in [&self.source, &self.response] {
            editor.update(cx, |state, cx| state.set_value(String::new(), window, cx));
        }
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
        let definition = self.definition_text(cx);
        GrpcCall {
            method: self.selected_method(cx),
            protos: self.protos.clone(),
            import_paths: self.import_paths.clone(),
            guessed: self.guessed && !definition.trim().is_empty(),
            definition,
        }
    }

    pub(super) fn message(&self, cx: &App) -> String {
        self.message.read(cx).value().to_string()
    }

    fn definition_text(&self, cx: &App) -> String {
        self.definition.read(cx).value().to_string()
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
        result: Result<Described, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keep = Some(self.selected_method(cx));
        match result {
            Ok(described) => {
                self.methods = apix_grpc::methods(&described.pool);
                self.pool = Some(described.pool);
                self.state = MethodsState::Loaded;
                self.notes = described.notes;
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

    /// Converts in the background, from the given bytes or else the source
    /// on screen.
    fn convert(&mut self, bytes: Option<Vec<u8>>, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.read(cx).value().to_string();
        let response = self.response.read(cx).value().to_string();
        let typed = self.method_path.read(cx).value().trim().to_string();
        let method = if typed.is_empty() {
            self.selected_method(cx)
        } else {
            typed
        };
        self.converting = true;
        self.convert_error = None;
        cx.notify();
        let work = cx.background_executor().spawn(async move {
            let response = Some(response.as_str()).filter(|r| !r.trim().is_empty());
            let method = Some(method.as_str()).filter(|m| !m.trim().is_empty());
            match bytes {
                Some(bytes) => apix_grpc::convert_bytes(&bytes, response, method),
                None => apix_grpc::convert(&ConvertInput {
                    source: &source,
                    response,
                    method,
                }),
            }
            .map_err(|e| e.to_string())
        });
        self._convert_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            this.update_in(cx, |this, window, cx| {
                this.converting = false;
                match result {
                    Ok(conversion) => this.use_conversion(conversion, window, cx),
                    Err(message) => this.convert_error = Some(message.into()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// Puts a result on screen and offers its methods, without any network.
    fn use_conversion(
        &mut self,
        conversion: Conversion,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let guessed = conversion.is_guess();
        let Conversion {
            definition,
            format,
            certainty,
            notes,
            pool,
        } = conversion;
        self.definition
            .update(cx, |state, cx| state.set_value(definition, window, cx));
        self.guessed = guessed;
        self.converted = Some((certainty, format));
        self.notes = notes;
        self.methods = apix_grpc::methods(&pool);
        self.pool = Some(pool);
        self.state = MethodsState::Loaded;
        let typed = self.method_path.read(cx).value().trim().to_string();
        let selected = self.selected_method(cx);
        let wanted = [typed, selected]
            .into_iter()
            .find(|m| self.methods.iter().any(|known| known.name == *m))
            .or_else(|| self.methods.first().map(|m| m.name.clone()));
        self.set_method_items(wanted, window, cx);
    }

    fn clear_definition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.definition
            .update(cx, |state, cx| state.set_value(String::new(), window, cx));
        self.guessed = false;
        self.converted = None;
        self.convert_error = None;
        self.notes.clear();
        self.methods.clear();
        self.pool = None;
        self.state = MethodsState::NotLoaded;
        let keep = Some(self.selected_method(cx));
        self.set_method_items(keep, window, cx);
        cx.notify();
    }

    fn pick_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("grpc.def_choose")),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let read = std::fs::read(&path);
            this.update_in(cx, |this, window, cx| match read {
                Ok(bytes) => {
                    // A binary descriptor set has nothing to show.
                    let text = std::str::from_utf8(&bytes)
                        .ok()
                        .filter(|t| !t.contains('\0'))
                        .map(str::to_owned)
                        .unwrap_or_default();
                    this.source
                        .update(cx, |state, cx| state.set_value(text, window, cx));
                    this.convert(Some(bytes), window, cx);
                }
                Err(e) => {
                    this.convert_error = Some(format!("{}: {e}", path.display()).into());
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn save_definition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.definition_text(cx);
        let is_set = text.trim_start().starts_with('{');
        let name = if is_set {
            "service.json"
        } else {
            "service.proto"
        };
        let (dir, _) = self
            .workspace
            .read_with(cx, |ws, _| ws.body_location())
            .unwrap_or_default();
        let dir = dir.unwrap_or_else(|| std::path::PathBuf::from("."));
        let picked = cx.prompt_for_new_path(&dir, Some(name));
        cx.spawn_in(window, async move |_, cx| {
            let Ok(Ok(Some(path))) = picked.await else {
                return;
            };
            let result = std::fs::write(&path, text);
            cx.update(|window, cx| {
                let note = match result {
                    Ok(()) => Notification::success(tr!(
                        "grpc.def_saved",
                        path = path.display().to_string()
                    )),
                    Err(e) => Notification::error(format!("{}: {e}", path.display())),
                };
                window.push_notification(note, cx);
            })
            .ok();
        })
        .detach();
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

    fn source_note(&self, cx: &App) -> (SharedString, bool) {
        let has_definition = !self.definition_text(cx).trim().is_empty();
        match &self.state {
            MethodsState::Failed(message) => (message.clone(), true),
            MethodsState::Loading => (tr!("grpc.loading"), false),
            MethodsState::Loaded if has_definition => (
                tr!("grpc.loaded_definition", count = self.methods.len()),
                false,
            ),
            MethodsState::Loaded if self.protos.is_empty() => (
                tr!("grpc.loaded_reflection", count = self.methods.len()),
                false,
            ),
            MethodsState::Loaded => (tr!("grpc.loaded_protos", count = self.methods.len()), false),
            MethodsState::NotLoaded if has_definition => (tr!("grpc.hint_definition"), false),
            MethodsState::NotLoaded if self.protos.is_empty() => {
                (tr!("grpc.hint_reflection"), false)
            }
            MethodsState::NotLoaded => (tr!("grpc.hint_protos"), false),
        }
    }

    /// The way the definition can be trusted, in a sentence.
    fn certainty_line(&self, cx: &App) -> Option<(SharedString, Hsla)> {
        let p = crate::theme::current();
        match &self.converted {
            Some((Certainty::Exact, format)) => {
                Some((tr!("grpc.def_exact", format = format.clone()), p.ok))
            }
            Some((Certainty::NumbersFromSource, format)) => {
                Some((tr!("grpc.def_numbers", format = format.clone()), p.warn))
            }
            Some((Certainty::Guessed, format)) => {
                Some((tr!("grpc.def_guessed", format = format.clone()), p.warn))
            }
            None if self.guessed && !self.definition_text(cx).trim().is_empty() => {
                Some((tr!("grpc.def_unverified"), p.warn))
            }
            None => None,
        }
    }

    fn render_definition(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = crate::theme::current();
        let has_definition = !self.definition_text(cx).trim().is_empty();
        let has_message = !self.message(cx).trim().is_empty();
        let label = |text: SharedString| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_muted)
                .child(text)
        };
        let certainty = self.certainty_line(cx);
        v_flex()
            .id("grpc-definition")
            .flex_1()
            .min_h_0()
            .gap_2()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .dir_row()
                    .flex_wrap()
                    .gap_1()
                    .items_center()
                    .child(label(tr!("grpc.def_source")))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(bidi::para(tr!("grpc.def_source_hint"))),
                    )
                    .child(
                        Button::new("grpc-def-message")
                            .ghost()
                            .xsmall()
                            .label(tr!("grpc.def_from_message"))
                            .disabled(!has_message)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                let text = this.message(cx);
                                this.source
                                    .update(cx, |state, cx| state.set_value(text, window, cx));
                                this.convert(None, window, cx);
                            })),
                    )
                    .child(
                        Button::new("grpc-def-file")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FolderOpen)
                            .label(tr!("grpc.def_from_file"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.pick_source(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .h(px(84.))
                    .child(Editor::new(&self.source).h_full()),
            )
            .child(label(tr!("grpc.def_response")))
            .child(
                div()
                    .flex_none()
                    .h(px(44.))
                    .child(Editor::new(&self.response).h_full()),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(label(tr!("grpc.def_method")))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .ltr()
                            .child(Input::new(&self.method_path).small()),
                    )
                    .when(self.converting, |d| {
                        d.child(Spinner::new().xsmall().color(p.accent))
                    })
                    .child(
                        Button::new("grpc-def-convert")
                            .primary()
                            .small()
                            .label(tr!("grpc.def_convert"))
                            .disabled(self.converting)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.convert(None, window, cx)
                            })),
                    ),
            )
            .children(
                self.convert_error
                    .clone()
                    .map(|message| div().ltr().text_xs().text_color(p.err).child(message)),
            )
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .pt_1()
                    .child(label(tr!("grpc.def_result")))
                    .child(div().flex_1())
                    .child(
                        Button::new("grpc-def-save")
                            .ghost()
                            .xsmall()
                            .label(tr!("grpc.def_save"))
                            .disabled(!has_definition)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.save_definition(window, cx)
                            })),
                    )
                    .child(
                        Button::new("grpc-def-clear")
                            .ghost()
                            .xsmall()
                            .label(tr!("grpc.def_clear"))
                            .disabled(!has_definition)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.clear_definition(window, cx)
                            })),
                    ),
            )
            .when_some(certainty, |d, (text, color)| {
                d.child(div().text_xs().text_color(color).child(bidi::para(text)))
            })
            .when(!has_definition, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!("grpc.def_none"))),
                )
            })
            .child(
                div()
                    .flex_none()
                    .h(px(260.))
                    .ltr()
                    .child(Editor::new(&self.definition).h_full()),
            )
            .into_any_element()
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
        let (note, failed) = self.source_note(cx);
        let view = self.view;
        let label = |text: SharedString| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_muted)
                .child(text)
        };
        let body = if view == View::Definition {
            self.render_definition(cx)
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .gap_2()
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
                .into_any_element()
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
            .when(!self.notes.is_empty(), |d| {
                d.child(
                    v_flex()
                        .id("grpc-description-notes")
                        .ltr()
                        .flex_none()
                        .max_h(px(84.))
                        .overflow_y_scroll()
                        .gap_0p5()
                        .text_xs()
                        .text_color(p.warn)
                        .children(self.notes.iter().map(|note| div().child(note.clone()))),
                )
            })
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
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("grpc-view-message")
                            .ghost()
                            .xsmall()
                            .selected(view == View::Message)
                            .label(tr!("grpc.tab_message"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.view = View::Message;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("grpc-view-definition")
                            .ghost()
                            .xsmall()
                            .selected(view == View::Definition)
                            .label(tr!("grpc.tab_definition"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.view = View::Definition;
                                cx.notify();
                            })),
                    ),
            )
            .child(body)
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
