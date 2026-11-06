//! The Setup window, shown once on the first start and from Help › Setup…:
//! the optional extras, each explained and each a single click — the IP
//! location database for the route map, an AI provider and key, and Apix as
//! an MCP server for Claude. Also the Connect to Claude window.

use std::path::PathBuf;

use apix_core::claude_mcp;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::tr;

impl Workspace {
    /// The first start: the Setup window, once.
    pub(super) fn setup_on_first_start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.setup_shown || crate::bench::mode().is_some() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(400))
                .await;
            this.update_in(cx, |this, window, cx| this.open_setup(window, cx))
                .ok();
        })
        .detach();
    }

    pub(super) fn open_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.settings.setup_shown {
            self.settings.setup_shown = true;
            let _ = self.settings.save();
        }
        let workspace = cx.entity().downgrade();
        let collection = self.collection.as_ref().map(|c| c.meta.name.clone());
        let view = cx.new(|_| SetupView {
            workspace: workspace.clone(),
            geo: apix_route::geo_dir().and_then(|d| apix_route::Geo::find(&d)),
            downloading: false,
            error: None,
            collection,
            _task: None,
        });
        super::tour::keep(&view);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("setup.title"))
                .width(px(720.))
                .child(div().pb_2().child(view.clone()))
        });
    }

    pub(super) fn open_connect_claude(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.clone() else {
            return self.notify_error(
                tr!("collection.open_first", key = super::key_hint("O")),
                window,
                cx,
            );
        };
        let root = std::path::absolute(&collection.root).unwrap_or(collection.root.clone());
        let workspace = cx.entity().downgrade();
        let view = cx.new(|_| ClaudeView {
            name: claude_mcp::server_name(&collection.meta.name),
            collection: collection.meta.name.clone(),
            root,
            command: claude_mcp::apix_command(),
            list_only: false,
            busy: false,
            _task: None,
        });
        super::tour::keep(&view);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(workspace.clone()))
                .bidi_title(tr!("claude.title"))
                .width(px(760.))
                .child(div().pb_2().child(view.clone()))
        });
    }
}

struct SetupView {
    workspace: WeakEntity<Workspace>,
    /// The location database in use, if any.
    geo: Option<PathBuf>,
    downloading: bool,
    error: Option<String>,
    /// The open collection, for connecting to Claude.
    collection: Option<String>,
    _task: Option<Task<()>>,
}

impl SetupView {
    fn download(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.downloading = true;
        self.error = None;
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = super::route_ui::download_geo().await;
            this.update_in(cx, |this, window, cx| {
                this.downloading = false;
                match result {
                    Ok(path) => {
                        window.push_notification(
                            Notification::success(tr!(
                                "route.downloaded",
                                file = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_default()
                            )),
                            cx,
                        );
                        this.geo = Some(path);
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

/// One extra: what it is, what it is for, and its button.
fn card(
    title: SharedString,
    about: SharedString,
    state: Option<AnyElement>,
    action: AnyElement,
) -> impl IntoElement {
    let p = crate::theme::current();
    h_flex()
        .dir_row()
        .gap_4()
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
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(bidi::para(about)),
                )
                .children(state),
        )
        .child(div().flex_none().child(action))
}

impl Render for SetupView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let small = |text: String, colour: Hsla| div().text_xs().text_color(colour).child(text);

        let geo_state = match (&self.geo, self.downloading, &self.error) {
            (_, true, _) => small(tr!("route.downloading").into(), p.text_muted),
            (_, _, Some(e)) => small(e.clone(), p.err),
            (Some(path), _, _) => small(
                tr!(
                    "setup.geo_installed",
                    file = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                )
                .into(),
                p.ok,
            ),
            (None, _, _) => small(tr!("setup.geo_missing").into(), p.text_muted),
        };
        let geo_button = Button::new("setup-geo")
            .small()
            .when_else(
                self.geo.is_some(),
                |b| b.outline().label(tr!("setup.geo_update")),
                |b| b.primary().label(tr!("route.download")),
            )
            .loading(self.downloading)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                if !this.downloading {
                    this.download(window, cx)
                }
            }));

        let ai_state = self.workspace.upgrade().map(|ws| {
            let settings = &ws.read(cx).settings;
            let provider = apix_core::ai::Provider::from_id(&settings.ai_provider);
            let on = settings.ai_assist && apix_core::ai::load_key(provider).is_some();
            if on {
                small(tr!("setup.ai_on", provider = provider.name()).into(), p.ok)
            } else {
                small(tr!("setup.ai_off").into(), p.text_muted)
            }
        });
        let workspace = self.workspace.clone();
        let ai_button = Button::new("setup-ai")
            .small()
            .outline()
            .label(tr!("setup.ai_open"))
            .on_click(move |_, window, cx| {
                workspace
                    .update(cx, |ws, cx| ws.open_settings(window, cx))
                    .ok();
            });

        let workspace = self.workspace.clone();
        let claude_button = Button::new("setup-claude")
            .small()
            .outline()
            .label(tr!("claude.title"))
            .disabled(self.collection.is_none())
            .on_click(move |_, window, cx| {
                workspace
                    .update(cx, |ws, cx| ws.open_connect_claude(window, cx))
                    .ok();
            });
        let claude_state = small(
            match &self.collection {
                Some(name) => tr!("setup.claude_for", collection = name.clone()).into(),
                None => tr!("setup.claude_no_collection").into(),
            },
            p.text_muted,
        );

        v_flex()
            .gap_3()
            .child(div().text_sm().child(bidi::para(tr!("setup.intro"))))
            .child(card(
                tr!("setup.geo_title"),
                tr!("setup.geo_about"),
                Some(geo_state.into_any_element()),
                geo_button.into_any_element(),
            ))
            .child(card(
                tr!("setup.ai_title"),
                tr!("setup.ai_about"),
                ai_state.map(IntoElement::into_any_element),
                ai_button.into_any_element(),
            ))
            .child(card(
                tr!("setup.claude_title"),
                tr!("setup.claude_about"),
                Some(claude_state.into_any_element()),
                claude_button.into_any_element(),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(tr!("setup.later")),
            )
    }
}

struct ClaudeView {
    name: String,
    collection: String,
    root: PathBuf,
    command: String,
    list_only: bool,
    busy: bool,
    _task: Option<Task<()>>,
}

impl ClaudeView {
    fn args(&self) -> Vec<String> {
        claude_mcp::server_args(&self.root, self.list_only)
    }

    fn add_to_desktop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let note = match claude_mcp::desktop_config_path() {
            None => Notification::error("no configuration folder"),
            Some(path) => {
                match claude_mcp::add_to_desktop(&path, &self.name, &self.command, &self.args()) {
                    Ok(_) => Notification::success(tr!(
                        "claude.added_desktop",
                        name = self.name.clone(),
                        file = path.display().to_string()
                    )),
                    Err(e) => Notification::error(e),
                }
            }
        };
        window.push_notification(note, cx);
    }

    fn remove_from_desktop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = claude_mcp::desktop_config_path() else {
            return;
        };
        let note = match claude_mcp::remove_from_desktop(&path, &self.name) {
            Ok(true) => Notification::info(tr!("claude.removed", name = self.name.clone())),
            Ok(false) => Notification::info(tr!("claude.not_there", name = self.name.clone())),
            Err(e) => Notification::error(e),
        };
        window.push_notification(note, cx);
    }

    fn add_to_code(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let words = claude_mcp::claude_code_command(&self.name, &self.command, &self.args());
        self.busy = true;
        let name = self.name.clone();
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut command = std::process::Command::new(&words[0]);
                    command.args(&words[1..]);
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt as _;
                        command.creation_flags(0x0800_0000);
                    }
                    match command.output() {
                        Ok(out) if out.status.success() => Ok(()),
                        Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_owned()),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            Err(tr!("claude.no_claude_code").to_string())
                        }
                        Err(e) => Err(e.to_string()),
                    }
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                let note = match result {
                    Ok(()) => Notification::success(tr!("claude.added_code", name = name.clone())),
                    Err(e) => Notification::error(e),
                };
                window.push_notification(note, cx);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

impl Render for ClaudeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::current();
        let args = self.args();
        let json = serde_json::to_string_pretty(&claude_mcp::desktop_entry(
            &self.name,
            &self.command,
            &args,
        ))
        .unwrap_or_default();
        let line = claude_mcp::shell_line(&claude_mcp::claude_code_command(
            &self.name,
            &self.command,
            &args,
        ));
        let code = |id: &'static str, text: String| {
            let copy = text.clone();
            h_flex()
                .gap_2()
                .items_start()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .p_2()
                        .rounded_md()
                        .bg(p.hover)
                        .text_xs()
                        .font_family(crate::fonts::MONO)
                        .child(text),
                )
                .child(
                    Button::new(id)
                        .xsmall()
                        .ghost()
                        .label(tr!("claude.copy"))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()))
                        }),
                )
        };
        v_flex()
            .gap_3()
            .child(div().text_sm().child(bidi::para(tr!(
                "claude.about",
                collection = self.collection.clone(),
                name = self.name.clone()
            ))))
            .child(
                Checkbox::new("claude-list-only")
                    .checked(self.list_only)
                    .label(tr!("claude.list_only"))
                    .on_click(cx.listener(|this, checked: &bool, _, cx| {
                        this.list_only = *checked;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Claude Desktop"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("claude-desktop")
                            .small()
                            .primary()
                            .label(tr!("claude.add_desktop"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.add_to_desktop(window, cx)
                            })),
                    )
                    .child(
                        Button::new("claude-desktop-remove")
                            .small()
                            .ghost()
                            .label(tr!("claude.remove_desktop"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.remove_from_desktop(window, cx)
                            })),
                    ),
            )
            .child(code("claude-copy-json", json))
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Claude Code"),
            )
            .child(
                h_flex().child(
                    Button::new("claude-code")
                        .small()
                        .outline()
                        .label(tr!("claude.add_code"))
                        .loading(self.busy)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            if !this.busy {
                                this.add_to_code(window, cx)
                            }
                        })),
                ),
            )
            .child(code("claude-copy-line", line))
            .child(
                div()
                    .text_xs()
                    .text_color(p.text_muted)
                    .child(bidi::para(tr!("claude.note"))),
            )
    }
}
