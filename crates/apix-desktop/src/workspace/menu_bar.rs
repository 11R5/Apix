//! The menu bar in the title bar: File, Request, Environment, View, Help.
//! Commands with a shortcut go through their action, so the menu shows the
//! shortcut and runs exactly what the key runs.

use std::path::PathBuf;

use apix_codegen::Shell;
use apix_core::GroupBy;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Sizable, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::auth_editor::AuthOwner;
use super::collections::method_items;
use super::*;
use crate::bidi::{self, Bidi as _};

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
    /// Some encrypted collection is unlocked.
    unlocked: bool,
    /// The open collection, when it is a single plain one that could be encrypted.
    plain_collection: Option<PathBuf>,
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
                .menu(tr!("menu.new_request"), Box::new(NewRequest))
                .submenu(
                    tr!("menu.new_request_with_method"),
                    window,
                    cx,
                    move |menu, _, _| method_items(menu, view.clone(), None),
                )
                .item(self.item(tr!("menu.new_folder"), |ws, window, cx| {
                    ws.new_folder(None, window, cx)
                }));
        }
        menu = menu
            .item(self.item(tr!("menu.new_collection"), Workspace::prompt_new_collection))
            .item(self.item(tr!("menu.new_project"), Workspace::prompt_new_project))
            .when(self.has_project, |menu| {
                menu.item(self.item(
                    tr!("menu.new_encrypted_collection"),
                    Workspace::prompt_new_encrypted_collection,
                ))
            })
            .separator()
            .menu(tr!("menu.open"), Box::new(OpenCollection))
            .menu(tr!("menu.settings"), Box::new(OpenSettings));
        if !self.recent.is_empty() {
            let (recent, view) = (self.recent.clone(), self.view.clone());
            menu = menu.submenu(
                tr!("menu.open_recent"),
                window,
                cx,
                move |mut menu, _, _| {
                    for dir in &recent {
                        let (view, target) = (view.clone(), dir.clone());
                        let name = dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| dir.display().to_string());
                        menu =
                            menu.item(PopupMenuItem::new(name).on_click(move |_, window, cx| {
                                view.update(cx, |this, cx| {
                                    this.open_or_create(target.clone(), window, cx)
                                })
                                .ok();
                            }));
                    }
                    menu
                },
            );
        }
        menu = menu.menu(tr!("menu.import_postman"), Box::new(ImportFile));
        if let Some(root) = self.plain_collection.clone() {
            menu = menu.separator().item(
                self.item(tr!("menu.encrypt_collection"), move |ws, window, cx| {
                    ws.prompt_encrypt(root.clone(), window, cx)
                }),
            );
        }
        if self.unlocked {
            menu = menu
                .separator()
                .item(self.item(tr!("menu.lock_collections"), Workspace::lock_collections));
        }
        if self.open {
            menu = menu
                .separator()
                .menu(tr!("menu.save"), Box::new(SaveRequest))
                .menu(tr!("menu.close_tab"), Box::new(CloseTab));
        }
        menu.separator()
            .item(PopupMenuItem::new(tr!("menu.quit")).on_click(|_, _, cx| cx.quit()))
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
            .menu(tr!("menu.send"), Box::new(SendRequest))
            .menu(tr!("menu.cancel"), Box::new(CancelRequest))
            .separator()
            .submenu(tr!("menu.method"), window, cx, move |menu, _, _| {
                menu.action_context(focus.clone())
                    .menu("GET", Box::new(MethodGet))
                    .menu("POST", Box::new(MethodPost))
                    .menu("PUT", Box::new(MethodPut))
                    .menu("PATCH", Box::new(MethodPatch))
                    .menu("DELETE", Box::new(MethodDelete))
                    .menu("HEAD", Box::new(MethodHead))
                    .menu("OPTIONS", Box::new(MethodOptions))
            })
            .submenu(tr!("menu.copy_as"), window, cx, move |menu, _, _| {
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
            .menu(tr!("menu.variables"), Box::new(EditVariables));
        if !self.open {
            return menu;
        }
        let (view, current, names) = (
            self.view.clone(),
            self.environment.clone(),
            self.environments.clone(),
        );
        menu = menu.submenu(
            tr!("menu.active_environment"),
            window,
            cx,
            move |menu, _, _| {
                let pick = |label: String, env: Option<String>| {
                    let view = view.clone();
                    PopupMenuItem::new(label).checked(current == env).on_click(
                        move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.set_environment(env.clone(), window, cx)
                            })
                            .ok();
                        },
                    )
                };
                names.iter().fold(
                    menu.item(pick(tr!("env.none").to_string(), None))
                        .separator(),
                    |menu, name| menu.item(pick(name.clone(), Some(name.clone()))),
                )
            },
        );
        let auth = if self.has_project {
            tr!("menu.project_auth")
        } else {
            tr!("menu.collection_auth")
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
            let group = |label: SharedString, by: GroupBy| {
                self.item(label, move |ws, window, cx| ws.set_group_by(by, window, cx))
                    .checked(group_by == by)
            };
            menu = menu
                .separator()
                .label(tr!("menu.group_by"))
                .item(group(tr!("menu.group_collection"), GroupBy::Collection))
                .item(group(tr!("menu.group_environment"), GroupBy::Environment));
        }
        menu
    }

    fn view_menu(&self, menu: PopupMenu) -> PopupMenu {
        menu.action_context(self.focus.clone())
            .menu(tr!("menu.command_palette"), Box::new(OpenPalette))
            .separator()
            .menu(tr!("menu.next_tab"), Box::new(NextTab))
            .menu(tr!("menu.previous_tab"), Box::new(PrevTab))
    }

    fn help(&self, menu: PopupMenu) -> PopupMenu {
        menu.link(tr!("menu.documentation"), format!("{REPO}#readme"))
            .link(tr!("menu.report_problem"), format!("{REPO}/issues"))
            .link(tr!("menu.source_code"), REPO)
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
            unlocked: !apix_core::vfs::unlocked_roots().is_empty(),
            plain_collection: match (&self.project, self.collections.first()) {
                (None, Some(c)) if !apix_core::vfs::is_virtual(&c.root) => Some(c.root.clone()),
                _ => None,
            },
            recent: self
                .app_state
                .recent_collections()
                .filter(|r| Some(*r) != self.workspace_root().as_ref())
                .take(8)
                .cloned()
                .collect(),
        };
        let button =
            |id: &'static str, label: SharedString| Button::new(id).ghost().xsmall().label(label);
        let (file, request, env, view, help) = (
            state.clone(),
            state.clone(),
            state.clone(),
            state.clone(),
            state,
        );
        h_flex()
            .id("menu-bar")
            .dir_row()
            .occlude()
            .gap_px()
            .items_center()
            .child(
                button("menu-file", tr!("menu.file"))
                    .dropdown_menu_with_anchor(bidi::menu_anchor(), move |menu, window, cx| {
                        file.file(menu, window, cx)
                    }),
            )
            .child(
                button("menu-request", tr!("menu.request"))
                    .dropdown_menu_with_anchor(bidi::menu_anchor(), move |menu, window, cx| {
                        request.request(menu, window, cx)
                    }),
            )
            .child(
                button("menu-environment", tr!("menu.environment"))
                    .dropdown_menu_with_anchor(bidi::menu_anchor(), move |menu, window, cx| {
                        env.environment(menu, window, cx)
                    }),
            )
            .child(
                button("menu-view", tr!("menu.view"))
                    .dropdown_menu_with_anchor(bidi::menu_anchor(), move |menu, _, _| {
                        view.view_menu(menu)
                    }),
            )
            .child(
                button("menu-help", tr!("menu.help"))
                    .dropdown_menu_with_anchor(bidi::menu_anchor(), move |menu, _, _| {
                        help.help(menu)
                    }),
            )
    }
}
