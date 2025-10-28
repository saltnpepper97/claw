use tauri::{
    menu::{Menu, MenuItem, Submenu},
    AppHandle,
};
use crate::history::{self, ClipboardEntry};

pub const TRAY_ID: &str = "claw-tray";

pub fn human_size_from_bytes(size: usize) -> String {
    let kb = size as f64 / 1024.0;
    if kb < 1024.0 {
        format!("{:.0} KB", kb)
    } else {
        format!("{:.1} MB", kb / 1024.0)
    }
}

fn clipboard_entry_label_lightweight(entry: &ClipboardEntry) -> String {
    if entry.content_type.starts_with("image/") {
        image_menu_label_lightweight(entry)
    } else if entry.content_type == "text" {
        format!("📝 Text ({} bytes)", entry.content_size)
    } else {
        format!("📎 {} ({} bytes)", entry.content_type, entry.content_size)
    }
}

fn image_menu_label_lightweight(entry: &ClipboardEntry) -> String {
    if let Some(src) = &entry.source_path {
        if src.starts_with("file://") {
            let path = &src[7..];
            if let Some(fname) = std::path::Path::new(path).file_name() {
                return format!("🖼️ {} ({})", fname.to_string_lossy(), human_size_from_bytes(entry.content_size));
            }
        } else if let Ok(url) = url::Url::parse(src) {
            let host = url.host_str().unwrap_or("web");
            let filename = url
                .path_segments()
                .and_then(|s| s.last())
                .unwrap_or("image");
            return format!("🖼️ {} / {} ({})", host, filename, human_size_from_bytes(entry.content_size));
        }
    }

    format!("🖼️ Image ({})", human_size_from_bytes(entry.content_size))
}

pub fn update_tray_menu(
    app: &AppHandle,
    tray_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app.tray_by_id(tray_id).ok_or("Tray not found")?;

    let history = history::load_history(app, 100)?;
    let recent_items = history.get_entries(Some(5));

    let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;

    let menu = if !recent_items.is_empty() {
        let mut history_items = Vec::new();

        for (idx, entry) in recent_items.iter().enumerate() {
            let display_text = clipboard_entry_label_lightweight(&entry);
            
            let item_id = format!("history_{}", idx);
            let menu_item = MenuItem::with_id(app, &item_id, display_text, true, None::<&str>)?;
            history_items.push(menu_item);
        }

        let history_submenu = Submenu::with_items(
            app,
            "Recent Clipboard",
            true,
            &history_items
                .iter()
                .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
                .collect::<Vec<_>>(),
        )?;

        let clear_i = MenuItem::with_id(app, "clear_history", "Clear History", true, None::<&str>)?;
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

        Menu::with_items(app, &[&show_i, &history_submenu, &clear_i, &quit_i])?
    } else {
        let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        Menu::with_items(app, &[&show_i, &quit_i])?
    };

    tray.set_menu(Some(menu))?;
    
    drop(history);

    Ok(())
}
