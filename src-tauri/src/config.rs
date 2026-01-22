use eyre::{eyre, Result};
use std::path::{Path, PathBuf};
use std::process;

use crate::theme::{find_theme_file, Theme};
use rune_cfg::{RuneConfig, RuneError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinds {
    pub up: String,
    pub down: String,
    pub delete: String,
    pub delete_all: String,
    pub select: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    pub history_limit: u64,
    pub enable_titlebar: bool,
    pub force_dark_mode: bool,
    pub keybinds: Keybinds,
    pub persist_history: bool,
}

/// rune_cfg 0.4.0 `from_file_with_base` requires both args share the same type `P`,
/// so we pass PathBuf for both.
fn rune_from_file_with_base(path: PathBuf, base_dir: PathBuf) -> Result<RuneConfig> {
    RuneConfig::from_file_with_base(path, base_dir)
        .map_err(|e: RuneError| eyre!("Failed to load config: {}", e))
}

// --- Load Config ---
pub fn load_config(path: &str) -> Result<(ClipboardConfig, Theme)> {
    let path_buf = PathBuf::from(path);
    let base_dir = path_buf
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    // IMPORTANT: load from file so rune_cfg can resolve gathers/imports
    let config = rune_from_file_with_base(path_buf.clone(), base_dir)?;

    // Load the theme block with priority system
    let theme = {
        let mut loaded_theme: Option<Theme> = None;

        // PRIORITY 1: Any imported doc containing theme.light.background
        for alias in config.import_aliases() {
            let test_path = format!("{}.theme.light.background", alias);
            if config.get::<String>(&test_path).is_ok() {
                loaded_theme = Some(Theme::from_config(&config, Some(&alias)));
                break;
            }
        }

        // PRIORITY 2: Top-level theme block
        if loaded_theme.is_none() {
            if config.get::<String>("theme.light.background").is_ok() {
                loaded_theme = Some(Theme::from_config(&config, None));
            }
        }

        // PRIORITY 3: clipboard.theme field (load external theme file)
        if loaded_theme.is_none() {
            if let Ok(theme_name) = config.get::<String>("clipboard.theme") {
                if let Some(theme_path) = find_theme_file(&theme_name) {
                    let theme_base = theme_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf();

                    if let Ok(theme_cfg) = rune_from_file_with_base(theme_path.clone(), theme_base) {
                        loaded_theme = Some(Theme::from_config(&theme_cfg, None));
                    }
                }
            }
        }

        // PRIORITY 4: Check for "theme" document
        if loaded_theme.is_none() && config.has_document("theme") {
            loaded_theme = Some(Theme::from_config(&config, Some("theme")));
        }

        // PRIORITY 5: Default theme
        loaded_theme.unwrap_or_else(Theme::default)
    };

    // Load clipboard config (0.4.0: snake_case/kebab-case handled by get/get_or)
    let history_limit = config.get_or("clipboard.history_max_length", 50u64);
    let enable_titlebar = config.get_or("clipboard.enable_titlebar", true);
    let force_dark_mode = config.get_or("clipboard.force_dark_mode", false);
    let persist_history = config.get_or("clipboard.persist_history", true);

    // Load keybinds
    let keybinds = Keybinds {
        up: config.get_or("clipboard.keybinds.up", "ArrowUp".to_string()),
        down: config.get_or("clipboard.keybinds.down", "ArrowDown".to_string()),
        delete: config.get_or("clipboard.keybinds.delete", "X".to_string()),
        delete_all: config.get_or("clipboard.keybinds.delete_all", "shift+X".to_string()),
        select: config.get_or("clipboard.keybinds.select", "Enter".to_string()),
    };

    let clipboard = ClipboardConfig {
        history_limit,
        enable_titlebar,
        force_dark_mode,
        keybinds,
        persist_history,
    };

    Ok((clipboard, theme))
}

// --- Config file discovery ---
pub fn find_config() -> Option<PathBuf> {
    if let Some(config_dir) = dirs::config_dir() {
        let user_config = config_dir.join("claw").join("claw.rune");
        if user_config.exists() {
            return Some(user_config);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let alt_config = home.join(".config/claw/claw.rune");
        if alt_config.exists() {
            return Some(alt_config);
        }
    }

    let default_config = Path::new("/usr/share/doc/claw/claw.rune");
    if default_config.exists() {
        return Some(default_config.to_path_buf());
    }

    None
}

/// Top-level config loader that exits gracefully on failure.
pub fn load_claw_config() -> (ClipboardConfig, Theme) {
    let path = find_config().expect("No claw.rune config found");
    match load_config(&path.to_string_lossy()) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("❌ Configuration error:\n{}", err);
            process::exit(1);
        }
    }
}
