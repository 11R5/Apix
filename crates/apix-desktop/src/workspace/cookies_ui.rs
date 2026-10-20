//! The cookie manager: every cookie the engine holds, with per-cookie
//! delete and "remove all". Cookies live in one jar shared by every send
//! and persist in `cookies.json` next to the app settings.

use apix_http::CookieInfo;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{IconName, Sizable, WindowExt as _, h_flex, v_flex};
use gpui_kit::*;

use crate::bidi::{self, Bidi as _, DialogBidi as _};
use crate::net;
use crate::theme::Palette;
use crate::tr;

struct CookiesView {
    palette: Palette,
}

/// `expires` as "in 3 d" style text, or "session".
fn expiry(cookie: &CookieInfo) -> SharedString {
    let Some(at) = cookie.expires else {
        return tr!("cookies.session");
    };
    match at.duration_since(std::time::SystemTime::now()) {
        Err(_) => tr!("cookies.expired"),
        Ok(left) => {
            let left = match left.as_secs() {
                s @ 0..3600 => format!("{} min", s.div_ceil(60)),
                s @ 3600..86_400 => format!("{} h", s / 3600),
                s => format!("{} d", s / 86_400),
            };
            tr!("cookies.expires_in", left = left)
        }
    }
}

impl Render for CookiesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let cookies = net::client().cookies();
        let count = cookies.len();

        let rows = cookies.into_iter().enumerate().map(|(ix, cookie)| {
            let value = if cookie.value.chars().count() > 40 {
                let cut: String = cookie.value.chars().take(40).collect();
                format!("{cut}…")
            } else {
                cookie.value.clone()
            };
            h_flex()
                .dir_row()
                .gap_3()
                .items_center()
                .px_2()
                .py_1()
                .rounded_md()
                .hover(|s| s.bg(p.surface))
                .child(
                    div()
                        .w(px(170.))
                        .flex_none()
                        .text_sm()
                        .truncate()
                        .child(cookie.domain.clone()),
                )
                .child(
                    div()
                        .w(px(130.))
                        .flex_none()
                        .text_sm()
                        .truncate()
                        .child(cookie.name.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(p.text_muted)
                        .truncate()
                        .child(value),
                )
                .child(
                    div()
                        .w(px(90.))
                        .flex_none()
                        .text_xs()
                        .text_color(p.text_muted)
                        .child(expiry(&cookie)),
                )
                .child(
                    Button::new(("cookie-del", ix))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Delete)
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            net::client().remove_cookie(
                                &cookie.domain,
                                &cookie.path,
                                &cookie.name,
                            );
                            cx.notify();
                        })),
                )
        });

        let body: AnyElement = if count == 0 {
            div()
                .py_6()
                .text_sm()
                .text_color(p.text_muted)
                .child(bidi::para(tr!("cookies.empty")))
                .into_any_element()
        } else {
            v_flex()
                .id("cookie-rows")
                .max_h(px(380.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        v_flex()
            .gap_2()
            .child(body)
            .child(
                h_flex()
                    .dir_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(p.text_muted)
                            .child(bidi::para(tr!("cookies.note"))),
                    )
                    .children((count > 0).then(|| {
                        Button::new("cookies-clear")
                            .ghost()
                            .small()
                            .label(tr!("cookies.clear_all"))
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                                net::client().clear_cookies();
                                cx.notify();
                            }))
                    })),
            )
    }
}

impl super::Workspace {
    pub(super) fn open_cookies(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let palette = self.palette;
        let view = cx.entity().downgrade();
        let cookies = cx.new(|_| CookiesView { palette });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .on_close(super::collections::refocus(view.clone()))
                .bidi_title(tr!("cookies.title"))
                .width(px(680.))
                .child(div().pb_2().child(cookies.clone()))
        });
    }
}
