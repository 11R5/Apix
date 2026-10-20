//! The command palette (Ctrl+K or Ctrl+P): jump to any request or run any
//! command by typing a few of its letters. Matching is fuzzy: the letters
//! must appear in order, and matches at word starts or in a row rank higher,
//! so "crus" finds "Create user".

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use apix_codegen::Shell;
use apix_core::{GroupBy, Method};
use gpui_kit::component::command::{Command, CommandGroup, CommandItem, CommandState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, WindowExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use super::{Workspace, key_hint};
use crate::bidi::Bidi as _;
use crate::theme;
use crate::tr;

/// A command's name from the locale key, as `(current language, English)`:
/// it shows in the current language and is also found by its English name.
/// `name!("palette.copy_as", format = label)` for `%{format}`.
macro_rules! name {
    ($key:tt $(, $arg:ident = $value:expr)* $(,)?) => {
        (
            tr!($key $(, $arg = $value)*),
            tr!($key, locale = "en" $(, $arg = $value)*),
        )
    };
}

type Run = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

/// Requests shown at most, so a huge collection stays quick to filter.
const MAX_REQUESTS: usize = 40;

#[derive(Clone)]
struct Entry {
    label: String,
    /// A command's English name, also searched when the UI is in another
    /// language; empty when it is the label.
    english: String,
    /// Muted text after the label: where a request lives, or a path.
    detail: String,
    method: Option<Method>,
    /// Shortcut hint, e.g. "Ctrl+S".
    keys: Option<String>,
    checked: bool,
    run: Run,
}

impl Entry {
    /// A command named by [`name!`].
    fn command((label, english): (SharedString, SharedString), run: Run) -> Self {
        let english = if english == label {
            String::new()
        } else {
            english.to_string()
        };
        Self {
            label: label.to_string(),
            english,
            detail: String::new(),
            method: None,
            keys: None,
            checked: false,
            run,
        }
    }

    fn keys(mut self, key: &str) -> Self {
        self.keys = Some(key_hint(key));
        self
    }

    /// The best match of `query` in the label, or (ranked lower) in the
    /// English name, or in the detail and label together.
    fn score(&self, query: &str) -> Option<i32> {
        let in_label = fuzzy(query, &self.label);
        let in_english = if self.english.is_empty() {
            None
        } else {
            fuzzy(query, &self.english).map(|s| s - 2)
        };
        let in_both = fuzzy(query, &format!("{} {}", self.detail, self.label)).map(|s| s - 6);
        in_label.max(in_english).max(in_both)
    }
}

/// How well `query` matches `text`, or `None` when its letters (spaces
/// ignored) do not all appear in order. Higher is better.
fn fuzzy(query: &str, text: &str) -> Option<i32> {
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let text: Vec<char> = text.chars().map(lower).collect();
    let mut score = 0;
    let mut at = 0;
    let mut previous: Option<usize> = None;
    for wanted in query.chars().filter(|c| !c.is_whitespace()).map(lower) {
        while at < text.len() && text[at] != wanted {
            at += 1;
        }
        if at == text.len() {
            return None;
        }
        score += 1;
        if at == 0 || !text[at - 1].is_alphanumeric() {
            score += 8;
        }
        match previous {
            Some(p) if p + 1 == at => score += 5,
            Some(p) => score -= (at - p - 1).min(8) as i32,
            None => score -= at.min(8) as i32,
        }
        previous = Some(at);
        at += 1;
    }
    Some(score - (text.len() / 24) as i32)
}

pub(super) struct CommandPalette {
    state: Entity<CommandState>,
    workspace: WeakEntity<Workspace>,
    colors: theme::Palette,
    requests: Vec<Entry>,
    commands: Vec<Entry>,
    query: String,
}

impl CommandPalette {
    /// The sections to show for the current query, best match first.
    fn sections(&self) -> Vec<(SharedString, Vec<&Entry>)> {
        [
            (
                tr!("palette.group.requests"),
                filter(&self.requests, &self.query, MAX_REQUESTS),
            ),
            (
                tr!("palette.group.commands"),
                filter(&self.commands, &self.query, usize::MAX),
            ),
        ]
        .into_iter()
        .filter(|(_, entries)| !entries.is_empty())
        .collect()
    }

    fn run_at(&self, ix: IndexPath) -> Option<Run> {
        let sections = self.sections();
        let (_, entries) = sections.get(ix.section)?;
        entries.get(ix.row).map(|e| e.run.clone())
    }
}

/// The entries matching `query`, best first (all of them, in order, when it is empty).
fn filter<'a>(entries: &'a [Entry], query: &str, limit: usize) -> Vec<&'a Entry> {
    if query.trim().is_empty() {
        return entries.iter().take(limit).collect();
    }
    let mut scored: Vec<(i32, &Entry)> = entries
        .iter()
        .filter_map(|e| e.score(query).map(|s| (s, e)))
        .collect();
    // Stable, so equal scores keep their original order.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().take(limit).map(|(_, e)| e).collect()
}

fn item(entry: &Entry, colors: theme::Palette, mono: SharedString) -> CommandItem {
    let entry = entry.clone();
    let keywords = [entry.detail.clone()];
    CommandItem::new()
        .label(entry.label.clone())
        .keywords(keywords)
        .checked(entry.checked)
        .child(move |_, _| {
            h_flex()
                .dir_row()
                .w_full()
                .gap_2()
                .items_center()
                .when_some(entry.method, |row, method| {
                    row.child(
                        div()
                            .w(px(58.))
                            .flex_none()
                            .text_xs()
                            .font_family(mono.clone())
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::method_color(method))
                            .child(method.as_str()),
                    )
                })
                .child(div().flex_none().text_sm().child(entry.label.clone()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(entry.detail.clone()),
                )
                .when_some(entry.keys.clone(), |row, keys| {
                    row.child(
                        div()
                            .flex_none()
                            .px_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(colors.hairline)
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(keys),
                    )
                })
        })
}

impl Render for CommandPalette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors;
        let mono = cx.theme().mono_font_family.clone();
        let this = cx.entity().downgrade();
        let confirm = this.clone();
        let workspace = self.workspace.clone();
        let cancel_workspace = self.workspace.clone();
        let mut command = Command::new(&self.state)
            .filterable(false)
            .bordered(false)
            .max_h(px(440.))
            .placeholder(tr!("palette.placeholder"))
            .on_query(move |query, _, cx| {
                let query = query.to_owned();
                this.update(cx, |palette, cx| {
                    palette.query = query;
                    cx.notify();
                })
                .ok();
            })
            .on_confirm(move |ix, window, cx| {
                let run = confirm
                    .read_with(cx, |palette, _| palette.run_at(ix))
                    .ok()
                    .flatten();
                // Close first, so commands that open a dialog of their own can.
                super::collections::close_dialog(&workspace, window, cx);
                if let Some(run) = run {
                    workspace.update(cx, |ws, cx| run(ws, window, cx)).ok();
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
                    .child(tr!("palette.empty"))
            });
        for (label, entries) in self.sections() {
            command = command.group(
                CommandGroup::new()
                    .label(label)
                    .items(entries.into_iter().map(|e| item(e, colors, mono.clone()))),
            );
        }
        div().w_full().child(command)
    }
}

impl Workspace {
    pub(super) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let requests = self.palette_requests();
        let commands = self.palette_commands();
        let workspace = cx.entity().downgrade();
        let colors = self.palette;
        let palette = cx.new(|cx| CommandPalette {
            state: cx.new(|cx| CommandState::new(window, cx)),
            workspace,
            colors,
            requests,
            commands,
            query: String::new(),
        });
        let state = palette.read(cx).state.clone();
        let view = cx.entity().downgrade();
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                state.update(cx, |state, cx| state.focus(window, cx));
            }
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .w(px(680.))
                .close_button(false)
                .text_start()
                .on_ok(|_, _, _| false)
                .child(palette.clone())
        });
    }

    fn palette_requests(&self) -> Vec<Entry> {
        let root = self.workspace_root();
        self.requests
            .iter()
            .map(|(path, request)| {
                let open = path.clone();
                Entry {
                    label: request.name.clone(),
                    english: String::new(),
                    detail: self.location(path, root.as_deref()),
                    method: Some(request.method),
                    keys: None,
                    checked: self.selected.as_ref() == Some(path),
                    run: Rc::new(move |ws, window, cx| ws.select(open.clone(), window, cx)),
                }
            })
            .collect()
    }

    /// "Users / admin" for a request in `users/admin/`: its collection's name
    /// and the folders below it.
    fn location(&self, path: &Path, root: Option<&Path>) -> String {
        let collection = self.collection_for(path);
        let base = collection.map(|c| c.root.as_path()).or(root);
        let folders = path
            .parent()
            .and_then(|dir| base.and_then(|b| dir.strip_prefix(b).ok()))
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut parts: Vec<String> = Vec::new();
        if self.project.is_some()
            && let Some(c) = collection
        {
            parts.push(c.meta.name.clone());
        }
        parts.extend(folders);
        parts.join(" / ")
    }

    fn palette_commands(&self) -> Vec<Entry> {
        let run =
            |f: fn(&mut Workspace, &mut Window, &mut Context<Workspace>)| -> Run { Rc::new(f) };
        let open = self.workspace_root().is_some();
        let mut out = Vec::new();
        if open {
            out.push(
                Entry::command(name!("palette.send_request"), run(Workspace::send)).keys("Enter"),
            );
            out.push(
                Entry::command(
                    name!("palette.save_request"),
                    run(|ws, window, cx| {
                        ws.save(window, cx);
                    }),
                )
                .keys("S"),
            );
            if !self.tabs.is_empty() {
                out.push(
                    Entry::command(name!("menu.close_tab"), run(Workspace::close_front_tab))
                        .keys("W"),
                );
            }
            out.push(
                Entry::command(
                    name!("menu.new_request"),
                    run(|ws, window, cx| ws.new_request(None, Method::Get, window, cx)),
                )
                .keys("N"),
            );
            for method in Method::ALL.into_iter().skip(1) {
                out.push(Entry::command(
                    name!("palette.new_method_request", method = method.as_str()),
                    Rc::new(move |ws, window, cx| ws.new_request(None, method, window, cx)),
                ));
            }
            out.push(Entry::command(
                name!("menu.new_folder"),
                run(|ws, window, cx| ws.new_folder(None, window, cx)),
            ));
        }
        out.push(Entry::command(
            name!("menu.new_collection"),
            run(Workspace::prompt_new_collection),
        ));
        out.push(Entry::command(
            name!("menu.new_project"),
            run(Workspace::prompt_new_project),
        ));
        out.push(
            Entry::command(
                name!("palette.open_folder"),
                run(Workspace::prompt_open_collection),
            )
            .keys("O"),
        );
        out.push(Entry::command(
            name!("menu.import_postman"),
            run(Workspace::prompt_import),
        ));
        if !open {
            out.extend(self.palette_recent());
            return out;
        }
        out.push(
            Entry::command(name!("menu.variables"), run(Workspace::open_env_editor)).keys("E"),
        );
        let current = self.environment.clone();
        out.push(Entry {
            checked: current.is_none(),
            ..Entry::command(
                name!("palette.environment_none"),
                run(|ws, window, cx| ws.set_environment(None, window, cx)),
            )
        });
        for name in self.environment_names() {
            let pick = name.clone();
            out.push(Entry {
                checked: current.as_deref() == Some(name.as_str()),
                ..Entry::command(
                    name!("palette.environment", name = name),
                    Rc::new(move |ws, window, cx| {
                        ws.set_environment(Some(pick.clone()), window, cx)
                    }),
                )
            });
        }
        for shell in Shell::ALL {
            out.push(Entry::command(
                name!("palette.copy_as", format = shell.label()),
                Rc::new(move |ws, window, cx| ws.copy_current_as(shell, window, cx)),
            ));
        }
        let owner = match (&self.project, self.collections.first()) {
            (Some(p), _) => Some((
                name!("menu.project_auth"),
                AuthOwner::Project(p.root.clone()),
            )),
            (None, Some(c)) => Some((
                name!("menu.collection_auth"),
                AuthOwner::Collection(c.root.clone()),
            )),
            (None, None) => None,
        };
        if let Some((label, owner)) = owner {
            let settings_label = match &owner {
                AuthOwner::Project(_) => name!("menu.project_settings"),
                _ => name!("menu.collection_settings"),
            };
            let settings_owner = owner.clone();
            out.push(Entry::command(
                label,
                Rc::new(move |ws, window, cx| ws.open_auth_dialog(owner.clone(), window, cx)),
            ));
            out.push(Entry::command(
                settings_label,
                Rc::new(move |ws, window, cx| {
                    ws.open_settings_dialog(settings_owner.clone(), window, cx)
                }),
            ));
        }
        out.push(Entry::command(
            name!("menu.cookies"),
            Rc::new(move |ws, window, cx| ws.open_cookies(window, cx)),
        ));
        if let Some(project) = &self.project {
            for (label, by) in [
                (name!("palette.group_by_collection"), GroupBy::Collection),
                (name!("palette.group_by_environment"), GroupBy::Environment),
            ] {
                out.push(Entry {
                    checked: project.meta.group_by == by,
                    ..Entry::command(
                        label,
                        Rc::new(move |ws, window, cx| ws.set_group_by(by, window, cx)),
                    )
                });
            }
        }
        out.extend(self.palette_recent());
        out
    }

    fn palette_recent(&self) -> Vec<Entry> {
        let current = self.workspace_root();
        self.app_state
            .recent_collections()
            .filter(|dir| Some(*dir) != current.as_ref())
            .take(8)
            .map(|dir| {
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| dir.display().to_string());
                let target: PathBuf = dir.clone();
                Entry {
                    detail: dir.display().to_string(),
                    ..Entry::command(
                        name!("palette.open_recent", name = name),
                        Rc::new(move |ws, window, cx| {
                            ws.open_or_create(target.clone(), window, cx)
                        }),
                    )
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: gpui's `test` macro would shadow the built-in `#[test]`.
    use super::fuzzy;

    #[test]
    fn fuzzy_needs_every_letter_in_order() {
        assert!(fuzzy("crus", "Create user").is_some());
        assert!(fuzzy("cu", "Create user").is_some());
        assert!(fuzzy("uc", "Create user").is_none());
        assert!(fuzzy("", "anything").is_some());
        assert!(fuzzy("ŞUBE", "İstanbul şube").is_some());
    }

    #[test]
    fn fuzzy_prefers_word_starts_and_runs() {
        let rank = |q: &str, a: &str, b: &str| fuzzy(q, a) > fuzzy(q, b);
        assert!(rank("user", "Get user", "Superuser report"));
        assert!(rank("nr", "New request", "Open recent"));
        assert!(rank("env", "Environment: test", "Send everything now"));
    }
}
