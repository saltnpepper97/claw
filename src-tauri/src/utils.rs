pub fn detect_content_type(bytes: &[u8]) -> String {
    if bytes.len() < 4 {
        return "text".to_string();
    }
    
    // Check for common image signatures
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return "image/png".to_string();
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg".to_string();
    }
    if bytes.starts_with(&[0x47, 0x49, 0x46]) {
        return "image/gif".to_string();
    }
    if bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return "image/webp".to_string();
    }
    if bytes.starts_with(b"BM") {
        return "image/bmp".to_string();
    }
    
    // Check if it's valid UTF-8 text
    if String::from_utf8(bytes.to_vec()).is_ok() {
        return "text".to_string();
    }
    
    "binary".to_string()
}

pub fn normalize_clipboard_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut trimmed = bytes.to_vec();

    // Remove trailing 0,0 bytes added by Wayland/X11 for images
    while trimmed.ends_with(&[0x30, 0x2C, 0x30]) || trimmed.ends_with(&[0x00]) {
        if trimmed.ends_with(&[0x30, 0x2C, 0x30]) {
            trimmed.truncate(trimmed.len() - 3);
        } else {
            trimmed.pop();
        }
    }

    trimmed
}

