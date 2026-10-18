//! The Body tab: the body type picker and an editor for each type.

use std::path::{Component, Path, PathBuf};

use apix_core::{Body, FormPart, GraphQl, KeyValue};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{IconName, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{KvRow, Workspace};
use crate::bidi::{self, Bidi as _};
use crate::theme::Palette;
use crate::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BodyKind {
    None,
    Json,
    Text,
    Xml,
    Form,
    Multipart,
    Binary,
    GraphQl,
}

impl BodyKind {
    /// In picker order.
    const ALL: [BodyKind; 8] = [
        BodyKind::None,
        BodyKind::Json,
        BodyKind::Text,
        BodyKind::Xml,
        BodyKind::Form,
        BodyKind::Multipart,
        BodyKind::Binary,
        BodyKind::GraphQl,
    ];

    pub(super) fn label(self) -> SharedString {
        match self {
            BodyKind::None => tr!("body.kind.none"),
            BodyKind::Json => "JSON".into(),
            BodyKind::Text => tr!("body.kind.text"),
            BodyKind::Xml => "XML".into(),
            BodyKind::Form => tr!("body.kind.form"),
            BodyKind::Multipart => "Multipart".into(),
            BodyKind::Binary => tr!("body.kind.binary"),
            BodyKind::GraphQl => "GraphQL".into(),
        }
    }

    fn of(body: &Body) -> Self {
        match body {
            Body::None => BodyKind::None,
            Body::Json(_) => BodyKind::Json,
            Body::Text(_) => BodyKind::Text,
            Body::Xml(_) => BodyKind::Xml,
            Body::Form(_) => BodyKind::Form,
            Body::Multipart(_) => BodyKind::Multipart,
            Body::Binary { .. } => BodyKind::Binary,
            Body::GraphQl(_) => BodyKind::GraphQl,
        }
    }

    fn language(self) -> &'static str {
        match self {
            BodyKind::Json => "json",
            _ => "text",
        }
    }
}

/// Adds GraphQL to the code editor's languages, with Apix's highlight query.
pub fn register_graphql() {
    use gpui_kit::component::highlighter::{LanguageConfig, LanguageRegistry};
    let config = LanguageConfig::new(
        "graphql",
        tree_sitter_graphql::LANGUAGE.into(),
        Vec::new(),
        include_str!("../graphql.scm"),
        "",
        "",
    );
    LanguageRegistry::singleton().register("graphql", &config);
}

/// Ctrl+Enter in one of the body editors, which keep the key for themselves.
pub(super) struct SendRequested;

/// One multipart field. Text and file keep their own input, so switching
/// between them loses nothing.
struct PartRow {
    key: Entity<InputState>,
    text: Entity<InputState>,
    path: Entity<InputState>,
    file: bool,
    /// Kept from the file; not editable yet.
    content_type: Option<String>,
    enabled: bool,
}

impl PartRow {
    fn new(part: &FormPart, window: &mut Window, cx: &mut App) -> Self {
        let input = |placeholder: SharedString, value: &str, window: &mut Window, cx: &mut App| {
            let value = value.to_owned();
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let (text, path) = if part.file {
            ("", part.value.as_str())
        } else {
            (part.value.as_str(), "")
        };
        Self {
            key: input(tr!("table.key"), &part.key, window, cx),
            text: input(tr!("table.value"), text, window, cx),
            path: input(tr!("body.file_path"), path, window, cx),
            file: part.file,
            content_type: part.content_type.clone(),
            enabled: part.enabled,
        }
    }

    fn to_part(&self, cx: &App) -> Option<FormPart> {
        let key = self.key.read(cx).value().trim().to_string();
        let value = if self.file {
            self.path.read(cx).value().trim().to_string()
        } else {
            self.text.read(cx).value().to_string()
        };
        (!key.is_empty()).then(|| FormPart {
            key,
            value,
            file: self.file,
            content_type: self.content_type.clone(),
            enabled: self.enabled,
        })
    }
}

pub(super) struct BodyEditor {
    palette: Palette,
    workspace: WeakEntity<Workspace>,
    kind: BodyKind,
    /// JSON, Text and XML share one editor.
    text: Entity<EditorState>,
    form: Vec<KvRow>,
    parts: Vec<PartRow>,
    binary: Entity<InputState>,
    query: Entity<EditorState>,
    variables: Entity<EditorState>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SendRequested> for BodyEditor {}

impl BodyEditor {
    pub(super) fn new(
        palette: Palette,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let query = cx.new(|cx| EditorState::new(window, cx).language("graphql"));
        let variables = cx.new(|cx| EditorState::new(window, cx).language("json"));
        let binary =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("body.binary_placeholder")));

        let mut subscriptions = Vec::new();
        for editor in [&text, &query, &variables] {
            subscriptions.push(
                cx.subscribe(editor, |_, _, event: &InputEvent, cx| match event {
                    InputEvent::PressEnter {
                        secondary: true, ..
                    } => cx.emit(SendRequested),
                    InputEvent::Change => cx.notify(),
                    _ => {}
                }),
            );
        }
        subscriptions.push(cx.subscribe(&binary, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        }));

        Self {
            palette,
            workspace,
            kind: BodyKind::None,
            text,
            form: Vec::new(),
            parts: Vec::new(),
            binary,
            query,
            variables,
            _subscriptions: subscriptions,
        }
    }

    /// Placeholders are kept inside the inputs; set them again after a language change.
    pub(super) fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.binary.update(cx, |state, cx| {
            state.set_placeholder(tr!("body.binary_placeholder"), window, cx)
        });
        for row in &self.form {
            row.retranslate(window, cx);
        }
        for row in &self.parts {
            for (input, text) in [
                (&row.key, tr!("table.key")),
                (&row.text, tr!("table.value")),
                (&row.path, tr!("body.file_path")),
            ] {
                input.update(cx, |state, cx| state.set_placeholder(text, window, cx));
            }
        }
        cx.notify();
    }

    pub(super) fn kind(&self) -> BodyKind {
        self.kind
    }

    /// Shows `body`; the editors of other types are cleared.
    pub(super) fn set_body(&mut self, body: &Body, window: &mut Window, cx: &mut Context<Self>) {
        let (mut text, mut binary, mut query, mut variables) = Default::default();
        self.form.clear();
        self.parts.clear();
        match body {
            Body::None => {}
            Body::Json(t) | Body::Text(t) | Body::Xml(t) => text = t.as_str(),
            Body::Form(rows) => {
                self.form = rows.iter().map(|kv| KvRow::new(kv, window, cx)).collect();
            }
            Body::Multipart(parts) => {
                self.parts = parts.iter().map(|p| PartRow::new(p, window, cx)).collect();
            }
            Body::Binary { file } => binary = file.as_str(),
            Body::GraphQl(gql) => {
                query = gql.query.as_str();
                variables = gql.variables.as_str();
            }
        }
        let text = text.to_owned();
        self.text
            .update(cx, |state, cx| state.set_value(text, window, cx));
        let binary = binary.to_owned();
        self.binary
            .update(cx, |state, cx| state.set_value(binary, window, cx));
        let query = query.to_owned();
        self.query
            .update(cx, |state, cx| state.set_value(query, window, cx));
        let variables = variables.to_owned();
        self.variables
            .update(cx, |state, cx| state.set_value(variables, window, cx));
        self.set_kind(BodyKind::of(body), cx);
    }

    fn set_kind(&mut self, kind: BodyKind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.text
            .update(cx, |state, cx| state.set_highlighter(kind.language(), cx));
        cx.notify();
    }

    /// The body as the editor shows it now.
    pub(super) fn body(&self, cx: &App) -> Body {
        let text = || self.text.read(cx).value().to_string();
        match self.kind {
            BodyKind::None => Body::None,
            BodyKind::Json => Body::Json(text()),
            BodyKind::Text => Body::Text(text()),
            BodyKind::Xml => Body::Xml(text()),
            BodyKind::Form => Body::Form(
                self.form
                    .iter()
                    .filter_map(|r| r.to_key_value(cx))
                    .collect(),
            ),
            BodyKind::Multipart => {
                Body::Multipart(self.parts.iter().filter_map(|r| r.to_part(cx)).collect())
            }
            BodyKind::Binary => Body::Binary {
                file: self.binary.read(cx).value().trim().to_string(),
            },
            BodyKind::GraphQl => Body::GraphQl(GraphQl {
                query: self.query.read(cx).value().to_string(),
                variables: self.variables.read(cx).value().to_string(),
            }),
        }
    }

    /// Opens the file picker. The first file goes into `target`; with
    /// `multiple`, each further file becomes a new multipart row after row
    /// `after`, under the same key.
    fn pick_files(
        &mut self,
        target: Entity<InputState>,
        after: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: after.is_some(),
            prompt: Some(tr!("body.choose_file")),
        });
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let (dir, root) = workspace
                .read_with(cx, |ws, _| ws.body_location())
                .unwrap_or_default();
            let mut paths = paths
                .iter()
                .map(|p| stored_path(p, dir.as_deref(), root.as_deref()));
            let Some(first) = paths.next() else {
                return;
            };
            let rest: Vec<String> = paths.collect();
            this.update_in(cx, |this, window, cx| {
                target.update(cx, |state, cx| state.set_value(first, window, cx));
                if let Some(ix) = after {
                    let key = this
                        .parts
                        .get(ix)
                        .map(|r| r.key.read(cx).value().to_string())
                        .unwrap_or_default();
                    for (n, path) in rest.into_iter().enumerate() {
                        let row = PartRow::new(&FormPart::file(key.clone(), path), window, cx);
                        this.parts.insert(ix + 1 + n, row);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = BodyKind::ALL.iter().position(|k| *k == self.kind);
        // Small, so all eight fit; on a narrow pane the strip scrolls.
        bidi::fill_tabs(
            TabBar::new("body-kind").segmented().small(),
            selected,
            BodyKind::ALL
                .iter()
                .map(|kind| Tab::new().label(kind.label()))
                .collect(),
            cx.listener(|this, ix: &usize, _, cx| this.set_kind(BodyKind::ALL[*ix], cx)),
        )
    }

    fn hint(&self, text: impl Into<SharedString>) -> Div {
        div()
            .pt_2()
            .text_xs()
            .text_color(self.palette.text_muted)
            .child(bidi::para(text))
    }

    fn render_form(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.form.iter().enumerate().map(|(ix, row)| {
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    Checkbox::new(("form-on", ix))
                        .checked(row.enabled)
                        .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(row) = this.form.get_mut(ix) {
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
                .child(remove_button(("form-del", ix), cx, move |this| {
                    if ix < this.form.len() {
                        this.form.remove(ix);
                    }
                }))
        });
        v_flex()
            .id("form-rows")
            .gap_1()
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .child(add_button("form-add", cx, |this, window, cx| {
                this.form
                    .push(KvRow::new(&KeyValue::new("", ""), window, cx));
            }))
            .when(self.form.is_empty(), |d| {
                d.child(self.hint(tr!("body.form_hint")))
            })
            .into_any_element()
    }

    fn render_multipart(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.parts.iter().enumerate().map(|(ix, row)| {
            let value = if row.file {
                let path = row.path.clone();
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&row.path).small()),
                    )
                    .child(
                        Button::new(("part-pick", ix))
                            .ghost()
                            .xsmall()
                            .icon(IconName::FolderOpen)
                            .tooltip(tr!("body.choose_files"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.pick_files(path.clone(), Some(ix), window, cx)
                            })),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .child(Input::new(&row.text).small())
                    .into_any_element()
            };
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    Checkbox::new(("part-on", ix))
                        .checked(row.enabled)
                        .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                            if let Some(row) = this.parts.get_mut(ix) {
                                row.enabled = *checked;
                            }
                            cx.notify();
                        })),
                )
                .child(div().flex_1().min_w_0().child(Input::new(&row.key).small()))
                .child(
                    Button::new(("part-type", ix))
                        .ghost()
                        .xsmall()
                        .w(px(44.))
                        .label(if row.file {
                            tr!("body.part_file")
                        } else {
                            tr!("body.part_text")
                        })
                        .tooltip(if row.file {
                            tr!("body.send_text_instead")
                        } else {
                            tr!("body.send_file_instead")
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(row) = this.parts.get_mut(ix) {
                                row.file = !row.file;
                            }
                            cx.notify();
                        })),
                )
                .child(value)
                .child(remove_button(("part-del", ix), cx, move |this| {
                    if ix < this.parts.len() {
                        this.parts.remove(ix);
                    }
                }))
        });
        v_flex()
            .id("part-rows")
            .gap_1()
            .size_full()
            .overflow_y_scroll()
            .children(rows)
            .child(add_button("part-add", cx, |this, window, cx| {
                this.parts
                    .push(PartRow::new(&FormPart::text("", ""), window, cx));
            }))
            .when(self.parts.is_empty(), |d| {
                d.child(self.hint(tr!("body.multipart_hint")))
            })
            .into_any_element()
    }

    fn render_binary(&self, cx: &mut Context<Self>) -> AnyElement {
        let path = self.binary.read(cx).value().trim().to_string();
        let status = if path.is_empty() || path.contains("{{") {
            None
        } else {
            let (dir, _) = self
                .workspace
                .upgrade()
                .map(|ws| ws.read(cx).body_location())
                .unwrap_or_default();
            let full = match &dir {
                Some(dir) if Path::new(&path).is_relative() => dir.join(&path),
                _ => PathBuf::from(&path),
            };
            Some(match std::fs::metadata(&full) {
                Ok(meta) if meta.is_file() => (
                    tr!(
                        "body.binary_size",
                        size = apix_http::human_size(meta.len() as usize)
                    ),
                    self.palette.text_muted,
                ),
                _ => (tr!("body.file_not_found"), self.palette.err),
            })
        };
        let target = self.binary.clone();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.binary).small()),
                    )
                    .child(
                        Button::new("binary-pick")
                            .ghost()
                            .small()
                            .icon(IconName::FolderOpen)
                            .label(tr!("body.choose"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.pick_files(target.clone(), None, window, cx)
                            })),
                    ),
            )
            .child(match status {
                Some((text, color)) => div().text_xs().text_color(color).child(bidi::para(text)),
                None => self.hint(tr!("body.binary_hint")),
            })
            .into_any_element()
    }

    fn render_graphql(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = self.palette;
        let variables = self.variables.read(cx).value().to_string();
        let invalid = !variables.trim().is_empty()
            && !variables.contains("{{")
            && serde_json::from_str::<serde_json::Value>(&variables).is_err();
        let label = |text: SharedString| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_muted)
                .child(text)
        };
        v_flex()
            .size_full()
            .gap_1()
            .child(label(tr!("body.graphql_query")))
            .child(
                div()
                    .flex_grow(1.)
                    .flex_basis(relative(0.65))
                    .min_h_0()
                    .child(Editor::new(&self.query).h_full()),
            )
            .child(
                h_flex()
                    .dir_row()
                    .pt_1()
                    .gap_2()
                    .child(label(tr!("body.graphql_variables")))
                    .when(invalid, |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(p.err)
                                .child(tr!("body.invalid_json")),
                        )
                    }),
            )
            .child(
                div()
                    .flex_grow(1.)
                    .flex_basis(relative(0.35))
                    .min_h_0()
                    .child(Editor::new(&self.variables).h_full()),
            )
            .into_any_element()
    }
}

impl Render for BodyEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.kind {
            BodyKind::None => self.hint(tr!("body.none_hint")).into_any_element(),
            BodyKind::Json | BodyKind::Text | BodyKind::Xml => {
                Editor::new(&self.text).h_full().into_any_element()
            }
            BodyKind::Form => self.render_form(cx),
            BodyKind::Multipart => self.render_multipart(cx),
            BodyKind::Binary => self.render_binary(cx),
            BodyKind::GraphQl => self.render_graphql(cx),
        };
        v_flex()
            .size_full()
            .gap_2()
            .child(h_flex().dir_row().w_full().min_w_0().child(self.picker(cx)))
            .child(div().flex_1().min_h_0().child(content))
    }
}

fn remove_button(
    id: impl Into<ElementId>,
    cx: &mut Context<BodyEditor>,
    remove: impl Fn(&mut BodyEditor) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(IconName::Close)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            remove(this);
            cx.notify();
        }))
}

fn add_button(
    id: &'static str,
    cx: &mut Context<BodyEditor>,
    add: impl Fn(&mut BodyEditor, &mut Window, &mut Context<BodyEditor>) + 'static,
) -> Div {
    h_flex().pt_1().child(
        Button::new(id)
            .ghost()
            .small()
            .icon(IconName::Plus)
            .label(tr!("table.add"))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                add(this, window, cx);
                cx.notify();
            })),
    )
}

/// How a picked file is saved: relative to the request's folder when it is
/// inside the collection, so the collection works on other machines;
/// otherwise as picked. Forward slashes, so the file is the same on every OS.
fn stored_path(path: &Path, dir: Option<&Path>, root: Option<&Path>) -> String {
    if let (Some(dir), Some(root)) = (dir, root)
        && path.starts_with(root)
        && let Some(relative) = relative_to(path, dir)
    {
        return relative.to_string_lossy().replace('\\', "/");
    }
    path.to_string_lossy().into_owned()
}

fn relative_to(path: &Path, dir: &Path) -> Option<PathBuf> {
    let mut base = dir.components().peekable();
    let mut target = path.components().peekable();
    while let (Some(a), Some(b)) = (base.peek(), target.peek())
        && a == b
    {
        base.next();
        target.next();
    }
    let mut out = PathBuf::new();
    for component in base {
        match component {
            Component::Normal(_) => out.push(".."),
            Component::CurDir => {}
            _ => return None,
        }
    }
    out.extend(target);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{Path, stored_path};

    #[test]
    fn graphql_highlight_query_compiles() {
        let language: tree_sitter::Language = tree_sitter_graphql::LANGUAGE.into();
        tree_sitter::Query::new(&language, include_str!("../graphql.scm")).unwrap();
    }

    #[test]
    fn picked_files_inside_the_collection_are_relative() {
        let root = Path::new("/c/shop");
        let dir = Path::new("/c/shop/orders");
        assert_eq!(
            stored_path(Path::new("/c/shop/orders/a.pdf"), Some(dir), Some(root)),
            "a.pdf"
        );
        assert_eq!(
            stored_path(Path::new("/c/shop/files/b.png"), Some(dir), Some(root)),
            "../files/b.png"
        );
        assert_eq!(
            stored_path(Path::new("/c/other/c.bin"), Some(dir), Some(root)),
            "/c/other/c.bin"
        );
        assert_eq!(
            stored_path(Path::new("/c/shop/d.bin"), None, Some(root)),
            "/c/shop/d.bin"
        );
    }
}
