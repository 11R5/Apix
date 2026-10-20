//! The Settings dialog (Ctrl+,). Every change applies and is saved at once.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::i18n;
use crate::settings::Settings;
use crate::theme::{Palette, Variant};
use crate::tr;

type Picker = SelectState<Vec<SharedString>>;

/// Auto-lock choices, in minutes; 0 never locks.
const AUTO_LOCK_MINUTES: [u32; 5] = [5, 15, 30, 60, 0];

fn auto_lock_label(minutes: u32) -> SharedString {
    match minutes {
        0 => tr!("settings.auto_lock_never"),
        60 => tr!("settings.auto_lock_hour"),
        m => tr!("settings.auto_lock_minutes", count = m),
    }
}

fn theme_label(variant: Variant) -> SharedString {
    match variant {
        Variant::Obsidian => tr!("settings.theme_obsidian"),
        Variant::Cyber => tr!("settings.theme_cyber"),
    }
}

pub(super) struct SettingsView {
    palette: Palette,
    workspace: WeakEntity<Workspace>,
    language: Entity<Picker>,
    theme: Entity<Picker>,
    auto_lock: Entity<Picker>,
    /// The theme the app started with; a change shows "after restart".
    started_theme: Variant,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(
        palette: Palette,
        workspace: WeakEntity<Workspace>,
        settings: &Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = i18n::current().code;
        let language_ix = i18n::LANGUAGES.iter().position(|l| l.code == current);
        let language = cx
            .new(|cx| SelectState::new(i18n::names(), language_ix.map(IndexPath::new), window, cx));
        let started_theme = Variant::chosen(settings);
        let theme_ix = Variant::ALL.iter().position(|v| *v == started_theme);
        let theme = cx.new(|cx| {
            SelectState::new(
                Variant::ALL.map(theme_label).to_vec(),
                theme_ix.map(IndexPath::new),
                window,
                cx,
            )
        });
        let lock_ix = AUTO_LOCK_MINUTES
            .iter()
            .position(|m| *m == settings.auto_lock_minutes)
            .unwrap_or(1);
        let auto_lock = cx.new(|cx| {
            SelectState::new(
                AUTO_LOCK_MINUTES.map(auto_lock_label).to_vec(),
                Some(IndexPath::new(lock_ix)),
                window,
                cx,
            )
        });

        let subscriptions = vec![
            cx.subscribe_in(
                &language,
                window,
                |this, state, _: &SelectEvent<Vec<SharedString>>, window, cx| {
                    let Some(ix) = state.read(cx).selected_index(cx).map(|ix| ix.row) else {
                        return;
                    };
                    let code = i18n::LANGUAGES[ix].code;
                    this.change(|s| s.language = code.to_owned(), cx);
                    i18n::set(code);
                    this.retranslate(window, cx);
                    this.workspace
                        .update(cx, |ws, cx| ws.retranslate(window, cx))
                        .ok();
                    window.refresh();
                },
            ),
            cx.subscribe_in(
                &theme,
                window,
                |this, state, _: &SelectEvent<Vec<SharedString>>, _, cx| {
                    if let Some(ix) = state.read(cx).selected_index(cx) {
                        let id = Variant::ALL[ix.row].id();
                        this.change(|s| s.theme = id.to_owned(), cx);
                    }
                    cx.notify();
                },
            ),
            cx.subscribe_in(
                &auto_lock,
                window,
                |this, state, _: &SelectEvent<Vec<SharedString>>, _, cx| {
                    if let Some(ix) = state.read(cx).selected_index(cx) {
                        let minutes = AUTO_LOCK_MINUTES[ix.row];
                        this.change(|s| s.auto_lock_minutes = minutes, cx);
                    }
                },
            ),
        ];
        Self {
            palette,
            workspace,
            language,
            theme,
            auto_lock,
            started_theme,
            _subscriptions: subscriptions,
        }
    }

    /// Changes the workspace's settings and saves them.
    fn change(&self, f: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        self.workspace
            .update(cx, |ws, cx| {
                f(&mut ws.settings);
                if let Err(e) = ws.settings.save() {
                    log_save_error(&e);
                }
                cx.notify();
            })
            .ok();
    }

    /// Relabels the pickers after a language change.
    fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let themes = Variant::ALL.map(theme_label).to_vec();
        self.theme
            .update(cx, |state, cx| state.set_items(themes, window, cx));
        let locks = AUTO_LOCK_MINUTES.map(auto_lock_label).to_vec();
        self.auto_lock
            .update(cx, |state, cx| state.set_items(locks, window, cx));
        cx.notify();
    }

    fn row(
        &self,
        title: SharedString,
        description: SharedString,
        control: impl IntoElement,
    ) -> impl IntoElement {
        let p = self.palette;
        h_flex()
            .dir_row()
            .gap_4()
            .items_center()
            .py_2()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(div().text_sm().text_color(p.text).child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(bidi::para(description)),
                    ),
            )
            .child(div().w(px(220.)).flex_none().child(control))
    }
}

fn log_save_error(e: &std::io::Error) {
    eprintln!("apix: could not save settings: {e}");
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let chosen = self
            .theme
            .read(cx)
            .selected_index(cx)
            .map_or(self.started_theme, |ix| Variant::ALL[ix.row]);
        let theme_note = if chosen == self.started_theme {
            tr!("settings.theme_description")
        } else {
            tr!("settings.theme_restart")
        };
        v_flex()
            .gap_1()
            .child(self.row(
                tr!("settings.language"),
                tr!("settings.language_description"),
                Select::new(&self.language).small(),
            ))
            .child(div().h_px().bg(p.hairline))
            .child(self.row(
                tr!("settings.theme"),
                theme_note,
                Select::new(&self.theme).small(),
            ))
            .child(div().h_px().bg(p.hairline))
            .child(self.row(
                tr!("settings.auto_lock"),
                tr!("settings.auto_lock_description"),
                Select::new(&self.auto_lock).small(),
            ))
    }
}

impl Workspace {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let palette = self.palette;
        let settings = self.settings.clone();
        let editor = cx.new(|cx| SettingsView::new(palette, view.clone(), &settings, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            let close_view = view.clone();
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .bidi_title(tr!("settings.title"))
                .width(px(620.))
                .child(editor.clone())
                .on_ok(|_, _, _| false)
                .footer(
                    h_flex().dir_row().w_full().justify_end().child(
                        Button::new("settings-close")
                            .primary()
                            .label(tr!("dialog.close"))
                            .on_click(move |_, window, cx| {
                                super::collections::close_dialog(&close_view, window, cx)
                            }),
                    ),
                )
        });
    }
}
