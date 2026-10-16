// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod theme;

use apix_core::Method;
use gpui_kit::*;

struct ApixWindow {
    method: Method,
    url: SharedString,
}

impl ApixWindow {
    fn sidebar(&self) -> impl IntoElement {
        let row = |label: &'static str, method: Method| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .rounded_md()
                .text_sm()
                .child(
                    div()
                        .w(px(52.))
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::method_color(method))
                        .child(method.as_str()),
                )
                .child(label)
        };

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .pt(px(40.)) // room for the transparent titlebar / traffic lights
            .gap_1()
            .border_r_1()
            .border_color(theme::hairline())
            .child(
                div()
                    .px_3()
                    .pb_2()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child("COLLECTIONS"),
            )
            .child(row("List users", Method::Get))
            .child(row("Create user", Method::Post))
            .child(row("Health", Method::Get))
    }

    fn request_bar(&self) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .p_2()
            .rounded_lg()
            .bg(theme::surface())
            .border_1()
            .border_color(theme::hairline())
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme::method_color(self.method).opacity(0.18))
                    .text_color(theme::method_color(self.method))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(self.method.as_str()),
            )
            .child(div().flex_1().text_sm().child(self.url.clone()))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child("Ctrl+Enter"),
            )
    }
}

impl Render for ApixWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            // Translucent on purpose: the OS blur shows through.
            .bg(theme::background())
            .text_color(theme::text())
            .child(self.sidebar())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_3()
                    .p_3()
                    .pt(px(40.))
                    .child(self.request_bar())
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme::hairline())
                            .text_sm()
                            .text_color(theme::text_muted())
                            .child("Send a request to see the response · Ctrl+K for commands"),
                    ),
            )
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(780.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Apix".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            window_background: WindowBackgroundAppearance::Blurred,
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|_| ApixWindow {
                method: Method::Get,
                url: "https://jsonplaceholder.typicode.com/users".into(),
            })
        })
        .expect("failed to open the Apix window");
    });
}
