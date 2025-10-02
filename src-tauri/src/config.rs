use std::path::{Path, PathBuf};
use eyre::Result;

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
    pub theme: String,
    pub history_limit: u64,
    pub enable_titlebar: bool,
    pub force_dark_mode: bool,
    pub keybinds: Keybinds,
}

// --- Helpers ---

// Try both hyphenated and underscored versions of a key
fn try_get_string(config: &RuneConfig, base_path: &str) -> Option<String> {
    let hyphenated = base_path.replace('_', "-");
    if let Ok(val) = config.get::<String>(&hyphenated) {
        return Some(val);
    }
    let underscored = base_path.replace('-', "_");
    if let Ok(val) = config.get::<String>(&underscored) {
        return Some(val);
    }
    None
}

fn try_get_bool(config: &RuneConfig, base_path: &str, default: bool) -> bool {
    let hyphenated = base_path.replace('_', "-");
    if let Ok(val) = config.get::<bool>(&hyphenated) {
        return val;
    }
    let underscored = base_path.replace('-', "_");
    if let Ok(val) = config.get::<bool>(&underscored) {
        return val;
    }
    default
}

fn try_get_number(config: &RuneConfig, base_path: &str, default: u64) -> u64 {
    let hyphenated = base_path.replace('_', "-");
    if let Ok(val) = config.get::<u64>(&hyphenated) {
        return val;
    }
    let underscored = base_path.replace('-', "_");
    if let Ok(val) = config.get::<u64>(&underscored) {
        return val;
    }
    default
}

// --- Load Config ---

pub fn load_config(path: &str) -> Result<ClipboardConfig> {
    let config = RuneConfig::from_file(path)?;

    let theme = try_get_string(&config, "clipboard.theme").unwrap_or_else(|| "default".into());
    let history_limit = try_get_number(&config, "clipboard.history_max_length", 50);
    let enable_titlebar = try_get_bool(&config, "clipboard.enable_titlebar", true);
    let force_dark_mode = try_get_bool(&config, "clipboard.force_dark_mode", false);

    // --- Keybinds ---
    let keybinds_path = "clipboard.keybinds";
    let keybinds = Keybinds {
        up: try_get_string(&config, &format!("{}.up", keybinds_path)).unwrap_or_default(),
        down: try_get_string(&config, &format!("{}.down", keybinds_path)).unwrap_or_default(),
        delete: try_get_string(&config, &format!("{}.delete", keybinds_path)).unwrap_or_default(),
        delete_all: try_get_string(&config, &format!("{}.delete_all", keybinds_path)).unwrap_or_default(),
        select: try_get_string(&config, &format!("{}.select", keybinds_path)).unwrap_or_default(),
    };

    Ok(ClipboardConfig {
        theme,
        history_limit,
        enable_titlebar,
        force_dark_mode,
        keybinds,
    })
}

pub fn find_config() -> Option<PathBuf> {
    // 1. User config
    if let Some(home) = dirs::config_dir() {
        let user_config = home.join("claw").join("claw.rune");
        if user_config.exists() {
            return Some(user_config);
        }
    }

    // 2. Fallback default config
    let default_config = Path::new("/usr/share/doc/claw/claw.rune");
    if default_config.exists() {
        return Some(default_config.to_path_buf());
    }

    None
}

pub fn load_claw_config() -> ClipboardConfig {
    let path = find_config().expect("No claw.rune config found");
    load_config(&path.to_string_lossy()).expect("Failed to parse claw.rune config")
}


// --- Test ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_claw_config() {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../examples/claw.rune");

        let config = load_config(path.to_str().unwrap()).expect("Failed to load Claw config");

        println!("Loaded clipboard config: {:#?}", config);

        assert_eq!(config.theme, "default");
        assert_eq!(config.history_limit, 50);
        assert_eq!(config.enable_titlebar, true);
        assert_eq!(config.force_dark_mode, false);
        assert_eq!(config.keybinds.up, "k");
        assert_eq!(config.keybinds.down, "j");
        assert_eq!(config.keybinds.delete, "x");
        assert_eq!(config.keybinds.select, "Return");
    }
}
