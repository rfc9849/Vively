#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use gpui_kit::{
    AppContext as _, Focusable as _, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, point, px, size,
};
use vively::{
    design::icons::VivelyAssets, state::app_state::VivelyApp, ui::install_shortcuts_and_menus,
};

fn main() {
    gpui_kit::application().with_assets(VivelyAssets).run(|cx| {
        gpui_kit::init(cx);
        install_shortcuts_and_menus(cx);

        let min_size = size(px(720.0), px(540.0));
        let mut restore_size = size(px(1380.0), px(880.0));
        if let Some(display) = cx.primary_display() {
            let available = display.visible_bounds().size;
            restore_size.width = restore_size
                .width
                .min((available.width * 0.8).max(min_size.width));
            restore_size.height = restore_size
                .height
                .min((available.height * 0.8).max(min_size.height));
        }
        let bounds = gpui_kit::Bounds::centered(None, restore_size, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            window_min_size: Some(min_size),
            app_id: Some("eu.r3pr3ss10n.vively".into()),
            titlebar: Some(TitlebarOptions {
                title: Some("Vively".into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: cfg!(target_os = "macos")
                    .then(|| point(px(15.0), px(14.0))),
            }),
            window_decorations: Some(if cfg!(target_os = "macos") {
                WindowDecorations::Client
            } else {
                WindowDecorations::Server
            }),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| {
            let entity =
                cx.new(|cx| VivelyApp::new(window, cx).expect("Failed to initialize Vively"));
            let fh = entity.read(cx).focus_handle(cx);
            fh.focus(window, cx);
            entity
        })
        .expect("Failed to open Vively window");

        cx.activate(true);
    });
}
