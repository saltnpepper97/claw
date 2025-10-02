use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
pub enum DesktopEnv {
    X11,
    Wayland,
    Unknown,
}

// Cache the desktop environment detection result
static DESKTOP_ENV: OnceLock<DesktopEnv> = OnceLock::new();

fn detect_desktop_env() -> DesktopEnv {
    // First check XDG_SESSION_TYPE which is the most reliable
    if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
        match session_type.to_lowercase().as_str() {
            "wayland" => return DesktopEnv::Wayland,
            "x11" => return DesktopEnv::X11,
            _ => {}
        }
    }
    
    // Fallback to checking display variables
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DesktopEnv::Wayland
    } else if std::env::var("DISPLAY").is_ok() {
        DesktopEnv::X11
    } else {
        DesktopEnv::Unknown
    }
}

pub fn current_desktop_env() -> DesktopEnv {
    *DESKTOP_ENV.get_or_init(|| detect_desktop_env())
}
