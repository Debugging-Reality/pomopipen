//! Stable vs. development build channel (fork addition).
//!
//! `tauri.conf.json` carries the `com.journ.pomopipen.dev` identifier, so
//! `npm run tauri dev` and ordinary builds keep their database, backgrounds and
//! logs in a separate folder. Only `scripts/publish-stable.ps1` builds with
//! `tauri.stable.conf.json`, which restores `com.journ.pomopipen` — the folder
//! holding the user's real study records.

use tauri::{AppHandle, Runtime};

pub fn is_dev<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config().identifier.ends_with(".dev")
}

/// Window title / tray tooltip, so a dev instance is never mistaken for the
/// everyday one when both are open.
pub fn app_label<R: Runtime>(app: &AppHandle<R>) -> &'static str {
    if is_dev(app) {
        "PomoPipen Dev"
    } else {
        "PomoPipen"
    }
}
