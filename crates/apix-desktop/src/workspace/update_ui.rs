//! Is there a newer Apix? Asked once at start (Settings › Check for updates)
//! and from Help › Check for updates…. The check is `apix_core::update`'s
//! (one GET to GitHub's latest release); nothing is downloaded: a newer
//! release is announced with a button that opens its page.

use apix_core::update::{self, Check, Version};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{Sizable as _, WindowExt as _};
use gpui_kit::*;

use super::Workspace;
use crate::{net, tr};

impl Workspace {
    /// `manual`: asked from the menu, so every answer is shown; at start only
    /// a newer release is, and failures stay quiet.
    pub(super) fn check_updates(
        &mut self,
        manual: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if manual {
            window.push_notification(Notification::info(tr!("update.checking")), cx);
        }
        cx.spawn_in(window, async move |this, cx| {
            let current = Version::current();
            let result = net::send(update::request())
                .await
                .and_then(|r| update::interpret(r.status, &r.body, &current));
            this.update_in(cx, |_, window, cx| {
                if let Some(note) = notification(result, &current, manual) {
                    window.push_notification(note, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The documentation's screenshot tour: the notice of a newer release.
    pub(super) fn tour_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = Version::current();
        let newer = Version::parse("0.2.0").unwrap_or_else(Version::current);
        let release = update::Release {
            version: newer,
            url: "https://github.com/11R5/Apix/releases".into(),
        };
        if let Some(note) = notification(Ok(Check::Newer(release)), &current, true) {
            window.push_notification(note, cx);
        }
    }
}

/// What to tell about a check; `None` when nothing is worth telling.
fn notification(
    result: Result<Check, String>,
    current: &Version,
    manual: bool,
) -> Option<Notification> {
    Some(match result {
        Ok(Check::Newer(release)) => {
            let url = release.url.clone();
            Notification::info(tr!(
                "update.available",
                version = release.version.to_string(),
                current = current.to_string()
            ))
            .title(tr!("update.title"))
            .action(move |_, _, _| {
                let url = url.clone();
                Button::new("update-open")
                    .primary()
                    .small()
                    .label(tr!("update.open"))
                    .on_click(move |_, _, cx| cx.open_url(&url))
            })
        }
        _ if !manual => return None,
        Ok(Check::UpToDate { .. }) => {
            Notification::success(tr!("update.up_to_date", current = current.to_string()))
        }
        Ok(Check::NoReleases) => {
            Notification::info(tr!("update.no_releases", current = current.to_string()))
        }
        Err(e) => Notification::error(tr!("update.failed", error = e)),
    })
}

impl Workspace {
    /// Help › Check for updates… and the palette.
    pub(super) fn check_updates_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.check_updates(true, window, cx);
    }
}
