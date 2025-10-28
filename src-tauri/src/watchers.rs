use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use crate::{config, utils::{normalize_clipboard_bytes, detect_content_type}, ConfigUpdate};

pub fn spawn_clipboard_watcher(
    app_handle: AppHandle,
    claw_config: Arc<RwLock<(config::ClipboardConfig, crate::theme::Theme)>>,
) {
    tauri::async_runtime::spawn(async move {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut poll_interval_ms = 300u64;
        let mut last_seen_hash: Option<u64> = None;
        let mut last_reinject_time = std::time::Instant::now();
        let mut consecutive_empty_reads = 0u32;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(poll_interval_ms)).await;

            let Ok(content_bytes) = crate::clipboard::get_clipboard() else {
                poll_interval_ms = 1000;
                continue;
            };

            // Check if clipboard is empty/invalid
            if crate::clipboard::should_ignore_bytes(&content_bytes) {
                consecutive_empty_reads += 1;
                
                // After 3 empty reads, try to restore from persistent memory
                if consecutive_empty_reads >= 3 {
                    if let Some(persistent_data) = crate::clipboard::get_persistent_clipboard() {
                        if !crate::clipboard::should_ignore_bytes(&persistent_data) {
                            eprintln!("Clipboard lost, restoring from persistent memory");
                            let _ = crate::clipboard::set_clipboard(&persistent_data);
                            
                            // FIXED: Use normalized hash to match what set_clipboard uses
                            let normalized = normalize_clipboard_bytes(&persistent_data);
                            let mut hasher = DefaultHasher::new();
                            normalized.hash(&mut hasher);
                            last_seen_hash = Some(hasher.finish());
                            last_reinject_time = std::time::Instant::now();
                            consecutive_empty_reads = 0;
                        }
                    }
                }
                
                drop(content_bytes);
                poll_interval_ms = 1000;
                continue;
            }

            // Reset empty counter - we have valid content
            consecutive_empty_reads = 0;

            // FIXED: Always use normalized bytes for hashing to match set_clipboard behavior
            let normalized = normalize_clipboard_bytes(&content_bytes);
            let mut hasher = DefaultHasher::new();
            normalized.hash(&mut hasher);
            let content_hash = hasher.finish();

            // Same content as before - just maintain it
            if Some(content_hash) == last_seen_hash {
                let elapsed = last_reinject_time.elapsed();
                if elapsed.as_secs() >= 2 {
                    let _ = crate::clipboard::set_clipboard(&content_bytes);
                    last_reinject_time = std::time::Instant::now();
                }
                drop(content_bytes);
                drop(normalized);
                poll_interval_ms = 300;
                continue;
            }

            // New content detected
            last_seen_hash = Some(content_hash);
            last_reinject_time = std::time::Instant::now();

            crate::clipboard::cache_clipboard_data(&content_bytes);

            // Check if this is content WE just wrote
            {
                let last = crate::LAST_WRITTEN_CLIPBOARD.lock().unwrap();
                if Some(content_hash) == *last {
                    drop(content_bytes);
                    drop(normalized);
                    poll_interval_ms = 300;
                    continue;
                }
                // Don't set it here - let set_clipboard handle it
            }

            poll_interval_ms = 300;

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

pub fn spawn_config_watcher(
    app_handle: AppHandle,
    claw_config: Arc<RwLock<(config::ClipboardConfig, crate::theme::Theme)>>,
) {
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
