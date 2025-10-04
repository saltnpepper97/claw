use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_store::StoreBuilder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub content_type: String, // "text", "image", etc.
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipboardHistory {
    pub entries: VecDeque<ClipboardEntry>,
    pub max_entries: usize,
}

impl Default for ClipboardHistory {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100, // Default max entries
        }
    }
}

impl ClipboardHistory {
    #[allow(dead_code)]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    pub fn add_entry(&mut self, content: String, content_type: String) {
        // Don't add if it's the same as the last entry
        if let Some(last) = self.entries.front() {
            if last.content == content {
                return;
            }
        }

        let entry = ClipboardEntry {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            timestamp: Utc::now(),
            content_type,
        };

        self.entries.push_front(entry);

        // Remove oldest entries if we exceed max_entries
        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }

    pub fn remove_entry(&mut self, id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn get_entries(&self, limit: Option<usize>) -> Vec<ClipboardEntry> {
        match limit {
            Some(n) => self.entries.iter().take(n).cloned().collect(),
            None => self.entries.iter().cloned().collect(),
        }
    }
}

const STORE_FILE: &str = "claw_history.json";
const HISTORY_KEY: &str = "history";

pub fn load_history(
    app_handle: &AppHandle,
    max_entries: usize,
) -> Result<ClipboardHistory, String> {
    let store = StoreBuilder::new(app_handle, PathBuf::from(STORE_FILE))
        .build()
        .map_err(|e| format!("Failed to create store: {}", e))?;

    let mut history = match store.get(HISTORY_KEY) {
        Some(value) => {
            let h: ClipboardHistory = serde_json::from_value(value.clone())
                .map_err(|e| format!("Failed to deserialize history: {}", e))?;
            h
        }
        None => ClipboardHistory::new(max_entries),
    };

    // always respect current max_entries
    history.max_entries = max_entries;
    Ok(history)
}

pub fn save_history(app_handle: &AppHandle, history: &ClipboardHistory) -> Result<(), String> {
    let store = StoreBuilder::new(app_handle, PathBuf::from(STORE_FILE))
        .build()
        .map_err(|e| format!("Failed to create store: {}", e))?;

    let value =
        serde_json::to_value(history).map_err(|e| format!("Failed to serialize history: {}", e))?;

    store.set(HISTORY_KEY.to_string(), value);
    store
        .save()
        .map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

pub fn add_to_history(
    app_handle: &AppHandle,
    content: String,
    content_type: String,
    max_entries: usize,
) -> Result<(), String> {
    let mut history = load_history(app_handle, max_entries)?;
    history.add_entry(content, content_type);
    save_history(app_handle, &history)
}
