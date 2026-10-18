// No console window behind the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod fonts;
mod i18n;
mod large_view;
mod net;
mod settings;
mod state;
mod theme;
mod workspace;

use std::path::{Path, PathBuf};

use gpui_kit::component::TitleBar;
use gpui_kit::*;

use crate::theme::{Palette, Variant};
use crate::workspace::Workspace;

// Every `tr!` looks its text up in locales/<code>.yml; English fills any gap.
rust_i18n::i18n!("locales", fallback = "en");

/// `apix-desktop <folder>` opens that collection. Without an argument, the
/// example collection is opened when running from the repository.
fn collection_dir() -> Option<PathBuf> {
    std::env::args_os().nth(1).map(PathBuf::from).or_else(|| {
        let example = Path::new("examples/jsonplaceholder");
        example.is_dir().then(|| example.to_path_buf())
    })
}

fn main() {
    let dir = collection_dir();

    // The Lucide icons (window controls, inputs, menus, locks) are embedded in the binary.
    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let embedded_fonts = fonts::register(cx);
            let settings = settings::Settings::load();
            i18n::set(settings.language_code());
            let palette = Palette::new(Variant::chosen(&settings));
            theme::apply(&palette, embedded_fonts, cx);
            workspace::bind_keys(cx);
            workspace::register_graphql();
            large_view::bind_keys(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(800.)),
                    cx,
                ))),
                window_min_size: Some(size(px(760.), px(480.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Apix".into()),
                    ..TitleBar::title_bar_options()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..TitleBar::window_options()
            };

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Workspace::new(dir, palette, window, cx))
            })
            .expect("failed to open the Apix window");
        });
}
