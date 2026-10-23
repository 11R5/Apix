//! Copy a request as a shell command or code, and turn a pasted curl command
//! into a request.

use std::path::PathBuf;

use apix_codegen::{Format, Lang, Shell, from_curl, looks_like_curl};
use apix_core::{Auth, Request};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use super::Workspace;
use crate::tr;

/// A paste adds many characters at once; typing adds one. Below this jump
/// a `curl ...` in the URL field is treated as typing and left alone.
const PASTE_MIN_CHARS: usize = 8;

impl Workspace {
    /// Copies `request`, with variables filled in from the active environment.
    fn copy_as(
        &mut self,
        request: Request,
        format: Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut resolved = request.resolve(&self.vars);
        // With a token in hand, the copied command works as is.
        if let Auth::OAuth2(config) = &resolved.request.auth
            && let Some(token) = apix_http::oauth2::cached(config)
        {
            resolved.request.auth = Auth::Bearer {
                token: token.access_token,
            };
        }
        cx.write_to_clipboard(ClipboardItem::new_string(format.render(&resolved.request)));
        let message = if resolved.missing.is_empty() {
            tr!("copy.copied", shell = format.label())
        } else {
            let names: Vec<_> = resolved.missing.into_iter().collect();
            tr!(
                "copy.copied_missing",
                shell = format.label(),
                names = names.join(", ")
            )
        };
        window.push_notification(Notification::success(message), cx);
    }

    /// The open request as edited, unsaved changes included.
    pub(super) fn copy_current_as(
        &mut self,
        format: Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let request = self.with_inherited_auth(self.build_request(cx), self.selected.as_deref());
        let request = self.with_inherited_settings(request, self.selected.as_deref());
        self.copy_as(request, format, window, cx);
    }

    /// A request from the sidebar; the open one is copied with its edits.
    pub(super) fn copy_saved_as(
        &mut self,
        path: PathBuf,
        format: Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected.as_ref() == Some(&path) {
            self.copy_current_as(format, window, cx);
        } else if let Some(request) = self.requests.get(&path).cloned() {
            let request = self.with_inherited_auth(request, Some(&path));
            let request = self.with_inherited_settings(request, Some(&path));
            self.copy_as(request, format, window, cx);
        }
    }

    /// Called on every URL change. A pasted curl command fills the whole
    /// request (method, URL, params, headers, auth, body) as unsaved edits.
    pub(super) fn on_url_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.url.read(cx).value().to_string();
        let grew = typed.chars().count().saturating_sub(self.last_url_len);
        self.last_url_len = typed.chars().count();
        if grew < PASTE_MIN_CHARS || !looks_like_curl(&typed) {
            return;
        }
        // The single-line field drops line breaks; the clipboard still has the original.
        let source = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .filter(|text| looks_like_curl(text))
            .unwrap_or(typed);
        match from_curl(&source) {
            Ok(import) => {
                let mut request = import.request;
                request.name = self.base.name.clone();
                request.description = self.base.description.clone();
                self.fill_editor(&request, window, cx);
                self.last_url_len = request.url.chars().count();
                let mut message = String::from(tr!("copy.curl_filled"));
                for warning in &import.warnings {
                    message.push_str("\n• ");
                    message.push_str(warning);
                }
                window.push_notification(
                    Notification::success(message)
                        .title(tr!("copy.curl_imported"))
                        .autohide(import.warnings.is_empty()),
                    cx,
                );
            }
            Err(e) => window.push_notification(Notification::error(e), cx),
        }
    }
}

/// "Copy as" items for a popup menu: the shells, then the languages.
/// `copy` runs with the chosen format.
pub(super) fn copy_items(
    menu: PopupMenu,
    view: WeakEntity<Workspace>,
    copy: fn(&mut Workspace, Format, &mut Window, &mut Context<Workspace>),
) -> PopupMenu {
    let item = |menu: PopupMenu, format: Format| {
        let view = view.clone();
        menu.item(
            PopupMenuItem::new(tr!("copy.as_shell", shell = format.label())).on_click(
                move |_, window, cx| {
                    view.update(cx, |this, cx| copy(this, format, window, cx))
                        .ok();
                },
            ),
        )
    };
    let menu = Shell::ALL
        .into_iter()
        .fold(menu, |menu, shell| item(menu, Format::Shell(shell)));
    Lang::ALL.into_iter().fold(menu.separator(), |menu, lang| {
        item(menu, Format::Code(lang))
    })
}
