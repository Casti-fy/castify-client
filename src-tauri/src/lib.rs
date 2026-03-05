mod commands;
mod error;
mod models;
mod services;
mod state;

use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

use state::AppState;

const DEFAULT_SERVER_URL: &str = "http://es.alpharesearch.io:3000";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(AppState::new(DEFAULT_SERVER_URL))
        .setup(|app| {
            // System tray
            let open = MenuItemBuilder::with_id("open", "Open Castify").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&open, &quit]).build()?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
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
            commands::settings::get_server_url,
            commands::settings::set_server_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running castify");
}
