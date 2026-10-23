//! The variables dialog: edit shared variables (`[vars]` of the project and
//! its collections) and environments without leaving Apix. Secrets go to
//! `<name>.local.toml`, which git ignores, or to the system keychain when
//! Settings say so. Nothing is written until Save,
//! so Cancel (or Esc) throws every change away, renames and deletes included.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use apix_core::dynamic::{self, DYNAMIC_VARIABLES};
use apix_core::store::{self, EnvVar, EnvironmentParts, StoreError, VarMap};
use apix_core::{Collection, Project};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Workspace, describe};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::theme::Palette;
use crate::tr;

/// What one page of the dialog edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    /// `[vars]` in `apix-project.toml`.
    ProjectVars(PathBuf),
    /// `[vars]` in a collection's `apix.toml`.
    CollectionVars(PathBuf),
    /// `environments/<name>.toml` and its secrets under `root`.
    Environment { root: PathBuf, name: String },
    /// The read-only list of `{{$...}}` variables.
    Dynamic,
}

/// A page as the workspace describes it: what it edits, its name in the
/// list, and the heading it sits under.
pub(super) struct PageSpec {
    pub target: Target,
    pub label: String,
    pub section: String,
}

struct Row {
    key: Entity<InputState>,
    value: Entity<InputState>,
    secret: bool,
}

struct Page {
    target: Target,
    label: String,
    section: String,
    /// Loaded when the page is first opened.
    rows: Option<Vec<Row>>,
    /// The rows as read from disk, to tell whether anything changed.
    original: Vec<EnvVar>,
    /// The files as read, so a secret keeps its placeholder in the shared file.
    parts: EnvironmentParts,
    /// Environments only: the name field, for renaming.
    name: Option<Entity<InputState>>,
    is_new: bool,
    deleted: bool,
}

impl Page {
    fn env_name(&self) -> Option<(&Path, &str)> {
        match &self.target {
            Target::Environment { root, name } => Some((root, name)),
            _ => None,
        }
    }

    /// The name the environment will have after Save.
    fn new_name(&self, cx: &App) -> Option<String> {
        let input = self.name.as_ref()?;
        Some(store::slug(input.read(cx).value().trim(), ""))
    }

    fn renamed(&self, cx: &App) -> bool {
        match (self.env_name(), self.new_name(cx)) {
            (Some((_, old)), Some(new)) => !self.is_new && old != new,
            _ => false,
        }
    }

    /// The rows with a key, as they would be saved.
    fn current(&self, cx: &App) -> Vec<EnvVar> {
        let Some(rows) = &self.rows else {
            return self.original.clone();
        };
        rows.iter()
            .map(|row| EnvVar {
                key: row.key.read(cx).value().trim().to_owned(),
                value: row.value.read(cx).value().to_string(),
                secret: row.secret,
            })
            .filter(|row| !row.key.is_empty())
            .collect()
    }

    fn dirty(&self, cx: &App) -> bool {
        self.is_new
            || self.deleted
            || self.renamed(cx)
            || (self.rows.is_some() && sorted(self.current(cx)) != self.original)
    }
}

fn sorted(mut rows: Vec<EnvVar>) -> Vec<EnvVar> {
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    rows
}

pub(super) struct EnvEditor {
    workspace: WeakEntity<Workspace>,
    palette: Palette,
    pages: Vec<Page>,
    current: usize,
    /// Where "New environment" puts the file: the project or collection root.
    new_root: PathBuf,
    new_env: Entity<InputState>,
    /// One sample value per dynamic variable, made when the dialog opens.
    samples: Vec<String>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl EnvEditor {
    pub(super) fn new(
        workspace: WeakEntity<Workspace>,
        palette: Palette,
        specs: Vec<PageSpec>,
        start: usize,
        new_root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let new_env = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("env.placeholder.new_environment"))
        });
        let subscription = cx.subscribe_in(
            &new_env,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.add_environment(window, cx);
                }
            },
        );
        let mut editor = Self {
            workspace,
            palette,
            pages: Vec::new(),
            current: 0,
            new_root,
            new_env,
            samples: DYNAMIC_VARIABLES
                .iter()
                .map(|(name, _)| dynamic::generate(name).unwrap_or_default())
                .collect(),
            error: None,
            _subscriptions: vec![subscription],
        };
        for spec in specs {
            editor.push_page(spec, false, window, cx);
        }
        editor.open_page(start.min(editor.pages.len().saturating_sub(1)), window, cx);
        editor
    }

    fn push_page(&mut self, spec: PageSpec, is_new: bool, window: &mut Window, cx: &mut App) {
        let name = match &spec.target {
            Target::Environment { name, .. } => {
                let name = name.clone();
                Some(cx.new(|cx| InputState::new(window, cx).default_value(name)))
            }
            _ => None,
        };
        self.pages.push(Page {
            target: spec.target,
            label: spec.label,
            section: spec.section,
            rows: None,
            original: Vec::new(),
            parts: EnvironmentParts::default(),
            name,
            is_new,
            deleted: false,
        });
    }

    fn open_page(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.pages.get_mut(ix) else {
            return;
        };
        self.current = ix;
        if page.rows.is_none() && page.target != Target::Dynamic {
            let parts = if page.is_new {
                Ok(EnvironmentParts::default())
            } else {
                read_parts(&page.target)
            };
            match parts {
                Ok(parts) => {
                    page.original = parts.rows();
                    let mut rows: Vec<Row> = page
                        .original
                        .iter()
                        .map(|var| new_row(var, window, cx))
                        .collect();
                    if rows.is_empty() {
                        rows.push(new_row(&empty_var(), window, cx));
                    }
                    page.rows = Some(rows);
                    page.parts = parts;
                }
                Err(e) => self.error = Some(describe(&e)),
            }
        }
        cx.notify();
    }

    fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let row = new_row(&empty_var(), window, cx);
        row.key.update(cx, |state, cx| state.focus(window, cx));
        if let Some(rows) = self.pages[self.current].rows.as_mut() {
            rows.push(row);
        }
        cx.notify();
    }

    fn add_environment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.new_env.read(cx).value().trim().to_owned();
        let name = store::slug(&title, "");
        if name.is_empty() {
            return;
        }
        let taken = self.pages.iter().any(|p| {
            p.env_name()
                .is_some_and(|(root, n)| root == self.new_root && n == name && !p.deleted)
        });
        if taken {
            self.error = Some(tr!("env.error.exists", name = name).to_string());
            cx.notify();
            return;
        }
        self.new_env
            .update(cx, |state, cx| state.set_value("", window, cx));
        let section = self
            .pages
            .iter()
            .find(
                |p| matches!(&p.target, Target::Environment { root, .. } if *root == self.new_root),
            )
            .map(|p| p.section.clone())
            .unwrap_or_else(|| tr!("env.section.environments").to_string());
        // After the last page of its section, so the list stays grouped.
        let at = self
            .pages
            .iter()
            .rposition(|p| p.section == section)
            .map_or(self.pages.len(), |ix| ix + 1);
        self.push_page(
            PageSpec {
                target: Target::Environment {
                    root: self.new_root.clone(),
                    name: name.clone(),
                },
                label: name,
                section,
            },
            true,
            window,
            cx,
        );
        let page = self.pages.pop().expect("just pushed");
        self.pages.insert(at, page);
        self.error = None;
        self.open_page(at, window, cx);
    }

    /// Checks every changed page before anything is written.
    fn validate(&self, cx: &App) -> Result<(), (usize, String)> {
        for (ix, page) in self.pages.iter().enumerate() {
            if page.deleted || !page.dirty(cx) {
                continue;
            }
            let mut seen = BTreeSet::new();
            for row in page.current(cx) {
                if !seen.insert(row.key.clone()) {
                    return Err((
                        ix,
                        tr!("env.error.defined_twice", key = row.key).to_string(),
                    ));
                }
            }
            if let (Some((root, _)), Some(new)) = (page.env_name(), page.new_name(cx)) {
                if new.is_empty() {
                    return Err((ix, tr!("env.error.needs_name").to_string()));
                }
                let clash = self.pages.iter().enumerate().any(|(other, p)| {
                    other != ix
                        && !p.deleted
                        && p.env_name().is_some_and(|(r, _)| r == root)
                        && p.new_name(cx).as_deref() == Some(new.as_str())
                });
                if clash {
                    return Err((ix, tr!("env.error.name_clash", name = new).to_string()));
                }
            }
        }
        Ok(())
    }

    /// Writes every change. Returns false (and shows why) if something failed.
    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Err((ix, message)) = self.validate(cx) {
            self.open_page(ix, window, cx);
            self.error = Some(message);
            cx.notify();
            return false;
        }
        let mut renamed = Vec::new();
        let mut failed = None;
        for ix in 0..self.pages.len() {
            let page = &self.pages[ix];
            if !page.dirty(cx) {
                continue;
            }
            match write_page(page, cx) {
                Ok(Some(rename)) => renamed.push(rename),
                Ok(None) => {}
                Err(message) => {
                    failed = Some((ix, message));
                    break;
                }
            }
        }
        self.workspace
            .update(cx, |ws, cx| {
                let active = ws.environment.clone();
                ws.reload(window, cx);
                // Keep the active environment active under its new name.
                for (old, new) in renamed {
                    if active.as_deref() == Some(old.as_str()) {
                        ws.set_environment(Some(new), window, cx);
                    }
                }
            })
            .ok();
        if let Some((ix, message)) = failed {
            self.open_page(ix, window, cx);
            self.error = Some(message);
            cx.notify();
            return false;
        }
        true
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mut items: Vec<AnyElement> = Vec::new();
        let mut section: Option<&str> = None;
        for (ix, page) in self.pages.iter().enumerate() {
            if section != Some(page.section.as_str()) {
                section = Some(page.section.as_str());
                items.push(
                    div()
                        .px_2()
                        .pt_3()
                        .pb_1()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(crate::i18n::upper(&page.section))
                        .into_any_element(),
                );
            }
            let active = ix == self.current;
            let label = page
                .new_name(cx)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| page.label.clone());
            items.push(
                div()
                    .id(("env-page", ix))
                    .flex()
                    .dir_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
                    .when(active, |row| row.bg(p.accent.opacity(0.12)))
                    .when(page.deleted, |row| {
                        row.line_through().text_color(p.text_muted)
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.open_page(ix, window, cx)
                    }))
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .when(page.dirty(cx), |row| {
                        row.child(div().text_color(p.warn).child("•"))
                    })
                    .into_any_element(),
            );
        }
        v_flex()
            .w(px(200.))
            .flex_none()
            .h_full()
            .border_e_1()
            .border_color(p.hairline)
            .pe(px(8.))
            .child(
                div()
                    .id("env-pages")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(items),
            )
            .child(
                h_flex()
                    .dir_row()
                    .mt_2()
                    .pt_2()
                    .gap_1()
                    .border_t_1()
                    .border_color(p.hairline)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.new_env).small()),
                    )
                    .child(
                        Button::new("env-add")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .tooltip(tr!("env.add_environment"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.add_environment(window, cx)
                            })),
                    ),
            )
    }

    fn render_header(&self, page: &Page, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let title = match &page.target {
            Target::ProjectVars(_) => tr!("env.title.project"),
            Target::CollectionVars(_) => tr!("env.title.collection", name = page.label),
            Target::Environment { .. } => tr!("env.title.environment"),
            Target::Dynamic => tr!("env.dynamic_variables"),
        };
        let ix = self.current;
        h_flex()
            .dir_row()
            .gap_2()
            .items_center()
            .pb_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text)
                    .child(title),
            )
            .when_some(page.name.as_ref(), |bar, name| {
                bar.child(div().w(px(220.)).child(Input::new(name).small()))
            })
            .child(div().flex_1())
            .when(page.name.is_some(), |bar| {
                let label = if page.deleted {
                    tr!("env.undo_delete")
                } else {
                    tr!("env.delete")
                };
                bar.child(
                    Button::new("env-delete")
                        .ghost()
                        .small()
                        .label(label)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let page = &mut this.pages[ix];
                            if page.is_new {
                                this.pages.remove(ix);
                                this.current = ix.saturating_sub(1);
                            } else {
                                page.deleted = !page.deleted;
                            }
                            cx.notify();
                        })),
                )
            })
    }

    fn render_rows(&self, page: &Page, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let can_be_secret = page.name.is_some();
        let rows = page.rows.iter().flatten().enumerate().map(|(ix, row)| {
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .w(px(180.))
                        .flex_none()
                        .child(Input::new(&row.key).small()),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&row.value)
                            .small()
                            .when(row.secret, |input| input.mask_toggle()),
                    ),
                )
                .when(can_be_secret, |line| {
                    line.child(
                        Checkbox::new(("env-secret", ix))
                            .label(tr!("env.secret"))
                            .checked(row.secret)
                            .tooltip(if apix_core::secrets::keychain_enabled() {
                                tr!("env.secret_tooltip_keychain")
                            } else {
                                tr!("env.secret_tooltip")
                            })
                            .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                                let page = &mut this.pages[this.current];
                                if let Some(row) = page.rows.as_mut().and_then(|r| r.get_mut(ix)) {
                                    row.secret = *checked;
                                    let masked = *checked;
                                    row.value.update(cx, |state, cx| {
                                        state.set_masked(masked, window, cx)
                                    });
                                }
                                cx.notify();
                            })),
                    )
                })
                .child(
                    Button::new(("env-del", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let page = &mut this.pages[this.current];
                            if let Some(rows) = page.rows.as_mut().filter(|r| ix < r.len()) {
                                rows.remove(ix);
                            }
                            cx.notify();
                        })),
                )
        });
        v_flex()
            .id("env-rows")
            // Names and values are data: left to right in any language.
            .ltr()
            .flex_1()
            .min_h_0()
            .gap_1()
            .overflow_y_scroll()
            .when(page.deleted, |list| list.opacity(0.4))
            .child(
                h_flex()
                    .gap_2()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(
                        div()
                            .w(px(180.))
                            .flex_none()
                            .child(crate::i18n::upper(&tr!("table.key"))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(crate::i18n::upper(&tr!("table.value"))),
                    ),
            )
            .children(rows)
            .child(
                h_flex().pt_1().child(
                    Button::new("env-add-row")
                        .ghost()
                        .small()
                        .icon(IconName::Plus)
                        .label(tr!("env.add_variable"))
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.add_row(window, cx)
                            }),
                        ),
                ),
            )
    }

    fn render_dynamic(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let mono = cx.theme().mono_font_family.clone();
        let rows = DYNAMIC_VARIABLES.iter().zip(&self.samples).enumerate().map(
            |(ix, ((name, about), sample))| {
                let text = format!("{{{{${name}}}}}");
                let copy = text.clone();
                h_flex()
                    .id(("dynamic", ix))
                    .gap_3()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
                    .on_click(move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                    })
                    .child(
                        div()
                            .w(px(230.))
                            .flex_none()
                            .whitespace_nowrap()
                            .font_family(mono.clone())
                            .text_color(p.accent)
                            .child(text),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(p.text)
                            .child(*about),
                    )
                    .child(
                        div()
                            .w(px(200.))
                            .flex_none()
                            .truncate()
                            .font_family(mono.clone())
                            .text_color(p.text_muted)
                            .child(sample.clone()),
                    )
            },
        );
        div()
            .id("dynamic-rows")
            .ltr()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .children(rows)
    }

    fn hint(&self, page: &Page) -> SharedString {
        match &page.target {
            Target::Environment { root, .. }
                if apix_core::secrets::keychain_enabled() && !apix_core::vfs::is_virtual(root) =>
            {
                tr!("env.hint.environment_keychain")
            }
            Target::Environment { name, .. } => tr!("env.hint.environment", name = name),
            Target::Dynamic => tr!("env.hint.dynamic"),
            _ => tr!("env.hint.shared"),
        }
    }
}

impl Render for EnvEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let Some(page) = self.pages.get(self.current) else {
            return div().into_any_element();
        };
        let content = if page.target == Target::Dynamic {
            self.render_dynamic(cx).into_any_element()
        } else {
            self.render_rows(page, cx).into_any_element()
        };
        let hint = self.hint(page);
        h_flex()
            .dir_row()
            .h(px(480.))
            .gap_3()
            .items_start()
            .child(self.render_list(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_header(page, cx))
                    .child(content)
                    .child(
                        div()
                            .pt_2()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(bidi::para(hint)),
                    )
                    .when_some(self.error.clone(), |col, error| {
                        col.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(p.err)
                                .child(bidi::para(error)),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Workspace {
    /// After Settings › Environment secrets changed: moves the secrets of every
    /// open environment to the chosen place. Others move when next saved.
    pub(super) fn move_environment_secrets(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let roots = self
            .project
            .iter()
            .map(|p| p.root.clone())
            .chain(self.collections.iter().map(|c| c.root.clone()));
        let mut moved = 0;
        let mut errors = Vec::new();
        for root in roots {
            for name in store::environment_names_at(&root) {
                match store::move_secrets(&root, &name) {
                    Ok(true) => moved += 1,
                    Ok(false) => {}
                    Err(e) => errors.push(describe(&e)),
                }
            }
        }
        if moved > 0 {
            let message = if apix_core::secrets::keychain_enabled() {
                tr!("env.secrets_moved_keychain", count = moved)
            } else {
                tr!("env.secrets_moved_file", count = moved)
            };
            window.push_notification(Notification::success(message), cx);
        }
        if !errors.is_empty() {
            self.notify_error(errors.join("\n"), window, cx);
        }
    }

    /// Opens the variables dialog on the active environment (or the shared variables).
    pub(super) fn open_env_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.workspace_root() else {
            return;
        };
        let specs = self.env_pages(&root);
        let start = specs
            .iter()
            .position(|spec| {
                matches!(&spec.target, Target::Environment { name, .. }
                    if self.environment.as_deref() == Some(name.as_str()))
            })
            .unwrap_or(0);
        let view = cx.entity().downgrade();
        let palette = self.palette;
        let editor =
            cx.new(|cx| EnvEditor::new(view.clone(), palette, specs, start, root, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            let save = editor.clone();
            let cancel_view = view.clone();
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .bidi_title(tr!("env.dialog_title"))
                .width(px(880.))
                .child(editor.clone())
                // Enter belongs to the fields (it adds an environment), not to the dialog.
                .on_ok(|_, _, _| false)
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("vars-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    super::collections::close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("vars-save")
                                .primary()
                                .label(tr!("dialog.save"))
                                .on_click(move |_, window, cx| {
                                    if save.update(cx, |editor, cx| editor.save(window, cx)) {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    /// Shared variables first, then the environments where they live, then
    /// the dynamic variables.
    fn env_pages(&self, root: &Path) -> Vec<PageSpec> {
        let mut pages = Vec::new();
        let shared = tr!("env.section.shared").to_string();
        let environments = tr!("env.section.environments").to_string();
        let env = |root: &Path, name: String, section: &str| PageSpec {
            target: Target::Environment {
                root: root.to_owned(),
                name: name.clone(),
            },
            label: name,
            section: section.to_owned(),
        };
        match &self.project {
            Some(project) => {
                pages.push(PageSpec {
                    target: Target::ProjectVars(project.root.clone()),
                    label: project.meta.name.clone(),
                    section: shared.clone(),
                });
                for c in &self.collections {
                    pages.push(PageSpec {
                        target: Target::CollectionVars(c.root.clone()),
                        label: c.meta.name.clone(),
                        section: shared.clone(),
                    });
                }
                for name in store::environment_names_at(root) {
                    pages.push(env(root, name, &environments));
                }
                // Environments a collection defines for itself.
                for c in &self.collections {
                    for name in c.environment_names() {
                        let section = tr!("env.section.collection_only", name = c.meta.name);
                        pages.push(env(&c.root, name, &section));
                    }
                }
            }
            None => {
                if let Some(c) = self.collections.first() {
                    pages.push(PageSpec {
                        target: Target::CollectionVars(c.root.clone()),
                        label: c.meta.name.clone(),
                        section: shared,
                    });
                    for name in c.environment_names() {
                        pages.push(env(&c.root, name, &environments));
                    }
                }
            }
        }
        pages.push(PageSpec {
            target: Target::Dynamic,
            label: tr!("env.dynamic_variables").to_string(),
            section: tr!("env.section.built_in").to_string(),
        });
        pages
    }
}

fn empty_var() -> EnvVar {
    EnvVar {
        key: String::new(),
        value: String::new(),
        secret: false,
    }
}

fn new_row(var: &EnvVar, window: &mut Window, cx: &mut App) -> Row {
    let (key, value, secret) = (var.key.clone(), var.value.clone(), var.secret);
    Row {
        key: cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("env.placeholder.name"))
                .default_value(key)
        }),
        value: cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("env.placeholder.value"))
                .masked(secret)
                .default_value(value)
        }),
        secret,
    }
}

fn read_parts(target: &Target) -> Result<EnvironmentParts, StoreError> {
    let shared = |vars: VarMap| EnvironmentParts {
        vars,
        secrets: VarMap::new(),
    };
    match target {
        Target::ProjectVars(root) => Project::open(root).map(|p| shared(p.meta.vars)),
        Target::CollectionVars(root) => Collection::open(root).map(|c| shared(c.meta.vars)),
        Target::Environment { root, name } => EnvironmentParts::read(root, name),
        Target::Dynamic => Ok(EnvironmentParts::default()),
    }
}

/// Writes one changed page. Returns `(old, new)` when an environment was renamed.
fn write_page(page: &Page, cx: &App) -> Result<Option<(String, String)>, String> {
    let rows = page.current(cx);
    let map = || -> VarMap {
        rows.iter()
            .map(|r| (r.key.clone(), r.value.clone()))
            .collect()
    };
    let err = |e: StoreError| describe(&e);
    match &page.target {
        Target::ProjectVars(root) => {
            let mut project = Project::open(root).map_err(err)?;
            project.meta.vars = map();
            project.save().map_err(err)?;
        }
        Target::CollectionVars(root) => {
            let mut collection = Collection::open(root).map_err(err)?;
            collection.meta.vars = map();
            store::save_collection_meta(root, &collection.meta).map_err(err)?;
        }
        Target::Environment { root, name } => {
            if page.deleted {
                if !page.is_new {
                    store::forget_keychain_secrets(root, name).map_err(err)?;
                    super::vault_ui::discard(&store::environment_files(root, name))?;
                }
                return Ok(None);
            }
            let mut current = name.clone();
            let mut renamed = None;
            if page.renamed(cx) {
                let title = page.new_name(cx).unwrap_or_default();
                current = store::rename_environment(root, name, &title).map_err(err)?;
                renamed = Some((name.clone(), current.clone()));
            } else if page.is_new {
                current = page.new_name(cx).unwrap_or_else(|| name.clone());
                if !store::environment_files(root, &current).is_empty() {
                    return Err(tr!("env.error.exists", name = current).to_string());
                }
            }
            // A page that was never opened has nothing new to write.
            if page.rows.is_some() {
                page.parts
                    .with_rows(&rows)
                    .write(root, &current)
                    .map_err(err)?;
            }
            return Ok(renamed);
        }
        Target::Dynamic => {}
    }
    Ok(None)
}
