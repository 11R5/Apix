//! History (Ctrl+H): every sent request with its response, kept on this
//! computer. Picking an entry puts that request back in the editor, as
//! unsaved edits of the request it came from, with the response it got.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use apix_core::history::{self, Entry, History, StoredResponse, Summary};
use apix_core::{Request, vfs};
use apix_http::{Response, Timing};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::{Finished, ResponseState, Workspace, key_hint};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme;
use crate::tr;

/// `<config>/Apix/history`, next to the settings.
fn store() -> Option<History> {
    Some(History::new(
        dirs::config_dir()?.join("Apix").join("history"),
    ))
}

/// A send that will be written to the history once its answer is in.
pub(super) struct PendingRecord {
    history: History,
    request: Request,
    path: Option<PathBuf>,
    at: u64,
    started: Instant,
}

impl PendingRecord {
    /// Runs on a background thread.
    pub(super) fn save(self, result: &Result<Response, String>) {
        let (status, error, size, duration, response, body) = match result {
            Ok(r) => (
                Some(r.status),
                None,
                r.body.len() as u64,
                r.timing.total,
                Some(StoredResponse {
                    status: r.status,
                    version: r.version.clone(),
                    headers: r.headers.clone(),
                    ttfb_us: r.timing.ttfb.as_micros() as u64,
                    total_us: r.timing.total.as_micros() as u64,
                    truncated: false,
                }),
                r.body[..r.body.len().min(history::MAX_BODY)].to_vec(),
            ),
            Err(message) => (
                None,
                Some(message.clone()),
                0,
                self.started.elapsed(),
                None,
                Vec::new(),
            ),
        };
        let entry = Entry {
            summary: Summary {
                id: history::new_id(self.at),
                at: self.at,
                method: self.request.method,
                url: self.request.url.clone(),
                name: self.request.name.clone(),
                path: self.path,
                status,
                error,
                duration_ms: duration.as_millis() as u64,
                size,
            },
            request: self.request,
            response,
            body,
        };
        if let Err(e) = self.history.record(&entry) {
            eprintln!("apix: could not save the history: {e}");
        }
    }
}

/// "just now", "5 min ago", "3 h ago", "2 days ago".
fn ago(at_ms: u64) -> SharedString {
    let secs = history::now_ms().saturating_sub(at_ms) / 1000;
    match secs {
        0..60 => tr!("history.ago_now"),
        60..3600 => tr!("history.ago_minutes", count = secs / 60),
        3600..86_400 => tr!("history.ago_hours", count = secs / 3600),
        _ => tr!("history.ago_days", count = secs / 86_400),
    }
}

struct HistoryView {
    state: Entity<CommandState>,
    workspace: WeakEntity<Workspace>,
    colors: theme::Palette,
    entries: Vec<Summary>,
    query: String,
    enabled: bool,
}

impl HistoryView {
    /// Entries whose name, URL, method or status contain every word typed.
    fn matching(&self) -> Vec<&Summary> {
        let words: Vec<String> = self
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        self.entries
            .iter()
            .filter(|e| {
                let text = format!(
                    "{} {} {} {}",
                    e.method,
                    e.name,
                    e.url,
                    e.status.map(|s| s.to_string()).unwrap_or_default()
                )
                .to_lowercase();
                words.iter().all(|w| text.contains(w))
            })
            .collect()
    }

    fn item(&self, entry: &Summary, mono: SharedString) -> CommandItem {
        let colors = self.colors;
        let entry = entry.clone();
        let (status_text, status_color) = match entry.status {
            Some(s) if s < 400 => (s.to_string(), colors.ok),
            Some(s) => (s.to_string(), colors.err),
            None => (tr!("history.failed").to_string(), colors.err),
        };
        let detail = format!(
            "{} · {}",
            apix_http::human_duration(Duration::from_millis(entry.duration_ms)),
            ago(entry.at)
        );
        CommandItem::new()
            .label(entry.url.clone())
            .child(move |_, _| {
                h_flex()
                    .dir_row()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w(px(58.))
                            .flex_none()
                            .text_xs()
                            .font_family(mono.clone())
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::method_color(entry.method))
                            .child(entry.method.as_str()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().child(entry.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(colors.text_muted)
                                    .font_family(mono.clone())
                                    .child(entry.url.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(status_color)
                            .child(status_text.clone()),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .flex_none()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(detail.clone()),
                    )
            })
    }
}

impl Render for HistoryView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let mono = cx.theme().mono_font_family.clone();
        let this = cx.entity().downgrade();
        let confirm = this.clone();
        let clear = this.clone();
        let workspace = self.workspace.clone();
        let cancel_workspace = self.workspace.clone();
        let matching = self.matching();
        let empty = if !self.enabled {
            tr!("history.off")
        } else if self.entries.is_empty() {
            tr!("history.empty")
        } else {
            tr!("palette.empty")
        };
        let mut command = Command::new(&self.state)
            .filterable(false)
            .bordered(false)
            .max_h(px(440.))
            .placeholder(tr!("history.placeholder"))
            .on_query(move |query, _, cx| {
                let query = query.to_owned();
                this.update(cx, |view, cx| {
                    view.query = query;
                    cx.notify();
                })
                .ok();
            })
            .on_confirm(move |ix, window, cx| {
                let id = confirm
                    .read_with(cx, |view, _| {
                        view.matching().get(ix.row).map(|e| e.id.clone())
                    })
                    .ok()
                    .flatten();
                super::collections::close_dialog(&workspace, window, cx);
                if let Some(id) = id {
                    workspace
                        .update(cx, |ws, cx| ws.open_history_entry(&id, window, cx))
                        .ok();
                }
            })
            .on_cancel(move |window, cx| {
                super::collections::close_dialog(&cancel_workspace, window, cx)
            })
            .empty(move |_, _, _| {
                div()
                    .p_4()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .child(bidi::para(empty.clone()))
            });
        if !matching.is_empty() {
            command = command.group(
                CommandGroup::new().items(matching.iter().map(|e| self.item(e, mono.clone()))),
            );
        }
        v_flex().w_full().gap_2().child(command).child(
            h_flex()
                .dir_row()
                .gap_2()
                .items_center()
                .px_2()
                .pb_1()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(bidi::para(tr!(
                            "history.note",
                            count = history::MAX_ENTRIES
                        ))),
                )
                .children((!self.entries.is_empty()).then(|| {
                    Button::new("history-clear")
                        .ghost()
                        .small()
                        .label(tr!("history.clear"))
                        .on_click(move |_, _, cx| {
                            if let Some(store) = store()
                                && let Err(e) = store.clear()
                            {
                                eprintln!("apix: could not clear the history: {e}");
                            }
                            clear
                                .update(cx, |view, cx| {
                                    view.entries.clear();
                                    cx.notify();
                                })
                                .ok();
                        })
                })),
        )
    }
}

impl Workspace {
    /// What `send` hands to the background to record, unless history is off
    /// or the request belongs to an encrypted collection (whose contents must
    /// never be written anywhere in the clear).
    pub(super) fn pending_record(&self, draft: &Request) -> Option<PendingRecord> {
        if !self.settings.history || crate::bench::mode().is_some() {
            return None;
        }
        let encrypted = match &self.selected {
            Some(path) => vfs::encrypted_root(path).is_some(),
            None => self
                .collection
                .as_ref()
                .is_some_and(|c| vfs::encrypted_root(&c.root).is_some()),
        };
        if encrypted {
            return None;
        }
        Some(PendingRecord {
            history: store()?,
            request: draft.clone(),
            path: self.selected.clone(),
            at: history::now_ms(),
            started: Instant::now(),
        })
    }

    pub(super) fn open_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entries = store().map(|s| s.list()).unwrap_or_default();
        let workspace = cx.entity().downgrade();
        let colors = crate::theme::current();
        let enabled = self.settings.history;
        let view = cx.new(|cx| HistoryView {
            state: cx.new(|cx| CommandState::new(window, cx)),
            workspace,
            colors,
            entries,
            query: String::new(),
            enabled,
        });
        let state = view.read(cx).state.clone();
        let refocus = cx.entity().downgrade();
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                state.update(cx, |state, cx| state.focus(window, cx));
            }
            dialog
                .on_close(super::collections::refocus(refocus.clone()))
                .bidi_title(tr!("history.title"))
                .w(px(760.))
                .text_start()
                .on_ok(|_, _, _| false)
                .child(view.clone())
        });
    }

    /// Puts a history entry back in the editor: in the tab of the request it
    /// was sent from (asking first if that tab has unsaved edits), or, when
    /// that file is gone, in the editor with no tab (Ctrl+S saves it anew).
    pub(super) fn open_history_entry(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = store().and_then(|s| s.load(id)) else {
            self.notify_error(tr!("history.missing"), window, cx);
            return;
        };
        let entry = Rc::new(entry);
        match entry
            .summary
            .path
            .clone()
            .filter(|p| self.requests.contains_key(p))
        {
            Some(path) => {
                self.open_tab(path, window, cx);
                // Edits that are exactly this entry lose nothing.
                if self.build_request(cx) == entry.request {
                    return self.show_history_entry(&entry, window, cx);
                }
                self.guard_unsaved(
                    window,
                    cx,
                    Rc::new(move |this, window, cx| this.show_history_entry(&entry, window, cx)),
                );
            }
            None => {
                self.park_front(cx);
                self.selected = None;
                self.show_request(Request::default(), window, cx);
                self.show_history_entry(&entry, window, cx);
            }
        }
    }

    fn show_history_entry(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.fill_editor(&entry.request, window, cx);
        self.task = None;
        let mut message = tr!("history.restored", when = ago(entry.summary.at)).to_string();
        match &entry.response {
            Some(stored) => {
                if stored.truncated {
                    message.push(' ');
                    message.push_str(&tr!("history.truncated"));
                }
                let response = Response::from_parts(
                    stored.status,
                    stored.version.clone(),
                    stored.headers.clone(),
                    entry.body.clone(),
                    Timing {
                        ttfb: Duration::from_micros(stored.ttfb_us),
                        total: Duration::from_micros(stored.total_us),
                    },
                );
                self.finish(Ok(Finished::prepare(response)), window, cx);
            }
            None => {
                self.raw_doc = None;
                self.response =
                    ResponseState::Failed(entry.summary.error.clone().unwrap_or_default());
            }
        }
        message.push(' ');
        message.push_str(&tr!("history.send_again", key = key_hint("Enter")));
        window.push_notification(Notification::info(message), cx);
        cx.notify();
    }
}
