use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, State, command};

use crate::config::ClipboardConfig;
use crate::clipboard::{set_clipboard, get_clipboard};
use crate::history::{load_history, save_history, add_to_history, ClipboardEntry};

#[command]
pub async fn set_system_clipboard(
    app_handle: AppHandle, 
    text: String,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<(), String> {
    set_clipboard(&text)?;

    let max_entries = config.read().await.history_limit as usize;
    add_to_history(&app_handle, text, "text".to_string(), max_entries)?;

    Ok(())
}

#[command]
pub async fn get_system_clipboard(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<String, String> {
    let content = get_clipboard()?;

    if !content.trim().is_empty() {
        let max_entries = config.read().await.history_limit as usize;
        add_to_history(&app_handle, content.clone(), "text".to_string(), max_entries)?;
    }

    Ok(content)
}

#[command]
pub async fn get_clipboard_history(
    app_handle: AppHandle, 
    limit: Option<usize>,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<Vec<ClipboardEntry>, String> {
    let max_entries = config.read().await.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    Ok(history.get_entries(limit))
}

#[command]
pub async fn clear_clipboard_history(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<(), String> {
    let max_entries = config.read().await.history_limit as usize;
    let mut history = load_history(&app_handle, max_entries)?;
    history.clear();
    save_history(&app_handle, &history)
}

#[command]
pub async fn remove_clipboard_entry(
    app_handle: AppHandle, 
    entry_id: String,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<bool, String> {
    let max_entries = config.read().await.history_limit as usize;
    let mut history = load_history(&app_handle, max_entries)?;
    let removed = history.remove_entry(&entry_id);
    save_history(&app_handle, &history)?;
    Ok(removed)
}

#[command]
pub async fn set_clipboard_from_history(
    app_handle: AppHandle, 
    entry_id: String,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<(), String> {
    let max_entries = config.read().await.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;

    if let Some(entry) = history.entries.iter().find(|e| e.id == entry_id) {
        set_clipboard(&entry.content)?;
        // Move this entry to the front of history
        add_to_history(&app_handle, entry.content.clone(), entry.content_type.clone(), max_entries)?;
        Ok(())
    } else {
        Err("Entry not found".to_string())
    }
}

#[command]
pub async fn get_history_stats(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<HistoryStats, String> {
    let max_entries = config.read().await.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    Ok(HistoryStats {
        total_entries: history.entries.len(),
        max_entries: history.max_entries,
    })
}

#[derive(serde::Serialize)]
pub struct HistoryStats {
    pub total_entries: usize,
    pub max_entries: usize,
}

#[command]
pub async fn get_theme(
    claw_config: State<'_, Arc<RwLock<crate::config::ClipboardConfig>>>,
) -> Result<crate::theme::Theme, String> {
    let theme_name = claw_config.read().await.theme.clone();
    crate::theme::load_theme(&theme_name).map_err(|e| e.to_string())
}

#[command]
pub async fn get_claw_config(
    claw_config: State<'_, Arc<RwLock<ClipboardConfig>>>
) -> Result<ClipboardConfig, String> {
    let cfg = claw_config.read().await;
    Ok(ClipboardConfig {
        enable_titlebar: cfg.enable_titlebar,
        force_dark_mode: cfg.force_dark_mode,
        theme: cfg.theme.clone(),
        history_limit: cfg.history_limit,
        keybinds: cfg.keybinds.clone(),
    })
}

