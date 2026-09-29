//! User-chosen app icon (fork addition).
//!
//! A picked JPEG/PNG/WebP is cropped to a centered square and stored as
//! `{data}/icon/app-icon.png` plus a multi-size `app-icon.ico`. It replaces the
//! icon of every window (and therefore the taskbar button), and — in the
//! everyday build only — the Start Menu shortcut's icon. The original file is
//! never modified; "restore default" goes back to the icon compiled into the exe.

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, RgbaImage};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::{AppHandle, Manager, Runtime};

use crate::db::DbState;
use crate::settings;

const MAX_BYTES: u64 = 20 * 1024 * 1024;
const ICON_SIZE: u32 = 256;
const ICO_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// The custom icon's pixels, applied to windows as they are created.
#[derive(Default)]
pub struct AppIconState(pub Mutex<Option<RgbaImage>>);

fn icon_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("icon")
}

/// Centered square crop, resized to `size`.
fn square(img: &DynamicImage, size: u32) -> RgbaImage {
    let side = img.width().min(img.height());
    let x = (img.width() - side) / 2;
    let y = (img.height() - side) / 2;
    img.crop_imm(x, y, side, side).resize_exact(size, size, FilterType::Lanczos3).to_rgba8()
}

/// Writes the PNG and ICO versions; returns the square RGBA image.
fn write_icon_files(img: &DynamicImage, dir: &Path) -> Result<RgbaImage, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let rgba = square(img, ICON_SIZE);
    rgba.save(dir.join("app-icon.png")).map_err(|e| e.to_string())?;

    let frames = ICO_SIZES
        .iter()
        .map(|&size| {
            let frame = if size == ICON_SIZE { rgba.clone() } else { square(img, size) };
            IcoFrame::as_png(frame.as_raw(), size, size, ExtendedColorType::Rgba8).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let file = std::fs::File::create(dir.join("app-icon.ico")).map_err(|e| e.to_string())?;
    IcoEncoder::new(file).encode_images(&frames).map_err(|e| e.to_string())?;
    Ok(rgba)
}

fn tauri_image(rgba: &RgbaImage) -> Image<'static> {
    Image::new_owned(rgba.as_raw().clone(), rgba.width(), rgba.height())
}

fn apply_to_windows<R: Runtime>(app: &AppHandle<R>, icon: Option<&RgbaImage>) {
    for window in app.webview_windows().values() {
        let result = match icon {
            Some(rgba) => window.set_icon(tauri_image(rgba)),
            None => match app.default_window_icon() {
                Some(default) => window.set_icon(default.clone()),
                None => Ok(()),
            },
        };
        if let Err(e) = result {
            log::warn!("[icon] could not set window icon: {e}");
        }
    }
}

/// Points the everyday build's Start Menu shortcut at `icon` (`None` = the exe's own icon).
#[cfg(windows)]
fn update_shortcut<R: Runtime>(app: &AppHandle<R>, icon: Option<&Path>) {
    use std::os::windows::process::CommandExt;
    if crate::channel::is_dev(app) {
        return; // Never touch the everyday shortcut from a dev build.
    }
    let Ok(appdata) = std::env::var("APPDATA") else { return };
    let lnk = Path::new(&appdata).join(r"Microsoft\Windows\Start Menu\Programs\PomoPipen.lnk");
    if !lnk.exists() {
        return;
    }
    let location = match icon {
        Some(path) => format!("{},0", path.display()),
        None => match std::env::current_exe() {
            Ok(exe) => format!("{},0", exe.display()),
            Err(_) => return,
        },
    };
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let status = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:POMO_LNK); $s.IconLocation=$env:POMO_ICON; $s.Save()",
        ])
        .env("POMO_LNK", &lnk)
        .env("POMO_ICON", &location)
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    match status {
        Ok(s) if s.success() => {
            // Ask Explorer to refresh its icon cache so Start shows the change.
            let _ = std::process::Command::new("ie4uinit.exe")
                .arg("-show")
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
        other => log::warn!("[icon] shortcut update failed: {other:?}"),
    }
}

#[cfg(not(windows))]
fn update_shortcut<R: Runtime>(_app: &AppHandle<R>, _icon: Option<&Path>) {}

/// Loads the saved custom icon at startup (if any) so windows can use it.
pub fn load_saved(data_dir: &Path, saved_path: &str) -> Option<RgbaImage> {
    if saved_path.is_empty() {
        return None;
    }
    let png = icon_dir(data_dir).join("app-icon.png");
    image::open(png).ok().map(|img| img.to_rgba8())
}

/// The custom icon, if one is set — for windows created after startup.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> Option<Image<'static>> {
    let state = app.try_state::<AppIconState>()?;
    let guard = state.0.lock().ok()?;
    guard.as_ref().map(tauri_image)
}

#[tauri::command]
pub fn app_icon_set(app: AppHandle, source_path: String, db: tauri::State<'_, DbState>) -> Result<String, String> {
    let file = std::fs::File::open(&source_path).map_err(|e| format!("Cannot open image: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Image is larger than 20 MB".into());
    }
    let img = image::load_from_memory(&bytes).map_err(|_| "Choose a JPEG, PNG, or WebP image".to_string())?;
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let dir = icon_dir(&data_dir);
    let rgba = write_icon_files(&img, &dir)?;
    let png = dir.join("app-icon.png").to_string_lossy().into_owned();

    apply_to_windows(&app, Some(&rgba));
    update_shortcut(&app, Some(&dir.join("app-icon.ico")));
    *app.state::<AppIconState>().0.lock().unwrap() = Some(rgba);
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        settings::save_setting(&conn, "app_icon", &png).map_err(|e| e.to_string())?;
    }
    Ok(png)
}

/// Puts the built-in icon back on every window and the shortcut (also used by
/// "reset all settings").
pub fn clear<R: Runtime>(app: &AppHandle<R>) {
    let had_custom = app
        .try_state::<AppIconState>()
        .map(|state| state.0.lock().map(|mut guard| guard.take().is_some()).unwrap_or(false))
        .unwrap_or(false);
    if had_custom {
        apply_to_windows(app, None);
        update_shortcut(app, None);
    }
}

#[tauri::command]
pub fn app_icon_reset(app: AppHandle, db: tauri::State<'_, DbState>) -> Result<(), String> {
    clear(&app);
    let conn = db.lock().map_err(|e| e.to_string())?;
    settings::save_setting(&conn, "app_icon", "").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_square_png_and_multi_size_ico() {
        let data_dir = std::env::temp_dir().join(format!("pomopipen-icon-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data_dir);
        // A wide 300×120 picture: the crop must be the centered 120×120 square.
        let mut img = RgbaImage::new(300, 120);
        for (x, _, px) in img.enumerate_pixels_mut() {
            *px = if (90..210).contains(&x) { image::Rgba([200, 30, 40, 255]) } else { image::Rgba([0, 0, 255, 255]) };
        }
        let rgba = write_icon_files(&DynamicImage::ImageRgba8(img), &icon_dir(&data_dir)).unwrap();
        assert_eq!(rgba.dimensions(), (ICON_SIZE, ICON_SIZE));
        assert_eq!(rgba.get_pixel(0, 0).0, [200, 30, 40, 255], "blue sides must be cropped away");

        let ico = image::open(icon_dir(&data_dir).join("app-icon.ico")).unwrap();
        assert_eq!(ico.width(), 256);
        assert_eq!(load_saved(&data_dir, "set").map(|i| i.dimensions()), Some((256, 256)));
        assert!(load_saved(&data_dir, "").is_none(), "empty setting means the default icon");
        let _ = std::fs::remove_dir_all(&data_dir);
    }
}
