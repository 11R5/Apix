//! The macOS menu bar. On a Mac the commands live here instead of in the title
//! bar; on Windows and Linux `menu_bar.rs` shows the same menus in the window.
//!
//! A command with a shortcut uses its action, so the menu shows the shortcut. The
//! rest run through `Run`, an index into the closures of the current menus.

use std::path::Path;
use std::rc::Rc;

use apix_codegen::{Format, Lang, Shell};
use apix_core::GroupBy;
use gpui_kit::component::input;
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use super::menu_bar::{MenuState, REPO};
use super::*;
use crate::tr_plain;

type Command = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

actions!(apix_macos, [Hide, HideOthers, ShowAll, Quit, Label]);

#[derive(Clone, PartialEq, Action)]
#[action(namespace = apix_macos, no_json)]
struct Run {
    index: usize,
}

/// What the menu bar shows now and the commands behind it.
#[derive(Default)]
struct NativeMenu {
    /// The language and state the menus were built from; `None` rebuilds them.
    shown: Option<(String, MenuState)>,
    commands: Vec<Command>,
    target: Option<(AnyWindowHandle, WeakEntity<Workspace>)>,
}

impl Global for NativeMenu {}

/// Registers what the menus run. Call before the window opens: `cmd-q` must be
/// bound when the first render builds the menus, so they show it. `Label` has no
/// handler on purpose, which is what greys out a heading.
pub fn init(cx: &mut App) {
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|run: &Run, cx| {
        let menu = cx.default_global::<NativeMenu>();
        menu.shown = None;
        let (Some((window, view)), Some(command)) =
            (menu.target.clone(), menu.commands.get(run.index).cloned())
        else {
            return;
        };
        window
            .update(cx, |_, window, cx| {
                view.update(cx, |ws, cx| {
                    command(ws, window, cx);
                    cx.notify();
                })
            })
            .ok();
    });
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
}

/// Rebuilds the menus when what they show has changed. Called on every render.
pub(super) fn refresh(ws: &Workspace, window: &Window, cx: &mut Context<Workspace>) {
    let shown = (rust_i18n::locale().to_string(), ws.menu_state(cx));
    if cx
        .try_global::<NativeMenu>()
        .is_some_and(|menu| menu.shown.as_ref() == Some(&shown))
    {
        return;
    }
    let mut builder = Builder::default();
    let menus = builder.menus(&shown.1);
    cx.set_menus(menus);
    let target = Some((window.window_handle(), shown.1.view.clone()));
    cx.set_global(NativeMenu {
        shown: Some(shown),
        commands: builder.commands,
        target,
    });
}

#[derive(Default)]
struct Builder {
    commands: Vec<Command>,
}

impl Builder {
    fn run(
        &mut self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> MenuItem {
        let index = self.commands.len();
        self.commands.push(Rc::new(f));
        MenuItem::action(label, Run { index })
    }

    fn check(
        &mut self,
        label: impl Into<SharedString>,
        on: bool,
        f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> MenuItem {
        let mut item = self.run(label, f);
        if let MenuItem::Action { checked, .. } = &mut item {
            *checked = on;
        }
        item
    }

    fn link(&mut self, label: impl Into<SharedString>, url: impl Into<String>) -> MenuItem {
        let url = url.into();
        self.run(label, move |_, _, cx| cx.open_url(&url))
    }

    fn menus(&mut self, state: &MenuState) -> Vec<Menu> {
        vec![
            self.app(),
            self.file(state),
            edit(),
            self.request(),
            self.environment(state),
            view(),
            self.help(),
        ]
    }

    fn app(&mut self) -> Menu {
        Menu::new("Apix").items([
            self.link(tr_plain!("menu.about_apix"), REPO),
            MenuItem::separator(),
            MenuItem::action(tr_plain!("menu.settings"), OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu(tr_plain!("menu.services"), SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(tr_plain!("menu.hide_apix"), Hide),
            MenuItem::action(tr_plain!("menu.hide_others"), HideOthers),
            MenuItem::action(tr_plain!("menu.show_all"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(tr_plain!("menu.quit"), Quit),
        ])
    }

    fn file(&mut self, state: &MenuState) -> Menu {
        let mut items = Vec::new();
        if state.open {
            let methods = Method::ALL
                .into_iter()
                .map(|method| {
                    self.run(method.as_str(), move |ws, window, cx| {
                        ws.new_request(None, method, window, cx)
                    })
                })
                .collect::<Vec<_>>();
            items.push(MenuItem::action(tr_plain!("menu.new_request"), NewRequest));
            items.push(MenuItem::submenu(
                Menu::new(tr_plain!("menu.new_request_with_method")).items(methods),
            ));
            items.push(self.run(tr_plain!("menu.new_folder"), |ws, window, cx| {
                ws.new_folder(None, window, cx)
            }));
        }
        items.push(self.run(
            tr_plain!("menu.new_collection"),
            Workspace::prompt_new_collection,
        ));
        items.push(self.run(tr_plain!("menu.new_project"), Workspace::prompt_new_project));
        if state.has_project {
            items.push(self.run(
                tr_plain!("menu.new_encrypted_collection"),
                Workspace::prompt_new_encrypted_collection,
            ));
        }
        items.push(MenuItem::separator());
        items.push(MenuItem::action(tr_plain!("menu.open"), OpenCollection));
        if !state.recent.is_empty() {
            let recent = state
                .recent
                .iter()
                .map(|dir| {
                    let target = dir.clone();
                    self.run(folder_name(dir), move |ws, window, cx| {
                        ws.open_or_create(target.clone(), window, cx)
                    })
                })
                .collect::<Vec<_>>();
            items.push(MenuItem::submenu(
                Menu::new(tr_plain!("menu.open_recent")).items(recent),
            ));
        }
        items.push(MenuItem::action(
            tr_plain!("menu.import_postman"),
            ImportFile,
        ));
        items.push(MenuItem::action(tr_plain!("menu.import_url"), ImportUrl));
        if state.open {
            items.push(self.run(
                tr_plain!("menu.export_postman"),
                Workspace::export_active_to_postman,
            ));
            items.push(self.run(tr_plain!("menu.export_docs"), Workspace::export_active_docs));
        }
        if let Some(root) = state.plain_collection.clone() {
            items.push(MenuItem::separator());
            items.push(self.run(
                tr_plain!("menu.move_into_project"),
                Workspace::prompt_move_into_project,
            ));
            items.push(self.run(
                tr_plain!("menu.encrypt_collection"),
                move |ws, window, cx| ws.prompt_encrypt(root.clone(), window, cx),
            ));
        }
        if state.unlocked {
            items.push(MenuItem::separator());
            items.push(self.run(
                tr_plain!("menu.lock_collections"),
                Workspace::lock_collections,
            ));
            let remembered: Vec<PathBuf> = apix_core::vfs::unlocked_roots()
                .into_iter()
                .filter(|root| apix_core::vfs::keychain::load(root).is_some())
                .collect();
            let several = remembered.len() > 1;
            for root in remembered {
                let forget = tr_plain!("sidebar.forget_password");
                let label = if several {
                    format!("{forget} · {}", folder_name(&root))
                } else {
                    forget.to_string()
                };
                items.push(self.run(label, move |ws, window, cx| {
                    ws.forget_saved_password(root.clone(), window, cx)
                }));
            }
        }
        if state.open {
            items.push(MenuItem::separator());
            items.push(MenuItem::action(tr_plain!("menu.save"), SaveRequest));
            items.push(MenuItem::action(tr_plain!("menu.close_tab"), CloseTab));
        }
        Menu::new(tr_plain!("menu.file")).items(items)
    }

    fn request(&mut self) -> Menu {
        let formats = Shell::ALL
            .into_iter()
            .map(Format::Shell)
            .map(|format| self.copy_as(format))
            .chain([MenuItem::separator()])
            .collect::<Vec<_>>()
            .into_iter()
            .chain(
                Lang::ALL
                    .into_iter()
                    .map(Format::Code)
                    .map(|format| self.copy_as(format))
                    .collect::<Vec<_>>(),
            );
        let copy_as = Menu::new(tr_plain!("menu.copy_as")).items(formats);
        // Esc stays out of the menu: it has to reach the window, which also closes
        // pickers and dialogs with it.
        let cancel = self.run(tr_plain!("menu.cancel"), Workspace::cancel);
        let cookies = self.run(tr_plain!("menu.cookies"), |ws, window, cx| {
            ws.open_cookies(window, cx)
        });
        Menu::new(tr_plain!("menu.request")).items([
            MenuItem::action(tr_plain!("menu.send"), SendRequest),
            cancel,
            MenuItem::action(tr_plain!("menu.load_test"), OpenLoadTest),
            MenuItem::action(tr_plain!("menu.compare"), CompareResponses),
            MenuItem::separator(),
            MenuItem::submenu(Menu::new(tr_plain!("menu.method")).items([
                MenuItem::action("GET", MethodGet),
                MenuItem::action("POST", MethodPost),
                MenuItem::action("PUT", MethodPut),
                MenuItem::action("PATCH", MethodPatch),
                MenuItem::action("DELETE", MethodDelete),
                MenuItem::action("HEAD", MethodHead),
                MenuItem::action("OPTIONS", MethodOptions),
            ])),
            MenuItem::submenu(copy_as),
            MenuItem::separator(),
            cookies,
        ])
    }

    fn copy_as(&mut self, format: Format) -> MenuItem {
        self.run(format.label(), move |ws, window, cx| {
            ws.copy_current_as(format, window, cx)
        })
    }

    fn environment(&mut self, state: &MenuState) -> Menu {
        let mut items = vec![MenuItem::action(tr_plain!("menu.variables"), EditVariables)];
        let menu = Menu::new(tr_plain!("menu.environment"));
        if !state.open {
            return menu.items(items);
        }
        let mut pick = vec![self.check(
            tr_plain!("env.none"),
            state.environment.is_none(),
            |ws, window, cx| ws.set_environment(None, window, cx),
        )];
        pick.push(MenuItem::separator());
        for name in &state.environments {
            let env = Some(name.clone());
            let on = state.environment == env;
            pick.push(self.check(name.clone(), on, move |ws, window, cx| {
                ws.set_environment(env.clone(), window, cx)
            }));
        }
        items.push(MenuItem::submenu(
            Menu::new(tr_plain!("menu.active_environment")).items(pick),
        ));
        items.push(MenuItem::separator());
        let (auth, settings) = if state.has_project {
            (
                tr_plain!("menu.project_auth"),
                tr_plain!("menu.project_settings"),
            )
        } else {
            (
                tr_plain!("menu.collection_auth"),
                tr_plain!("menu.collection_settings"),
            )
        };
        items.push(self.run(auth, |ws, window, cx| {
            if let Some(owner) = auth_owner(ws) {
                ws.open_auth_dialog(owner, window, cx)
            }
        }));
        items.push(self.run(settings, |ws, window, cx| {
            if let Some(owner) = auth_owner(ws) {
                ws.open_settings_dialog(owner, window, cx)
            }
        }));
        if let Some(group_by) = state.group_by {
            items.push(MenuItem::separator());
            items.push(MenuItem::action(tr_plain!("menu.group_by"), Label));
            for (label, by) in [
                (tr_plain!("menu.group_collection"), GroupBy::Collection),
                (tr_plain!("menu.group_environment"), GroupBy::Environment),
            ] {
                items.push(self.check(label, group_by == by, move |ws, window, cx| {
                    ws.set_group_by(by, window, cx)
                }));
            }
        }
        menu.items(items)
    }

    fn help(&mut self) -> Menu {
        Menu::new(tr_plain!("menu.help")).items([
            self.link(tr_plain!("menu.documentation"), format!("{REPO}#readme")),
            self.link(tr_plain!("menu.report_problem"), format!("{REPO}/issues")),
            self.link(tr_plain!("menu.source_code"), REPO),
            MenuItem::separator(),
            MenuItem::action(tr_plain!("menu.setup"), OpenSetup),
            MenuItem::action(tr_plain!("menu.connect_claude"), ConnectClaude),
            MenuItem::action(tr_plain!("menu.check_updates"), CheckForUpdates),
            MenuItem::action(format!("Apix {}", env!("CARGO_PKG_VERSION")), Label),
        ])
    }
}

fn edit() -> Menu {
    Menu::new(tr_plain!("menu.edit")).items([
        MenuItem::os_action(tr_plain!("menu.undo"), input::Undo, OsAction::Undo),
        MenuItem::os_action(tr_plain!("menu.redo"), input::Redo, OsAction::Redo),
        MenuItem::separator(),
        MenuItem::os_action(tr_plain!("menu.cut"), input::Cut, OsAction::Cut),
        MenuItem::os_action(tr_plain!("menu.copy"), input::Copy, OsAction::Copy),
        MenuItem::os_action(tr_plain!("menu.paste"), input::Paste, OsAction::Paste),
        MenuItem::os_action(
            tr_plain!("menu.select_all"),
            input::SelectAll,
            OsAction::SelectAll,
        ),
    ])
}

fn view() -> Menu {
    Menu::new(tr_plain!("menu.view")).items([
        MenuItem::action(tr_plain!("menu.command_palette"), OpenPalette),
        MenuItem::action(tr_plain!("menu.history"), OpenHistory),
        MenuItem::action(tr_plain!("menu.runner"), OpenRunner),
        MenuItem::action(tr_plain!("menu.flows"), OpenFlows),
        MenuItem::action(tr_plain!("menu.monitors"), OpenMonitors),
        MenuItem::action(tr_plain!("menu.mock"), OpenMock),
        MenuItem::action(tr_plain!("menu.capture"), OpenCapture),
        MenuItem::action(tr_plain!("menu.git"), OpenGit),
        MenuItem::action(tr_plain!("menu.route"), OpenRoute),
        MenuItem::action(tr_plain!("menu.plugins"), OpenPlugins),
        MenuItem::separator(),
        MenuItem::action(tr_plain!("menu.next_tab"), NextTab),
        MenuItem::action(tr_plain!("menu.previous_tab"), PrevTab),
    ])
}

fn auth_owner(ws: &Workspace) -> Option<AuthOwner> {
    match (&ws.project, ws.collections.first()) {
        (Some(p), _) => Some(AuthOwner::Project(p.root.clone())),
        (None, Some(c)) => Some(AuthOwner::Collection(c.root.clone())),
        (None, None) => None,
    }
}

fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string())
}
