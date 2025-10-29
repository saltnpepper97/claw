mod clipboard;
mod commands;
mod config;
mod detect;
mod history;
mod theme;
mod tray;
mod utils;
mod watchers;
mod window;

use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use tauri::{
    generate_handler,
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Listener
};
use tauri_plugin_cli::CliExt;
use tokio::sync::RwLock;

use theme::Theme;

static LAST_WRITTEN_CLIPBOARD: Lazy<Mutex<Option<u64>>> = Lazy::new(|| Mutex::new(None));

use commands::{
    clear_clipboard_history, get_claw_config, get_clipboard_history, get_history_stats,
    get_system_clipboard, get_theme, remove_clipboard_entry, set_clipboard_from_history,
    set_system_clipboard, get_clipboard_entry_content
};
use config::{load_claw_config, ClipboardConfig};

#[derive(serde::Serialize, Clone)]
struct ConfigUpdate {
    enable_titlebar: bool,
    force_dark_mode: bool,
    theme: Theme,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()      
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.contains(&"--toggle".into()) {
                window::toggle_main_window(app);
                return;
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.unminimize();
            }
        }))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            let app_handle = app.handle();

            let claw_config = Arc::new(RwLock::new(load_claw_config()));
            app.manage(claw_config.clone());

            // Cleanup history on exit if persistence is disabled
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                app_handle.clone().once("tauri://exit", move |_event| {
                    let app_handle = app_handle.clone();
                    let claw_config = claw_config.clone();
                    tauri::async_runtime::spawn(async move {
                        let persist = claw_config.read().await.0.persist_history;
                        if !persist {
                            if let Ok(mut hist) = history::load_history(&app_handle, 100) {
                                hist.clear();
                                let _ = history::save_history(&app_handle, &hist);
                            }
                        }
                    });
                });
            }

            let main_window = app.get_webview_window("main").unwrap();

            // Parse CLI arguments
            let cli_matches = app.cli().matches()?;
            let should_hide = cli_matches.args.contains_key("hide") && 
                              cli_matches.args.get("hide").unwrap().value.as_bool().unwrap_or(false);

            if should_hide {
                main_window.hide().ok();
            }

            // Setup window close handler
            window::setup_window_close_handler(app_handle.clone());

            // Create initial tray menu
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // Build tray icon with unique ID
            let _tray = TrayIconBuilder::with_id(tray::TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event({
                    let app_handle = app_handle.clone();
                    move |app, event| {
                        handle_tray_menu_event(app, event, &app_handle);
                    }
                })       
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        window::toggle_main_window(&app);
                    }
                })
                .build(app)?;

            let _ = tray::update_tray_menu(&app_handle, tray::TRAY_ID);

            // Load and manage config
            let claw_config = Arc::new(RwLock::new(load_claw_config()));
            app.manage(claw_config.clone());

            // Emit initial config to frontend
            emit_initial_config(app_handle.clone(), claw_config.clone());

            // Start clipboard watcher
            watchers::spawn_clipboard_watcher(app_handle.clone(), claw_config.clone());

            // Start config watcher
            watchers::spawn_config_watcher(app_handle.clone(), claw_config.clone());

            // Setup history listener (must be after config is set up)
            setup_history_listener(app_handle.clone());

            // Setup logging in debug mode
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            Ok(())
        })
        .invoke_handler(generate_handler![
            set_system_clipboard,
            get_system_clipboard,
            get_clipboard_history,
            get_clipboard_entry_content,
            clear_clipboard_history,
            remove_clipboard_entry,
            set_clipboard_from_history,
            get_history_stats,
            get_theme,
            get_claw_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn handle_tray_menu_event(
    app: &tauri::AppHandle,
    event: tauri::menu::MenuEvent,
    app_handle: &tauri::AppHandle,
) {
    let event_id = event.id.as_ref();
    match event_id {
        "show" => {
            window::show_main_window(app);
        }
        "quit" => {
            app.exit(0);
        }
        "clear_history" => {
            if let Ok(mut hist) = history::load_history(app_handle, 100) {
                hist.clear();
                let _ = history::save_history(app_handle, &hist);
                drop(hist);
                let _ = app_handle.emit("history-updated", "");
                let _ = tray::update_tray_menu(app_handle, tray::TRAY_ID);
            }
        }
        id if id.starts_with("history_") => {
            if let Ok(idx) = id.strip_prefix("history_").unwrap().parse::<usize>() {
                if let Ok(hist) = history::load_history(app_handle, 100) {
                    let entries = hist.get_entries(Some(5));
                    if let Some(entry) = entries.get(idx) {
                        if let Some(content) = hist.get_entry_content(&entry.id) {
                            clipboard::cache_clipboard_data(&content);
                            let _ = clipboard::set_clipboard(&content);
                            drop(content);
                        }
                    }
                    drop(hist);
                }
            }
        }
        _ => {}
    }
}

fn setup_history_listener(app_handle: tauri::AppHandle) {
    app_handle.clone().listen("history-updated", move |_event| {
        let app_clone = app_handle.clone();
        if let Err(e) = tray::update_tray_menu(&app_clone, tray::TRAY_ID) {
            eprintln!("Failed to update tray menu: {}", e);
        }
    });
}

fn emit_initial_config(
    app_handle: tauri::AppHandle,
    claw_config: Arc<RwLock<(ClipboardConfig, Theme)>>,
) {
    tauri::async_runtime::spawn(async move {
        let cfg = claw_config.read().await;
        let initial = ConfigUpdate {
            enable_titlebar: cfg.0.enable_titlebar,
            force_dark_mode: cfg.0.force_dark_mode,
            theme: cfg.1.clone(),
        };
        let _ = app_handle.emit("config-reloaded", initial);
    });
}
