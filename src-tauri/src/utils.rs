// Author: Dustin Pilgrim
// License: MIT

pub fn detect_content_type(bytes: &[u8]) -> String {
    if bytes.len() < 4 {
        return "text".to_string();
    }

    // ---- Common image signatures ----
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

    // ---- Text-ish clipboard formats (file copies) ----
    // These are *UTF-8* payloads but semantically not "plain text":
    // - text/uri-list: lines of file:// URIs (and comments starting with #)
    // - x-special/gnome-copied-files: first line "copy" or "cut", then URIs
    if let Ok(s) = std::str::from_utf8(bytes) {
        // GNOME: "copy\nfile://...\nfile://...\n"
        if (s.starts_with("copy\n") || s.starts_with("cut\n")) && s.contains("file://") {
            return "x-special/gnome-copied-files".to_string();
        }

        // uri-list: may contain "# comment" lines; usually contains file:// lines
        // Heuristic: any line starting with file:// (after optional whitespace)
        if s.lines()
            .map(|l| l.trim_start())
            .any(|l| l.starts_with("file://"))
        {
            return "text/uri-list".to_string();
        }

        return "text".to_string();
    }

    "binary".to_string()
}

pub fn normalize_clipboard_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }

    // Always remove trailing NUL bytes. Some clipboard providers append them.
    let mut trimmed = bytes.to_vec();
    while trimmed.last() == Some(&0x00) {
        trimmed.pop();
    }

    // Some environments leave a literal trailing "0,0" marker (you already guard against it).
    // Keep your behavior, but only if it is *exactly* at the end.
    while trimmed.ends_with(b"0,0") {
        trimmed.truncate(trimmed.len().saturating_sub(3));
        while trimmed.last() == Some(&0x00) {
            trimmed.pop();
        }
    }

    // IMPORTANT: do NOT do “smart trimming” here (spaces/newlines),
    // because uri-lists and gnome-copied-files are line-based formats.
    trimmed
}
