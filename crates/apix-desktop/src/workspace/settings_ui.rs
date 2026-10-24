//! The Settings dialog (Ctrl+,). Every change applies and is saved at once.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{IndexPath, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use super::Workspace;
use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::i18n;
use crate::settings::Settings;
use crate::theme::{self, ACCENTS, Appearance, ThemeDef};
use crate::tr;

type Picker = SelectState<Vec<SharedString>>;
type Changed = SelectEvent<Vec<SharedString>>;

/// Auto-lock choices, in minutes; 0 never locks.
const AUTO_LOCK_MINUTES: [u32; 5] = [5, 15, 30, 60, 0];

fn auto_lock_label(minutes: u32) -> SharedString {
    match minutes {
        0 => tr!("settings.auto_lock_never"),
        60 => tr!("settings.auto_lock_hour"),
        m => tr!("settings.auto_lock_minutes", count = m),
    }
}

/// "On" / "Off", in picker order.
fn history_choices() -> Vec<SharedString> {
    vec![tr!("reqset.on"), tr!("reqset.off")]
}

/// Where environment secrets go, in picker order: file, keychain.
fn secrets_choices() -> Vec<SharedString> {
    vec![
        tr!("settings.secrets_file"),
        tr!("settings.secrets_keychain"),
    ]
}

fn appearance_choices() -> Vec<SharedString> {
    Appearance::ALL.map(Appearance::label).to_vec()
}

/// The themes of one kind, as the pickers list them.
struct ThemeList {
    ids: Vec<String>,
    labels: Vec<SharedString>,
}

impl ThemeList {
    fn of(themes: &[ThemeDef], light: bool) -> Self {
        let defs: Vec<&ThemeDef> = themes.iter().filter(|t| t.light == light).collect();
        Self {
            ids: defs.iter().map(|t| t.id.clone()).collect(),
            labels: defs.iter().map(|t| t.label()).collect(),
        }
    }

    fn index_of(&self, id: &str) -> Option<IndexPath> {
        self.ids
            .iter()
            .position(|t| t.eq_ignore_ascii_case(id))
            .map(IndexPath::new)
    }
}

/// The accents in picker order: the theme's own, the named ones, and a
/// `#RRGGBB` written in the settings file by hand.
fn accent_ids(current: &str) -> Vec<String> {
    let mut ids: Vec<String> = std::iter::once(String::new())
        .chain(ACCENTS.iter().map(|(id, _, _)| id.to_string()))
        .collect();
    if !ids.iter().any(|id| id == current) {
        ids.push(current.to_owned());
    }
    ids
}

fn picker(
    items: Vec<SharedString>,
    selected: Option<IndexPath>,
    window: &mut Window,
    cx: &mut Context<SettingsView>,
) -> Entity<Picker> {
    cx.new(|cx| SelectState::new(items, selected, window, cx))
}

pub(super) struct SettingsView {
    workspace: WeakEntity<Workspace>,
    language: Entity<Picker>,
    appearance: Entity<Picker>,
    dark_theme: Entity<Picker>,
    light_theme: Entity<Picker>,
    accent: Entity<Picker>,
    glass: Entity<Picker>,
    auto_lock: Entity<Picker>,
    history: Entity<Picker>,
    secrets: Entity<Picker>,
    prod: Entity<Picker>,
    dark_themes: ThemeList,
    light_themes: ThemeList,
    accents: Vec<String>,
    /// Theme files that could not be read.
    theme_errors: Vec<String>,
    /// The appearance picker shows this on the next paint (picking a theme
    /// of the other kind switches it).
    pending_appearance: Option<usize>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(
        workspace: WeakEntity<Workspace>,
        settings: &Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = i18n::current().code;
        let language_ix = i18n::LANGUAGES.iter().position(|l| l.code == current);
        let language = picker(i18n::names(), language_ix.map(IndexPath::new), window, cx);

        let (themes, theme_errors) = theme::themes();
        let dark_themes = ThemeList::of(&themes, false);
        let light_themes = ThemeList::of(&themes, true);
        let appearance_ix = Appearance::ALL
            .iter()
            .position(|a| *a == Appearance::from_id(&settings.appearance));
        let appearance = picker(
            appearance_choices(),
            appearance_ix.map(IndexPath::new),
            window,
            cx,
        );
        let dark_theme = picker(
            dark_themes.labels.clone(),
            dark_themes.index_of(&settings.theme),
            window,
            cx,
        );
        let light_theme = picker(
            light_themes.labels.clone(),
            light_themes.index_of(&settings.light_theme),
            window,
            cx,
        );
        let accents = accent_ids(&settings.accent);
        let accent_ix = accents.iter().position(|a| *a == settings.accent);
        let accent = picker(
            accents.iter().map(|a| theme::accent_label(a)).collect(),
            accent_ix.map(IndexPath::new),
            window,
            cx,
        );
        let glass_ix = if settings.glass { 0 } else { 1 };
        let glass = picker(
            history_choices(),
            Some(IndexPath::new(glass_ix)),
            window,
            cx,
        );

        let lock_ix = AUTO_LOCK_MINUTES
            .iter()
            .position(|m| *m == settings.auto_lock_minutes)
            .unwrap_or(1);
        let auto_lock = picker(
            AUTO_LOCK_MINUTES.map(auto_lock_label).to_vec(),
            Some(IndexPath::new(lock_ix)),
            window,
            cx,
        );
        let history_ix = if settings.history { 0 } else { 1 };
        let history = picker(
            history_choices(),
            Some(IndexPath::new(history_ix)),
            window,
            cx,
        );
        let secrets_ix = if settings.secrets_in_keychain { 1 } else { 0 };
        let secrets = picker(
            secrets_choices(),
            Some(IndexPath::new(secrets_ix)),
            window,
            cx,
        );
        let prod_ix = if settings.confirm_prod { 0 } else { 1 };
        let prod = picker(history_choices(), Some(IndexPath::new(prod_ix)), window, cx);

        let subscriptions = vec![
            cx.subscribe_in(&prod, window, |this, state, _: &Changed, _, cx| {
                if let Some(ix) = state.read(cx).selected_index(cx) {
                    this.change(|s| s.confirm_prod = ix.row == 0, cx);
                }
            }),
            cx.subscribe_in(&secrets, window, |this, state, _: &Changed, window, cx| {
                let Some(ix) = state.read(cx).selected_index(cx) else {
                    return;
                };
                let keychain = ix.row == 1;
                this.change(|s| s.secrets_in_keychain = keychain, cx);
                apix_core::secrets::use_keychain(keychain);
                this.workspace
                    .update(cx, |ws, cx| ws.move_environment_secrets(window, cx))
                    .ok();
            }),
            cx.subscribe_in(&history, window, |this, state, _: &Changed, _, cx| {
                if let Some(ix) = state.read(cx).selected_index(cx) {
                    this.change(|s| s.history = ix.row == 0, cx);
                }
            }),
            cx.subscribe_in(&language, window, |this, state, _: &Changed, window, cx| {
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
            }),
            cx.subscribe_in(&appearance, window, |this, state, _: &Changed, _, cx| {
                if let Some(ix) = state.read(cx).selected_index(cx) {
                    let id = Appearance::ALL[ix.row].id();
                    this.change_theme(|s| s.appearance = id.to_owned(), cx);
                }
            }),
            cx.subscribe_in(&dark_theme, window, |this, state, _: &Changed, _, cx| {
                let picked = state.read(cx).selected_index(cx);
                if let Some(id) = picked.and_then(|ix| this.dark_themes.ids.get(ix.row).cloned()) {
                    // Picking a dark theme while the light one shows means "show it".
                    this.change_theme(
                        |s| {
                            s.theme = id;
                            if s.appearance == Appearance::Light.id() {
                                s.appearance = Appearance::Dark.id().into();
                            }
                        },
                        cx,
                    );
                }
            }),
            cx.subscribe_in(&light_theme, window, |this, state, _: &Changed, _, cx| {
                let picked = state.read(cx).selected_index(cx);
                if let Some(id) = picked.and_then(|ix| this.light_themes.ids.get(ix.row).cloned()) {
                    this.change_theme(
                        |s| {
                            s.light_theme = id;
                            if s.appearance == Appearance::Dark.id() {
                                s.appearance = Appearance::Light.id().into();
                            }
                        },
                        cx,
                    );
                }
            }),
            cx.subscribe_in(&accent, window, |this, state, _: &Changed, _, cx| {
                let picked = state.read(cx).selected_index(cx);
                if let Some(id) = picked.and_then(|ix| this.accents.get(ix.row).cloned()) {
                    this.change_theme(|s| s.accent = id, cx);
                }
            }),
            cx.subscribe_in(&glass, window, |this, state, _: &Changed, _, cx| {
                if let Some(ix) = state.read(cx).selected_index(cx) {
                    this.change_theme(|s| s.glass = ix.row == 0, cx);
                }
            }),
            cx.subscribe_in(&auto_lock, window, |this, state, _: &Changed, _, cx| {
                if let Some(ix) = state.read(cx).selected_index(cx) {
                    let minutes = AUTO_LOCK_MINUTES[ix.row];
                    this.change(|s| s.auto_lock_minutes = minutes, cx);
                }
            }),
        ];
        Self {
            workspace,
            language,
            appearance,
            dark_theme,
            light_theme,
            accent,
            glass,
            auto_lock,
            history,
            secrets,
            prod,
            dark_themes,
            light_themes,
            accents,
            theme_errors,
            pending_appearance: None,
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

    /// Changes a theme setting and repaints the app in it.
    fn change_theme(&mut self, f: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        self.change(f, cx);
        let Some(settings) = self.settings(cx) else {
            return;
        };
        theme::set(theme::from_settings(&settings, cx), cx);
        let want = Appearance::from_id(&settings.appearance);
        let ix = Appearance::ALL.iter().position(|a| *a == want);
        if ix != self.appearance.read(cx).selected_index(cx).map(|ix| ix.row) {
            self.pending_appearance = ix;
        }
        cx.notify();
    }

    fn settings(&self, cx: &App) -> Option<Settings> {
        self.workspace
            .read_with(cx, |ws, _| ws.settings.clone())
            .ok()
    }

    /// Re-reads the theme files (after "Open folder").
    fn reload_themes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(settings) = self.settings(cx) else {
            return;
        };
        let (themes, errors) = theme::themes();
        self.theme_errors = errors;
        self.dark_themes = ThemeList::of(&themes, false);
        self.light_themes = ThemeList::of(&themes, true);
        let (dark_items, dark_ix) = (
            self.dark_themes.labels.clone(),
            self.dark_themes.index_of(&settings.theme),
        );
        let (light_items, light_ix) = (
            self.light_themes.labels.clone(),
            self.light_themes.index_of(&settings.light_theme),
        );
        self.dark_theme.update(cx, |state, cx| {
            state.set_items(dark_items, window, cx);
            state.set_selected_index(dark_ix, window, cx);
        });
        self.light_theme.update(cx, |state, cx| {
            state.set_items(light_items, window, cx);
            state.set_selected_index(light_ix, window, cx);
        });
        theme::set(theme::from_settings(&settings, cx), cx);
        cx.notify();
    }

    /// Opens the themes folder, first writing an example to copy from when
    /// it has no themes yet.
    fn open_themes_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dir) = theme::themes_dir() else {
            return;
        };
        let empty = std::fs::read_dir(&dir).map_or(true, |entries| {
            !entries
                .flatten()
                .any(|e| e.path().extension().is_some_and(|x| x == "toml"))
        });
        if empty {
            let example = theme::builtin()
                .into_iter()
                .next()
                .expect("a built-in theme");
            let written = std::fs::create_dir_all(&dir)
                .and_then(|_| std::fs::write(dir.join("my-theme.toml"), example.to_toml()));
            if let Err(e) = written {
                self.theme_errors = vec![format!("{}: {e}", dir.display())];
                cx.notify();
                return;
            }
        }
        cx.open_with_system(&dir);
        self.reload_themes(window, cx);
    }

    /// Relabels the pickers after a language change.
    fn retranslate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (themes, _) = theme::themes();
        self.dark_themes = ThemeList::of(&themes, false);
        self.light_themes = ThemeList::of(&themes, true);
        let accents: Vec<SharedString> = self
            .accents
            .iter()
            .map(|a| theme::accent_label(a))
            .collect();
        let lists = [
            (&self.appearance, appearance_choices()),
            (&self.dark_theme, self.dark_themes.labels.clone()),
            (&self.light_theme, self.light_themes.labels.clone()),
            (&self.accent, accents),
            (&self.glass, history_choices()),
            (
                &self.auto_lock,
                AUTO_LOCK_MINUTES.map(auto_lock_label).to_vec(),
            ),
            (&self.history, history_choices()),
            (&self.secrets, secrets_choices()),
            (&self.prod, history_choices()),
        ];
        for (picker, items) in lists {
            picker.update(cx, |state, cx| state.set_items(items, window, cx));
        }
        cx.notify();
    }

    fn row(
        &self,
        title: SharedString,
        description: SharedString,
        control: impl IntoElement,
    ) -> impl IntoElement {
        let p = crate::theme::current();
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

    fn heading(&self, text: SharedString) -> impl IntoElement {
        div()
            .pt_3()
            .pb_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(crate::theme::current().accent)
            .child(text)
    }
}

fn log_save_error(e: &std::io::Error) {
    eprintln!("apix: could not save settings: {e}");
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(ix) = self.pending_appearance.take() {
            self.appearance.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(ix)), window, cx)
            });
        }
        let p = crate::theme::current();
        let line = || div().w_full().flex_none().h_px().bg(p.hairline);
        let folder = theme::themes_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        v_flex()
            .id("settings-rows")
            .max_h(px(560.))
            .overflow_y_scroll()
            .pr_2()
            .child(self.heading(tr!("settings.section_general")))
            .child(self.row(
                tr!("settings.language"),
                tr!("settings.language_description"),
                Select::new(&self.language).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.auto_lock"),
                tr!("settings.auto_lock_description"),
                Select::new(&self.auto_lock).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.secrets"),
                tr!("settings.secrets_description"),
                Select::new(&self.secrets).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.confirm_prod"),
                tr!("settings.confirm_prod_description"),
                Select::new(&self.prod).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.history"),
                tr!("settings.history_description"),
                Select::new(&self.history).small(),
            ))
            .child(self.heading(tr!("settings.section_look")))
            .child(self.row(
                tr!("settings.appearance"),
                tr!("settings.appearance_description"),
                Select::new(&self.appearance).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.dark_theme"),
                tr!("settings.dark_theme_description"),
                Select::new(&self.dark_theme).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.light_theme"),
                tr!("settings.light_theme_description"),
                Select::new(&self.light_theme).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.accent"),
                tr!("settings.accent_description"),
                Select::new(&self.accent).small(),
            ))
            .child(line())
            .child(self.row(
                tr!("settings.glass"),
                tr!("settings.glass_description"),
                Select::new(&self.glass).small(),
            ))
            .child(line())
            .child(
                self.row(
                    tr!("settings.theme_files"),
                    tr!("settings.theme_files_description", folder = folder),
                    Button::new("settings-themes-folder")
                        .small()
                        .outline()
                        .w_full()
                        .label(tr!("settings.theme_files_open"))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.open_themes_folder(window, cx)
                        })),
                ),
            )
            .children(self.theme_errors.iter().map(|error| {
                div()
                    .ltr()
                    .text_xs()
                    .text_color(p.err)
                    .child(tr!("settings.theme_file_error", error = error))
            }))
    }
}

impl Workspace {
    pub(super) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let settings = self.settings.clone();
        let editor = cx.new(|cx| SettingsView::new(view.clone(), &settings, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            let close_view = view.clone();
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .bidi_title(tr!("settings.title"))
                .width(px(640.))
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
