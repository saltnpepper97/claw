use std::sync::Arc;
use tauri::{command, AppHandle, Emitter, State};
use tokio::sync::RwLock;
use crate::clipboard::{get_clipboard_for_paste, set_clipboard, cache_clipboard_data};
use crate::config::ClipboardConfig;
use crate::history::{load_history, save_history, ClipboardEntry, ClipboardHistory};
use crate::theme::Theme;
use crate::detect_content_type;

#[command]
pub async fn set_system_clipboard(
    app_handle: AppHandle,
    text: String,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<(), String> {
    let content = text.as_bytes().to_vec();
    let content_type = detect_content_type(&content);

    // Cache it in persistent memory before setting
    cache_clipboard_data(&content);
    
    set_clipboard(&content)?;

    let max_entries = config.read().await.0.history_limit as usize;

    let source_path = if content.starts_with(b"file://") {
        Some(String::from_utf8_lossy(&content[7..]).to_string())
    } else {
        None
    };

    crate::history::add_to_history(
        &app_handle,
        &content,
        content_type,
        max_entries,
        source_path,
    )?;

    let _ = app_handle.emit("history-updated", "");
    Ok(())
}

#[command]
pub async fn get_system_clipboard(
    _app_handle: AppHandle,
    _config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<ClipboardData, String> {
    eprintln!("=== get_system_clipboard command called ===");
    
    let bytes = get_clipboard_for_paste()?;
    
    eprintln!("Got {} bytes from get_clipboard_for_paste", bytes.len());
    
    if !bytes.is_empty() {
        let content_type = detect_content_type(&bytes);
        eprintln!("Content type: {}", content_type);
        
        Ok(ClipboardData {
            content: bytes,
            content_type,
        })
    } else {
        eprintln!("Bytes are empty, returning empty clipboard");
        Ok(ClipboardData {
            content: vec![],
            content_type: "text".to_string(),
        })
    }
}

#[command]
pub async fn get_clipboard_history(
    app_handle: AppHandle,
    limit: Option<usize>,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<Vec<ClipboardEntry>, String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    
    // Return entries WITHOUT content (content field is already empty)
    Ok(history.get_entries(limit))
}

#[command]
pub async fn get_clipboard_entry_content(
    app_handle: AppHandle,
    entry_id: String,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<Vec<u8>, String> {
    let max_entries = config.read().await.0.history_limit as usize;
    let history = load_history(&app_handle, max_entries)?;
    
    history.get_entry_content(&entry_id)
        .ok_or_else(|| "Entry not found".to_string())
}

#[command]
pub async fn clear_clipboard_history(
    app_handle: AppHandle,
    config: State<'_, Arc<RwLock<(ClipboardConfig, Theme)>>>,
) -> Result<(), String> {
    let max_entries = config.read().await.0.history_limit as usize;

    {
        let mut last = crate::LAST_WRITTEN_CLIPBOARD.lock().unwrap();
        *last = None;
    }

    let mut history = load_history(&app_handle, max_entries)?;
    history.clear();
    
    save_history(&app_handle, &ClipboardHistory::default())?;
    
    // Explicitly drop to free memory
    drop(history);

    crate::clipboard::set_clipboard(&[])?;

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

    if let Some(content) = history.get_entry_content(&entry_id) {
        cache_clipboard_data(&content);
        
        set_clipboard(&content)?;
        // Explicitly drop content after use
        drop(content);
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
    
    let stats = HistoryStats {
        total_entries: history.entries.len(),
        max_entries: history.max_entries,
        total_size_bytes: history.entries.iter().map(|e| e.content_size).sum(),
    };
    
    Ok(stats)
}

#[derive(serde::Serialize)]
pub struct HistoryStats {
    pub total_entries: usize,
    pub max_entries: usize,
    pub total_size_bytes: usize,
}

#[derive(serde::Serialize)]
pub struct ClipboardData {
    pub content: Vec<u8>,
    pub content_type: String,
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
