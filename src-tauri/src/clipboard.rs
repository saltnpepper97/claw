use crate::detect::DesktopEnv;
use std::io::Read;
use std::sync::Mutex;
use std::hash::{DefaultHasher, Hash, Hasher};
use once_cell::sync::Lazy;
use wl_clipboard_rs::copy::{MimeType, Options, Source};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType as PasteMimeType, Seat};
use x11_clipboard::Clipboard as X11Clipboard;
use crate::{LAST_WRITTEN_CLIPBOARD, detect_content_type};
use crate::normalize_clipboard_bytes;

// Keep a global Wayland clipboard owner alive
static WAYLAND_CLIPBOARD_OWNER: Lazy<Mutex<Options>> = Lazy::new(|| Mutex::new(Options::new()));

/// Set Wayland clipboard
pub fn set_wayland_clipboard_bytes(data: &[u8]) -> Result<(), String> {
    let content_type = detect_content_type(data);
    
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

    let opts = WAYLAND_CLIPBOARD_OWNER.lock().unwrap();
    opts.clone().copy(Source::Bytes(data.into()), mime_type)
        .map_err(|e| e.to_string())
}

/// Check if bytes should be ignored (0,0 or <meta> artifacts)
/// This now normalizes text payloads (removes NULs and trims ASCII whitespace)
/// so we catch variants like "0,0\n" or "0,0\0".
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
        .filter(|&b| b != 0) // remove NUL
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

/// Get Wayland clipboard with robust filtering: prefer images, but carefully handle text offers.
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
            let _ = pipe.read_to_end(&mut bytes);
            drop(pipe); // explicitly close the pipe immediately
            if !bytes.is_empty() {
                // Normalize small artifacts immediately
                if should_ignore_bytes(&bytes) {
                    continue;
                }

                // For text MIME, ensure we don't prematurely return tiny artifacts
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
                        return Ok(clean);
                    }
                    continue;
                }

                // For image MIME offers, double-check it's an image before selecting as candidate
                if detect_content_type(&bytes).starts_with("image/") {
                    // guard against tiny bogus image blobs
                    if bytes.len() < 100 {
                        continue;
                    }
                    candidate_image = Some(bytes);
                }
            }
        }
    }

    // Force file descriptors closed
    std::fs::File::open("/dev/null").ok();

    if let Some(img) = candidate_image {
        return Ok(img);
    }

    Ok(vec![])
}


/// Set X11 clipboard
pub fn set_x11_clipboard(data: &[u8]) -> Result<(), String> {
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

/// Get X11 clipboard
pub fn get_x11_clipboard_bytes() -> Result<Vec<u8>, String> {
    let clipboard =
        X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;
    let contents = clipboard
        .load(
            clipboard.getter.atoms.clipboard,
            clipboard.getter.atoms.incr,
            clipboard.getter.atoms.property,
            std::time::Duration::from_secs(3),
        )
        .map_err(|e| format!("Failed to get X11 clipboard: {}", e))?;
    Ok(contents)
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
    // First, attempt Wayland, then X11 if unknown or fails
    let bytes = match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => get_wayland_clipboard_bytes(),
        DesktopEnv::X11 => get_x11_clipboard_bytes(),
        DesktopEnv::Unknown => get_wayland_clipboard_bytes().or_else(|_| get_x11_clipboard_bytes()),
    }?;

    // Immediately discard junk
    if bytes.is_empty() || should_ignore_bytes(&bytes) {
        return Ok(vec![]);
    }

    let content_type = detect_content_type(&bytes);

    // Images take absolute priority
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


#[allow(dead_code)]
pub fn set_clipboard_text(text: &str) -> Result<(), String> {
    set_clipboard(text.as_bytes())
}

#[allow(dead_code)]
pub fn get_clipboard_text() -> Result<String, String> {
    let bytes = get_clipboard()?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

