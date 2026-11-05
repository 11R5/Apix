//! The Git window (the branch in the status bar, or View › Git…): what
//! changed in the open folder since the last commit, each file compared field
//! by field like `apix diff`, and committing it, switching or making a
//! branch, pulling and pushing. Git itself does the work (`apix_core::git`),
//! so the user's own settings, hooks and credentials apply.

use std::path::PathBuf;

use apix_core::diff::{self, FieldChange, FileDiff, Line};
use apix_core::git::{Change, ChangeKind, Repo};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::tr;

impl Workspace {
    pub(super) fn open_git(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.workspace_root() else {
            return self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
        };
        let Some(repo) = Repo::at(&base) else {
            return self.notify_error(tr!("git.not_a_repo"), window, cx);
        };
        let workspace = cx.entity().downgrade();
        let view = cx.new(|cx| GitView::new(workspace.clone(), base, repo, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("git.title"))
                .width(px(980.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

pub(super) struct GitView {
    workspace: WeakEntity<Workspace>,
    base: PathBuf,
    repo: Repo,
    branch: String,
    branches: Vec<String>,
    changes: Vec<Change>,
    log: Vec<apix_core::git::Commit>,
    selected: Option<usize>,
    diff: Option<Result<Option<FileDiff>, String>>,
    message: Entity<InputState>,
    new_branch: Entity<InputState>,
    /// What runs now (a pull, a push …), for the buttons.
    busy: Option<SharedString>,
    _task: Option<Task<()>>,
}

impl GitView {
    fn new(
        workspace: WeakEntity<Workspace>,
        base: PathBuf,
        repo: Repo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let message =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("git.message_placeholder")));
        let new_branch =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("git.new_branch_placeholder")));
        let mut view = Self {
            workspace,
            base,
            repo,
            branch: String::new(),
            branches: Vec::new(),
            changes: Vec::new(),
            log: Vec::new(),
            selected: None,
            diff: None,
            message,
            new_branch,
            busy: None,
            _task: None,
        };
        view.read();
        view
    }

    /// Reads branch, changes and log (quick: a few git calls).
    fn read(&mut self) {
        self.branch = self.repo.branch().unwrap_or_default();
        self.branches = self.repo.branches().unwrap_or_default();
        self.changes = self.repo.working_changes().unwrap_or_default();
        self.log = self.repo.log(6).unwrap_or_default();
        if self.selected.is_some_and(|i| i >= self.changes.len()) {
            self.selected = None;
            self.diff = None;
        }
    }

    /// Reads again after an operation; the status bar and sidebar too.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.read();
        // The list may be different now.
        self.selected = None;
        self.diff = None;
        // The status bar and the sidebar marks follow.
        self.workspace
            .update(cx, |ws, cx| ws.refresh_git_now(cx))
            .ok();
        cx.notify();
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(change) = self.changes.get(ix).cloned() else {
            return;
        };
        self.selected = Some(ix);
        self.diff = None;
        let (repo, base) = (self.repo.clone(), self.base.clone());
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let old = if change.kind == ChangeKind::Added {
                        None
                    } else {
                        repo.show("HEAD", &change.path).map_err(|e| e.to_string())?
                    };
                    let new = std::fs::read(base.join(&change.path)).ok();
                    Ok(diff::file(&change.path, old.as_deref(), new.as_deref()))
                })
                .await;
            this.update(cx, |this, cx| {
                this.diff = Some(result);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Runs a git operation off the UI thread and reports how it went.
    fn run(
        &mut self,
        label: SharedString,
        work: impl FnOnce(Repo) -> Result<String, String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(label);
        let repo = self.repo.clone();
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { work(repo) }).await;
            this.update_in(cx, |this, window, cx| {
                this.busy = None;
                match result {
                    Ok(text) if !text.trim().is_empty() => {
                        window.push_notification(Notification::success(text), cx)
                    }
                    Ok(_) => {}
                    Err(e) => window.push_notification(Notification::error(e), cx),
                }
                this.refresh(cx);
            })
            .ok();
        }));
        cx.notify();
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.message.read(cx).value().trim().to_string();
        if message.is_empty() {
            window.push_notification(Notification::warning(tr!("git.message_needed")), cx);
            return;
        }
        self.message
            .update(cx, |state, cx| state.set_value("", window, cx));
        let done = tr!("git.committed").to_string();
        self.run(
            tr!("git.commit"),
            move |repo| {
                repo.commit_folder(&message)
                    .map(|hash| format!("{done} {hash}"))
                    .map_err(|e| e.to_string())
            },
            window,
            cx,
        );
    }

    fn render_diff(&self) -> AnyElement {
        let p = crate::theme::current();
        let Some(diff) = &self.diff else {
            let text = if self.selected.is_some() {
                tr!("git.reading")
            } else {
                tr!("git.pick_file")
            };
            return div()
                .text_sm()
                .text_color(p.text_muted)
                .child(text)
                .into_any_element();
        };
        let file = match diff {
            Err(e) => {
                return div()
                    .text_sm()
                    .text_color(p.err)
                    .child(e.clone())
                    .into_any_element();
            }
            Ok(None) => {
                return div()
                    .text_sm()
                    .text_color(p.text_muted)
                    .child(tr!("git.same_meaning"))
                    .into_any_element();
            }
            Ok(Some(file)) => file,
        };
        let mono = |text: String, colour: Hsla| {
            div()
                .text_xs()
                .font_family(crate::fonts::MONO)
                .text_color(colour)
                .child(text)
        };
        let title = match (&file.method, &file.name) {
            (Some(m), Some(n)) => format!("{m} {n}"),
            _ => file.path.clone(),
        };
        let mut body = v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .children(file.note.clone().map(|n| mono(n, p.text_muted)));
        if matches!(file.status, diff::Status::Added | diff::Status::Removed)
            && file.fields.is_empty()
            && file.lines.is_empty()
        {
            body = body.child(div().text_xs().text_color(p.text_muted).child(
                if file.status == diff::Status::Added {
                    tr!("git.new_file")
                } else {
                    tr!("git.deleted_file")
                },
            ));
        }
        for field in &file.fields {
            body = body.child(field_row(field));
        }
        for line in context(&file.lines) {
            body = body.child(match line {
                None => mono("…".into(), p.text_muted),
                Some(Line::Same(t)) => mono(format!("  {t}"), p.text_muted),
                Some(Line::Removed(t)) => mono(format!("- {t}"), p.err),
                Some(Line::Added(t)) => mono(format!("+ {t}"), p.ok),
            });
        }
        body.into_any_element()
    }
}

/// Changed lines with two lines around them; `None` for lines left out.
pub(super) fn context(lines: &[Line]) -> Vec<Option<&Line>> {
    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !matches!(l, Line::Same(_)))
        .map(|(i, _)| i)
        .collect();
    let mut out = Vec::new();
    let mut skipped = false;
    for (i, line) in lines.iter().enumerate() {
        if changed.iter().any(|&c| i + 2 >= c && i <= c + 2) {
            if skipped && !out.is_empty() {
                out.push(None);
            }
            skipped = false;
            out.push(Some(line));
        } else {
            skipped = true;
        }
    }
    out
}

pub(super) fn field_row(field: &FieldChange) -> AnyElement {
    let p = crate::theme::current();
    let mono = |text: String, colour: Hsla| {
        div()
            .text_xs()
            .font_family(crate::fonts::MONO)
            .text_color(colour)
            .child(text)
    };
    if let Some((old, new)) = field.multiline() {
        let lines = diff::lines(old, new);
        return v_flex()
            .pl_2()
            .child(mono(format!("{}:", field.field), p.text))
            .children(context(&lines).into_iter().map(|l| match l {
                None => mono("  …".into(), p.text_muted),
                Some(Line::Same(t)) => mono(format!("    {t}"), p.text_muted),
                Some(Line::Removed(t)) => mono(format!("  - {t}"), p.err),
                Some(Line::Added(t)) => mono(format!("  + {t}"), p.ok),
            }))
            .into_any_element();
    }
    let show = |v: &Option<serde_json::Value>| {
        v.as_ref()
            .map(diff::show)
            .unwrap_or_else(|| tr!("git.none").into())
    };
    h_flex()
        .pl_2()
        .gap_2()
        .flex_wrap()
        .child(mono(format!("{}:", field.field), p.text))
        .child(mono(show(&field.old), p.err))
        .child(mono("→".into(), p.text_muted))
        .child(mono(show(&field.new), p.ok))
        .into_any_element()
}

impl Render for GitView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = window;
        let p = crate::theme::current();
        let busy = self.busy.clone();
        let view = cx.entity().downgrade();

        let branch_picker = Button::new("git-branch")
            .small()
            .outline()
            .icon(IconName::ChevronDown)
            .label(format!("\u{2387} {}", self.branch))
            .disabled(busy.is_some())
            .dropdown_menu({
                let (branches, current, view) =
                    (self.branches.clone(), self.branch.clone(), view.clone());
                move |menu, _, _| {
                    branches.iter().fold(menu, |menu, name| {
                        let (view, name) = (view.clone(), name.clone());
                        menu.item(
                            PopupMenuItem::new(name.clone())
                                .checked(name == current)
                                .on_click(move |_, window, cx| {
                                    let name = name.clone();
                                    view.update(cx, |this, cx| {
                                        this.run(
                                            tr!("git.switching"),
                                            move |repo| {
                                                repo.switch(&name)
                                                    .map(|_| String::new())
                                                    .map_err(|e| e.to_string())
                                            },
                                            window,
                                            cx,
                                        )
                                    })
                                    .ok();
                                }),
                        )
                    })
                }
            });
        let toolbar = h_flex()
            .dir_row()
            .gap_2()
            .items_center()
            .child(branch_picker)
            .child(
                div()
                    .w(px(200.))
                    .child(Input::new(&self.new_branch).small()),
            )
            .child(
                Button::new("git-new-branch")
                    .small()
                    .outline()
                    .icon(IconName::Plus)
                    .label(tr!("git.new_branch"))
                    .disabled(busy.is_some())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let name = this.new_branch.read(cx).value().trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        this.new_branch
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        this.run(
                            tr!("git.new_branch"),
                            move |repo| {
                                repo.create_branch(&name)
                                    .map(|_| String::new())
                                    .map_err(|e| e.to_string())
                            },
                            window,
                            cx,
                        );
                    })),
            )
            .child(div().flex_1())
            .children(
                busy.clone()
                    .map(|b| div().text_xs().text_color(p.text_muted).child(b)),
            )
            .child(
                Button::new("git-pull")
                    .small()
                    .outline()
                    .icon(IconName::ArrowDown)
                    .label(tr!("git.pull"))
                    .disabled(busy.is_some())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let (done, same) = (
                            tr!("git.pulled").to_string(),
                            tr!("git.up_to_date").to_string(),
                        );
                        this.run(
                            tr!("git.pull"),
                            move |repo| {
                                repo.pull()
                                    .map(|out| {
                                        if out.contains("Already up to date") {
                                            same
                                        } else {
                                            done
                                        }
                                    })
                                    .map_err(|e| e.to_string())
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("git-push")
                    .small()
                    .outline()
                    .icon(IconName::ArrowUp)
                    .label(tr!("git.push"))
                    .disabled(busy.is_some())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        let done = tr!("git.pushed", branch = this.branch.clone()).to_string();
                        this.run(
                            tr!("git.push"),
                            move |repo| repo.push().map(|_| done).map_err(|e| e.to_string()),
                            window,
                            cx,
                        )
                    })),
            );

        let files = self.changes.iter().enumerate().map(|(i, change)| {
            let (colour, letter) = super::git_ui::change_mark(change.kind);
            h_flex()
                .id(("git-file", i))
                .dir_row()
                .gap_2()
                .px_2()
                .py(px(3.))
                .rounded_md()
                .cursor_pointer()
                .when(self.selected == Some(i), |d| d.bg(p.accent.opacity(0.14)))
                .hover(|s| s.bg(p.hover))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(i, cx)))
                .child(
                    div()
                        .w(px(12.))
                        .flex_none()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(colour)
                        .child(letter.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .child(apix_core::git::slashes(&change.path)),
                )
        });
        let left = v_flex()
            .w(px(320.))
            .flex_none()
            .gap_1()
            .when(!self.changes.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(tr!("git.changes_count", count = self.changes.len())),
                )
            })
            .child(
                div()
                    .id("git-files")
                    .h(px(300.))
                    .overflow_y_scroll()
                    .children(files)
                    .when(self.changes.is_empty(), |d| {
                        d.child(
                            div()
                                .text_sm()
                                .text_color(p.text_muted)
                                .child(tr!("git.clean")),
                        )
                    }),
            )
            .child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("git.recent")),
            )
            .children(self.log.iter().map(|commit| {
                h_flex()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .flex_none()
                            .font_family(crate::fonts::MONO)
                            .text_color(p.accent)
                            .child(commit.hash.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(commit.subject.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(p.text_muted)
                            .child(super::history_ui::ago(commit.time * 1000)),
                    )
            }));
        let right = div()
            .id("git-diff")
            .flex_1()
            .min_w_0()
            .h(px(420.))
            .overflow_y_scroll()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(p.hairline)
            .child(self.render_diff());

        let commit = h_flex()
            .dir_row()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(Input::new(&self.message).small()))
            .child(
                Button::new("git-commit")
                    .primary()
                    .small()
                    .label(tr!("git.commit"))
                    .disabled(busy.is_some() || self.changes.is_empty())
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.commit(window, cx)),
                    ),
            );

        v_flex()
            .gap_3()
            .child(toolbar)
            .child(
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_start()
                    .child(left)
                    .child(right),
            )
            .child(commit)
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("git.commit_note"))),
            )
    }
}
