use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_store::StoreBuilder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub content_type: String,
    pub source_path: Option<String>,
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
            max_entries: 100,
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

    pub fn add_entry(&mut self, content: Vec<u8>, content_type: String, source_path: Option<String>) {
        // Don't add if identical to last entry
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
            source_path,
        };

        self.entries.push_front(entry);

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
            // Try to deserialize with new format
            match serde_json::from_value::<ClipboardHistory>(value.clone()) {
                Ok(h) => h,
                Err(_) => {
                    eprintln!("Old history format detected, starting fresh");
                    ClipboardHistory::new(max_entries)
                }
            }
        }
        None => ClipboardHistory::new(max_entries),
    };

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
    content: &[u8],
    content_type: String,
    max_entries: usize,
    source_path: Option<String>
) -> Result<(), String> {
    let mut history = load_history(app_handle, max_entries)?;
    history.add_entry(content.to_vec(), content_type, source_path);
    save_history(app_handle, &history)
}
