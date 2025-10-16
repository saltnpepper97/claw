use eyre::{Result, eyre};
use std::path::{Path, PathBuf};
use std::process;

use crate::theme::{find_theme_file, Theme};
use rune_cfg::RuneConfig;
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
}

// --- Load Config ---
pub fn load_config(path: &str) -> Result<(ClipboardConfig, Theme)> {
    let config = RuneConfig::from_file(path)
        .map_err(|e| eyre!("Failed to load config: {}", e))?;

    // Load the theme block with priority system
    let theme = {
        let mut loaded_theme = None;

        // PRIORITY 1: Check for aliased gather imports (gather "path" as alias)
        let aliases = config.import_aliases();
        for alias in aliases {
            if config.has_document(&alias) {
                let test_path = format!("{}.theme.light.background", alias);
                if config.get::<String>(&test_path).is_ok() {
                    loaded_theme = Some(Theme::from_config(&config, Some(&alias)));
                    break;
                }
            }
        }

        // PRIORITY 2: Check for top-level theme block (from non-aliased gather)
        if loaded_theme.is_none() {
            if config.get::<String>("theme.light.background").is_ok() {
                loaded_theme = Some(Theme::from_config(&config, None));
            }
        }

        // PRIORITY 3: Check for clipboard.theme field
        if loaded_theme.is_none() {
            if let Ok(theme_name) = config.get::<String>("clipboard.theme") {
                // Try to find and load the theme file
                if let Some(theme_path) = find_theme_file(&theme_name) {
                    if let Ok(theme_cfg) = RuneConfig::from_file(&theme_path) {
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
        loaded_theme.unwrap_or_else(|| Theme::default())
    };

    // Load clipboard config with proper validation
    let history_limit = config.get_or("clipboard.history_max_length", 50u64);
    let enable_titlebar = config.get_or("clipboard.enable_titlebar", true);
    let force_dark_mode = config.get_or("clipboard.force_dark_mode", false);

    // Load keybinds with proper defaults
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
    };

    Ok((clipboard, theme))
}

// --- Config file discovery ---
pub fn find_config() -> Option<PathBuf> {
    if let Some(home) = dirs::config_dir() {
        let user_config = home.join("claw").join("claw.rune");
        if user_config.exists() {
            return Some(user_config);
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
