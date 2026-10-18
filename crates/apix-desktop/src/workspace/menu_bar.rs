//! The menu bar in the title bar: File, Request, Environment, View, Help.
//! Commands with a shortcut go through their action, so the menu shows the
//! shortcut and runs exactly what the key runs.

use std::path::PathBuf;

use apix_codegen::Shell;
use apix_core::GroupBy;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Sizable, h_flex};
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use super::collections::method_items;
use super::*;

const REPO: &str = "https://github.com/11R5/Apix";

/// What the menus show, read when the bar renders.
#[derive(Clone)]
struct MenuState {
    view: WeakEntity<Workspace>,
    focus: FocusHandle,
    open: bool,
    has_project: bool,
    group_by: Option<GroupBy>,
    environment: Option<String>,
    environments: Vec<String>,
    recent: Vec<PathBuf>,
}

impl MenuState {
    /// A menu item that runs `f` on the workspace.
    fn item(
        &self,
        label: impl Into<SharedString>,
        f: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
    ) -> PopupMenuItem {
        let view = self.view.clone();
        PopupMenuItem::new(label).on_click(move |_, window, cx| {
            view.update(cx, |this, cx| f(this, window, cx)).ok();
        })
    }

    fn file(&self, menu: PopupMenu, window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
        let mut menu = menu.action_context(self.focus.clone());
        if self.open {
            let view = self.view.clone();
            menu = menu
                .menu("New request", Box::new(NewRequest))
                .submenu("New request with method", window, cx, move |menu, _, _| {
                    method_items(menu, view.clone(), None)
                })
                .item(self.item("New folder…", |ws, window, cx| {
                    ws.new_folder(None, window, cx)
                }));
        }
        menu = menu
            .item(self.item("New collection…", Workspace::prompt_new_collection))
            .item(self.item("New project…", Workspace::prompt_new_project))
            .separator()
            .menu("Open…", Box::new(OpenCollection));
        if !self.recent.is_empty() {
            let (recent, view) = (self.recent.clone(), self.view.clone());
            menu = menu.submenu("Open recent", window, cx, move |mut menu, _, _| {
                for dir in &recent {
                    let (view, target) = (view.clone(), dir.clone());
                    let name = dir
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| dir.display().to_string());
                    menu = menu.item(PopupMenuItem::new(name).on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| {
                            this.open_or_create(target.clone(), window, cx)
                        })
                        .ok();
                    }));
                }
                menu
            });
        }
        menu = menu.menu("Import from Postman…", Box::new(ImportFile));
        if self.open {
            menu = menu
                .separator()
                .menu("Save", Box::new(SaveRequest))
                .menu("Close tab", Box::new(CloseTab));
        }
        menu.separator()
            .item(PopupMenuItem::new("Quit").on_click(|_, _, cx| cx.quit()))
    }

    fn request(
        &self,
        menu: PopupMenu,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let view = self.view.clone();
        let focus = self.focus.clone();
        menu.action_context(self.focus.clone())
            .menu("Send", Box::new(SendRequest))
            .menu("Cancel", Box::new(CancelRequest))
            .separator()
            .submenu("Method", window, cx, move |menu, _, _| {
                menu.action_context(focus.clone())
                    .menu("GET", Box::new(MethodGet))
                    .menu("POST", Box::new(MethodPost))
                    .menu("PUT", Box::new(MethodPut))
                    .menu("PATCH", Box::new(MethodPatch))
                    .menu("DELETE", Box::new(MethodDelete))
                    .menu("HEAD", Box::new(MethodHead))
                    .menu("OPTIONS", Box::new(MethodOptions))
            })
            .submenu("Copy as", window, cx, move |menu, _, _| {
                Shell::ALL.into_iter().fold(menu, |menu, shell| {
                    let view = view.clone();
                    menu.item(
                        PopupMenuItem::new(shell.label()).on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| this.copy_current_as(shell, window, cx))
                                .ok();
                        }),
                    )
                })
            })
    }

    fn environment(
        &self,
        menu: PopupMenu,
        window: &mut Window,
        cx: &mut Context<PopupMenu>,
    ) -> PopupMenu {
        let mut menu = menu
            .action_context(self.focus.clone())
            .menu("Variables and environments…", Box::new(EditVariables));
        if !self.open {
            return menu;
        }
        let (view, current, names) = (
            self.view.clone(),
            self.environment.clone(),
            self.environments.clone(),
        );
        menu = menu.submenu("Active environment", window, cx, move |menu, _, _| {
            let pick = |label: String, env: Option<String>| {
                let view = view.clone();
                PopupMenuItem::new(label)
                    .checked(current == env)
                    .on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| this.set_environment(env.clone(), window, cx))
                            .ok();
                    })
            };
            names.iter().fold(
                menu.item(pick("No environment".into(), None)).separator(),
                |menu, name| menu.item(pick(name.clone(), Some(name.clone()))),
            )
        });
        let auth = if self.has_project {
            "Project auth…"
        } else {
            "Collection auth…"
        };
        menu = menu.separator().item(self.item(auth, |ws, window, cx| {
            let owner = match (&ws.project, ws.collections.first()) {
                (Some(p), _) => AuthOwner::Project(p.root.clone()),
                (None, Some(c)) => AuthOwner::Collection(c.root.clone()),
                (None, None) => return,
            };
            ws.open_auth_dialog(owner, window, cx)
        }));
        if let Some(group_by) = self.group_by {
            let group = |label: &str, by: GroupBy| {
                self.item(label.to_owned(), move |ws, window, cx| {
                    ws.set_group_by(by, window, cx)
                })
                .checked(group_by == by)
            };
            menu = menu
                .separator()
                .label("Group the sidebar by")
                .item(group("Collection", GroupBy::Collection))
                .item(group("Environment", GroupBy::Environment));
        }
        menu
    }

    fn view_menu(&self, menu: PopupMenu) -> PopupMenu {
        menu.action_context(self.focus.clone())
            .menu("Command palette…", Box::new(OpenPalette))
            .separator()
            .menu("Next tab", Box::new(NextTab))
            .menu("Previous tab", Box::new(PrevTab))
    }

    fn help(&self, menu: PopupMenu) -> PopupMenu {
        menu.link("Documentation", format!("{REPO}#readme"))
            .link("Report a problem", format!("{REPO}/issues"))
            .link("Source code", REPO)
            .separator()
            .label(format!("Apix {}", env!("CARGO_PKG_VERSION")))
    }
}

impl Workspace {
    /// The File · Request · Environment · View · Help buttons. They occlude the
    /// title bar's drag area, so on Windows a click opens the menu instead of
    /// grabbing the window.
    pub(super) fn render_menu_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = MenuState {
            view: cx.entity().downgrade(),
            focus: self.focus.clone(),
            open: self.workspace_root().is_some(),
            has_project: self.project.is_some(),
            group_by: self.project.as_ref().map(|p| p.meta.group_by),
            environment: self.environment.clone(),
            environments: self.environment_names(),
            recent: self
                .app_state
                .recent_collections()
                .filter(|r| Some(*r) != self.workspace_root().as_ref())
                .take(8)
                .cloned()
                .collect(),
        };
        let button =
            |id: &'static str, label: &'static str| Button::new(id).ghost().xsmall().label(label);
        let (file, request, env, view, help) = (
            state.clone(),
            state.clone(),
            state.clone(),
            state.clone(),
            state,
        );
        h_flex()
            .id("menu-bar")
            .occlude()
            .gap_px()
            .items_center()
            .child(
                button("menu-file", "File")
                    .dropdown_menu(move |menu, window, cx| file.file(menu, window, cx)),
            )
            .child(
                button("menu-request", "Request")
                    .dropdown_menu(move |menu, window, cx| request.request(menu, window, cx)),
            )
            .child(
                button("menu-environment", "Environment")
                    .dropdown_menu(move |menu, window, cx| env.environment(menu, window, cx)),
            )
            .child(
                button("menu-view", "View").dropdown_menu(move |menu, _, _| view.view_menu(menu)),
            )
            .child(button("menu-help", "Help").dropdown_menu(move |menu, _, _| help.help(menu)))
    }
}
