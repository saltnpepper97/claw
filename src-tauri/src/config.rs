use eyre::{Result, eyre};
use std::path::{Path, PathBuf};
use std::process;

use crate::theme::{find_theme_file, Theme};
use rune_cfg::{RuneConfig, Value, RuneError};
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

/// Helper: tries key as-is, then _ → -, then - → _
fn get_config_or<T>(
    config: &RuneConfig,
    key: &str,
    default: T,
) -> T
where
    T: Clone + TryFrom<Value, Error = RuneError>,
{
    let variants = [
        key.to_string(),
        key.replace('_', "-"),
        key.replace('-', "_"),
    ];

    for k in variants {
        if let Ok(val) = config.get::<T>(&k) {
            return val;
        }
    }

    default
}

/// Manually parse and load gather imports since rune_cfg doesn't do this automatically
fn load_config_with_gather(path: &Path) -> Result<RuneConfig> {
    use std::fs;
    use regex::Regex;

    // Read the main config file
    let content = fs::read_to_string(path)
        .map_err(|e| eyre!("Failed to read config file: {}", e))?;

    // Parse gather statements manually, but only from non-commented lines
    // Format: gather "path/to/file.rune" [as alias]
    let gather_regex = Regex::new(r#"gather\s+"([^"]+)"(?:\s+as\s+(\w+))?"#).unwrap();
    
    // Create the main config
    let mut config = RuneConfig::from_str(&content)
        .map_err(|e| eyre!("Failed to parse main config: {}", e))?;

    // Process each line to find gather statements (excluding comments)
    for line in content.lines() {
        let trimmed = line.trim();
        
        // Skip commented lines
        if trimmed.starts_with('#') {
            continue;
        }
        
        // Find gather statement in this line
        if let Some(caps) = gather_regex.captures(line) {
            let gather_path_str = &caps[1];
            let alias = caps.get(2).map(|m| m.as_str().to_string());

            // Expand tilde if present
            let expanded_path = if gather_path_str.starts_with("~/") {
                dirs::home_dir()
                    .ok_or_else(|| eyre!("Could not determine home directory"))?
                    .join(&gather_path_str[2..])
            } else {
                // If relative path, make it relative to config directory
                let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
                config_dir.join(gather_path_str)
            };

            // Check if file exists
            if !expanded_path.exists() {
                continue;
            }

            // Load the gathered file
            let gather_content = fs::read_to_string(&expanded_path)
                .map_err(|e| eyre!("Failed to read gather file {:?}: {}", expanded_path, e))?;

            let gather_config = RuneConfig::from_str(&gather_content)
                .map_err(|e| eyre!("Failed to parse gather file {:?}: {}", expanded_path, e))?;

            // Get the document from the gathered config
            if let Some(doc) = gather_config.document() {
                let import_alias = alias.unwrap_or_else(|| {
                    // Use filename without extension as default alias
                    expanded_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("imported")
                        .to_string()
                });

                config.inject_import(import_alias, doc.clone());
            }
        }
    }

    Ok(config)
}

// --- Load Config ---
pub fn load_config(path: &str) -> Result<(ClipboardConfig, Theme)> {
    let path_buf = PathBuf::from(path);
    
    // Use our custom loader that handles gather statements
    let config = load_config_with_gather(&path_buf)?;

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
                if let Some(theme_path) = find_theme_file(&theme_name) {
                    if let Ok(theme_cfg) = RuneConfig::from_file(&theme_path) {
                        loaded_theme = Some(Theme::from_config(&theme_cfg, None));
                    }
                }
            }
        }

        // PRIORITY 4: Check for "theme" document
        if loaded_theme.is_none() {
            if config.has_document("theme") {
                loaded_theme = Some(Theme::from_config(&config, Some("theme")));
            }
        }

        // PRIORITY 5: Default theme
        loaded_theme.unwrap_or_else(|| Theme::default())
    };

    // Load clipboard config with flexible key names
    let history_limit = get_config_or(&config, "clipboard.history_max_length", 50u64);
    let enable_titlebar = get_config_or(&config, "clipboard.enable_titlebar", true);
    let force_dark_mode = get_config_or(&config, "clipboard.force_dark_mode", false);
    let persist_history = get_config_or(&config, "clipboard.persist_history", true);

    // Load keybinds
    let keybinds = Keybinds {
        up: get_config_or(&config, "clipboard.keybinds.up", "ArrowUp".to_string()),
        down: get_config_or(&config, "clipboard.keybinds.down", "ArrowDown".to_string()),
        delete: get_config_or(&config, "clipboard.keybinds.delete", "X".to_string()),
        delete_all: get_config_or(&config, "clipboard.keybinds.delete_all", "shift+X".to_string()),
        select: get_config_or(&config, "clipboard.keybinds.select", "Enter".to_string()),
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
    // First check XDG_CONFIG_HOME or ~/.config
    if let Some(config_dir) = dirs::config_dir() {
        let user_config = config_dir.join("claw").join("claw.rune");
        if user_config.exists() {
            return Some(user_config);
        }
    }

    // Fallback: check ~/.config explicitly (in case dirs crate fails)
    if let Some(home) = dirs::home_dir() {
        let alt_config = home.join(".config/claw/claw.rune");
        if alt_config.exists() {
            return Some(alt_config);
        }
    }

    // System-wide config
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
