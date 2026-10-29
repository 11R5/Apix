//! GraphQL schemas for the body editor: fetching one with the open
//! request's URL, headers and auth, keeping it on disk per endpoint, and
//! completion and hover docs in the query editor from it.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use apix_core::graphql::{self, Schema, SuggestionKind};
use apix_core::history;
use gpui_kit::component::input::{CompletionProvider, HoverProvider, Rope, RopeExt as _};
use gpui_kit::*;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionItemKind, CompletionItemTag, CompletionResponse,
    CompletionTextEdit, Documentation, Hover, HoverContents, MarkupContent, MarkupKind, Range,
    TextEdit,
};

use super::Workspace;
use super::body_editor::SchemaState;
use crate::{net, tr};

/// The schema the query editor completes from; the body editor swaps it.
pub(super) type SharedSchema = Rc<RefCell<Option<Rc<Schema>>>>;

pub(super) struct GraphQlLsp(pub SharedSchema);

/// A suggestion as the editor's completion item, replacing `from..to`.
fn completion_item(s: graphql::Suggestion, text: &Rope, from: usize, to: usize) -> CompletionItem {
    CompletionItem {
        label: s.label,
        kind: Some(match s.kind {
            SuggestionKind::Keyword => CompletionItemKind::KEYWORD,
            SuggestionKind::Field => CompletionItemKind::FIELD,
            SuggestionKind::Argument => CompletionItemKind::PROPERTY,
            SuggestionKind::Value => CompletionItemKind::ENUM_MEMBER,
            SuggestionKind::Type => CompletionItemKind::CLASS,
        }),
        detail: Some(s.detail),
        documentation: (!s.documentation.is_empty()).then_some(Documentation::MarkupContent(
            MarkupContent {
                kind: MarkupKind::Markdown,
                value: s.documentation,
            },
        )),
        tags: s.deprecated.then(|| vec![CompletionItemTag::DEPRECATED]),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: Range::new(text.offset_to_position(from), text.offset_to_position(to)),
            new_text: s.insert,
        })),
        ..Default::default()
    }
}

/// Completion in the variables editor (JSON): the variables the query
/// declares, input fields and enum values, read against the query in its
/// own editor.
pub(super) struct VariablesLsp {
    pub schema: SharedSchema,
    pub query: WeakEntity<gpui_kit::component::input::EditorState>,
}

impl CompletionProvider for VariablesLsp {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let schema = self.schema.borrow().clone();
        let query = self.query.upgrade().map(|q| q.read(cx).value().to_string());
        let (Some(schema), Some(query)) = (schema, query) else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let json = text.to_string();
        let (from, to, suggestions) = graphql::variable_suggestions(&schema, &query, &json, offset);
        let items = suggestions
            .into_iter()
            .map(|s| completion_item(s, text, from, to))
            .collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        self.schema.borrow().is_some()
            && !new_text.is_empty()
            && (matches!(new_text, "{" | "[" | ":")
                || new_text.chars().all(|c| c == '"')
                || new_text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_'))
    }
}

impl CompletionProvider for GraphQlLsp {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        _: CompletionContext,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<CompletionResponse>> {
        let Some(schema) = self.0.borrow().clone() else {
            return Task::ready(Ok(CompletionResponse::Array(Vec::new())));
        };
        let source = text.to_string();
        let (start, suggestions) = graphql::suggestions(&schema, &source, offset);
        let before = source[..start].trim_end();
        let items = suggestions
            .into_iter()
            .map(|s| {
                // `... on User` replaces the dots already typed.
                let from = if s.insert.starts_with("...") && before.ends_with("...") {
                    before.len() - 3
                } else {
                    start
                };
                completion_item(s, text, from, offset)
            })
            .collect();
        Task::ready(Ok(CompletionResponse::Array(items)))
    }

    fn is_completion_trigger(&self, _: usize, new_text: &str, _: &mut App) -> bool {
        self.0.borrow().is_some()
            && (matches!(new_text, "(" | "{" | "[" | ":")
                || (!new_text.is_empty()
                    && new_text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')))
    }
}

impl HoverProvider for GraphQlLsp {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        _: &mut App,
    ) -> Task<Result<Option<Hover>>> {
        let docs = self
            .0
            .borrow()
            .as_ref()
            .and_then(|schema| graphql::describe_at(schema, &text.to_string(), offset));
        Task::ready(Ok(docs.map(|value| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: None,
        })))
    }
}

/// Where the schema fetched from `url` is kept: one file per endpoint.
fn cache_file(url: &str) -> Option<PathBuf> {
    // FNV-1a: the same name on every run and every Rust version.
    let hash = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    Some(
        dirs::config_dir()?
            .join("Apix")
            .join("graphql")
            .join(format!("{hash:016x}.json")),
    )
}

/// The schema fetched from `url` before, and when.
fn cached(url: &str) -> Option<(Schema, u64)> {
    let path = cache_file(url)?;
    let json = std::fs::read_to_string(&path).ok()?;
    let at = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);
    Some((Schema::from_introspection(&json).ok()?, at))
}

fn keep(url: &str, json: &str) {
    let Some(path) = cache_file(url) else {
        return;
    };
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|_| std::fs::write(&path, json));
    if let Err(e) = written {
        eprintln!("apix: could not keep the GraphQL schema: {e}");
    }
}

impl Workspace {
    /// The open request as it would be sent (inherited auth and settings,
    /// variables filled in), for asking its server for the schema.
    fn schema_request(&self, cx: &App) -> Result<apix_core::Request, SharedString> {
        let request = self.with_inherited_auth(self.build_request(cx), self.selected.as_deref());
        let request = self.with_inherited_settings(request, self.selected.as_deref());
        if request.url.is_empty() {
            return Err(tr!("request.error.no_url"));
        }
        let resolved = request.resolve(&self.vars);
        if !resolved.missing.is_empty() {
            return Err(super::undefined_message(resolved.missing));
        }
        Ok(resolved.request)
    }

    /// Shows the schema kept for the open request's endpoint, if there is one.
    pub(super) fn load_cached_schema(&mut self, cx: &mut Context<Self>) {
        let found = self
            .schema_request(cx)
            .ok()
            .and_then(|request| cached(&request.url));
        self.body_editor.update(cx, |editor, cx| match found {
            Some((schema, at)) => editor.set_schema(Some(schema), SchemaState::Ready { at }, cx),
            None => editor.set_schema(None, SchemaState::None, cx),
        });
    }

    /// Asks the open request's server for its schema and keeps it.
    pub(super) fn fetch_schema(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let request = match self.schema_request(cx) {
            Ok(request) => request,
            Err(message) => {
                self.body_editor.update(cx, |editor, cx| {
                    editor.set_schema_state(SchemaState::Failed(message), cx)
                });
                return;
            }
        };
        self.body_editor.update(cx, |editor, cx| {
            editor.set_schema_state(SchemaState::Fetching, cx)
        });
        let url = request.url.clone();
        let fetch = net::spawn(async move {
            apix_http::graphql::fetch_schema(net::client(), &request)
                .await
                .map_err(|e| net::error_chain(&e))
        });
        let editor = self.body_editor.downgrade();
        self.schema_task = Some(cx.spawn_in(window, async move |_, cx| {
            let result = fetch.await.and_then(|r| r);
            editor
                .update(cx, |editor, cx| match result {
                    Ok(fetched) => {
                        keep(&url, &fetched.json);
                        let at = history::now_ms();
                        editor.set_schema(Some(fetched.schema), SchemaState::Ready { at }, cx);
                    }
                    Err(message) => {
                        editor.set_schema_state(SchemaState::Failed(message.into()), cx)
                    }
                })
                .ok();
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::cache_file;

    #[test]
    fn each_endpoint_keeps_its_own_schema_file() {
        let name = |url: &str| cache_file(url).unwrap().file_name().unwrap().to_owned();
        // The same on every run, so a restart finds it again.
        assert_eq!(
            name("http://127.0.0.1:8769/graphql"),
            "741c02c82f5996d8.json"
        );
        assert_ne!(
            name("https://staging.example.com/graphql"),
            name("https://example.com/graphql")
        );
    }
}
