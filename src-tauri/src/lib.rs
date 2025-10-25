mod clipboard;
mod commands;
mod config;
mod detect;
mod history;
mod theme;
mod utils;

use std::sync::{Arc, Mutex};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use once_cell::sync::Lazy;
use tauri::{
    generate_handler,
    menu::{Menu, MenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Listener
};
use tauri_plugin_cli::CliExt;
use tokio::sync::RwLock;
use history::ClipboardEntry;

use theme::Theme;
use utils::{detect_content_type, normalize_clipboard_bytes};

static LAST_WRITTEN_CLIPBOARD: Lazy<Mutex<Option<u64>>> = Lazy::new(|| Mutex::new(None));

use commands::{
    clear_clipboard_history, get_claw_config, get_clipboard_history, get_history_stats,
    get_system_clipboard, get_theme, remove_clipboard_entry, set_clipboard_from_history,
    set_system_clipboard, get_clipboard_entry_content
};
use config::load_claw_config;

#[derive(serde::Serialize, Clone)]
struct ConfigUpdate {
    enable_titlebar: bool,
    force_dark_mode: bool,
    theme: Theme,
}

// ============================================================================
// UTILITY FUNCTIONS (top)
// ============================================================================

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn human_size_from_bytes(size: usize) -> String {
    let kb = size as f64 / 1024.0;
    if kb < 1024.0 {
        format!("{:.0} KB", kb)
    } else {
        format!("{:.1} MB", kb / 1024.0)
    }
}

fn clipboard_entry_label_lightweight(entry: &ClipboardEntry) -> String {
    if entry.content_type.starts_with("image/") {
        image_menu_label_lightweight(entry)
    } else if entry.content_type == "text" {
        format!("📝 Text ({} bytes)", entry.content_size)
    } else {
        format!("📎 {} ({} bytes)", entry.content_type, entry.content_size)
    }
}

fn image_menu_label_lightweight(entry: &ClipboardEntry) -> String {
    if let Some(src) = &entry.source_path {
        if src.starts_with("file://") {
            let path = &src[7..];
            if let Some(fname) = std::path::Path::new(path).file_name() {
                return format!("🖼️ {} ({})", fname.to_string_lossy(), human_size_from_bytes(entry.content_size));
            }
        } else if let Ok(url) = url::Url::parse(src) {
            let host = url.host_str().unwrap_or("web");
            let filename = url
                .path_segments()
                .and_then(|s| s.last())
                .unwrap_or("image");
            return format!("🖼️ {} / {} ({})", host, filename, human_size_from_bytes(entry.content_size));
        }
    }

    format!("🖼️ Image ({})", human_size_from_bytes(entry.content_size))
}

fn update_tray_menu(
    app: &tauri::AppHandle,
    tray_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app.tray_by_id(tray_id).ok_or("Tray not found")?;

    let history = history::load_history(app, 100)?;
    let recent_items = history.get_entries(Some(5));

    let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;

    let menu = if !recent_items.is_empty() {
        let mut history_items = Vec::new();

        for (idx, entry) in recent_items.iter().enumerate() {
            let display_text = clipboard_entry_label_lightweight(&entry);
            
            let item_id = format!("history_{}", idx);
            let menu_item = MenuItem::with_id(app, &item_id, display_text, true, None::<&str>)?;
            history_items.push(menu_item);
        }

        let history_submenu = Submenu::with_items(
            app,
            "Recent Clipboard",
            true,
            &history_items
                .iter()
                .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
                .collect::<Vec<_>>(),
        )?;

        let clear_i = MenuItem::with_id(app, "clear_history", "Clear History", true, None::<&str>)?;
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        Menu::with_items(app, &[&show_i, &history_submenu, &clear_i, &quit_i])?
    } else {
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        Menu::with_items(app, &[&show_i, &quit_i])?
    };

    tray.set_menu(Some(menu))?;
    
    drop(history);

    Ok(())
}

// ============================================================================
// MAIN RUN FUNCTION
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()      
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.contains(&"--toggle".into()) {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
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

            let main_window = app.get_webview_window("main").unwrap();

            // Parse CLI arguments
            let cli_matches = app.cli().matches()?;
            let should_hide = cli_matches.args.contains_key("hide") && 
                              cli_matches.args.get("hide").unwrap().value.as_bool().unwrap_or(false);

            if should_hide {
                main_window.hide().ok();
            }

            main_window.on_window_event({
                let app_handle = app_handle.clone();
                move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                }
            });

            // Create initial tray menu
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // Build tray icon with unique ID
            let tray_id = "claw-tray";
            let _tray = TrayIconBuilder::with_id(tray_id)
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event({
                    let app_handle = app_handle.clone();
                    let tray_id_clone = tray_id.to_string();
                    move |app, event| {
                        let event_id = event.id.as_ref();
                        match event_id {
                            "show" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "quit" => {
                                app.exit(0);
                            }
                            "clear_history" => {
                                if let Ok(mut hist) = history::load_history(&app_handle, 100) {
                                    hist.clear();
                                    let _ = history::save_history(&app_handle, &hist);
                                    drop(hist);
                                    let _ = app_handle.emit("history-updated", "");
                                    let _ = update_tray_menu(&app_handle, &tray_id_clone);
                                }
                            }
                            id if id.starts_with("history_") => {
                                if let Ok(idx) = id.strip_prefix("history_").unwrap().parse::<usize>() {
                                    if let Ok(hist) = history::load_history(&app_handle, 100) {
                                        let entries = hist.get_entries(Some(5));
                                        if let Some(entry) = entries.get(idx) {
                                            // Load content on-demand instead of keeping in memory
                                            if let Some(content) = hist.get_entry_content(&entry.id) {
                                                crate::clipboard::cache_clipboard_data(&content);
                                                let _ = crate::clipboard::set_clipboard(&content);
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
                })       
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        toggle_main_window(&app);
                    }
                })
                .build(app)?;

            let _ = update_tray_menu(&app_handle, tray_id);

            // Listen for history updates to refresh tray menu
            {
                let app_handle_for_listener = app_handle.clone();
                let tray_id_for_listener = tray_id.to_string();
                app_handle.listen("history-updated", move |_event| {
                    if let Err(e) = update_tray_menu(&app_handle_for_listener, &tray_id_for_listener) {
                        eprintln!("Failed to update tray menu: {}", e);
                    }
                });
            }

            let claw_config = Arc::new(RwLock::new(load_claw_config()));
            app.manage(claw_config.clone());

            // Emit initial config to frontend
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
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

            // Clipboard watcher task
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                tauri::async_runtime::spawn(async move {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};

                    let mut poll_interval_ms = 300u64;  // How often to check for NEW clipboard changes
                    let mut last_seen_hash: Option<u64> = None;
                    let mut last_reinject_time = std::time::Instant::now();

                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval_ms)).await;

                        let Ok(content_bytes) = crate::clipboard::get_clipboard() else {
                            poll_interval_ms = 1000;
                            continue;
                        };

                        if crate::clipboard::should_ignore_bytes(&content_bytes) {
                            drop(content_bytes);
                            poll_interval_ms = 1000;
                            continue;
                        }

                        let mut hasher = DefaultHasher::new();
                        content_bytes.hash(&mut hasher);
                        let content_hash = hasher.finish();

                        if Some(content_hash) == last_seen_hash {
                            let elapsed = last_reinject_time.elapsed();
                            if elapsed.as_secs() >= 2 {  // Re-inject every 2 seconds
                                let _ = crate::clipboard::set_clipboard(&content_bytes);
                                last_reinject_time = std::time::Instant::now();
                            }
                            drop(content_bytes);
                            poll_interval_ms = 300;  // Check frequently for new changes
                            continue;
                        }

                        last_seen_hash = Some(content_hash);
                        last_reinject_time = std::time::Instant::now();

                        crate::clipboard::cache_clipboard_data(&content_bytes);

                        {
                            let mut last = crate::LAST_WRITTEN_CLIPBOARD.lock().unwrap();
                            if Some(content_hash) == *last {
                                drop(content_bytes);
                                poll_interval_ms = 300;
                                continue;
                            }
                            *last = Some(content_hash);
                        }

                        poll_interval_ms = 300;

                        let normalized = normalize_clipboard_bytes(&content_bytes);
                        drop(content_bytes);
                        
                        if normalized.is_empty() || crate::clipboard::should_ignore_bytes(&normalized) {
                            drop(normalized);
                            continue;
                        }

                        let history_limit = claw_config.read().await.0.history_limit as usize;
                        let content_type = detect_content_type(&normalized);
                        
                        if let Err(e) = crate::history::add_to_history(
                            &app_handle,
                            &normalized,
                            content_type,
                            history_limit,
                            None,
                        ) {
                            eprintln!("Failed to add to history: {}", e);
                        } else {
                            let _ = app_handle.emit("history-updated", "");
                        }

                        drop(normalized);
                    }
                });
            }   

            // Config hot-reload task
            {
                let app_handle = app_handle.clone();
                let claw_config = claw_config.clone();
                tauri::async_runtime::spawn(async move {
                    use notify::Config;
                    use std::collections::HashSet;
                    use std::path::PathBuf;
                    use std::sync::mpsc::channel;

                    let main_config_path: PathBuf =
                        config::find_config().expect("No claw.rune config found");

                    let mut watched_paths = HashSet::new();
                    watched_paths.insert(main_config_path.clone());

                    let gather_paths = || -> Vec<PathBuf> {
                        let content =
                            std::fs::read_to_string(&main_config_path).unwrap_or_default();
                        let gather_regex =
                            regex::Regex::new(r#"gather\s+"([^"]+)"(?:\s+as\s+(\w+))?"#).unwrap();
                        gather_regex
                            .captures_iter(&content)
                            .filter_map(|cap| {
                                let path_str = &cap[1];
                                let expanded_path = if path_str.starts_with("~/") {
                                    dirs::home_dir().map(|h| h.join(&path_str[2..]))
                                } else {
                                    Some(PathBuf::from(path_str))
                                }?;
                                if expanded_path.exists() {
                                    Some(expanded_path)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    };

                    for path in gather_paths() {
                        watched_paths.insert(path);
                    }

                    let (tx, rx) = channel();
                    let mut watcher: RecommendedWatcher =
                        Watcher::new(tx, Config::default()).expect("Failed to create file watcher");

                    for path in &watched_paths {
                        watcher
                            .watch(path, RecursiveMode::NonRecursive)
                            .expect("Failed to watch file");
                    }

                    loop {
                        match rx.recv() {
                            Ok(event) => {
                                if let Ok(ev) = event {
                                    if let EventKind::Modify(_) = ev.kind {
                                        if let Ok(new_config) =
                                            config::load_config(&main_config_path.to_string_lossy())
                                        {
                                            *claw_config.write().await = new_config.clone();

                                            let update = ConfigUpdate {
                                                enable_titlebar: new_config.0.enable_titlebar,
                                                force_dark_mode: new_config.0.force_dark_mode,
                                                theme: new_config.1.clone(),
                                            };

                                            let _ = app_handle.emit("config-reloaded", update);

                                            let new_paths: HashSet<_> =
                                                gather_paths().into_iter().collect();
                                            for path in new_paths.difference(&watched_paths) {
                                                watcher
                                                    .watch(path, RecursiveMode::NonRecursive)
                                                    .ok();
                                            }
                                            watched_paths = new_paths
                                                .union(
                                                    &[main_config_path.clone()]
                                                        .into_iter()
                                                        .collect(),
                                                )
                                                .cloned()
                                                .collect();
                                        } else {
                                            eprintln!("Failed to reload config");
                                        }
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

