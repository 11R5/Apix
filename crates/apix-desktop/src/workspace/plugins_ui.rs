//! The Plugins window (View › Plugins…): the WebAssembly plugins in
//! `<config>/Apix/plugins` and what each adds, installing one from a file,
//! removing one, and importing a file with an importer plugin.

use std::path::PathBuf;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::tr;

impl Workspace {
    pub(super) fn open_plugins(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = cx.entity().downgrade();
        let view = cx.new(|_| PluginsView {
            workspace: workspace.clone(),
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("plugins.title"))
                .width(px(760.))
                .child(div().pb_2().child(view.clone()))
        });
    }

    /// Reads `file` with importer `plugin` and opens what it made, a new
    /// collection next to the file.
    fn import_with_plugin(
        &mut self,
        plugin: String,
        file: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = (|| -> Result<PathBuf, String> {
            let found =
                apix_plugins::find(&plugin).ok_or_else(|| format!("{plugin} is not installed"))?;
            let content = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let imported = found.import(&file_name, &content)?;
            let plan = apix_import::plugin::plan(&imported, &file_name);
            let parent = file.parent().map(PathBuf::from).unwrap_or_default();
            let stem = apix_import::slug(&plan.meta.name, "imported");
            let root = (1..)
                .map(|n| {
                    if n == 1 {
                        parent.join(&stem)
                    } else {
                        parent.join(format!("{stem}-{n}"))
                    }
                })
                .find(|p| !p.exists())
                .expect("a free name exists");
            plan.write(&root).map_err(|e| e.to_string())?;
            Ok(root)
        })();
        match result {
            Ok(root) => self.open_or_create(root, window, cx),
            Err(e) => self.notify_error(e, window, cx),
        }
    }
}

struct PluginsView {
    workspace: WeakEntity<Workspace>,
}

impl PluginsView {
    fn install(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("plugins.install")),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(file) = paths.into_iter().next()
            {
                this.update_in(cx, |_, window, cx| {
                    let note = match apix_plugins::install(&file) {
                        Ok(path) => Notification::success(tr!(
                            "plugins.installed",
                            file = path.display().to_string()
                        )),
                        Err(e) => Notification::error(e),
                    };
                    window.push_notification(note, cx);
                    cx.notify();
                })
                .ok();
                this.update_in(cx, |this, window, cx| this.refresh_auth(window, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// The Auth tab learns about plugins installed or removed here.
    fn refresh_auth(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ws) = self.workspace.upgrade() {
            let editor = ws.read(cx).auth_editor.clone();
            editor.update(cx, |e, cx| e.refresh_plugins(window, cx));
        }
    }

    fn import(&mut self, plugin: String, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("plugins.import_prompt")),
        });
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |_, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(file) = paths.into_iter().next()
            {
                workspace
                    .update_in(cx, |_, window, cx| {
                        window.close_dialog(cx);
                        // After this dialog is gone: opening the new
                        // collection may ask about unsaved changes, and that
                        // dialog must not be opened under the closing one.
                        let ws = cx.entity().downgrade();
                        window.defer(cx, move |window, cx| {
                            ws.update(cx, |ws, cx| ws.import_with_plugin(plugin, file, window, cx))
                                .ok();
                        });
                    })
                    .ok();
            }
        })
        .detach();
    }
}

impl Render for PluginsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let installed = apix_plugins::installed();
        let folder = apix_plugins::folder().unwrap_or_default();
        let badge = |text: String| {
            div()
                .px_2()
                .rounded_md()
                .bg(p.accent.opacity(0.14))
                .text_xs()
                .child(text)
        };
        let cards = installed.iter().enumerate().map(|(i, entry)| match entry {
            Ok(plugin) => {
                let m = &plugin.manifest;
                let mut offers = Vec::new();
                if let Some(a) = &m.auth {
                    offers.push(tr!("plugins.offers_auth", label = a.label.clone()).to_string());
                }
                if let Some(v) = &m.import {
                    offers.push(tr!("plugins.offers_import", label = v.label.clone()).to_string());
                }
                if let Some(v) = &m.visualize {
                    offers.push(tr!("plugins.offers_view", label = v.label.clone()).to_string());
                }
                let (name, path) = (m.name.clone(), plugin.path.clone());
                h_flex()
                    .dir_row()
                    .gap_3()
                    .items_start()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(p.hairline)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(m.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(p.text_muted)
                                            .child(m.version.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(p.text_muted)
                                    .child(m.description.clone()),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .flex_wrap()
                                    .children(offers.into_iter().map(badge)),
                            ),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .gap_1()
                            .children(m.import.as_ref().map(|_| {
                                let name = name.clone();
                                Button::new(("plugin-import", i))
                                    .small()
                                    .outline()
                                    .label(tr!("plugins.import"))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.import(name.clone(), window, cx)
                                        },
                                    ))
                            }))
                            .child(
                                Button::new(("plugin-remove", i))
                                    .small()
                                    .ghost()
                                    .label(tr!("plugins.remove"))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            let note = match std::fs::remove_file(&path) {
                                                Ok(()) => {
                                                    apix_plugins::reload();
                                                    this.refresh_auth(window, cx);
                                                    Notification::info(tr!(
                                                        "plugins.removed",
                                                        name = name.clone()
                                                    ))
                                                }
                                                Err(e) => Notification::error(e.to_string()),
                                            };
                                            window.push_notification(note, cx);
                                            cx.notify();
                                        },
                                    )),
                            ),
                    )
                    .into_any_element()
            }
            Err((path, error)) => div()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(p.err.opacity(0.5))
                .text_xs()
                .text_color(p.err)
                .child(format!(
                    "{}: {error}",
                    path.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                ))
                .into_any_element(),
        });
        let open_folder = folder.clone();
        v_flex()
            .gap_3()
            .child(div().text_sm().child(bidi::para(tr!("plugins.about"))))
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .child(
                        Button::new("plugins-install")
                            .small()
                            .primary()
                            .label(tr!("plugins.install"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.install(window, cx)
                            })),
                    )
                    .child(
                        Button::new("plugins-folder")
                            .small()
                            .outline()
                            .label(tr!("route.open_folder"))
                            .on_click(move |_, _, cx| {
                                let _ = std::fs::create_dir_all(&open_folder);
                                cx.open_url(&format!("file:///{}", open_folder.display()));
                            }),
                    )
                    .child(
                        Button::new("plugins-reload")
                            .small()
                            .ghost()
                            .label(tr!("plugins.reload"))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                                apix_plugins::reload();
                                cx.notify();
                            })),
                    ),
            )
            .children(cards)
            .when(installed.is_empty(), |d| {
                d.child(
                    div()
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(bidi::para(tr!(
                            "plugins.none",
                            folder = folder.display().to_string()
                        ))),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("plugins.note"))),
            )
    }
}

use gpui_kit::prelude::FluentBuilder as _;
