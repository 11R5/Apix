//! Encrypted collections in the app: unlock, lock (by hand, and after 15
//! idle minutes), create, encrypt or decrypt a collection, change its
//! password. The slow parts (Argon2, encrypting every file) run in the
//! background; the dialog shows progress and errors.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use apix_core::store::{self, CollectionMeta};
use apix_core::vfs::{self, KdfCost, RecoveryKey, Vault};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::collections::{Then, close_dialog, refocus};
use super::{Workspace, describe};
use crate::bidi::{Bidi as _, DialogBidi as _};
use crate::theme::Palette;
use crate::tr;

/// New passwords are at least this long.
const MIN_PASSWORD: usize = 8;

/// What a password dialog asks for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    /// The password (or the recovery key) of an existing collection.
    Unlock,
    /// A new password, typed twice.
    NewPassword,
    /// A collection name and a new password.
    NewCollection,
}

/// The text and kind of a password dialog.
struct Dialog {
    title: SharedString,
    ok: SharedString,
    ask: Ask,
    note: SharedString,
}

/// The finished background job of a password dialog.
enum Outcome {
    Unlocked(Vault),
    Created(Vault, RecoveryKey),
    Done,
}

type Job = Box<dyn FnOnce() -> Result<Outcome, String> + Send>;
type MakeJob = Rc<dyn Fn(Secret) -> Result<Job, String>>;
type Submit = Rc<dyn Fn(&mut Window, &mut App)>;
type OnDone = Rc<dyn Fn(&mut Workspace, Outcome, &mut Window, &mut Context<Workspace>)>;

/// What the user typed.
struct Secret {
    name: String,
    password: String,
    recovery: bool,
}

pub(super) struct PasswordForm {
    palette: Palette,
    ask: Ask,
    name: Option<Entity<InputState>>,
    password: Entity<InputState>,
    confirm: Option<Entity<InputState>>,
    recovery: bool,
    note: SharedString,
    error: Option<String>,
    busy: bool,
    submit: Option<Submit>,
    _subscriptions: Vec<Subscription>,
}

impl PasswordForm {
    fn new(
        palette: Palette,
        ask: Ask,
        note: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let masked = |placeholder: SharedString, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .masked(true)
            })
        };
        let name = (ask == Ask::NewCollection).then(|| {
            cx.new(|cx| InputState::new(window, cx).placeholder(tr!("vault.name_placeholder")))
        });
        let password = masked(
            if ask == Ask::Unlock {
                tr!("auth.field.password")
            } else {
                tr!("vault.new_password_placeholder", count = MIN_PASSWORD)
            },
            window,
            cx,
        );
        let confirm =
            (ask != Ask::Unlock).then(|| masked(tr!("vault.repeat_password"), window, cx));
        let mut subscriptions = Vec::new();
        for field in name.iter().chain([&password]).chain(confirm.iter()) {
            subscriptions.push(cx.subscribe_in(
                field,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event
                        && let Some(submit) = this.submit.clone()
                    {
                        // After this update: submitting reads the form again.
                        window.defer(cx, move |window, cx| submit(window, cx));
                    }
                },
            ));
        }
        Self {
            palette,
            ask,
            name,
            password,
            confirm,
            recovery: false,
            note,
            error: None,
            busy: false,
            submit: None,
            _subscriptions: subscriptions,
        }
    }

    /// The typed values, or why they cannot be used.
    fn secret(&self, cx: &App) -> Result<Secret, String> {
        let text = |field: &Entity<InputState>| field.read(cx).value().to_string();
        let password = text(&self.password);
        let name = self.name.as_ref().map(text).unwrap_or_default();
        if self.ask == Ask::NewCollection && name.trim().is_empty() {
            return Err(tr!("vault.error.no_name").into());
        }
        if password.is_empty() {
            return Err(if self.recovery {
                tr!("vault.error.no_recovery_key").into()
            } else {
                tr!("vault.error.no_password").into()
            });
        }
        if let Some(confirm) = &self.confirm {
            if password.chars().count() < MIN_PASSWORD {
                return Err(tr!("vault.error.too_short", count = MIN_PASSWORD).into());
            }
            if text(confirm) != password {
                return Err(tr!("vault.error.mismatch").into());
            }
        }
        Ok(Secret {
            name: name.trim().to_owned(),
            password,
            recovery: self.recovery,
        })
    }

    fn toggle_recovery(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.recovery = !self.recovery;
        self.error = None;
        let recovery = self.recovery;
        self.password.update(cx, |state, cx| {
            state.set_masked(!recovery, window, cx);
            state.set_value("", window, cx);
            state.set_placeholder(
                if recovery {
                    tr!("vault.recovery_placeholder")
                } else {
                    tr!("auth.field.password")
                },
                window,
                cx,
            );
            state.focus(window, cx);
        });
        cx.notify();
    }
}

impl Render for PasswordForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let password = if self.recovery {
            Input::new(&self.password)
        } else {
            Input::new(&self.password).mask_toggle()
        };
        v_flex()
            .gap_3()
            .when(!self.note.is_empty(), |col| {
                col.child(
                    div()
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(self.note.clone()),
                )
            })
            .children(self.name.as_ref().map(Input::new))
            .child(password)
            .children(self.confirm.as_ref().map(|c| Input::new(c).mask_toggle()))
            .when(self.ask == Ask::Unlock, |col| {
                col.child(
                    h_flex().dir_row().child(
                        Button::new("vault-recovery")
                            .ghost()
                            .xsmall()
                            .label(if self.recovery {
                                tr!("vault.use_password")
                            } else {
                                tr!("vault.use_recovery")
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.toggle_recovery(window, cx)
                            })),
                    ),
                )
            })
            .when_some(self.error.clone(), |col, error| {
                col.child(div().text_sm().text_color(p.err).child(error))
            })
            .when(self.busy, |col| {
                col.child(
                    div()
                        .text_sm()
                        .text_color(p.text_muted)
                        .child(tr!("vault.working")),
                )
            })
    }
}

impl Workspace {
    /// A dialog that collects a password, runs `make_job` in the background
    /// and hands its outcome to `done`, keeping the dialog open on errors.
    fn password_dialog(
        &mut self,
        spec: Dialog,
        make_job: MakeJob,
        done: OnDone,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let palette = self.palette;
        let Dialog {
            title,
            ok,
            ask,
            note,
        } = spec;
        let ok_label = ok;
        let form = cx.new(|cx| PasswordForm::new(palette, ask, note, window, cx));
        let view = cx.entity().downgrade();
        let submit: Submit = {
            let (form, view) = (form.downgrade(), view.clone());
            Rc::new(move |window, cx| {
                let Some(form) = form.upgrade() else { return };
                if form.read(cx).busy {
                    return;
                }
                let secret = form.read(cx).secret(cx);
                let job = secret.and_then(|s| make_job(s));
                let job = match job {
                    Ok(job) => job,
                    Err(message) => {
                        form.update(cx, |f, cx| {
                            f.error = Some(message);
                            cx.notify();
                        });
                        return;
                    }
                };
                form.update(cx, |f, cx| {
                    f.busy = true;
                    f.error = None;
                    cx.notify();
                });
                let (form, done) = (form.downgrade(), done.clone());
                view.update(cx, |_, cx| {
                    cx.spawn_in(window, async move |this, cx| {
                        let result = cx.background_spawn(async move { job() }).await;
                        this.update_in(cx, |ws, window, cx| match result {
                            Ok(outcome) => {
                                window.close_dialog(cx);
                                ws.focus.focus(window, cx);
                                done(ws, outcome, window, cx);
                            }
                            Err(message) => {
                                form.update(cx, |f, cx| {
                                    f.busy = false;
                                    f.error = Some(message);
                                    cx.notify();
                                })
                                .ok();
                            }
                        })
                        .ok();
                    })
                    .detach();
                })
                .ok();
            })
        };
        form.update(cx, |f, _| f.submit = Some(submit.clone()));
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            // The dialog takes focus when it opens; hand it to the first field once.
            if first.replace(false) {
                let first = {
                    let f = form.read(cx);
                    f.name.clone().unwrap_or_else(|| f.password.clone())
                };
                first.update(cx, |state, cx| state.focus(window, cx));
            }
            let (cancel_view, submit) = (view.clone(), submit.clone());
            dialog
                .on_close(refocus(view.clone()))
                .bidi_title(title.clone())
                .w(px(460.))
                .child(form.clone())
                .on_ok(|_, _, _| false)
                .footer(
                    h_flex()
                        .dir_row()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("vault-cancel")
                                .ghost()
                                .label(tr!("dialog.cancel"))
                                .on_click(move |_, window, cx| {
                                    close_dialog(&cancel_view, window, cx)
                                }),
                        )
                        .child(
                            Button::new("vault-ok")
                                .primary()
                                .label(ok_label.clone())
                                .on_click(move |_, window, cx| submit(window, cx)),
                        ),
                )
        });
    }

    /// Asks for the password of the encrypted collection at `root`, then runs `then`.
    pub(super) fn prompt_unlock(
        &mut self,
        root: PathBuf,
        then: Then,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = folder_name(&root);
        let target = root.clone();
        let make_job: MakeJob = Rc::new(move |secret: Secret| {
            let root = target.clone();
            Ok(Box::new(move || {
                let vault = if secret.recovery {
                    Vault::unlock_with_recovery(&root, &secret.password)
                } else {
                    Vault::unlock(&root, &secret.password)
                };
                vault.map(Outcome::Unlocked).map_err(|e| describe(&e))
            }) as Job)
        });
        let done: OnDone = Rc::new(move |ws, outcome, window, cx| {
            if let Outcome::Unlocked(vault) = outcome {
                vfs::unlock(vault);
                then(ws, window, cx);
            }
        });
        self.password_dialog(
            Dialog {
                title: tr!("vault.unlock_title", name = name),
                ok: tr!("vault.unlock"),
                ask: Ask::Unlock,
                note: match self.settings.auto_lock_minutes {
                    0 => tr!("vault.unlock_note_no_auto_lock"),
                    minutes => tr!("vault.unlock_note", count = minutes),
                },
            },
            make_job,
            done,
            window,
            cx,
        );
    }

    /// File › New encrypted collection: a name and a password, then where to put it.
    pub(super) fn prompt_new_encrypted_collection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Inside a project the collection goes into the project; otherwise ask for a folder.
        let parent = self.project.as_ref().map(|p| p.root.clone());
        let make_job: MakeJob = Rc::new(move |secret: Secret| {
            let parent = parent.clone().ok_or_else(|| {
                tr!("vault.error.open_project_first", menu = encrypt_menu_path()).to_string()
            })?;
            let root = store::unique_path(&parent, &store::slug(&secret.name, "collection"), "");
            Ok(Box::new(move || {
                let (mut vault, recovery) =
                    Vault::create(&root, &secret.password, KdfCost::default())
                        .map_err(|e| describe(&e))?;
                let meta = CollectionMeta {
                    name: secret.name.clone(),
                    ..CollectionMeta::default()
                };
                // Inside the vault: the name is encrypted too.
                let text = toml::to_string_pretty(&meta).map_err(|e| e.to_string())?;
                vault
                    .write(store::COLLECTION_FILE, text.as_bytes())
                    .map_err(|e| describe(&e))?;
                Ok(Outcome::Created(vault, recovery))
            }) as Job)
        });
        let done: OnDone = Rc::new(|ws, outcome, window, cx| {
            if let Outcome::Created(vault, recovery) = outcome {
                vfs::unlock(vault);
                ws.reload(window, cx);
                ws.show_recovery_key(recovery, window, cx);
            }
        });
        if self.project.is_none() {
            self.notify_error(
                tr!("vault.new_needs_project", menu = encrypt_menu_path()).to_string(),
                window,
                cx,
            );
            return;
        }
        self.password_dialog(
            Dialog {
                title: tr!("vault.new_title"),
                ok: tr!("vault.create"),
                ask: Ask::NewCollection,
                note: tr!("vault.new_note"),
            },
            make_job,
            done,
            window,
            cx,
        );
    }

    /// Encrypts the plain collection at `root` in place. The plain files go to the trash.
    pub(super) fn prompt_encrypt(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = folder_name(&root);
        let target = root.clone();
        let make_job: MakeJob = Rc::new(move |secret: Secret| {
            let root = target.clone();
            Ok(Box::new(move || encrypt_in_place(&root, &secret.password)) as Job)
        });
        let done: OnDone = Rc::new(move |ws, outcome, window, cx| {
            if let Outcome::Created(vault, recovery) = outcome {
                vfs::unlock(vault);
                ws.reload(window, cx);
                ws.show_recovery_key(recovery, window, cx);
            }
        });
        self.password_dialog(
            Dialog {
                title: tr!("vault.encrypt_title", name = name),
                ok: tr!("vault.encrypt"),
                ask: Ask::NewPassword,
                note: tr!("vault.encrypt_note"),
            },
            make_job,
            done,
            window,
            cx,
        );
    }

    /// Writes the encrypted collection at `root` back as plain files.
    pub(super) fn confirm_decrypt(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = folder_name(&root);
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (view, root) = (view.clone(), root.clone());
            alert
                .on_close(refocus(view.clone()))
                .confirm()
                .text_start()
                .title(tr!("vault.decrypt_title", name = name))
                .description(tr!("vault.decrypt_description"))
                .ok_text(tr!("vault.decrypt"))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |ws, cx| match decrypt_in_place(&root) {
                        Ok(()) => {
                            ws.reload(window, cx);
                            window.push_notification(
                                Notification::success(tr!("vault.decrypted")),
                                cx,
                            );
                        }
                        Err(message) => ws.notify_error(message, window, cx),
                    })
                    .ok();
                    true
                })
        });
    }

    pub(super) fn prompt_change_password(
        &mut self,
        root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = folder_name(&root);
        let target = root.clone();
        let make_job: MakeJob = Rc::new(move |secret: Secret| {
            let root = target.clone();
            Ok(Box::new(move || {
                vfs::with_vault(&root, |vault, _| vault.change_password(&secret.password))
                    .ok_or_else(|| tr!("vault.error.locked").to_string())?
                    .map_err(|e| describe(&e))?;
                Ok(Outcome::Done)
            }) as Job)
        });
        let done: OnDone = Rc::new(|_, _, window, cx| {
            window.push_notification(Notification::success(tr!("vault.password_changed")), cx);
        });
        self.password_dialog(
            Dialog {
                title: tr!("vault.change_password_title", name = name),
                ok: tr!("vault.change_password"),
                ask: Ask::NewPassword,
                note: tr!("vault.change_password_note"),
            },
            make_job,
            done,
            window,
            cx,
        );
    }

    /// Shows the recovery key once; "Done" needs a tick that it was stored.
    fn show_recovery_key(&mut self, key: RecoveryKey, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = key.as_str().to_owned().into();
        let stored = Rc::new(Cell::new(false));
        let view = cx.entity().downgrade();
        let p = self.palette;
        window.open_dialog(cx, move |dialog, _, cx| {
            let mono = cx.theme().mono_font_family.clone();
            let (copy, tick, done_view) = (text.clone(), stored.clone(), view.clone());
            let checked = stored.get();
            dialog
                .on_close(refocus(view.clone()))
                .bidi_heading(tr!("vault.recovery.title"))
                .w(px(580.))
                .close_button(false)
                .overlay_closable(false)
                .keyboard(false)
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_sm().child(tr!("vault.recovery.intro")))
                        .child(
                            h_flex()
                                // The key is data: left to right in any language.
                                .ltr()
                                .gap_2()
                                .items_center()
                                .p_3()
                                .rounded_md()
                                .border_1()
                                .border_color(p.hairline)
                                .child(
                                    div()
                                        .flex_1()
                                        .font_family(mono)
                                        .text_lg()
                                        .text_color(p.accent)
                                        .child(text.clone()),
                                )
                                .child(
                                    Button::new("recovery-copy")
                                        .small()
                                        .label(tr!("vault.recovery.copy"))
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copy.to_string(),
                                            ))
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(p.warn)
                                .child(tr!("vault.recovery.warning")),
                        )
                        .child(
                            Checkbox::new("recovery-stored")
                                .dir_row()
                                .label(tr!("vault.recovery.stored"))
                                .checked(checked)
                                .on_click(move |value: &bool, window, _| {
                                    tick.set(*value);
                                    window.refresh();
                                }),
                        ),
                )
                .footer(
                    h_flex().dir_row().w_full().justify_end().child(
                        Button::new("recovery-done")
                            .primary()
                            .label(tr!("vault.recovery.done"))
                            .disabled(!checked)
                            .on_click(move |_, window, cx| close_dialog(&done_view, window, cx)),
                    ),
                )
        });
    }

    /// Locks every unlocked collection, asking first about unsaved edits in them.
    pub(super) fn lock_collections(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let roots = vfs::unlocked_roots();
        if roots.is_empty() {
            return;
        }
        self.guard_tabs_under(
            roots.clone(),
            window,
            cx,
            Rc::new(move |ws, window, cx| ws.lock_now(&roots, window, cx)),
        );
    }

    fn lock_now(&mut self, roots: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        for root in roots {
            self.forget_tabs(root, window, cx);
            vfs::lock(root);
        }
        // A lone encrypted collection closes; a project shows it as locked.
        let single_vault = self.project.is_none()
            && self
                .collections
                .first()
                .is_some_and(|c| roots.iter().any(|r| c.root.starts_with(r)));
        if single_vault {
            self.close_workspace(window, cx);
        } else {
            self.reload(window, cx);
        }
    }

    /// Checks every 30 seconds whether to lock after the idle time set in Settings.
    /// Collections with unsaved edits stay unlocked until they are saved.
    pub(super) fn auto_lock_task(window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(30))
                    .await;
                let Ok(()) = this.update_in(cx, |ws, window, cx| {
                    let minutes = ws.settings.auto_lock_minutes;
                    let idle = Duration::from_secs(u64::from(minutes) * 60);
                    if minutes == 0 || ws.last_input.get().elapsed() < idle {
                        return;
                    }
                    let roots: Vec<PathBuf> = vfs::unlocked_roots()
                        .into_iter()
                        .filter(|root| !ws.has_dirty_tab_under(root, cx))
                        .collect();
                    if !roots.is_empty() {
                        ws.lock_now(&roots, window, cx);
                        window.push_notification(
                            Notification::info(tr!("vault.auto_locked", count = minutes))
                                .title(tr!("vault.locked_title")),
                            cx,
                        );
                    }
                }) else {
                    break;
                };
            }
        })
    }

    /// A locked encrypted collection in the sidebar; a click asks for its password.
    pub(super) fn render_locked(
        &self,
        ix: usize,
        root: PathBuf,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = self.palette;
        let name = folder_name(&root);
        h_flex()
            .dir_row()
            .id(("locked", ix))
            .gap_2()
            .items_center()
            .px_2()
            .py_1()
            .rounded_md()
            .text_sm()
            .text_color(p.text_muted)
            .cursor_pointer()
            .hover(|s| s.bg(hsla(0., 0., 1., 0.05)))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.prompt_unlock(
                    root.clone(),
                    Rc::new(|this, window, cx| this.reload(window, cx)),
                    window,
                    cx,
                )
            }))
            .child(Icon::new(IconName::Lock).xsmall())
            .child(div().flex_1().min_w_0().truncate().child(name))
            .child(div().text_xs().child(tr!("vault.locked")))
            .into_any_element()
    }

    /// Notes user activity, for auto-lock.
    pub(super) fn touch(&self) {
        self.last_input.set(Instant::now());
    }
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string())
}

/// Encrypts into a sibling folder, moves the plain one to the trash, then
/// takes its place. Returns the vault, unlocked, and its recovery key.
fn encrypt_in_place(root: &Path, password: &str) -> Result<Outcome, String> {
    let staging = root.with_file_name(format!("{}.encrypting", folder_name(root)));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    let (mut vault, recovery) = Vault::encrypt_folder(root, &staging, password, KdfCost::default())
        .map_err(|e| describe(&e))?;
    // Keep the git history and other dot-folders with the collection.
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if name.starts_with('.') && name != ".gitignore" {
            std::fs::rename(&path, staging.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    trash::delete(root).map_err(|e| tr!("vault.error.trash_plain", error = e).to_string())?;
    std::fs::rename(&staging, root).map_err(|e| e.to_string())?;
    vault.relocate(root);
    Ok(Outcome::Created(vault, recovery))
}

/// The reverse: export beside, move the encrypted folder to the trash, take its place.
fn decrypt_in_place(root: &Path) -> Result<(), String> {
    let staging = root.with_file_name(format!("{}.decrypting", folder_name(root)));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    vfs::with_vault(root, |vault, _| vault.export(&staging))
        .ok_or_else(|| tr!("vault.error.unlock_first").to_string())?
        .map_err(|e| describe(&e))?;
    for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if name.starts_with('.') && name != ".gitignore" && !staging.join(&name).exists() {
            std::fs::rename(&path, staging.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    vfs::lock(root);
    trash::delete(root).map_err(|e| tr!("vault.error.trash_encrypted", error = e).to_string())?;
    std::fs::rename(&staging, root).map_err(|e| e.to_string())
}

/// Removes files: from the vault when they live in an unlocked encrypted
/// collection (which has no trash), otherwise to the system trash.
pub(super) fn discard(paths: &[PathBuf]) -> Result<(), String> {
    for path in paths {
        // A whole encrypted collection is a real folder: lock it, then trash it.
        if vfs::is_encrypted(path) {
            vfs::lock(path);
            trash::delete(path).map_err(|e| tr!("vault.error.trash", error = e).to_string())?;
        } else if vfs::is_virtual(path) {
            let removed = if vfs::is_dir(path) {
                vfs::remove_dir_all(path)
            } else {
                vfs::remove_file(path)
            };
            removed.map_err(|e| e.to_string())?;
        } else {
            trash::delete(path).map_err(|e| tr!("vault.error.trash", error = e).to_string())?;
        }
    }
    Ok(())
}

/// `File › Encrypt this collection`, in the interface language.
fn encrypt_menu_path() -> String {
    format!(
        "{} › {}",
        tr!("menu.file"),
        tr!("menu.encrypt_collection").trim_end_matches('…')
    )
}
