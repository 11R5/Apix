//! "Save as variable" on a response: a value from the body, a header or
//! the status goes into the active environment (or, with none, the
//! collection's variables), and optionally a line in the Tests script keeps
//! it up to date on every send.

use std::cell::Cell;

use apix_core::store::{self, EnvironmentParts};
use apix_core::{Collection, Project};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::collections::{close_dialog, refocus};
use super::{ResponseState, Workspace, describe};
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::tr;

impl Workspace {
    pub(super) fn prompt_extract(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ResponseState::Done(done) = &self.response else {
            return;
        };
        let response = &done.response;
        let view_of = apix_script::ResponseView::new(
            response.status,
            response.reason.unwrap_or(""),
            &response.headers,
            &response.body,
            response.timing.total,
        );
        let target = cx.new(|cx| {
            InputState::new(window, cx).placeholder(if done.json {
                "body.data.token"
            } else {
                "header.X-Token"
            })
        });
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("token"));
        let keep = std::rc::Rc::new(Cell::new(false));
        let view = cx.entity().downgrade();
        let first = Cell::new(true);
        let palette = self.palette;
        let where_to = match &self.environment {
            Some(env) => tr!("extract.into_env", env = env),
            None => tr!("extract.into_collection"),
        };
        window.open_dialog(cx, move |dialog, window, cx| {
            if first.replace(false) {
                target.update(cx, |state, cx| state.focus(window, cx));
            }
            let confirm = {
                let (target, name, keep, view, view_of) = (
                    target.clone(),
                    name.clone(),
                    keep.clone(),
                    view.clone(),
                    view_of.clone(),
                );
                std::rc::Rc::new(move |window: &mut Window, cx: &mut App| -> bool {
                    let path = target.read(cx).value().trim().to_string();
                    let var = name.read(cx).value().trim().to_string();
                    if path.is_empty() || var.is_empty() {
                        return false;
                    }
                    let value = match apix_script::value_at(&view_of, &path) {
                        Ok(value) => value,
                        Err(error) => {
                            window.push_notification(
                                Notification::error(tr!("extract.error", error = error)),
                                cx,
                            );
                            return false;
                        }
                    };
                    let keep = keep.get();
                    view.update(cx, |this, cx| {
                        this.save_extracted(&var, &value, window, cx);
                        if keep && let Some(expr) = apix_script::expression_for(&path) {
                            let scope = if this.environment.is_some() {
                                "pm.environment"
                            } else {
                                "pm.collectionVariables"
                            };
                            let line = format!(
                                "{scope}.set({}, {expr});",
                                serde_json::to_string(&var).unwrap_or_default()
                            );
                            this.script_editor
                                .update(cx, |editor, cx| editor.append_test(&line, window, cx));
                            this.request_tab = 4;
                        }
                        this.focus.focus(window, cx);
                    })
                    .ok();
                    true
                })
            };
            let on_enter = confirm.clone();
            let cancel_view = view.clone();
            let keep_now = keep.get();
            let toggle = keep.clone();
            let label =
                |text: SharedString| div().text_xs().text_color(palette.text_muted).child(text);
            dialog
                .on_close(refocus(view.clone()))
                .bidi_title(tr!("extract.title"))
                .width(px(460.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(label(tr!("extract.path")))
                        .child(Input::new(&target))
                        .child(label(tr!("extract.name")))
                        .child(Input::new(&name))
                        .child(
                            Checkbox::new("extract-keep")
                                .dir_row()
                                .checked(keep_now)
                                .label(tr!("extract.keep"))
                                .on_click(move |checked: &bool, window, _| {
                                    toggle.set(*checked);
                                    window.refresh();
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.text_muted)
                                .child(bidi::para(where_to.clone())),
                        ),
                )
                .on_ok(move |_, window, cx| on_enter(window, cx))
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("extract-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("extract-ok")
                                .primary()
                                .label(tr!("extract.ok"))
                                .on_click(move |_, window, cx| {
                                    if confirm(window, cx) {
                                        window.close_dialog(cx);
                                    }
                                }),
                        ),
                )
        });
    }

    /// Writes `name = value` into the active environment (where it is
    /// defined: the collection's, else the project's), or with none into the
    /// collection's variables. A secret stays secret.
    fn save_extracted(
        &mut self,
        name: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(collection) = self.collection.clone() else {
            return;
        };
        let result = match &self.environment {
            Some(env) => {
                let project_root = self.project.as_ref().map(|p| p.root.clone());
                let root = if store::environment_files(&collection.root, env).is_empty()
                    && let Some(project_root) =
                        project_root.filter(|r| !store::environment_files(r, env).is_empty())
                {
                    project_root
                } else {
                    collection.root.clone()
                };
                EnvironmentParts::read(&root, env).and_then(|mut parts| {
                    if parts.secrets.contains_key(name) {
                        parts.secrets.insert(name.to_owned(), value.to_owned());
                    } else {
                        parts.vars.insert(name.to_owned(), value.to_owned());
                    }
                    parts.write(&root, env)
                })
            }
            None => Collection::open(&collection.root).and_then(|mut c| {
                c.meta.vars.insert(name.to_owned(), value.to_owned());
                store::save_collection_meta(&collection.root, &c.meta)
            }),
        };
        match result {
            Ok(()) => {
                // The file's value wins over what a script set earlier.
                for scope in [
                    &mut self.script_vars.environment,
                    &mut self.script_vars.collection,
                    &mut self.script_vars.local,
                ] {
                    scope.remove(name);
                }
                if self.environment.is_none() {
                    self.collection = Collection::open(&collection.root).ok();
                    if let Some(c) = &self.collection
                        && let Some(slot) = self.collections.iter_mut().find(|x| x.root == c.root)
                    {
                        *slot = c.clone();
                    }
                }
                if let Some(project) = &self.project
                    && let Ok(fresh) = Project::open(&project.root)
                {
                    self.project = Some(fresh);
                }
                self.rebuild_vars(window, cx);
                let shown: String = value.chars().take(60).collect();
                let message = match &self.environment {
                    Some(env) => tr!("extract.saved_env", name = name, env = env, value = shown),
                    None => tr!("extract.saved_collection", name = name, value = shown),
                };
                window.push_notification(Notification::success(message), cx);
                cx.notify();
            }
            Err(e) => self.notify_error(describe(&e), window, cx),
        }
    }
}
