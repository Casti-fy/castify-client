mod error;
mod models;
mod services;
mod state;

mod commands;

/// Default API base URL
pub const DEFAULT_SERVER_URL: &str = "https://casti.fyi";

use state::AppState;
use tauri::Manager;

struct TrayState {
    quit_requested: std::sync::atomic::AtomicBool,
    tray_ready: std::sync::atomic::AtomicBool,
}

/// Show the main window from the tray: Dock on macOS (Regular), then show + focus.
fn show_main_window_from_tray<R: tauri::Runtime, M: Manager<R>>(m: &M) {
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = m
            .app_handle()
            .set_activation_policy(tauri::ActivationPolicy::Regular)
        {
            log::error!("set_activation_policy(Regular) failed: {e}");
        }
    }
    if let Some(w) = m.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Hide the main window to tray: no Dock on macOS (Accessory) after the window is hidden.
fn hide_main_window_to_tray<R: tauri::Runtime, M: Manager<R>>(m: &M) {
    if let Some(w) = m.get_webview_window("main") {
        let _ = w.hide();
    }
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = m
            .app_handle()
            .set_activation_policy(tauri::ActivationPolicy::Accessory)
        {
            log::error!("set_activation_policy(Accessory) failed: {e}");
        }
    }
}

pub fn run() {
    use tauri::{
        menu::{MenuBuilder, MenuItemBuilder},
        tray::TrayIconBuilder,
        WindowEvent,
    };
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
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
            // Clear any leftover temp audio from prior runs before sync kicks off.
            services::helpers::clear_all_temp_dirs();

            // Create platform-specific config store and AppState
            let store: std::sync::Arc<dyn services::config_store::ConfigStore> =
                std::sync::Arc::new(services::tauri_store::TauriConfigStore::new(app.handle()));
            let app_state = AppState::new(DEFAULT_SERVER_URL, store);
            app.manage(app_state);
            app.manage(TrayState {
                quit_requested: std::sync::atomic::AtomicBool::new(false),
                tray_ready: std::sync::atomic::AtomicBool::new(false),
            });

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

            // Wire 401 handler to emit auth-expired event to the frontend
            {
                use tauri::Emitter;
                let handle = app.handle().clone();
                let state = app.state::<AppState>();
                tauri::async_runtime::block_on(async {
                    state.api.write().await.set_on_unauthorized(
                        std::sync::Arc::new(move || {
                            let _ = handle.emit("auth-expired", ());
                        }),
                    );
                });
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

            let logged_in = state.store.get_token().is_ok();

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

            // Use the same embedded image as the window icon (from bundle icon list in
            // tauri.conf). Tauri's TrayIconBuilder::icon drops the image if Image→tray
            // conversion fails without error; set_icon after build() fixes that. If there is no
            // default window icon, fall back to 32x32.png (always in git).
            let (tray_image, template_tray) =
                if let Some(img) = app.handle().default_window_icon().cloned() {
                    (img, false)
                } else {
                    // Fallback must use a tracked icon (see src-tauri/icons/); tray-template.png
                    // is optional locally but not in git for some dev machines.
                    (
                        tauri::include_image!("./icons/32x32.png"),
                        true,
                    )
                };

            let tray_builder = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("Castify")
                .icon(tray_image.clone())
                .icon_as_template(template_tray)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        show_main_window_from_tray(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        show_main_window_from_tray(app);
                    }
                    "quit" => {
                        if let Some(state) = app.try_state::<TrayState>() {
                            state.quit_requested.store(
                                true,
                                std::sync::atomic::Ordering::SeqCst,
                            );
                        }
                        app.exit(0);
                    }
                    _ => {}
                });

            match tray_builder.build(app) {
                Ok(tray) => {
                    if let Err(err) = tray.set_icon(Some(tray_image.clone())) {
                        log::error!("tray: set_icon after build failed: {err}");
                    }
                    if let Err(err) = tray.set_icon_as_template(template_tray) {
                        log::error!("tray: set_icon_as_template after build failed: {err}");
                    }
                    if let Err(err) = tray.set_visible(true) {
                        log::error!("failed to set system tray visibility: {err}");
                    }

                    if let Some(state) = app.try_state::<TrayState>() {
                        state
                            .tray_ready
                            .store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    app.manage(tray);
                    log::info!("system tray initialized");

                    if !logged_in {
                        show_main_window_from_tray(app);
                    }
                    // Logged-in macOS: defer Accessory to RunEvent::Ready (see run handler) so the
                    // status item is created under the default Regular policy; immediate Accessory
                    // after tray build has been seen to break menu-bar icon visibility in release.
                }
                Err(err) => {
                    log::error!("failed to initialize system tray: {err}");
                }
            }

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
            commands::sync::backfill_feed,
            commands::sync::get_sync_interval,
            commands::sync::set_sync_interval,
            commands::sync::clear_sync_cache,
            commands::billing::create_checkout,
            commands::billing::create_portal,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let tray_ready = window
                    .app_handle()
                    .try_state::<TrayState>()
                    .map(|state| state.tray_ready.load(std::sync::atomic::Ordering::SeqCst))
                    .unwrap_or(false);

                if tray_ready {
                    hide_main_window_to_tray(window.app_handle());
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building castify")
        .run(|app, event| {
            if let tauri::RunEvent::Ready = &event {
                #[cfg(target_os = "macos")]
                {
                    if let Some(state) = app.try_state::<AppState>() {
                        if state.store.get_token().is_ok() {
                            let tray_ready = app
                                .try_state::<TrayState>()
                                .map(|s| {
                                    s.tray_ready.load(std::sync::atomic::Ordering::SeqCst)
                                })
                                .unwrap_or(false);
                            if tray_ready {
                                if let Err(e) =
                                    app.set_activation_policy(tauri::ActivationPolicy::Accessory)
                                {
                                    log::error!(
                                        "set_activation_policy(Accessory) after Ready failed: {e}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let quit_requested = app
                    .try_state::<TrayState>()
                    .map(|state| {
                        state
                            .quit_requested
                            .load(std::sync::atomic::Ordering::SeqCst)
                    })
                    .unwrap_or(false);
                if quit_requested {
                    return;
                }

                let tray_ready = app
                    .try_state::<TrayState>()
                    .map(|state| state.tray_ready.load(std::sync::atomic::Ordering::SeqCst))
                    .unwrap_or(false);

                if tray_ready {
                    api.prevent_exit();
                    hide_main_window_to_tray(app);
                } else {
                    // macOS often requests app termination when there are no visible windows and
                    // the dock icon is hidden (Accessory / hidden main). If the tray is not
                    // operational, allowing exit would quit immediately with "no tray" — stay
                    // alive and show the main window so the app remains usable.
                    api.prevent_exit();
                    show_main_window_from_tray(app);
                }
            }
        });
}
