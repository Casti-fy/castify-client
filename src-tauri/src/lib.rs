mod error;
mod models;
mod services;
mod state;

mod commands;

/// Default API base URL
pub const DEFAULT_SERVER_URL: &str = "https://casti.fyi";

use state::AppState;

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Create platform-specific config store and AppState
            let store: std::sync::Arc<dyn services::config_store::ConfigStore> =
                std::sync::Arc::new(services::tauri_store::TauriConfigStore::new(app.handle()));
            let app_state = AppState::new(DEFAULT_SERVER_URL, store);
            app.manage(app_state);

            // Wire Tauri's event emitter to AppState so services can emit
            // progress events without depending on Tauri directly.
            {
                use tauri::Emitter;
                let handle = app.handle().clone();
                let state = app.state::<AppState>();
                let _ = state.on_progress.set(std::sync::Arc::new(move |event| {
                    let _ = handle.emit("sync-progress", event);
                }));
            }

            // Set Tauri's resource dir as an extra binary search path
            if let Ok(dir) = app.path().resource_dir() {
                let state = app.state::<AppState>();
                state.extra_bin_dirs.write().unwrap().push(dir);
            }

            // Restore token from store
            let state = app.state::<AppState>();
            if let Ok(token) = state.store.get_token() {
                tauri::async_runtime::block_on(async {
                    state.api.write().await.set_token(Some(token));
                });
            }
            
            // Auto-start periodic sync if authenticated.
            // IMPORTANT: must run after on_progress and extra_bin_dirs are set above.
            let state_sync = (*state).clone();
            tauri::async_runtime::spawn(async move {
                services::sync::auto_start_sync(&state_sync).await;
            });

            // System tray
            let open = MenuItemBuilder::with_id("open", "Open Castify").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&open, &quit]).build()?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(false)
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
            commands::auth::fetch_plans,
            commands::feeds::list_feeds,
            commands::feeds::create_feed,
            commands::feeds::get_feed_detail,
            commands::feeds::delete_feed,
            commands::sync::sync_feed,
            commands::sync::get_sync_interval,
            commands::sync::set_sync_interval,
            commands::sync::clear_sync_cache,
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
