// リリースビルドでは Windows のコンソールウィンドウを出さない
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod components;
mod session;
mod text_input;
mod theme;
mod views;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};

use crate::app::RenamerView;

fn main() {
    tracing_subscriber::fmt::init();

    Application::new().run(|cx: &mut App| {
        text_input::bind_keys(cx);
        app::bind_keys(cx);

        // Windows / Linux ではウィンドウを閉じてもプロセスが残るため明示的に終了する
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Flex Renamer".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RenamerView::new(window, cx)),
        )
        .expect("ウィンドウを開けませんでした");

        cx.activate(true);
    });
}
