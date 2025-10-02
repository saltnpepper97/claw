mod clipboard;
mod commands;
mod config;
mod detect;
mod history;
mod theme;

use std::sync::Arc;
use tauri::{Emitter, generate_handler, Manager};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use tokio::sync::RwLock;

use config::load_claw_config;
use commands::{
    set_system_clipboard, 
    get_system_clipboard, 
    get_clipboard_history, 
    clear_clipboard_history, 
    remove_clipboard_entry, 
    set_clipboard_from_history, 
    get_history_stats,
    get_theme,
    get_claw_config,
};

#[derive(serde::Serialize, Clone)]
struct ConfigUpdate {
    enable_titlebar: bool,
    force_dark_mode: bool,
    theme: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle();

            // Load config once at startup
            let claw_config = Arc::new(RwLock::new(load_claw_config()));
            println!("Loaded config: {:?}", claw_config);

            // Manage Tauri state
            app.manage(claw_config.clone());

            // --- Emit initial config to frontend ---
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                tauri::async_runtime::spawn(async move {
                    let cfg = claw_config.read().await;
                    let initial = ConfigUpdate {
                        enable_titlebar: cfg.enable_titlebar,
                        force_dark_mode: cfg.force_dark_mode,
                        theme: cfg.theme.clone(),
                    };
                    let _ = app_handle.emit("config-reloaded", initial);
                });
            }

            // --- Clipboard watcher task ---
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::time::{sleep, Duration};

                    let mut last_content = String::new();
                    loop {
                        let history_limit = claw_config.read().await.history_limit as usize;

                        if let Ok(content) = crate::clipboard::get_clipboard() {
                            if !content.is_empty() && content != last_content {
                                last_content = content.clone();

                                if let Err(e) = crate::history::add_to_history(
                                    &app_handle,
                                    content.clone(),
                                    "text".to_string(),
                                    history_limit,
                                ) {
                                    eprintln!("Failed to add to history: {}", e);
                                }

                                let _ = app_handle.emit("history-updated", &content);
                            }
                        }

                        sleep(Duration::from_millis(550)).await;
                    }
                });
            }

            // --- Config hot-reload task ---
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                tauri::async_runtime::spawn(async move {
                    use std::sync::mpsc::channel;
                    use std::path::PathBuf;
                    use notify::Config;

                    let config_path: PathBuf = config::find_config().expect("No claw.rune config found");

                    let (tx, rx) = channel();
                    let mut watcher: RecommendedWatcher =
                        Watcher::new(tx, Config::default()).expect("Failed to create file watcher");

                    watcher.watch(&config_path, RecursiveMode::NonRecursive)
                        .expect("Failed to watch config file");

                    loop {
                        match rx.recv() {
                            Ok(event) => {
                                if let EventKind::Modify(_) = event.unwrap().kind {
                                    if let Ok(new_config) = config::load_config(&config_path.to_string_lossy()) {
                                        *claw_config.write().await = new_config.clone();
                                        println!("Config hot-reloaded!");

                                        let update = ConfigUpdate {
                                            enable_titlebar: new_config.enable_titlebar,
                                            force_dark_mode: new_config.force_dark_mode,
                                            theme: new_config.theme.clone(),
                                        };

                                        let _ = app_handle.emit("config-reloaded", update);
                                    } else {
                                        eprintln!("Failed to reload config");
                                    }
                                }
                            }
                            Err(e) => eprintln!("Watch error: {:?}", e),
                        }
                    }
                });
            }

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
