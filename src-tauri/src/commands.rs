use std::sync::Arc;
use tauri::{command, AppHandle, Emitter, State};
use tokio::sync::RwLock;
use crate::clipboard::{get_clipboard, set_clipboard};
use crate::config::ClipboardConfig;
use crate::history::{add_to_history, load_history, save_history, ClipboardEntry};
use crate::theme::Theme;

#[command]
pub async fn set_system_clipboard(
    app_handle: AppHandle,
    text: String,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<(), String> {
    set_clipboard(&text)?;
    let max_entries = config.read().await.0.history_limit as usize;
    add_to_history(&app_handle, text, "text".to_string(), max_entries)?;
    let _ = app_handle.emit("history-updated", "");
    Ok(())
}

#[command]
pub async fn get_system_clipboard(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<String, String> {
    let content = get_clipboard()?;
    if !content.trim().is_empty() {
        let max_entries = config.read().await.0.history_limit as usize;
        add_to_history(
            &app_handle,
            content.clone(),
            "text".to_string(),
            max_entries,
        )?;
        let _ = app_handle.emit("history-updated", "");
    }
    Ok(content)
}

#[command]
pub async fn get_clipboard_history(
    app_handle: AppHandle,
    limit: Option<usize>,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<Vec<ClipboardEntry>, String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    Ok(history.get_entries(limit))
}

#[command]
pub async fn clear_clipboard_history(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<(), String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let mut history = load_history(&app_handle, max_entries)?;
    history.clear();
    save_history(&app_handle, &history)?;
    let _ = app_handle.emit("history-updated", "");
    Ok(())
}

#[command]
pub async fn remove_clipboard_entry(
    app_handle: AppHandle,
    entry_id: String,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<bool, String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let mut history = load_history(&app_handle, max_entries)?;
    let removed = history.remove_entry(&entry_id);
    save_history(&app_handle, &history)?;
    let _ = app_handle.emit("history-updated", "");
    Ok(removed)
}

#[command]
pub async fn set_clipboard_from_history(
    app_handle: AppHandle,
    entry_id: String,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<(), String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    if let Some(entry) = history.entries.iter().find(|e| e.id == entry_id) {
        set_clipboard(&entry.content)?;
        add_to_history(
            &app_handle,
            entry.content.clone(),
            entry.content_type.clone(),
            max_entries,
        )?;
        let _ = app_handle.emit("history-updated", "");
        Ok(())
    } else {
        Err("Entry not found".to_string())
    }
}

#[command]
pub async fn get_history_stats(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<HistoryStats, String> {
    let max_entries = config.read().await.0.history_limit as usize;
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
    claw_config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<Theme, String> {
    let theme = claw_config.read().await.1.clone();
    Ok(theme)
}

#[command]
pub async fn get_claw_config(
    claw_config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<ClipboardConfig, String> {
    let cfg = claw_config.read().await;
    Ok(cfg.0.clone())
}
