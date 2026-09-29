/// All #[tauri::command] functions exposed to the Svelte frontend via Tauri IPC.
///
/// Commands are grouped by domain: Timer, Settings, Themes, Stats.
/// Each command returns `Result<T, String>` so errors surface cleanly in JS.
use log::LevelFilter;
use tauri::{AppHandle, Emitter, Manager, State};

use std::sync::Arc;

use crate::audio::{self, AudioManager};
use crate::notifications;
use crate::db::{calendar, queries, DbState};
use crate::db::calendar::FocusRecord;
use crate::db::queries::SubjectFilter;
use crate::jots::{self, Jot};
use crate::subjects::{self, Subject};
use crate::settings::{self, Settings};
use crate::shortcuts;
use crate::tasks::{self, Task};
use crate::themes::{self, Theme};
use crate::timer::{TimerController, TimerSnapshot};
use crate::tray::{self, TrayState};
use crate::websocket::{self, WsState};

// ---------------------------------------------------------------------------
// CMD-01 — Timer commands
// ---------------------------------------------------------------------------

/// Toggle the timer: start if idle, resume if paused, pause if running.
/// This is the primary action bound to the space bar and the play/pause button.
#[tauri::command]
pub fn timer_toggle(timer: State<'_, TimerController>) {
    timer.toggle();
}

/// Reset the current round's timer without advancing the sequence.
#[tauri::command]
pub fn timer_reset(timer: State<'_, TimerController>) {
    timer.reset();
}

/// Skip the current round: fires Complete immediately and advances to the next.
#[tauri::command]
pub fn timer_skip(timer: State<'_, TimerController>) {
    timer.skip();
}

/// Restart the current round from zero without advancing the sequence.
/// Round type and round number are preserved; only elapsed time is reset.
#[tauri::command]
pub fn timer_restart_round(timer: State<'_, TimerController>) {
    timer.restart_round();
}

/// Return a full snapshot of the current timer state.
/// Called once on frontend mount to hydrate stores.
#[tauri::command]
pub fn timer_get_state(timer: State<'_, TimerController>) -> TimerSnapshot {
    timer.get_snapshot()
}

// ---------------------------------------------------------------------------
// CMD-02 — Settings commands
// ---------------------------------------------------------------------------

/// Return all current settings.
#[tauri::command]
pub fn settings_get(db: State<'_, DbState>) -> Result<Settings, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    settings::load(&conn).map_err(|e| {
        log::error!("[settings] failed to load settings: {e}");
        e.to_string()
    })
}

/// Persist a single setting and emit `settings:changed` with the updated set.
///
/// `key` must be one of the DB column names (see `settings::defaults::DEFAULTS`).
/// `value` is always a string; the loader converts it to the appropriate type.
#[tauri::command]
pub fn settings_set(
    key: String,
    value: String,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    tray_state: State<'_, Arc<TrayState>>,
    ws_state: State<'_, Arc<WsState>>,
    app: AppHandle,
) -> Result<Settings, String> {
    log::debug!("[settings] set {key}={value}");
    let new_settings = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        settings::save_setting(&conn, &key, &value).map_err(|e| {
            log::error!("[settings] failed to save '{key}': {e}");
            e.to_string()
        })?;
        // When SIT is turned off, cascade-reset the dependent tray settings so
        // the close-to-tray handler cannot hide the window with no icon to
        // restore from.
        if key == "tray_icon_enabled" && value == "false" {
            settings::save_setting(&conn, "min_to_tray", "false").map_err(|e| e.to_string())?;
            settings::save_setting(&conn, "min_to_tray_on_close", "false").map_err(|e| e.to_string())?;
        }
        settings::load(&conn).map_err(|e| {
            log::error!("[settings] failed to reload after save: {e}");
            e.to_string()
        })?
    };

    // Apply verbose_logging change immediately without a restart.
    if key == "verbose_logging" {
        if new_settings.verbose_logging {
            log::set_max_level(LevelFilter::Debug);
            log::info!("Verbose logging enabled — log level set to DEBUG");
        } else {
            log::set_max_level(LevelFilter::Info);
            log::info!("Verbose logging disabled — log level set to INFO");
        }
    }

    // Keep the timer engine in sync when time-related settings change.
    timer.apply_settings(new_settings.clone());

    // Broadcast an updated snapshot so the frontend immediately reflects any
    // changed settings (round count, durations, etc.) regardless of timer
    // state.  The timer:reset handler only calls timerState.set(), so emitting
    // while running does not interrupt the countdown; the next timer:tick
    // event will reconcile total_secs from the engine within one second.
    app.emit("timer:reset", &timer.get_snapshot()).ok();

    // Propagate volume and tick-sound changes to the audio engine (optional state).
    if let Some(audio) = app.try_state::<Arc<AudioManager>>() {
        audio.apply_settings(&new_settings);
    }

    // Apply always-on-top window flag immediately when the setting changes,
    // accounting for the current round type so break_always_on_top takes
    // effect without waiting for the next round transition.
    if matches!(key.as_str(), "always_on_top" | "break_always_on_top") {
        if let Some(window) = app.get_webview_window("main") {
            let snap = timer.get_snapshot();
            let is_break = snap.round_type != "work";
            let effective_aot = new_settings.always_on_top
                && !(new_settings.break_always_on_top && is_break);
            let _ = window.set_always_on_top(effective_aot);
        }
    }

    // Sync tray countdown mode when the dial setting changes, then immediately
    // re-render the icon so it matches the dial without waiting for a timer event.
    if key == "dial_countdown" {
        *tray_state.countdown_mode.lock().unwrap() = new_settings.dial_countdown;
        let snap = timer.get_snapshot();
        let progress = if snap.total_secs > 0 {
            snap.elapsed_secs as f32 / snap.total_secs as f32
        } else {
            0.0
        };
        tray::update_icon(&tray_state, &snap.round_type, snap.is_paused, progress);
    }

    // Update tray icon colors when the active theme changes.
    if matches!(key.as_str(), "theme_mode" | "theme_light" | "theme_dark") {
        let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let tray_theme_name = match new_settings.theme_mode.as_str() {
            "dark" => &new_settings.theme_dark,
            _ => &new_settings.theme_light,
        };
        if let Some(theme) = themes::find(&data_dir, tray_theme_name) {
            *tray_state.colors.lock().unwrap() = tray::TrayColors::from_colors_map(&theme.colors);
            let snap = timer.get_snapshot();
            let progress = if snap.total_secs > 0 {
                snap.elapsed_secs as f32 / snap.total_secs as f32
            } else {
                0.0
            };
            tray::update_icon(&tray_state, &snap.round_type, snap.is_paused, progress);
        }
    }

    // Create or destroy the tray when tray_icon_enabled or min_to_tray changes.
    // The tray exists when either flag is true.
    // On Linux, spawn tray creation on a background thread to avoid blocking
    // the main thread on KDE Plasma 6 / Wayland (D-Bus StatusNotifier hang).
    if matches!(key.as_str(), "tray_icon_enabled" | "min_to_tray") {
        if new_settings.tray_icon_enabled || new_settings.min_to_tray {
            #[cfg(target_os = "linux")]
            {
                let app_handle = app.clone();
                let ts = Arc::clone(&tray_state);
                std::thread::spawn(move || {
                    tray::create_tray(&app_handle, &ts);
                });
            }
            #[cfg(not(target_os = "linux"))]
            tray::create_tray(&app, &tray_state);
        } else {
            tray::destroy_tray(&tray_state);
        }
    }

    // Re-register global shortcuts when any shortcut key changes or the enabled flag toggles.
    if matches!(key.as_str(), "shortcut_toggle" | "shortcut_reset" | "shortcut_skip" | "shortcut_restart" | "shortcut_jot" | "global_shortcuts_enabled") {
        shortcuts::register_all(&app, &new_settings);
    }

    // Start or stop the WebSocket server when the enabled flag or port changes.
    if matches!(key.as_str(), "websocket_enabled" | "websocket_port") {
        let ws = Arc::clone(&*ws_state);
        let port = new_settings.websocket_port;
        let enabled = new_settings.websocket_enabled;
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            // Always stop the old server first.
            websocket::stop(&ws).await;
            if enabled {
                websocket::start(port, app_clone, &ws).await;
            }
        });
    }

    app.emit("settings:changed", &new_settings).ok();
    Ok(new_settings)
}

// ---------------------------------------------------------------------------
// CMD-06 — Shortcuts command
// ---------------------------------------------------------------------------

/// Re-register all global shortcuts from the current settings.
/// The frontend can call this after bulk-updating shortcut settings.
#[tauri::command]
pub fn shortcuts_reload(db: State<'_, DbState>, app: AppHandle) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let s = settings::load(&conn).map_err(|e| e.to_string())?;
    shortcuts::register_all(&app, &s);
    Ok(())
}

/// Reset all settings to factory defaults and return the resulting settings.
#[tauri::command]
pub fn settings_reset_defaults(
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    tray_state: State<'_, Arc<TrayState>>,
    app: AppHandle,
) -> Result<Settings, String> {
    log::info!("[settings] reset to defaults");
    let new_settings = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        // Delete all rows so seed_defaults can insert fresh defaults.
        conn.execute("DELETE FROM settings", [])
            .map_err(|e| e.to_string())?;
        settings::seed_defaults(&conn).map_err(|e| e.to_string())?;
        settings::load(&conn).map_err(|e| e.to_string())?
    };

    timer.apply_settings(new_settings.clone());
    *tray_state.countdown_mode.lock().unwrap() = new_settings.dial_countdown;

    // Broadcast a reset snapshot so the frontend dial and display reflect the
    // restored default durations without requiring the user to manually reset.
    {
        let snap = timer.get_snapshot();
        if !snap.is_running && !snap.is_paused {
            app.emit("timer:reset", &snap).ok();
        }
    }

    // After reset, defaults have tray_icon_enabled=false and min_to_tray=false,
    // so destroy any active tray icon.
    tray::destroy_tray(&tray_state);

    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // The icon setting is gone now; show the built-in icon again.
    crate::app_icon::clear(&app);

    // Clear custom alert sounds: delete files from disk and reset in-memory paths.
    if let Some(audio_state) = app.try_state::<Arc<AudioManager>>() {
        let audio_dir = data_dir.join("audio");
        for stem in [audio::STEM_WORK, audio::STEM_SHORT, audio::STEM_LONG, audio::STEM_CLICK] {
            if let Ok(entries) = std::fs::read_dir(&audio_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
        audio_state.clear_custom_path("work_alert");
        audio_state.clear_custom_path("short_break_alert");
        audio_state.clear_custom_path("long_break_alert");
        audio_state.clear_custom_path("button_click");
        log::info!("[audio] custom sounds cleared on settings reset");
    }

    let tray_theme_name = match new_settings.theme_mode.as_str() {
        "dark" => &new_settings.theme_dark,
        _ => &new_settings.theme_light,
    };
    if let Some(theme) = themes::find(&data_dir, tray_theme_name) {
        *tray_state.colors.lock().unwrap() = tray::TrayColors::from_colors_map(&theme.colors);
    }
    shortcuts::register_all(&app, &new_settings);
    app.emit("settings:changed", &new_settings).ok();
    Ok(new_settings)
}

// ---------------------------------------------------------------------------
// CMD-03 — Theme commands
// ---------------------------------------------------------------------------

/// List all available themes (17 bundled + any user-created ones).
#[tauri::command]
pub fn themes_list(app: AppHandle) -> Result<Vec<Theme>, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(themes::list_all(&data_dir))
}

// ---------------------------------------------------------------------------
// CMD-04 — Sessions commands
// ---------------------------------------------------------------------------

/// Deletes all rows from the `sessions` table (irreversible bulk clear).
/// Emits `sessions:cleared` so any open stats window can refresh immediately.
#[tauri::command]
pub fn sessions_clear(db: State<'_, DbState>, app: AppHandle) -> Result<(), String> {
    log::info!("[sessions] clearing all session history");
    let conn = db.lock().map_err(|e| e.to_string())?;
    let n = conn.execute("DELETE FROM sessions", []).map_err(|e| {
        log::error!("[sessions] failed to clear history: {e}");
        e.to_string()
    })?;
    log::info!("[sessions] cleared {n} rows");
    app.emit("sessions:cleared", ()).ok();
    Ok(())
}

// Week calendar editing (fork addition): focus records added, moved, resized
// or deleted by hand. Each change emits `sessions:changed` (stats, charts,
// task totals and the timer's today count refresh) and schedules a Google
// Calendar sync.

#[tauri::command]
pub fn sessions_create(record: FocusRecord, db: State<'_, DbState>, app: AppHandle) -> Result<i64, String> {
    let id = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        calendar::create_record(&conn, &record, chrono::Utc::now().timestamp())?
    };
    log::info!("[sessions] added by hand: id={id} start={} {}s", record.started_at, record.duration_secs);
    sessions_changed(&app, record.started_at);
    Ok(id)
}

#[tauri::command]
pub fn sessions_update(id: i64, record: FocusRecord, db: State<'_, DbState>, app: AppHandle) -> Result<(), String> {
    let before = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        calendar::update_record(&conn, id, &record, chrono::Utc::now().timestamp())?
    };
    log::info!(
        "[sessions] edited by hand: id={id} start {}→{} {}s→{}s",
        before.started_at, record.started_at, before.duration_secs, record.duration_secs
    );
    sessions_changed(&app, before.started_at.min(record.started_at));
    Ok(())
}

/// Returns the deleted record, so the calendar can offer to put it back.
#[tauri::command]
pub fn sessions_delete(id: i64, db: State<'_, DbState>, app: AppHandle) -> Result<FocusRecord, String> {
    let removed = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        calendar::delete_record(&conn, id)?
    };
    log::info!("[sessions] deleted by hand: id={id} start={} {}s", removed.started_at, removed.duration_secs);
    sessions_changed(&app, removed.started_at);
    Ok(removed)
}

fn sessions_changed(app: &AppHandle, earliest_start: i64) {
    app.emit("sessions:changed", ()).ok();
    crate::gcal::sessions_edited(app, earliest_start);
}

// CMD-05 — Stats commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn stats_get_week_events(
    start: i64,
    end: i64,
    subject: Option<SubjectFilter>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::db::calendar::WeekEvent>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    crate::db::calendar::get_week_events(&conn, start, end, subject.unwrap_or_default())
}

#[tauri::command]
pub fn stats_get_range_events(
    start: i64,
    end: i64,
    subject: Option<SubjectFilter>,
    db: State<'_, DbState>,
) -> Result<Vec<crate::db::calendar::WeekEvent>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    crate::db::calendar::get_range_events(&conn, start, end, subject.unwrap_or_default())
}

/// Batched stats for Today + This Week tabs (minimises IPC round-trips).
#[tauri::command]
pub fn stats_get_detailed(
    subject: Option<SubjectFilter>,
    db: State<'_, DbState>,
) -> Result<DetailedStats, String> {
    let filter = subject.unwrap_or_default();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let today = queries::get_daily_stats(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query daily stats: {e}");
        e.to_string()
    })?;
    let week = queries::get_weekly_stats(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query weekly stats: {e}");
        e.to_string()
    })?;
    let streak = queries::get_streak(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query streak: {e}");
        e.to_string()
    })?;
    Ok(DetailedStats { today, week, streak })
}

/// Heatmap data + lifetime totals for the All Time tab.
#[tauri::command]
pub fn stats_get_heatmap(
    subject: Option<SubjectFilter>,
    db: State<'_, DbState>,
) -> Result<HeatmapStats, String> {
    let filter = subject.unwrap_or_default();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let entries = queries::get_heatmap_data(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query heatmap data: {e}");
        e.to_string()
    })?;
    let raw = queries::get_all_time_stats(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query all-time stats: {e}");
        e.to_string()
    })?;
    let streak = queries::get_streak(&conn, filter).map_err(|e| {
        log::error!("[stats] failed to query streak for heatmap: {e}");
        e.to_string()
    })?;
    Ok(HeatmapStats {
        entries,
        total_rounds: raw.completed_work_sessions as u32,
        total_hours: (raw.total_work_secs / 3600) as u32,
        longest_streak: streak.longest,
    })
}

// ---------------------------------------------------------------------------
// Subject commands
// ---------------------------------------------------------------------------

/// All subjects. Archived ones are excluded unless `include_archived` is true.
#[tauri::command]
pub fn subjects_list(
    include_archived: Option<bool>,
    db: State<'_, DbState>,
) -> Result<Vec<Subject>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    subjects::list(&conn, include_archived.unwrap_or(false)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn subjects_create(
    name: String,
    color: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Subject, String> {
    let subject = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        subjects::create(&conn, &name, &color).map_err(|e| e.to_string())?
    };
    app.emit("subjects:changed", ()).ok();
    Ok(subject)
}

/// Patch a subject. Omitted fields are left untouched.
#[tauri::command]
pub fn subjects_update(
    id: i64,
    name: Option<String>,
    color: Option<String>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Subject, String> {
    let subject = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        subjects::update(&conn, id, name.as_deref(), color.as_deref())
            .map_err(|e| e.to_string())?
    };
    app.emit("subjects:changed", ()).ok();
    Ok(subject)
}

#[tauri::command]
pub fn subjects_set_archived(
    id: i64,
    archived: bool,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Subject, String> {
    let subject = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        subjects::set_archived(&conn, id, archived).map_err(|e| e.to_string())?
    };
    app.emit("subjects:changed", ()).ok();
    Ok(subject)
}

/// Delete a subject. Sessions recorded against it are kept and become
/// uncategorised — the caller should make that clear in its confirmation.
#[tauri::command]
pub fn subjects_delete(
    id: i64,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<(), String> {
    let cleared_active = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        subjects::delete(&conn, id).map_err(|e| e.to_string())?;

        // If the deleted subject was the active one, drop the selection too,
        // otherwise new rounds would keep pointing at a row that is gone.
        // The active task clears with it — it necessarily belonged to this
        // subject (tasks_set_active pins the two together).
        let s = settings::load(&conn).map_err(|e| e.to_string())?;
        if s.active_subject_id == Some(id) {
            settings::save_setting(&conn, "active_subject_id", "")
                .map_err(|e| e.to_string())?;
            settings::save_setting(&conn, "active_task_id", "").map_err(|e| e.to_string())?;
            Some(settings::load(&conn).map_err(|e| e.to_string())?)
        } else {
            None
        }
    };
    if let Some(new_settings) = cleared_active {
        timer.apply_settings(new_settings.clone());
        app.emit("settings:changed", &new_settings).ok();
    }
    app.emit("subjects:changed", ()).ok();
    Ok(())
}

#[tauri::command]
pub fn subjects_reorder(
    ids: Vec<i64>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut conn = db.lock().map_err(|e| e.to_string())?;
        subjects::reorder(&mut conn, &ids).map_err(|e| e.to_string())?;
    }
    app.emit("subjects:changed", ()).ok();
    Ok(())
}

/// Select the subject new work rounds are attributed to (`None` clears it).
///
/// Always clears `active_task_id` too — this is the coarse picker (in the
/// timer window), and `active_task_id` must never point at a task from a
/// different subject. Picking a *specific* task (which pins its subject as a
/// side effect) goes through `tasks_set_active` instead.
///
/// A round already under way is re-tagged as well: the session row is written
/// on the first tick, so without this, fixing a wrong subject mid-round would
/// silently apply only from the next round.
#[tauri::command]
pub fn subjects_set_active(
    id: Option<i64>,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<Settings, String> {
    let new_settings = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        if let Some(id) = id {
            // Reject unknown ids rather than storing a dangling reference.
            subjects::get(&conn, id).map_err(|e| e.to_string())?;
        }
        let value = id.map(|v| v.to_string()).unwrap_or_default();
        settings::save_setting(&conn, "active_subject_id", &value)
            .map_err(|e| e.to_string())?;
        settings::save_setting(&conn, "active_task_id", "").map_err(|e| e.to_string())?;
        if let Err(e) = queries::retag_open_work_session(&conn, id, None) {
            log::warn!("[subjects] failed to re-tag the in-flight session: {e}");
        }
        settings::load(&conn).map_err(|e| e.to_string())?
    };
    log::info!("[subjects] active subject set to {id:?}");
    timer.apply_settings(new_settings.clone());
    app.emit("settings:changed", &new_settings).ok();
    Ok(new_settings)
}

/// Focus time per subject. `days` limits the window (`None` = all time).
#[tauri::command]
pub fn stats_get_subject_breakdown(
    days: Option<u32>,
    db: State<'_, DbState>,
) -> Result<Vec<queries::SubjectTotal>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    queries::get_subject_breakdown(&conn, days).map_err(|e| {
        log::error!("[stats] failed to query subject breakdown: {e}");
        e.to_string()
    })
}

// ---------------------------------------------------------------------------
// Task commands
// ---------------------------------------------------------------------------

/// All tasks. Done ones are excluded unless `include_done` is true.
/// The frontend buckets the flat list into per-subject sections itself.
#[tauri::command]
pub fn tasks_list(
    include_done: Option<bool>,
    db: State<'_, DbState>,
) -> Result<Vec<Task>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    tasks::list(&conn, include_done.unwrap_or(false)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tasks_create(
    title: String,
    subject_id: Option<i64>,
    est_minutes: Option<u32>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let task = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::create(&conn, &title, subject_id, est_minutes).map_err(|e| e.to_string())?
    };
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

#[tauri::command]
pub fn tasks_rename(
    id: i64,
    title: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let task = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::rename(&conn, id, &title).map_err(|e| e.to_string())?
    };
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

/// `est_minutes: None` clears the estimate.
#[tauri::command]
pub fn tasks_set_estimate(
    id: i64,
    est_minutes: Option<u32>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let task = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::set_estimate(&conn, id, est_minutes).map_err(|e| e.to_string())?
    };
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

/// Move a task to a different section. `subject_id: None` moves it to
/// Uncategorised.
#[tauri::command]
pub fn tasks_move(
    id: i64,
    subject_id: Option<i64>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let task = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::move_to_subject(&conn, id, subject_id).map_err(|e| e.to_string())?
    };
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

#[tauri::command]
pub fn tasks_set_done(
    id: i64,
    done: bool,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let task = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::set_done(&conn, id, done).map_err(|e| e.to_string())?
    };
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

/// Delete a task. Sessions logged against it are kept and become
/// unattributed — the caller should make that clear in its confirmation.
#[tauri::command]
pub fn tasks_delete(
    id: i64,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<(), String> {
    let cleared_active = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        tasks::delete(&conn, id).map_err(|e| e.to_string())?;
        forget_active_task(&conn, id)?
    };
    if let Some(new_settings) = cleared_active {
        timer.apply_settings(new_settings.clone());
        app.emit("settings:changed", &new_settings).ok();
    }
    app.emit("tasks:changed", ()).ok();
    Ok(())
}

/// After task `id` was deleted: if it was the active one, drop the pointer
/// too — the subject stays selected, only the specific task clears. Returns
/// the new settings when they changed.
fn forget_active_task(conn: &rusqlite::Connection, id: i64) -> Result<Option<Settings>, String> {
    let s = settings::load(conn).map_err(|e| e.to_string())?;
    if s.active_task_id != Some(id) {
        return Ok(None);
    }
    settings::save_setting(conn, "active_task_id", "").map_err(|e| e.to_string())?;
    if let Err(e) = queries::retag_open_work_session(conn, s.active_subject_id, None) {
        log::warn!("[tasks] failed to re-tag the in-flight session: {e}");
    }
    Ok(Some(settings::load(conn).map_err(|e| e.to_string())?))
}

#[tauri::command]
pub fn tasks_reorder(
    subject_id: Option<i64>,
    ids: Vec<i64>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<(), String> {
    {
        let mut conn = db.lock().map_err(|e| e.to_string())?;
        tasks::reorder(&mut conn, subject_id, &ids).map_err(|e| e.to_string())?;
    }
    app.emit("tasks:changed", ()).ok();
    Ok(())
}

/// Select the task new work rounds are attributed to (`None` clears it).
///
/// Picking a task pins its subject too (`active_subject_id` is set to the
/// task's own subject) — the two must always agree. Clearing the task
/// (`id: None`) leaves the subject selection untouched; you can still be
/// generally working on a subject without a specific item picked.
///
/// A round already under way is re-tagged as well, same reasoning as
/// `subjects_set_active`.
#[tauri::command]
pub fn tasks_set_active(
    id: Option<i64>,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<Settings, String> {
    let new_settings = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let subject_id = match id {
            Some(task_id) => {
                let task = tasks::get(&conn, task_id).map_err(|e| e.to_string())?;
                settings::save_setting(
                    &conn,
                    "active_subject_id",
                    &task.subject_id.map(|v| v.to_string()).unwrap_or_default(),
                )
                .map_err(|e| e.to_string())?;
                task.subject_id
            }
            // Keep whatever subject was already active; only the task clears.
            None => settings::load(&conn).map_err(|e| e.to_string())?.active_subject_id,
        };
        settings::save_setting(&conn, "active_task_id", &id.map(|v| v.to_string()).unwrap_or_default())
            .map_err(|e| e.to_string())?;
        if let Err(e) = queries::retag_open_work_session(&conn, subject_id, id) {
            log::warn!("[tasks] failed to re-tag the in-flight session: {e}");
        }
        settings::load(&conn).map_err(|e| e.to_string())?
    };
    log::info!("[tasks] active task set to {id:?}");
    timer.apply_settings(new_settings.clone());
    app.emit("settings:changed", &new_settings).ok();
    Ok(new_settings)
}

// ---------------------------------------------------------------------------
// Jot commands (碎碎念)
// ---------------------------------------------------------------------------

/// Every jot: open ones newest first, then handled ones.
#[tauri::command]
pub fn jots_list(db: State<'_, DbState>) -> Result<Vec<Jot>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    jots::list(&conn).map_err(|e| e.to_string())
}

/// Write a jot down. Where it came up (active subject, focus round under way)
/// is read here from the timer, not trusted from the caller.
#[tauri::command]
pub fn jots_create(
    body: String,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<Jot, String> {
    let snap = timer.get_snapshot();
    let jot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let context = jots::Context {
            subject_id: settings::load(&conn).map_err(|e| e.to_string())?.active_subject_id,
            in_focus: snap.round_type == "work" && (snap.is_running || snap.is_paused),
        };
        jots::create(&conn, &body, context).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    Ok(jot)
}

#[tauri::command]
pub fn jots_edit(id: i64, body: String, db: State<'_, DbState>, app: AppHandle) -> Result<Jot, String> {
    let jot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        jots::edit(&conn, id, &body).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    Ok(jot)
}

/// Cross a jot off, or bring it back.
#[tauri::command]
pub fn jots_set_done(id: i64, done: bool, db: State<'_, DbState>, app: AppHandle) -> Result<Jot, String> {
    let jot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        jots::set_done(&conn, id, done).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    Ok(jot)
}

/// Turn a jot into a task in `subject_id`'s section (`None` = Uncategorised).
#[tauri::command]
pub fn jots_to_task(
    id: i64,
    subject_id: Option<i64>,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<Task, String> {
    let (_, task) = {
        let mut conn = db.lock().map_err(|e| e.to_string())?;
        jots::to_task(&mut conn, id, subject_id).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    app.emit("tasks:changed", ()).ok();
    Ok(task)
}

/// Undo `jots_to_task`: removes the task it made and reopens the jot.
#[tauri::command]
pub fn jots_untask(
    id: i64,
    db: State<'_, DbState>,
    timer: State<'_, TimerController>,
    app: AppHandle,
) -> Result<Jot, String> {
    let (jot, cleared_active) = {
        let mut conn = db.lock().map_err(|e| e.to_string())?;
        let (jot, removed) = jots::untask(&mut conn, id).map_err(|e| e.to_string())?;
        let cleared = match removed {
            Some(task_id) => forget_active_task(&conn, task_id)?,
            None => None,
        };
        (jot, cleared)
    };
    if let Some(new_settings) = cleared_active {
        timer.apply_settings(new_settings.clone());
        app.emit("settings:changed", &new_settings).ok();
    }
    app.emit("jots:changed", ()).ok();
    app.emit("tasks:changed", ()).ok();
    Ok(jot)
}

/// Delete a jot; returns it so `jots_restore` can put it back.
#[tauri::command]
pub fn jots_delete(id: i64, db: State<'_, DbState>, app: AppHandle) -> Result<Jot, String> {
    let jot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        jots::delete(&conn, id).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    Ok(jot)
}

#[tauri::command]
pub fn jots_restore(jot: Jot, db: State<'_, DbState>, app: AppHandle) -> Result<Jot, String> {
    let jot = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        jots::restore(&conn, &jot).map_err(|e| e.to_string())?
    };
    app.emit("jots:changed", ()).ok();
    Ok(jot)
}

/// Delete every crossed-off or converted jot; returns how many.
#[tauri::command]
pub fn jots_clear_handled(db: State<'_, DbState>, app: AppHandle) -> Result<usize, String> {
    let n = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        jots::clear_handled(&conn).map_err(|e| e.to_string())?
    };
    log::info!("[jots] cleared {n} handled jots");
    app.emit("jots:changed", ()).ok();
    Ok(n)
}

// ---------------------------------------------------------------------------
// CMD-05 — Window commands
// ---------------------------------------------------------------------------

/// Show or hide the main window.
#[tauri::command]
pub fn window_set_visibility(visible: bool, app: AppHandle) -> Result<(), String> {
    log::debug!("[window] set visibility={visible}");
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    if visible {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// CMD-07 — Audio commands
// ---------------------------------------------------------------------------

/// Copy a user-selected audio file into the app config dir for the given cue slot.
///
/// `cue` must be one of: `"work_alert"`, `"short_break_alert"`, `"long_break_alert"`.
/// `src_path` is the full path to the file chosen by the user.
///
/// The file is stored with a fixed stem (e.g. `custom_work_alert.mp3`) so that
/// selecting a new file for the same slot automatically replaces the old one —
/// no orphan files accumulate.
///
/// Returns the original filename for display in the UI.
#[tauri::command]
pub fn audio_set_custom(
    cue: String,
    src_path: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<String, String> {
    let audio_state = app
        .try_state::<Arc<AudioManager>>()
        .ok_or_else(|| "audio engine is not available".to_string())?;

    let stem = cue_to_stem(&cue)?;

    let audio_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("audio");
    std::fs::create_dir_all(&audio_dir).map_err(|e| e.to_string())?;

    let src = std::path::Path::new(&src_path);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp3");

    // Remove any existing custom file for this slot (preserves zero orphans).
    if let Ok(entries) = std::fs::read_dir(&audio_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
                let _ = std::fs::remove_file(&p);
            }
        }
    }

    let dest = audio_dir.join(format!("{stem}.{ext}"));
    std::fs::copy(src, &dest).map_err(|e| e.to_string())?;

    // Verify the copied file is decodable before committing. If it fails,
    // clean up the orphan and sync in-memory state to default (the old file
    // was already deleted above).
    if let Err(e) = audio::probe_audio_file(&dest) {
        let _ = std::fs::remove_file(&dest);
        audio_state.clear_custom_path(&cue);
        return Err(e);
    }

    audio_state.set_custom_path(&cue, dest);

    let display_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("custom")
        .to_string();

    // Persist the original filename so it survives restarts.
    let name_key = cue_to_name_key(&cue)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    settings::save_setting(&conn, name_key, &display_name).map_err(|e| e.to_string())?;

    log::info!("[audio] custom sound set cue={cue} file={display_name}");
    Ok(display_name)
}

/// Play the button-click sound (red primary buttons), when enabled in settings.
#[tauri::command]
pub fn audio_play_click(db: State<'_, DbState>, app: AppHandle) -> Result<(), String> {
    let enabled = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        settings::load(&conn).map_err(|e| e.to_string())?.click_sound_enabled
    };
    if let (true, Some(audio_state)) = (enabled, app.try_state::<Arc<AudioManager>>()) {
        audio_state.play_cue(audio::AudioCue::ButtonClick);
    }
    Ok(())
}

/// Restore the built-in sound for the given cue slot by deleting the custom file.
#[tauri::command]
pub fn audio_clear_custom(
    cue: String,
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<(), String> {
    let audio_state = app
        .try_state::<Arc<AudioManager>>()
        .ok_or_else(|| "audio engine is not available".to_string())?;

    let stem = cue_to_stem(&cue)?;

    let audio_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("audio");

    if let Ok(entries) = std::fs::read_dir(&audio_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.file_stem().and_then(|s| s.to_str()) == Some(stem) {
                std::fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
        }
    }

    audio_state.clear_custom_path(&cue);

    // Remove the persisted display name.
    let name_key = cue_to_name_key(&cue)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM settings WHERE key = ?1", rusqlite::params![name_key])
        .map_err(|e| e.to_string())?;

    log::info!("[audio] custom sound cleared cue={cue}");
    Ok(())
}

/// Return the display names of any currently configured custom audio files.
/// Fields are `null` when the built-in sound is in use for that slot.
#[tauri::command]
pub fn audio_get_custom_info(
    db: State<'_, DbState>,
    app: AppHandle,
) -> Result<audio::CustomAudioInfo, String> {
    let audio_state = app
        .try_state::<Arc<AudioManager>>()
        .ok_or_else(|| "audio engine is not available".to_string())?;

    // Start from the AudioManager's paths (determines which slots are active).
    let mut info = audio_state.get_custom_info();

    // Override each active slot's name with the persisted original filename.
    let conn = db.lock().map_err(|e| e.to_string())?;
    let override_name = |stored: &Option<String>, key: &str| -> Option<String> {
        stored.as_ref()?; // slot not active — leave as None
        settings::get_setting(&conn, key).or_else(|| stored.clone())
    };
    info.work_alert = override_name(&info.work_alert, "custom_work_alert_name");
    info.short_break_alert = override_name(&info.short_break_alert, "custom_short_break_alert_name");
    info.long_break_alert = override_name(&info.long_break_alert, "custom_long_break_alert_name");
    info.button_click = override_name(&info.button_click, "custom_button_click_name");

    Ok(info)
}

// ---------------------------------------------------------------------------
// CMD-08 — Notification command
// ---------------------------------------------------------------------------

/// Show a desktop notification with the given title and body.
///
/// String construction (including translation) is the caller's (frontend's)
/// responsibility. This command is a thin platform-dispatch wrapper.
#[tauri::command]
pub fn notification_show(title: String, body: String, app: AppHandle) {
    notifications::show(&app, &title, &body);
}

// ---------------------------------------------------------------------------
// CMD-09 — Diagnostic log commands
// ---------------------------------------------------------------------------

/// Open the application log directory in the OS file manager.
#[tauri::command]
pub fn open_log_dir(app: AppHandle) {
    match app.path().app_log_dir() {
        Ok(log_dir) => {
            if let Err(e) = tauri_plugin_opener::open_path(&log_dir, None::<&str>) {
                log::warn!("[log] failed to open log dir {}: {e}", log_dir.display());
            }
        }
        Err(e) => log::warn!("[log] failed to resolve log dir: {e}"),
    }
}

/// Return the compile-time build version string.
#[tauri::command]
pub fn app_version() -> &'static str {
    env!("APP_BUILD_VERSION")
}

// ---------------------------------------------------------------------------
// CMD-10 — Platform commands
// ---------------------------------------------------------------------------

/// Returns whether the app has macOS Accessibility permission.
/// On macOS, calls AXIsProcessTrusted() from the ApplicationServices framework.
/// On all other platforms, always returns true.
#[tauri::command]
pub fn accessibility_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        unsafe { AXIsProcessTrusted() }
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Returns whether the system tray is supported on this platform/install.
/// On Linux, probes for libayatana-appindicator3 / libappindicator3 at runtime.
/// On macOS and Windows, always returns true.
#[tauri::command]
pub fn tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        tray::appindicator_available()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Return the application log directory path as a string.
#[tauri::command]
pub fn get_log_dir(app: AppHandle) -> Result<String, String> {
    app.path()
        .app_log_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| {
            log::warn!("[log] failed to resolve log dir: {e}");
            e.to_string()
        })
}

// ---------------------------------------------------------------------------
// CMD-11 — Updater commands
// ---------------------------------------------------------------------------

/// Information about an available update returned to the frontend.
#[derive(serde::Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

/// Check whether a newer version is available.
/// Returns `Some(UpdateInfo)` when an update is available, or `None` when
/// the running version is already the latest.
/// Errors (e.g. network failure) are surfaced as a string so the frontend
/// can display a non-blocking message.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    log::info!("[updater] checking for updates");
    let updater = app.updater().map_err(|e| {
        log::error!("[updater] failed to build updater: {e}");
        e.to_string()
    })?;
    match updater.check().await {
        Ok(Some(update)) => {
            log::info!("[updater] update available: v{}", update.version);
            Ok(Some(UpdateInfo {
                version: update.version.clone(),
                body: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
            }))
        }
        Ok(None) => {
            log::info!("[updater] already up to date");
            Ok(None)
        }
        Err(e) => {
            log::warn!("[updater] update check failed: {e}");
            Err(e.to_string())
        }
    }
}

/// Download, verify, and install the pending update, then relaunch immediately.
/// Should only be called after `check_update` has returned `Some(UpdateInfo)`.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    log::info!("[updater] install requested — checking for update");
    let updater = app.updater().map_err(|e| {
        log::error!("[updater] failed to build updater: {e}");
        e.to_string()
    })?;
    let update = updater
        .check()
        .await
        .map_err(|e| {
            log::error!("[updater] update check failed during install: {e}");
            e.to_string()
        })?
        .ok_or_else(|| {
            log::warn!("[updater] install_update called but no update is available");
            "No update available".to_string()
        })?;
    log::info!("[updater] downloading and installing v{}", update.version);
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| {
            log::error!("[updater] download/install failed: {e}");
            e.to_string()
        })?;
    log::info!("[updater] install complete — relaunching");
    app.restart();
}

fn cue_to_stem(cue: &str) -> Result<&'static str, String> {
    match cue {
        "work_alert" => Ok(audio::STEM_WORK),
        "short_break_alert" => Ok(audio::STEM_SHORT),
        "long_break_alert" => Ok(audio::STEM_LONG),
        "button_click" => Ok(audio::STEM_CLICK),
        _ => Err(format!("unknown audio cue: '{cue}'")),
    }
}

fn cue_to_name_key(cue: &str) -> Result<&'static str, String> {
    match cue {
        "work_alert" => Ok("custom_work_alert_name"),
        "short_break_alert" => Ok("custom_short_break_alert_name"),
        "long_break_alert" => Ok("custom_long_break_alert_name"),
        "button_click" => Ok("custom_button_click_name"),
        _ => Err(format!("unknown audio cue: '{cue}'")),
    }
}

// ---------------------------------------------------------------------------
// Stats payload types
// ---------------------------------------------------------------------------

/// Batched payload for Today + This Week tabs.
#[derive(serde::Serialize)]
pub struct DetailedStats {
    pub today: queries::DailyStats,
    pub week: Vec<queries::DayStat>,
    pub streak: queries::StreakInfo,
}

/// Payload for the All Time tab.
#[derive(serde::Serialize)]
pub struct HeatmapStats {
    pub entries: Vec<queries::HeatmapEntry>,
    pub total_rounds: u32,
    pub total_hours: u32,
    pub longest_streak: u32,
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    fn seed_sessions(conn: &Connection) {
        conn.execute_batch("
            INSERT INTO sessions (started_at, ended_at, round_type, duration_secs, completed)
            VALUES (1000, 1060, 'work', 60, 1),
                   (2000, 2300, 'short-break', 300, 1);
        ").unwrap();
    }

    #[test]
    fn sessions_clear_removes_all_rows() {
        let conn = setup();
        seed_sessions(&conn);
        let before: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(before, 2);

        let n = conn.execute("DELETE FROM sessions", []).unwrap();
        assert_eq!(n, 2);

        let after: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(after, 0);
    }

    #[test]
    fn sessions_clear_on_empty_table_returns_zero() {
        let conn = setup();
        let n = conn.execute("DELETE FROM sessions", []).unwrap();
        assert_eq!(n, 0);
    }
}

