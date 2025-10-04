use dirs;
use rune_cfg::RuneConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ThemeColors {
    pub background: String,
    #[serde(rename = "background-alt")]
    pub background_alt: String,
    #[serde(rename = "titlebar-background")]
    pub titlebar_background: String,
    #[serde(rename = "text-primary")]
    pub text_primary: String,
    #[serde(rename = "text-secondary")]
    pub text_secondary: String,
    pub hover: String,
    #[serde(rename = "hover-titlebar")]
    pub hover_titlebar: String,
    pub selected: String,
    #[serde(rename = "selected-foreground")]
    pub selected_foreground: String,
    pub highlight: String,
    pub outline: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Theme {
    pub light: ThemeColors,
    pub dark: ThemeColors,
}

impl Theme {
    /// Load theme from a RuneConfig, optionally from a document alias
    pub fn from_config(cfg: &RuneConfig, doc_alias: Option<&str>) -> Self {
        let get_value = |base: &str, key: &str| -> String {
            if let Some(alias) = doc_alias {
                // First try alias.theme.base.key
                let full_path = format!("{alias}.theme.{base}.{key}");
                if let Ok(val) = cfg.get::<String>(&full_path) {
                    return val;
                }
                // fallback to alias.base.key (in case flat structure)
                let full_path = format!("{alias}.{base}.{key}");
                return cfg.get::<String>(&full_path).unwrap_or_default();
            }
            // fallback to top-level theme block
            cfg.get::<String>(&format!("theme.{base}.{key}"))
                .unwrap_or_default()
        };

        let light = ThemeColors {
            background: get_value("light", "background"),
            background_alt: get_value("light", "background-alt"),
            titlebar_background: get_value("light", "titlebar-background"),
            text_primary: get_value("light", "text-primary"),
            text_secondary: get_value("light", "text-secondary"),
            hover: get_value("light", "hover"),
            hover_titlebar: get_value("light", "hover-titlebar"),
            selected: get_value("light", "selected"),
            selected_foreground: get_value("light", "selected-foreground"),
            highlight: get_value("light", "highlight"),
            outline: get_value("light", "outline"),
        };

        let dark = ThemeColors {
            background: get_value("dark", "background"),
            background_alt: get_value("dark", "background-alt"),
            titlebar_background: get_value("dark", "titlebar-background"),
            text_primary: get_value("dark", "text-primary"),
            text_secondary: get_value("dark", "text-secondary"),
            hover: get_value("dark", "hover"),
            hover_titlebar: get_value("dark", "hover-titlebar"),
            selected: get_value("dark", "selected"),
            selected_foreground: get_value("dark", "selected-foreground"),
            highlight: get_value("dark", "highlight"),
            outline: get_value("dark", "outline"),
        };

        Self { light, dark }
    }
}

/// Search for a theme file on disk
pub fn find_theme_file(theme_name: &str) -> Option<PathBuf> {
    let path = Path::new(theme_name);
    if path.exists() {
        return Some(path.to_path_buf());
    }

    if let Some(config_dir) = dirs::config_dir() {
        let user_path = config_dir
            .join("claw")
            .join("themes")
            .join(format!("{}.rune", theme_name));
        if user_path.exists() {
            return Some(user_path);
        }
    }

    let system_path = Path::new("/usr/share/doc/claw/themes").join(format!("{}.rune", theme_name));
    if system_path.exists() {
        return Some(system_path);
    }

    None
}
