use crate::detect::DesktopEnv;
use std::io::Read;
use wl_clipboard_rs::copy::{MimeType as CopyMimeType, Options, Source};
use wl_clipboard_rs::paste::{get_contents, ClipboardType, Error, MimeType as PasteMimeType, Seat};
use x11_clipboard::Clipboard as X11Clipboard;

pub fn set_wayland_clipboard(text: &str) -> Result<(), String> {
    let opts = Options::new();
    opts.copy(
        Source::Bytes(text.to_string().into_bytes().into()),
        CopyMimeType::Autodetect,
    )
    .map_err(|e| e.to_string())
}

pub fn get_wayland_clipboard() -> Result<String, String> {
    match get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        PasteMimeType::Text,
    ) {
        Ok((mut pipe, _)) => {
            let mut contents = vec![];
            pipe.read_to_end(&mut contents).map_err(|e| e.to_string())?;
            Ok(String::from_utf8(contents).map_err(|e| e.to_string())?)
        }
        Err(Error::NoSeats) | Err(Error::ClipboardEmpty) | Err(Error::NoMimeType) => {
            Ok(String::new())
        }
        Err(err) => Err(err.to_string()),
    }
}

pub fn set_x11_clipboard(text: &str) -> Result<(), String> {
    let clipboard =
        X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;

    clipboard
        .store(
            clipboard.setter.atoms.clipboard,
            clipboard.setter.atoms.utf8_string,
            text.as_bytes(),
        )
        .map_err(|e| format!("Failed to set X11 clipboard: {}", e))?;

    Ok(())
}

pub fn get_x11_clipboard() -> Result<String, String> {
    let clipboard =
        X11Clipboard::new().map_err(|e| format!("Failed to create X11 clipboard: {}", e))?;

    let contents = clipboard
        .load(
            clipboard.getter.atoms.clipboard,
            clipboard.getter.atoms.utf8_string,
            clipboard.getter.atoms.property,
            std::time::Duration::from_secs(3),
        )
        .map_err(|e| format!("Failed to get X11 clipboard: {}", e))?;

    String::from_utf8(contents).map_err(|e| format!("Invalid UTF-8 in clipboard: {}", e))
}

// Cross-platform clipboard functions
pub fn set_clipboard(text: &str) -> Result<(), String> {
    match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => set_wayland_clipboard(text),
        DesktopEnv::X11 => set_x11_clipboard(text),
        DesktopEnv::Unknown => {
            // Try Wayland first, then X11
            if let Ok(()) = set_wayland_clipboard(text) {
                Ok(())
            } else {
                set_x11_clipboard(text)
            }
        }
    }
}

pub fn get_clipboard() -> Result<String, String> {
    match crate::detect::current_desktop_env() {
        DesktopEnv::Wayland => get_wayland_clipboard(),
        DesktopEnv::X11 => get_x11_clipboard(),
        DesktopEnv::Unknown => {
            // Try Wayland first, then X11
            if let Ok(content) = get_wayland_clipboard() {
                if !content.is_empty() {
                    Ok(content)
                } else {
                    get_x11_clipboard()
                }
            } else {
                get_x11_clipboard()
            }
        }
    }
}
