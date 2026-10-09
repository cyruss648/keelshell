#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ai_credentials;
mod ai_request_options;
mod ai_settings;
mod assistant;
mod batch_commands;
mod command_history;
mod command_suggestions;
mod command_text;
mod design;
mod directory_compare;
mod emulator;
mod files;
mod i18n;
mod jump_host_picker;
mod mcp_bridge;
mod monitor;
mod profile_sync;
mod proxy_editor;
mod runtime_bridge;
mod snippet_editor;
mod snippet_parameters;
mod ssh_bridge;
mod target_parameters;
mod terminal;
mod tunnels;
mod updater;
mod vault_settings;
mod workflow_commands;
mod workspace;

#[cfg(test)]
mod ui_tests;

use gpui_kit::*;
use keelshell_core::{AppState, StateStore};
use std::sync::Arc;

fn main() {
    if keelshell_ai::run_local_agent_directory_launcher() {
        return;
    }
    if updater::run_update_helper() {
        return;
    }
    let path = match std::env::var_os("KEELSHELL_DATA_DIR") {
        Some(path) => std::path::PathBuf::from(path).join("state.json"),
        None => match StateStore::default_path() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("Cannot resolve application data: {error}");
                std::process::exit(1);
            }
        },
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
    {
        Ok(runtime) => Arc::new(runtime),
        Err(error) => {
            eprintln!("Cannot start networking runtime: {error}");
            std::process::exit(1);
        }
    };
    let store = Arc::new(StateStore::new(path));
    // Initial disk loading precedes the UI event loop. Subsequent writes use workers.
    let (state, load_error) = match store.load() {
        Ok(state) => (state, None),
        Err(error) => (
            AppState::default(),
            Some(format!(
                "Configuration could not be loaded: {error}. Existing file preserved."
            )),
        ),
    };
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            cx.set_app_identity("app.keelshell.desktop", "KeelShell");
            gpui_kit::init(cx);
            design::apply(state.settings.theme, None, cx);
            i18n::set_language(state.settings.language, cx);
            workspace::bind_keys(cx);
            terminal::install_shutdown(cx);
            let options = WindowOptions {
                // Wayland desktop lookup and X11 WM_CLASS match keelshell.desktop.
                app_id: Some("keelshell".into()),
                #[cfg(target_os = "linux")]
                icon: linux_window_icon(),
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1440.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(900.), px(580.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("KeelShell".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
                window.on_window_should_close(cx, |_, cx| {
                    terminal::shutdown_and_quit(cx);
                    false
                });
                cx.new(|cx| {
                    workspace::Workspace::new(store, state, load_error, runtime, window, cx)
                })
            }) {
                eprintln!("Unable to open KeelShell: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
}

#[cfg(target_os = "linux")]
fn linux_window_icon() -> Option<Arc<image::RgbaImage>> {
    // GPUI consumes this RGBA image on X11. Wayland uses the desktop-file icon
    // through app_id instead; no runtime filesystem path is required here.
    match image::load_from_memory_with_format(
        include_bytes!("../../../assets/icons/png/256.png"),
        image::ImageFormat::Png,
    ) {
        Ok(icon) => Some(Arc::new(icon.into_rgba8())),
        Err(error) => {
            eprintln!("Unable to decode the embedded KeelShell window icon: {error}");
            None
        }
    }
}
