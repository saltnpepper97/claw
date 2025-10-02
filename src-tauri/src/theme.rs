use std::path::{Path, PathBuf};
use rune_cfg::RuneConfig;
use dirs;

#[derive(Debug, Clone, Default, serde::Serialize)]
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
    pub highlight: String,
    pub outline: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Theme {
    pub light: ThemeColors,
    pub dark: ThemeColors,
}

pub fn find_theme_file(theme_name: &str) -> Option<PathBuf> {
    // User config first
    if let Some(config_dir) = dirs::config_dir() {
        let user_path = config_dir.join("claw").join("themes").join(format!("{}.rune", theme_name));
        if user_path.exists() {
            return Some(user_path);
        }
    }
    // Fallback system theme
    let system_path = Path::new("/usr/share/doc/claw/themes")
        .join(format!("{}.rune", theme_name));
    if system_path.exists() {
        return Some(system_path);
    }
    None
}

pub fn load_theme(theme_name: &str) -> Result<Theme, eyre::Report> {
    let path = find_theme_file(theme_name)
        .ok_or_else(|| eyre::eyre!("Theme file not found: {}", theme_name))?;
    let cfg = RuneConfig::from_file(path)?;

    // Helper to parse a color key
    fn get_color(cfg: &RuneConfig, base: &str, key: &str) -> String {
        let path = format!("theme.{}.{}", base, key);
        cfg.get::<String>(&path).unwrap_or_default()
    }

    let light = ThemeColors {
        background: get_color(&cfg, "light", "background"),
        background_alt: get_color(&cfg, "light", "background-alt"),
        titlebar_background: get_color(&cfg, "light", "titlebar-background"),
        text_primary: get_color(&cfg, "light", "text-primary"),
        text_secondary: get_color(&cfg, "light", "text-secondary"),
        hover: get_color(&cfg, "light", "hover"),
        hover_titlebar: get_color(&cfg, "light", "hover-titlebar"),
        selected: get_color(&cfg, "light", "selected"),
        highlight: get_color(&cfg, "light", "highlight"),
        outline: get_color(&cfg, "light", "outline"),
    };

    let dark = ThemeColors {
        background: get_color(&cfg, "dark", "background"),
        background_alt: get_color(&cfg, "dark", "background-alt"),
        titlebar_background: get_color(&cfg, "dark", "titlebar-background"),
        text_primary: get_color(&cfg, "dark", "text-primary"),
        text_secondary: get_color(&cfg, "dark", "text-secondary"),
        hover: get_color(&cfg, "dark", "hover"),
        hover_titlebar: get_color(&cfg, "dark", "hover-titlebar"),
        selected: get_color(&cfg, "dark", "selected"),
        highlight: get_color(&cfg, "dark", "highlight"),
        outline: get_color(&cfg, "dark", "outline"),
    };

    Ok(Theme { light, dark })
}
