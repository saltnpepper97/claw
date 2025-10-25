use std::collections::VecDeque;
use std::path::PathBuf;
use std::fs;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_store::StoreBuilder;

// Maximum size per entry (5MB)
const MAX_ENTRY_SIZE: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub content_type: String,
    pub source_path: Option<String>,
    // Store size instead of content
    pub content_size: usize,
    #[serde(skip)]
    pub content: Vec<u8>,
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
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    pub fn add_entry(&mut self, content: Vec<u8>, content_type: String, source_path: Option<String>) {
        // Skip oversized entries
        if content.len() > MAX_ENTRY_SIZE {
            eprintln!("Skipping entry: size {} exceeds limit", content.len());
            return;
        }

        if let Some(last) = self.entries.front() {
            if last.content_size == content.len() {
                if let Some(last_content) = self.get_entry_content_internal(&last.id) {
                    if last_content == content {
                        return;
                    }
                }
            }
        }

        let content_size = content.len();
        let entry = ClipboardEntry {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.clone(),
            timestamp: Utc::now(),
            content_type,
            source_path,
            content_size,
        };

        if let Err(e) = self.save_entry_content(&entry) {
            eprintln!("Failed to save clipboard content: {}", e);
            return;
        }

        let mut entry_for_memory = entry;
        entry_for_memory.content = Vec::new(); // Free the Vec
        entry_for_memory.content.shrink_to_fit(); // Release capacity

        self.entries.push_front(entry_for_memory);

        // Remove old entries and clean up their files
        while self.entries.len() > self.max_entries {
            if let Some(old_entry) = self.entries.pop_back() {
                self.delete_entry_file(&old_entry.id);
            }
        }
    }

    fn save_entry_content(&self, entry: &ClipboardEntry) -> std::io::Result<()> {
        if !entry.content.is_empty() {
            let path = self.get_entry_path(&entry.id);
            fs::create_dir_all("history")?;
            fs::write(path, &entry.content)?;
        }
        Ok(())
    }

    fn get_entry_path(&self, id: &str) -> PathBuf {
        PathBuf::from("history").join(format!("{}.bin", id))
    }

    fn delete_entry_file(&self, id: &str) {
        let path = self.get_entry_path(id);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    fn load_entry_content_from_disk(entry_id: &str) -> std::io::Result<Vec<u8>> {
        let path = PathBuf::from("history").join(format!("{}.bin", entry_id));
        if path.exists() {
            fs::read(path)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn remove_entry(&mut self, id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|entry| entry.id == id) {
            self.entries.remove(pos);
            self.delete_entry_file(id);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        // Delete all entry files
        for entry in &self.entries {
            self.delete_entry_file(&entry.id);
        }
        self.entries.clear();
        self.entries.shrink_to_fit(); // Release memory
    }

    // Internal method that doesn't cache
    fn get_entry_content_internal(&self, id: &str) -> Option<Vec<u8>> {
        Self::load_entry_content_from_disk(id).ok()
    }

    // Public method for API calls - loads fresh from disk each time
    pub fn get_entry_content(&self, id: &str) -> Option<Vec<u8>> {
        // Verify entry exists
        if !self.entries.iter().any(|e| e.id == id) {
            return None;
        }
        
        Self::load_entry_content_from_disk(id).ok()
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
    
    // Ensure no content is in memory
    for entry in &mut history.entries {
        entry.content = Vec::new();
        entry.content.shrink_to_fit();
    }
    
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
    save_history(app_handle, &history)?;
    
    // Explicitly drop to free memory
    drop(history);
    Ok(())
}
