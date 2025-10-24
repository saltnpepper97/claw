use crate::detect::DesktopEnv;
use std::io::Read;
use std::sync::Mutex;
use std::hash::{DefaultHasher, Hash, Hasher};
use once_cell::sync::Lazy;
use wl_clipboard_rs::copy::{MimeType, Source};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType as PasteMimeType, Seat};
use x11_clipboard::Clipboard as X11Clipboard;
use crate::{LAST_WRITTEN_CLIPBOARD, detect_content_type};
use crate::normalize_clipboard_bytes;

// CRITICAL: Keep the most recent clipboard data in memory
// This acts as a clipboard manager - even if the source app closes,
// we can still serve this content
static PERSISTENT_CLIPBOARD_DATA: Lazy<Mutex<Option<Vec<u8>>>> = Lazy::new(|| Mutex::new(None));

/// Set Wayland clipboard
pub fn set_wayland_clipboard_bytes(data: &[u8]) -> Result<(), String> {
    let content_type = detect_content_type(data);
    
    // ALWAYS store the most recent data in memory
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

    // Try to set system clipboard, but don't fail if it doesn't work
    // The important thing is we have it in memory
    wl_clipboard_rs::copy::Options::new()
        .copy(Source::Bytes(data.into()), mime_type)
        .map_err(|e| e.to_string())
}

/// Check if bytes should be ignored (0,0 or <meta> artifacts)
pub fn should_ignore_bytes(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }

    if bytes.len() <= 2 {
        return true;
    }

    if bytes == b"0,0" {
        return true;
    }

    if bytes.starts_with(b"file://") || bytes.starts_with(b"<meta") {
        return true;
    }

    let mut clean = bytes
        .iter()
        .cloned()
        .filter(|&b| b != 0)
        .collect::<Vec<u8>>();
    while !clean.is_empty() && (clean.first().unwrap().is_ascii_whitespace()) {
        clean.remove(0);
    }
    while !clean.is_empty() && (clean.last().unwrap().is_ascii_whitespace()) {
        clean.pop();
    }

    if clean.is_empty() {
        return true;
    }

    if clean == b"0,0".to_vec() {
        return true;
    }

    if detect_content_type(&clean).starts_with("image/") && clean.len() < 100 {
        return true;
    }

    false
}

/// Get Wayland clipboard with fallback to persistent memory
pub fn get_wayland_clipboard_bytes() -> Result<Vec<u8>, String> {
    // First try to get from actual clipboard
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
            let _ = pipe.read_to_end(&mut bytes);
            drop(pipe);
            if !bytes.is_empty() {
                if should_ignore_bytes(&bytes) {
                    continue;
                }

                if *mime == PasteMimeType::Text {
                    let mut clean = bytes.clone().into_iter().filter(|&b| b != 0).collect::<Vec<u8>>();
                    while !clean.is_empty() && clean.first().unwrap().is_ascii_whitespace() {
                        clean.remove(0);
                    }
                    while !clean.is_empty() && clean.last().unwrap().is_ascii_whitespace() {
                        clean.pop();
                    }
                    if clean.is_empty() {
                        continue;
                    }
                    if let Ok(_) = String::from_utf8(clean.clone()) {
                        // Store it in memory too
                        *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(clean.clone());
                        return Ok(clean);
                    }
                    continue;
                }

                if detect_content_type(&bytes).starts_with("image/") {
                    if bytes.len() < 100 {
                        continue;
                    }
                    candidate_image = Some(bytes);
                }
            }
        }
    }

    std::fs::File::open("/dev/null").ok();

    if let Some(img) = candidate_image {
        // Store it in memory too
        *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(img.clone());
        return Ok(img);
    }

    // FALLBACK: Return from memory if clipboard is empty
    if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
        return Ok(data.clone());
    }

    Ok(vec![])
}

/// Set X11 clipboard
pub fn set_x11_clipboard(data: &[u8]) -> Result<(), String> {
    // Store in memory first
    *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(data.to_vec());
    
    let clipboard =
        X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;
    clipboard
        .store(
            clipboard.setter.atoms.clipboard,
            clipboard.setter.atoms.incr,
            data,
        )
        .map_err(|e| format!("Failed to set X11 clipboard: {}", e))?;
    Ok(())
}

/// Get X11 clipboard with fallback
pub fn get_x11_clipboard_bytes() -> Result<Vec<u8>, String> {
    let clipboard =
        X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;
    
    match clipboard.load(
        clipboard.getter.atoms.clipboard,
        clipboard.getter.atoms.incr,
        clipboard.getter.atoms.property,
        std::time::Duration::from_secs(3),
    ) {
        Ok(contents) => {
            // Store it in memory too
            *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(contents.clone());
            Ok(contents)
        },
        Err(_) => {
            // Fallback to persistent memory if clipboard load fails
            if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
                Ok(data.clone())
            } else {
                Ok(vec![])
            }
        }
    }
}

/// Set clipboard based on current environment
pub fn set_clipboard(data: &[u8]) -> Result<(), String> {
    let content_type = detect_content_type(data);

    // Track last hash only for text
    if content_type == "text" {
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

/// Get clipboard based on current environment
pub fn get_clipboard() -> Result<Vec<u8>, String> {
    let bytes = match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => get_wayland_clipboard_bytes(),
        DesktopEnv::X11 => get_x11_clipboard_bytes(),
        DesktopEnv::Unknown => get_wayland_clipboard_bytes().or_else(|_| get_x11_clipboard_bytes()),
    }?;

    // If we got empty bytes from system clipboard, check persistent memory
    if bytes.is_empty() {
        if let Some(data) = PERSISTENT_CLIPBOARD_DATA.lock().unwrap().as_ref() {
            return Ok(data.clone());
        }
    }

    if should_ignore_bytes(&bytes) {
        // Even if bytes are garbage, check if we have good data in memory
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
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            return Ok(normalize_clipboard_bytes(text.as_bytes()));
        } else {
            return Ok(vec![]);
        }
    }

    Ok(bytes)
}

/// Get the most recent clipboard item from persistent memory
/// This is useful when the clipboard is empty but we have data stored
#[allow(dead_code)]
pub fn get_persistent_clipboard() -> Option<Vec<u8>> {
    PERSISTENT_CLIPBOARD_DATA.lock().unwrap().clone()
}

/// Store clipboard data in persistent memory without setting system clipboard
/// Used by the watcher to cache detected clipboard content
pub fn cache_clipboard_data(data: &[u8]) {
    if !data.is_empty() && !should_ignore_bytes(data) {
        *PERSISTENT_CLIPBOARD_DATA.lock().unwrap() = Some(data.to_vec());
    }
}

#[allow(dead_code)]
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    set_clipboard(text.as_bytes())
}

#[allow(dead_code)]
pub fn get_clipboard_text() -> Result<String, String> {
    let bytes = get_clipboard()?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}
