//! Import user-selected images into app-owned storage for the asset protocol.
//! The original is never modified; the size and format are checked before writing.

use std::io::Read;
use std::path::Path;
use tauri::{AppHandle, Manager};

const MAX_BYTES: u64 = 20 * 1024 * 1024;

fn file_format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("png") }
    else if bytes.starts_with(b"\xff\xd8\xff") { Some("jpg") }
    else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" { Some("webp") }
    else { None }
}

#[tauri::command]
pub fn background_import(app: AppHandle, source_path: String, target: String) -> Result<String, String> {
    if target != "timer" && target != "calendar" { return Err("Unknown background target".into()); }
    let source = Path::new(&source_path);
    let file = std::fs::File::open(source).map_err(|e| format!("Cannot open image: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Please select a regular image file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Image is larger than 20 MB".into()); }
    let format = file_format(&bytes).ok_or("Choose a JPEG, PNG, or WebP image")?;
    let directory = app.path().app_data_dir().map_err(|e| e.to_string())?.join("backgrounds");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?.as_nanos();
    let destination = directory.join(format!("{target}-{stamp}.{format}"));
    std::fs::write(&destination, bytes).map_err(|e| e.to_string())?;
    Ok(destination.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_image_signatures_only() {
        assert_eq!(file_format(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(file_format(b"\xff\xd8\xffrest"), Some("jpg"));
        assert_eq!(file_format(b"RIFFaaaaWEBPrest"), Some("webp"));
        assert_eq!(file_format(b"not an image"), None);
    }
}
