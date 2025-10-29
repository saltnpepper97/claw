use crate::detect::DesktopEnv;
use std::io::Read;
use std::sync::Mutex;
use std::hash::{DefaultHasher, Hash, Hasher};
use once_cell::sync::Lazy;
use wl_clipboard_rs::copy::{MimeType, Source};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType as PasteMimeType, Seat};
use x11_clipboard::Clipboard as X11Clipboard;
use crate::LAST_WRITTEN_CLIPBOARD;
use crate::utils::{detect_content_type, normalize_clipboard_bytes};

pub static PERSISTENT_CLIPBOARD_DATA: Lazy<Mutex<Option<Vec<u8>>>> = Lazy::new(|| Mutex::new(None));

/// Set Wayland clipboard
pub fn set_wayland_clipboard_bytes(data: &[u8]) -> Result<(), String> {
    let content_type = detect_content_type(data);
    
    // Store BEFORE setting to avoid race condition
    *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(data.to_vec());
    
    let mime_type = if content_type.starts_with("image/") {
        match content_type.as_str() {
            "image/png" => MimeType::Specific("image/png".into()),
            "image/jpeg" => MimeType::Specific("image/jpeg".into()),
            "image/gif" => MimeType::Specific("image/gif".into()),
            "image/webp" => MimeType::Specific("image/webp".into()),
            "image/bmp" => MimeType::Specific("image/bmp".into()),
            _ => MimeType::Autodetect,
        }
    } else {
        MimeType::Autodetect
    };

    wl_clipboard_rs::copy::Options::new()
        .copy(Source::Bytes(data.into()), mime_type)
        .map_err(|e| e.to_string())
}

/// Check if bytes should be ignored
pub fn should_ignore_bytes(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }

    // Allow shebang scripts
    if bytes.starts_with(b"#!") {
        return false;
    }

    if bytes == b"0,0" {
        return true;
    }

    // Allow file:// URIs (file copies), but reject junk
    if bytes.starts_with(b"file://") {
        let clean: Vec<u8> = bytes
            .iter()
            .cloned()
            .filter(|&b| b != 0 && !b.is_ascii_whitespace())
            .collect();
        return clean.is_empty() || clean == b"file://";
    }

    // Keep old guard for <meta tags (tiny icons)
    if bytes.starts_with(b"<meta") {
        return true;
    }

    // Remove NULs and whitespace for other checks
    let clean = bytes
        .iter()
        .cloned()
        .filter(|&b| b != 0)
        .skip_while(|b| b.is_ascii_whitespace())
        .collect::<Vec<u8>>();

    if clean.is_empty() || clean == b"0,0" {
        return true;
    }

    // Ignore tiny images
    if detect_content_type(&clean).starts_with("image/") && clean.len() < 100 {
        return true;
    }

    false
}

/// Get Wayland clipboard - reads from system
pub fn get_wayland_clipboard_bytes() -> Result<Vec<u8>, String> {
    let mimes = [
        PasteMimeType::Text,
        PasteMimeType::Specific("image/png".into()),
        PasteMimeType::Specific("image/jpeg".into()),
        PasteMimeType::Specific("image/gif".into()),
        PasteMimeType::Specific("image/webp".into()),
        PasteMimeType::Specific("image/bmp".into()),
    ];

    let mut candidate_image: Option<Vec<u8>> = None;

    for mime in &mimes {
        if let Ok((mut pipe, _)) = get_contents(ClipboardType::Regular, Seat::Unspecified, *mime) {
            let mut bytes = Vec::with_capacity(1024);
            if pipe.read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                drop(pipe);

                if *mime == PasteMimeType::Text {
                    let clean = bytes.iter().cloned().filter(|&b| b != 0).collect::<Vec<u8>>();
                    if !should_ignore_bytes(&clean) {
                        if String::from_utf8(clean.clone()).is_ok() {
                            *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(clean.clone());
                            return Ok(clean);
                        }
                    }
                    continue;
                }

                if detect_content_type(&bytes).starts_with("image/") && bytes.len() >= 100 {
                    candidate_image = Some(bytes);
                }
            } else {
                drop(pipe);
            }
        }
    }

    if let Some(img) = candidate_image {
        *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(img.clone());
        return Ok(img);
    }

    if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
        return Ok(data.clone());
    }

    Ok(vec![])
}

/// Set X11 clipboard
pub fn set_x11_clipboard(data: &[u8]) -> Result<(), String> {
    *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(data.to_vec());
    
    let clipboard = X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;
    clipboard
        .store(
            clipboard.setter.atoms.clipboard,
            clipboard.setter.atoms.incr,
            data,
        )
        .map_err(|e| format!("Failed to set X11 clipboard: {}", e))?;
    Ok(())
}

/// Get X11 clipboard - reads from system
pub fn get_x11_clipboard_bytes() -> Result<Vec<u8>, String> {
    let clipboard = X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;
    
    match clipboard.load(
        clipboard.getter.atoms.clipboard,
        clipboard.getter.atoms.incr,
        clipboard.getter.atoms.property,
        std::time::Duration::from_secs(3),
    ) {
        Ok(contents) => {
            *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(contents.clone());
            Ok(contents)
        },
        Err(_) => {
            if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
                Ok(data.clone())
            } else {
                Ok(vec![])
            }
        }
    }
}

/// Internal: set clipboard with optional hash update
fn set_clipboard_inner(data: &[u8], update_last_written: bool) -> Result<(), String> {
    let content_type = detect_content_type(data);

    if update_last_written && content_type == "text" {
        let normalized = normalize_clipboard_bytes(data);
        let mut hasher = DefaultHasher::new();
        normalized.hash(&mut hasher);
        *LAST_WRITTEN_CLIPBOARD.lock().unwrap() = Some(hasher.finish());
    }

    match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => set_wayland_clipboard_bytes(data),
        DesktopEnv::X11 => set_x11_clipboard(data),
        DesktopEnv::Unknown => set_wayland_clipboard_bytes(data).or_else(|_| set_x11_clipboard(data)),
    }
}

/// Set clipboard and update hash (normal use)
pub fn set_clipboard(data: &[u8]) -> Result<(), String> {
    set_clipboard_inner(data, true)
}

/// Set clipboard WITHOUT updating hash (used by watcher keep-alive)
pub fn set_clipboard_no_hash(data: &[u8]) -> Result<(), String> {
    set_clipboard_inner(data, false)
}

/// Get clipboard based on current environment
pub fn get_clipboard() -> Result<Vec<u8>, String> {
    let bytes = match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => get_wayland_clipboard_bytes(),
        DesktopEnv::X11 => get_x11_clipboard_bytes(),
        DesktopEnv::Unknown => get_wayland_clipboard_bytes().or_else(|_| get_x11_clipboard_bytes()),
    }?;

    if bytes.is_empty() {
        if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
            return Ok(data.clone());
        }
    }

    if should_ignore_bytes(&bytes) {
        if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
            if !should_ignore_bytes(data) {
                return Ok(data.clone());
            }
        }
        return Ok(vec![]);
    }

    let content_type = detect_content_type(&bytes);

    if content_type.starts_with("image/") {
        return Ok(bytes);
    }

    if content_type == "text" {
        if let Ok(_) = String::from_utf8(bytes.clone()) {
            return Ok(normalize_clipboard_bytes(&bytes));
        } else {
            return Ok(vec![]);
        }
    }

    Ok(bytes)
}

/// Get clipboard for frontend - ALWAYS returns from persistent memory
pub fn get_clipboard_for_paste() -> Result<Vec<u8>, String> {
    if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
        if should_ignore_bytes(data) {
            return Ok(vec![]);
        }

        let content_type = detect_content_type(data);

        if content_type.starts_with("image/") {
            return Ok(data.clone());
        }

        if content_type == "text" {
            if let Ok(_) = String::from_utf8(data.clone()) {
                return Ok(normalize_clipboard_bytes(data));
            } else {
                return Ok(vec![]);
            }
        }

        Ok(data.clone())
    } else {
        Ok(vec![])
    }
}

/// Get the most recent clipboard item from persistent memory
pub fn get_persistent_clipboard() -> Option<Vec<u8>> {
    PERSISTENT_CLIPBOARD_DATA.lock().unwrap().clone()
}

/// Store clipboard data in persistent memory without setting system clipboard
pub fn cache_clipboard_data(data: &[u8]) {
    if !data.is_empty() && !should_ignore_bytes(data) {
        *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(data.to_vec());
    }
}

