pub mod defaults;

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All user-configurable settings, fully typed.
///
/// Time fields are in **seconds** (converted from stored minutes).
/// `volume` is in the **0.0–1.0** range (converted from stored 0–100).
///
/// This struct is serialized to JSON and sent to the Svelte frontend via Tauri IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub always_on_top: bool,
    pub break_always_on_top: bool,
    pub auto_start_work: bool,
    pub auto_start_break: bool,
    pub tray_icon_enabled: bool,
    pub min_to_tray: bool,
    pub min_to_tray_on_close: bool,
    pub notifications_enabled: bool,
    /// Number of work rounds before a long break.
    pub long_break_interval: u32,
    pub short_breaks_enabled: bool,
    pub long_breaks_enabled: bool,
    /// When true the dial arc starts full and subtracts; when false it fills from empty.
    pub dial_countdown: bool,
    pub theme_mode: String,
    pub theme_light: String,
    pub theme_dark: String,
    /// Per-theme background images, as JSON owned by the frontend:
    /// `{ "<theme name>": { "timer": {path, opacity, fit, scale}, "calendar": {...} } }`.
    pub theme_backgrounds: String,
    /// Statistics window zoom, in percent.
    pub stats_zoom: u32,
    /// Path of the user-chosen app icon PNG; empty = the built-in icon.
    pub app_icon: String,
    /// Classic Tomato timer look: "mechanical" (tomato kitchen timer) or "ring"
    /// (real tomato inside a countdown ring). Other themes ignore it.
    pub timer_appearance: String,
    /// Opacity (0–100) of Classic Tomato's built-in basket frame around the week calendar.
    pub basket_frame_opacity: u32,
    /// Strength (0–100) of the basket's floor grid inside the calendar.
    pub basket_floor_opacity: u32,
    /// Classic Tomato page backgrounds: "vine" (tomatoes on the vine), "varieties"
    /// (a variety chart) or "none" (plain paper).
    pub classic_pattern: String,
    /// Weeks in the stats window (calendar, "this week", heatmap rows) start on
    /// Monday instead of Sunday.
    pub week_starts_monday: bool,
    pub tick_sounds_during_work: bool,
    /// A short sound whenever a red primary button is pressed.
    pub click_sound_enabled: bool,
    pub tick_sounds_during_break: bool,
    /// Work round duration in seconds.
    pub time_work_secs: u32,
    /// Short break duration in seconds.
    pub time_short_break_secs: u32,
    /// Long break duration in seconds.
    pub time_long_break_secs: u32,
    /// Audio volume in the 0.0–1.0 range.
    pub volume: f32,
    pub shortcut_toggle: String,
    pub shortcut_reset: String,
    pub shortcut_skip: String,
    pub shortcut_restart: String,
    /// Global shortcut that brings the timer forward with the jot pad open.
    pub shortcut_jot: String,
    pub websocket_enabled: bool,
    pub websocket_port: u16,
    pub language: String,
    pub verbose_logging: bool,
    pub check_for_updates: bool,
    pub global_shortcuts_enabled: bool,
    /// Local shortcut key bindings (KeyboardEvent.key strings, frontend-only).
    pub local_shortcut_toggle: String,
    pub local_shortcut_reset: String,
    pub local_shortcut_skip: String,
    pub local_shortcut_volume_down: String,
    pub local_shortcut_volume_up: String,
    pub local_shortcut_mute: String,
    pub local_shortcut_fullscreen: String,
    /// Opens the jot pad (碎碎念) in the timer window.
    pub local_shortcut_jot: String,
    /// Last known window X coordinate (physical pixels). `None` = use OS default.
    pub window_x: Option<i32>,
    /// Last known window Y coordinate (physical pixels). `None` = use OS default.
    pub window_y: Option<i32>,
    /// Last known window width (physical pixels). `None` = use OS default.
    pub window_width: Option<u32>,
    /// Last known window height (physical pixels). `None` = use OS default.
    pub window_height: Option<u32>,
    /// Subject that new work rounds are attributed to. `None` = uncategorised.
    ///
    /// Kept in settings rather than in a separate state object so the timer
    /// thread, which already holds the settings mutex, can read it when a
    /// round starts — and so the choice survives a restart.
    pub active_subject_id: Option<i64>,
    /// Task that new work rounds are attributed to. `None` = no specific task.
    /// Always kept consistent with `active_subject_id`: setting a task pins
    /// its subject too; setting a subject directly clears this.
    pub active_task_id: Option<i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            break_always_on_top: false,
            auto_start_work: true,
            auto_start_break: true,
            tray_icon_enabled: false,
            min_to_tray: false,
            min_to_tray_on_close: false,
            notifications_enabled: false,
            long_break_interval: 4,
            short_breaks_enabled: true,
            long_breaks_enabled: true,
            dial_countdown: true,
            theme_mode: "auto".to_string(),
            theme_light: "Classic Tomato".to_string(),
            theme_dark: "Classic Tomato".to_string(),
            theme_backgrounds: "{}".to_string(),
            stats_zoom: 100,
            app_icon: String::new(),
            timer_appearance: "mechanical".to_string(),
            basket_frame_opacity: 100,
            basket_floor_opacity: 40,
            classic_pattern: "vine".to_string(),
            week_starts_monday: false,
            tick_sounds_during_work: false,
            click_sound_enabled: true,
            tick_sounds_during_break: false,
            time_work_secs: 25 * 60,
            time_short_break_secs: 5 * 60,
            time_long_break_secs: 15 * 60,
            volume: 1.0,
            #[cfg(target_os = "macos")]
            shortcut_toggle: "Super+Shift+1".to_string(),
            #[cfg(not(target_os = "macos"))]
            shortcut_toggle: "Control+F1".to_string(),
            #[cfg(target_os = "macos")]
            shortcut_reset: "Super+Shift+2".to_string(),
            #[cfg(not(target_os = "macos"))]
            shortcut_reset: "Control+F2".to_string(),
            #[cfg(target_os = "macos")]
            shortcut_skip: "Super+Shift+3".to_string(),
            #[cfg(not(target_os = "macos"))]
            shortcut_skip: "Control+F3".to_string(),
            #[cfg(target_os = "macos")]
            shortcut_restart: "Super+Shift+4".to_string(),
            #[cfg(not(target_os = "macos"))]
            shortcut_restart: "Control+F4".to_string(),
            #[cfg(target_os = "macos")]
            shortcut_jot: "Super+Alt+N".to_string(),
            #[cfg(not(target_os = "macos"))]
            shortcut_jot: "Control+Alt+N".to_string(),
            websocket_enabled: false,
            websocket_port: 1314,
            language: "en".to_string(),
            verbose_logging: false,
            check_for_updates: true,
            global_shortcuts_enabled: false,
            local_shortcut_toggle: " ".to_string(),
            local_shortcut_reset: "ArrowLeft".to_string(),
            local_shortcut_skip: "ArrowRight".to_string(),
            local_shortcut_volume_down: "ArrowDown".to_string(),
            local_shortcut_volume_up: "ArrowUp".to_string(),
            local_shortcut_mute: "m".to_string(),
            local_shortcut_fullscreen: "F11".to_string(),
            local_shortcut_jot: "n".to_string(),
            window_x: None,
            window_y: None,
            window_width: None,
            window_height: None,
            active_subject_id: None,
            active_task_id: None,
        }
    }
}

/// Seed the `settings` table with default values for any missing keys.
/// Uses `INSERT OR IGNORE` so existing customizations are preserved.
///
/// Shortcut defaults are platform-specific and seeded before the common
/// defaults so that INSERT OR IGNORE lets them win on first launch.
pub fn seed_defaults(conn: &Connection) -> Result<()> {
    // Platform-specific shortcut defaults (seeded first so they win).
    #[cfg(target_os = "macos")]
    let shortcut_defaults: &[(&str, &str)] = &[
        ("shortcut_toggle",  "Super+Shift+1"),
        ("shortcut_reset",   "Super+Shift+2"),
        ("shortcut_skip",    "Super+Shift+3"),
        ("shortcut_restart", "Super+Shift+4"),
        ("shortcut_jot",     "Super+Alt+N"),
    ];
    #[cfg(not(target_os = "macos"))]
    let shortcut_defaults: &[(&str, &str)] = &[
        ("shortcut_toggle",  "Control+F1"),
        ("shortcut_reset",   "Control+F2"),
        ("shortcut_skip",    "Control+F3"),
        ("shortcut_restart", "Control+F4"),
        ("shortcut_jot",     "Control+Alt+N"),
    ];

    for (key, value) in shortcut_defaults {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }

    // Must run before DEFAULTS seeds an empty `theme_backgrounds`.
    migrate_legacy_backgrounds(conn)?;

    for (key, value) in defaults::DEFAULTS {
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
    }
    // One-time visual migration: the rejected first collection and old app
    // defaults open as Cherry Soda. After this marker is stored, later manual
    // theme choices are never rewritten on startup.
    if get_setting(conn, "design_v2_migrated").is_none() {
        conn.execute(
            "UPDATE settings SET value = 'Cherry Soda'
             WHERE key IN ('theme_light', 'theme_dark')
             AND value IN ('Pomotroid', 'Pomotroid Light',
               'Rosewood Garden', 'Lagoon Citrus', 'Cherry Blossom Blue',
               'Tropical Guava', 'Lavender Dusk')",
            [],
        )?;
        save_setting(conn, "design_v2_migrated", "true")?;
    }
    log::debug!("[settings] defaults seeded");
    Ok(())
}

/// One-time move from the single global background of the first design pass
/// to per-theme backgrounds: the existing images go to the theme in use when
/// upgrading. The legacy rows are left in place untouched.
fn migrate_legacy_backgrounds(conn: &Connection) -> Result<()> {
    if get_setting(conn, "theme_backgrounds").is_some() {
        return Ok(());
    }
    let theme = match get_setting(conn, "theme_mode").as_deref() {
        Some("dark") => get_setting(conn, "theme_dark"),
        _ => get_setting(conn, "theme_light"),
    }
    .unwrap_or_else(|| Settings::default().theme_light);

    let mut entry = serde_json::Map::new();
    for (target, path_key, opacity_key, default_opacity) in [
        ("timer", "timer_background_path", "background_strength", 60),
        ("calendar", "calendar_background_path", "calendar_background_strength", 100),
    ] {
        let path = get_setting(conn, path_key).unwrap_or_default();
        if path.is_empty() {
            continue;
        }
        let opacity = get_setting(conn, opacity_key)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(default_opacity)
            .min(100);
        entry.insert(
            target.to_string(),
            serde_json::json!({ "path": path, "opacity": opacity, "fit": "tile", "scale": 100 }),
        );
    }
    let mut all = serde_json::Map::new();
    if !entry.is_empty() {
        all.insert(theme, serde_json::Value::Object(entry));
    }
    save_setting(conn, "theme_backgrounds", &serde_json::Value::Object(all).to_string())
}

/// Load all settings from the database. Falls back to `Settings::default()`
/// values for any key that is missing or cannot be parsed.
pub fn load(conn: &Connection) -> Result<Settings> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let map: HashMap<String, String> = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    log::debug!("[settings] loaded {} keys from db", map.len());
    let d = Settings::default();
    Ok(Settings {
        always_on_top: parse_bool(&map, "always_on_top", d.always_on_top),
        break_always_on_top: parse_bool(&map, "break_always_on_top", d.break_always_on_top),
        auto_start_work: parse_bool(&map, "auto_start_work", d.auto_start_work),
        auto_start_break: parse_bool(&map, "auto_start_break", d.auto_start_break),
        tray_icon_enabled: parse_bool(&map, "tray_icon_enabled", d.tray_icon_enabled),
        min_to_tray: parse_bool(&map, "min_to_tray", d.min_to_tray),
        min_to_tray_on_close: parse_bool(&map, "min_to_tray_on_close", d.min_to_tray_on_close),
        notifications_enabled: parse_bool(&map, "notifications", d.notifications_enabled),
        long_break_interval: parse_u32(&map, "work_rounds", d.long_break_interval),
        short_breaks_enabled: parse_bool(&map, "short_breaks_enabled", d.short_breaks_enabled),
        long_breaks_enabled: parse_bool(&map, "long_breaks_enabled", d.long_breaks_enabled),
        dial_countdown: parse_bool(&map, "dial_countdown", d.dial_countdown),
        theme_mode: map
            .get("theme_mode")
            .cloned()
            .unwrap_or(d.theme_mode),
        theme_light: map
            .get("theme_light")
            .cloned()
            .unwrap_or(d.theme_light),
        theme_dark: map
            .get("theme_dark")
            .cloned()
            .unwrap_or(d.theme_dark),
        theme_backgrounds: map.get("theme_backgrounds").cloned().unwrap_or(d.theme_backgrounds),
        stats_zoom: parse_u32(&map, "stats_zoom", d.stats_zoom).clamp(50, 200),
        app_icon: map.get("app_icon").cloned().unwrap_or_default(),
        timer_appearance: match map.get("timer_appearance").map(String::as_str) {
            Some(v @ ("mechanical" | "ring")) => v.to_string(),
            _ => d.timer_appearance,
        },
        basket_frame_opacity: parse_u32(&map, "basket_frame_opacity", d.basket_frame_opacity).min(100),
        basket_floor_opacity: parse_u32(&map, "basket_floor_opacity", d.basket_floor_opacity).min(100),
        classic_pattern: match map.get("classic_pattern").map(String::as_str) {
            Some(v @ ("vine" | "varieties" | "none")) => v.to_string(),
            _ => d.classic_pattern,
        },
        week_starts_monday: parse_bool(&map, "week_starts_monday", d.week_starts_monday),
        tick_sounds_during_work: parse_bool(&map, "tick_sounds_work", d.tick_sounds_during_work),
        click_sound_enabled: parse_bool(&map, "click_sound_enabled", d.click_sound_enabled),
        tick_sounds_during_break: parse_bool(
            &map,
            "tick_sounds_break",
            d.tick_sounds_during_break,
        ),
        // DB stores seconds directly (since MIGRATION_2).
        time_work_secs: parse_u32(&map, "time_work_secs", d.time_work_secs),
        time_short_break_secs: parse_u32(&map, "time_short_break_secs", d.time_short_break_secs),
        time_long_break_secs: parse_u32(&map, "time_long_break_secs", d.time_long_break_secs),
        // DB stores 0–100; convert to 0.0–1.0.
        volume: (parse_u32(&map, "volume", (d.volume * 100.0) as u32) as f32 / 100.0)
            .clamp(0.0, 1.0),
        shortcut_toggle: map
            .get("shortcut_toggle")
            .cloned()
            .unwrap_or(d.shortcut_toggle),
        shortcut_reset: map
            .get("shortcut_reset")
            .cloned()
            .unwrap_or(d.shortcut_reset),
        shortcut_skip: map
            .get("shortcut_skip")
            .cloned()
            .unwrap_or(d.shortcut_skip),
        shortcut_restart: map
            .get("shortcut_restart")
            .cloned()
            .unwrap_or(d.shortcut_restart),
        shortcut_jot: map.get("shortcut_jot").cloned().unwrap_or(d.shortcut_jot),
        websocket_enabled: parse_bool(&map, "websocket_enabled", d.websocket_enabled),
        websocket_port: parse_u32(&map, "websocket_port", d.websocket_port as u32) as u16,
        language: map.get("language").cloned().unwrap_or(d.language),
        verbose_logging: parse_bool(&map, "verbose_logging", d.verbose_logging),
        check_for_updates: parse_bool(&map, "check_for_updates", d.check_for_updates),
        global_shortcuts_enabled: parse_bool(&map, "global_shortcuts_enabled", d.global_shortcuts_enabled),
        local_shortcut_toggle: map.get("local_shortcut_toggle").cloned().unwrap_or(d.local_shortcut_toggle),
        local_shortcut_reset: map.get("local_shortcut_reset").cloned().unwrap_or(d.local_shortcut_reset),
        local_shortcut_skip: map.get("local_shortcut_skip").cloned().unwrap_or(d.local_shortcut_skip),
        local_shortcut_volume_down: map.get("local_shortcut_volume_down").cloned().unwrap_or(d.local_shortcut_volume_down),
        local_shortcut_volume_up: map.get("local_shortcut_volume_up").cloned().unwrap_or(d.local_shortcut_volume_up),
        local_shortcut_mute: map.get("local_shortcut_mute").cloned().unwrap_or(d.local_shortcut_mute),
        local_shortcut_fullscreen: map.get("local_shortcut_fullscreen").cloned().unwrap_or(d.local_shortcut_fullscreen),
        local_shortcut_jot: map.get("local_shortcut_jot").cloned().unwrap_or(d.local_shortcut_jot),
        window_x: parse_opt_i32(&map, "window_x"),
        window_y: parse_opt_i32(&map, "window_y"),
        window_width: parse_opt_u32(&map, "window_width"),
        window_height: parse_opt_u32(&map, "window_height"),
        active_subject_id: parse_opt_i64(&map, "active_subject_id"),
        active_task_id: parse_opt_i64(&map, "active_task_id"),
    })
}

/// Upsert a single setting by its DB key. The caller is responsible for
/// converting typed values back to their stored string representation.
pub fn save_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .ok()
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn parse_bool(map: &HashMap<String, String>, key: &str, default: bool) -> bool {
    map.get(key).map(|v| v == "true").unwrap_or(default)
}

fn parse_u32(map: &HashMap<String, String>, key: &str, default: u32) -> u32 {
    map.get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_opt_i32(map: &HashMap<String, String>, key: &str) -> Option<i32> {
    map.get(key)?.parse().ok()
}

fn parse_opt_u32(map: &HashMap<String, String>, key: &str) -> Option<u32> {
    map.get(key)?.parse().ok()
}

/// An empty string parses to `None`, which is how "no subject" is stored.
fn parse_opt_i64(map: &HashMap<String, String>, key: &str) -> Option<i64> {
    map.get(key)?.parse().ok()
}

// ---------------------------------------------------------------------------
// Tests (DATA-02 acceptance criteria)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn defaults_round_trip() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        let s = load(&conn).unwrap();

        assert_eq!(s.time_work_secs, 25 * 60);
        assert_eq!(s.time_short_break_secs, 5 * 60);
        assert_eq!(s.time_long_break_secs, 15 * 60);
        assert_eq!(s.long_break_interval, 4);
        assert!((s.volume - 1.0).abs() < f32::EPSILON);
        #[cfg(target_os = "macos")]
        {
            assert_eq!(s.shortcut_toggle, "Super+Shift+1");
            assert_eq!(s.shortcut_reset, "Super+Shift+2");
            assert_eq!(s.shortcut_skip, "Super+Shift+3");
            assert_eq!(s.shortcut_restart, "Super+Shift+4");
        }
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(s.shortcut_toggle, "Control+F1");
            assert_eq!(s.shortcut_reset, "Control+F2");
            assert_eq!(s.shortcut_skip, "Control+F3");
            assert_eq!(s.shortcut_restart, "Control+F4");
        }
        assert!(!s.always_on_top);
        assert!(!s.websocket_enabled);
        assert_eq!(s.websocket_port, 1314);
        assert_eq!(s.theme_mode, "auto");
        assert_eq!(s.theme_light, "Classic Tomato");
        assert_eq!(s.theme_dark, "Classic Tomato");
        assert_eq!(s.timer_appearance, "mechanical");
        assert_eq!(s.basket_frame_opacity, 100);
        assert_eq!(s.basket_floor_opacity, 40);
        assert_eq!(s.classic_pattern, "vine");
        assert_eq!(s.language, "en");
        assert!(!s.verbose_logging);
        assert_eq!(s.active_subject_id, None);
        assert_eq!(s.active_task_id, None);
    }

    #[test]
    fn active_subject_id_round_trips_and_clears() {
        let conn = setup();
        seed_defaults(&conn).unwrap();

        save_setting(&conn, "active_subject_id", "7").unwrap();
        assert_eq!(load(&conn).unwrap().active_subject_id, Some(7));

        // The frontend clears the selection by writing an empty string.
        save_setting(&conn, "active_subject_id", "").unwrap();
        assert_eq!(load(&conn).unwrap().active_subject_id, None);
    }

    #[test]
    fn seed_is_idempotent() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        // Second seed must not overwrite existing values.
        save_setting(&conn, "always_on_top", "true").unwrap();
        seed_defaults(&conn).unwrap();
        let s = load(&conn).unwrap();
        assert!(s.always_on_top, "seed_defaults must not overwrite saved value");
    }

    #[test]
    fn old_theme_preferences_migrate_once_and_later_choices_are_kept() {
        let conn = setup();
        save_setting(&conn, "theme_light", "Pomotroid Light").unwrap();
        save_setting(&conn, "theme_dark", "Lavender Dusk").unwrap();
        seed_defaults(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().theme_light, "Cherry Soda");
        assert_eq!(load(&conn).unwrap().theme_dark, "Cherry Soda");
        save_setting(&conn, "theme_light", "Pomotroid Light").unwrap();
        seed_defaults(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().theme_light, "Pomotroid Light");
    }

    #[test]
    fn legacy_backgrounds_move_to_the_active_theme_once() {
        let conn = setup();
        save_setting(&conn, "theme_mode", "light").unwrap();
        save_setting(&conn, "theme_light", "Citrus Club").unwrap();
        save_setting(&conn, "timer_background_path", r"C:\bg\timer-1.jpg").unwrap();
        save_setting(&conn, "background_strength", "35").unwrap();
        save_setting(&conn, "calendar_background_path", "").unwrap();
        seed_defaults(&conn).unwrap();

        let json: serde_json::Value =
            serde_json::from_str(&load(&conn).unwrap().theme_backgrounds).unwrap();
        assert_eq!(json["Citrus Club"]["timer"]["path"], r"C:\bg\timer-1.jpg");
        assert_eq!(json["Citrus Club"]["timer"]["opacity"], 35);
        assert_eq!(json["Citrus Club"]["timer"]["fit"], "tile");
        assert!(json["Citrus Club"].get("calendar").is_none());
        assert!(json.get("Cherry Soda").is_none());

        // Later edits by the frontend are never overwritten.
        save_setting(&conn, "theme_backgrounds", "{}").unwrap();
        seed_defaults(&conn).unwrap();
        assert_eq!(load(&conn).unwrap().theme_backgrounds, "{}");
    }

    #[test]
    fn fresh_install_has_no_backgrounds_and_default_zoom() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s.theme_backgrounds, "{}");
        assert_eq!(s.stats_zoom, 100);
        save_setting(&conn, "stats_zoom", "900").unwrap();
        assert_eq!(load(&conn).unwrap().stats_zoom, 200);
    }

    #[test]
    fn weeks_start_on_sunday_until_changed() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        assert!(!load(&conn).unwrap().week_starts_monday);
        save_setting(&conn, "week_starts_monday", "true").unwrap();
        assert!(load(&conn).unwrap().week_starts_monday);
    }

    #[test]
    fn timer_appearance_accepts_only_known_values() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "timer_appearance", "ring").unwrap();
        assert_eq!(load(&conn).unwrap().timer_appearance, "ring");
        save_setting(&conn, "timer_appearance", "spinning-wheel").unwrap();
        assert_eq!(load(&conn).unwrap().timer_appearance, "mechanical");
    }

    #[test]
    fn save_and_reload_bool() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "always_on_top", "true").unwrap();
        let s = load(&conn).unwrap();
        assert!(s.always_on_top);
    }

    #[test]
    fn save_and_reload_volume() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "volume", "50").unwrap();
        let s = load(&conn).unwrap();
        assert!((s.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn save_and_reload_time() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "time_work_secs", "1800").unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s.time_work_secs, 1800);
    }

    #[test]
    fn save_and_reload_sub_minute_time() {
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "time_work_secs", "339").unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s.time_work_secs, 339);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let conn = setup();
        // No seed — table is empty.
        let s = load(&conn).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn reset_defaults_restores_all_settings() {
        // Mutate several settings (timer-related and others).
        let conn = setup();
        seed_defaults(&conn).unwrap();
        save_setting(&conn, "time_work_secs", "2700").unwrap();
        save_setting(&conn, "time_short_break_secs", "600").unwrap();
        save_setting(&conn, "work_rounds", "8").unwrap();
        save_setting(&conn, "always_on_top", "true").unwrap();

        // Simulate the reset_defaults command: wipe all rows then re-seed.
        conn.execute("DELETE FROM settings", []).unwrap();
        seed_defaults(&conn).unwrap();

        let s = load(&conn).unwrap();
        // Timer settings must be restored to defaults.
        assert_eq!(s.time_work_secs, 25 * 60, "work duration must reset to 25 min");
        assert_eq!(s.time_short_break_secs, 5 * 60, "short break must reset to 5 min");
        assert_eq!(s.long_break_interval, 4, "work rounds must reset to 4");
        // Non-timer settings are also wiped and reseeded to their defaults.
        assert!(!s.always_on_top, "always_on_top must reset to default false");
    }

    #[test]
    fn migration_2_converts_mins_to_secs_and_removes_old_keys() {
        // Simulate a pre-migration DB: schema version 1, `*_mins` keys present.
        let conn = Connection::open_in_memory().unwrap();
        // Run only migration 1 manually to get v1 state. This mirrors the whole
        // of MIGRATION_1: a real v1 database has all four tables, and later
        // migrations are entitled to assume they exist (MIGRATION_7 alters
        // `sessions`).
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS sessions (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 started_at    INTEGER NOT NULL,
                 ended_at      INTEGER,
                 round_type    TEXT NOT NULL CHECK(round_type IN ('work', 'short-break', 'long-break')),
                 duration_secs INTEGER NOT NULL CHECK(duration_secs > 0),
                 completed     INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0, 1))
             );
             CREATE TABLE IF NOT EXISTS custom_themes (
                 id     INTEGER PRIMARY KEY AUTOINCREMENT,
                 name   TEXT NOT NULL UNIQUE,
                 colors TEXT NOT NULL
             );
             INSERT INTO schema_version VALUES (1);
             COMMIT;",
        )
        .unwrap();
        conn.execute("INSERT INTO settings (key, value) VALUES ('time_work_mins', '30')", []).unwrap();
        conn.execute("INSERT INTO settings (key, value) VALUES ('time_short_break_mins', '7')", []).unwrap();
        conn.execute("INSERT INTO settings (key, value) VALUES ('time_long_break_mins', '20')", []).unwrap();

        // Now run the full migration suite — only MIGRATION_2 should fire.
        crate::db::migrations::run(&conn).unwrap();

        // New keys must exist with correct second values.
        let work: String = conn.query_row("SELECT value FROM settings WHERE key = 'time_work_secs'", [], |r| r.get(0)).unwrap();
        assert_eq!(work, "1800");
        let short: String = conn.query_row("SELECT value FROM settings WHERE key = 'time_short_break_secs'", [], |r| r.get(0)).unwrap();
        assert_eq!(short, "420");
        let long: String = conn.query_row("SELECT value FROM settings WHERE key = 'time_long_break_secs'", [], |r| r.get(0)).unwrap();
        assert_eq!(long, "1200");

        // Old keys must be gone.
        let old_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM settings WHERE key LIKE '%_mins'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(old_count, 0, "old *_mins keys must be absent after MIGRATION_2");
    }

    #[test]
    fn boolean_settings_survive_multiple_writes() {
        // Writing the same boolean key repeatedly must not corrupt the value.
        let conn = setup();
        seed_defaults(&conn).unwrap();

        for _ in 0..5 {
            save_setting(&conn, "auto_start_work", "true").unwrap();
        }
        let s = load(&conn).unwrap();
        assert!(s.auto_start_work, "auto_start_work must remain true after repeated writes");

        for _ in 0..5 {
            save_setting(&conn, "auto_start_work", "false").unwrap();
        }
        let s = load(&conn).unwrap();
        assert!(!s.auto_start_work, "auto_start_work must be false after repeated false writes");
    }
}
