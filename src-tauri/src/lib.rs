#[cfg(feature = "cli")]
pub mod cli;
mod error;
#[cfg(feature = "cli")]
mod storage;
mod models;
mod services;
mod state;

#[cfg(feature = "gui")]
mod commands;

/// Default API base URL; used by both GUI and CLI when no config is set.
pub const DEFAULT_SERVER_URL: &str = "https://casti.fyi";

#[cfg(feature = "gui")]
use state::AppState;

#[cfg(feature = "gui")]
pub fn run() {
    use tauri::{
        menu::{MenuBuilder, MenuItemBuilder},
        tray::TrayIconBuilder,
        Manager, WindowEvent,
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(AppState::new(DEFAULT_SERVER_URL))
        .setup(|app| {
            // Restore token from store
            if let Ok(token) = crate::services::keychain::get_token(app.handle()) {
                let state = app.state::<AppState>();
                tauri::async_runtime::block_on(async {
                    state.api.write().await.set_token(Some(token));
                });
            }

            // System tray
            let open = MenuItemBuilder::with_id("open", "Open Castify").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&open, &quit]).build()?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::login,
            commands::auth::register,
            commands::auth::check_auth,
            commands::auth::logout,
            commands::feeds::list_feeds,
            commands::feeds::create_feed,
            commands::feeds::get_feed_detail,
            commands::feeds::delete_feed,
            commands::sync::sync_feed,
            commands::sync::start_periodic_sync,
            commands::sync::stop_periodic_sync,
            commands::billing::create_checkout,
            commands::billing::create_portal,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide the window instead of closing; app keeps running in tray
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building castify")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
                // Hide all windows instead of quitting
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
        });
}
