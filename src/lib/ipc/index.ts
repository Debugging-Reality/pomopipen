// Typed wrappers around Tauri invoke() and listen().
// All backend communication goes through this module.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import type {
  TimerState,
  Settings,
  Theme,
  CustomAudioInfo,
  GcalStatus,
  DetailedStats,
  HeatmapStats,
  UpdateInfo,
  Subject,
  SubjectFilter,
  SubjectTotal,
  Task,
  WeekEvent,
  FocusRecord,
  Jot,
  JotCapture,
} from '$lib/types';

// --- Timer commands ---

export const timerToggle = () => invoke<void>('timer_toggle');
export const timerReset = () => invoke<void>('timer_reset');
export const timerRestartRound = () => invoke<void>('timer_restart_round');
export const timerSkip = () => invoke<void>('timer_skip');
export const getTimerState = () => invoke<TimerState>('timer_get_state');

// --- Settings commands ---

export const getSettings = () => invoke<Settings>('settings_get');
/** Save a single setting key/value pair and receive the full updated settings. */
export const setSetting = (key: string, value: string) =>
  invoke<Settings>('settings_set', { key, value });
export const importBackground = (sourcePath: string, target: 'timer' | 'calendar') =>
  invoke<string>('background_import', { sourcePath, target });
export const resetSettings = () => invoke<Settings>('settings_reset_defaults');
export const reloadShortcuts = () => invoke<void>('shortcuts_reload');

// --- Theme commands ---

export const getThemes = () => invoke<Theme[]>('themes_list');

// --- Notification commands ---

export const notificationShow = (title: string, body: string) =>
  invoke<void>('notification_show', { title, body });

// --- Window commands ---

export const setWindowVisibility = (visible: boolean) =>
  invoke<void>('window_set_visibility', { visible });

// --- Audio commands ---

export const getCustomAudioInfo = () => invoke<CustomAudioInfo>('audio_get_custom_info');

/** Copy `srcPath` to the config dir for `cue`; returns the display name. */
export const setCustomAudio = (cue: string, srcPath: string) =>
  invoke<string>('audio_set_custom', { cue, srcPath });

/** Delete the custom file for `cue` and revert to the built-in sound. */
export const clearCustomAudio = (cue: string) => invoke<void>('audio_clear_custom', { cue });
/** Plays the button-click cue if it is enabled (the backend checks the setting). */
export const audioPlayClick = () => invoke<void>('audio_play_click');

/** Open a native file picker filtered to audio formats. Returns a path or null. */
export const openAudioFilePicker = (): Promise<string | null> =>
  dialogOpen({
    multiple: false,
    filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'ogg'] }],
  }) as Promise<string | null>;

// --- Diagnostic log commands ---

/** Open the application log directory in the OS file manager. */
export const openLogDir = () => invoke<void>('open_log_dir');

/** Return the resolved log directory path as a string. */
export const getLogDir = () => invoke<string>('get_log_dir');

/** Return the compile-time build version string (e.g. `1.0.0-dev.80+20b2d87`). */
export const appVersion = () => invoke<string>('app_version');

// --- Sessions commands ---

export const clearSessionHistory = () => invoke<void>('sessions_clear');

/** Week calendar: add a completed focus record by hand; returns its id. */
export const sessionsCreate = (record: FocusRecord) => invoke<number>('sessions_create', { record });
/** Move, resize or re-tag a completed focus record. */
export const sessionsUpdate = (id: number, record: FocusRecord) => invoke<void>('sessions_update', { id, record });
/** Delete a completed focus record; returns it so it can be put back. */
export const sessionsDelete = (id: number) => invoke<FocusRecord>('sessions_delete', { id });

// --- Stats commands ---

/** UTC seconds representing local midnight boundaries, end exclusive. */
export const statsGetWeekEvents = (start: number, end: number, subject?: SubjectFilter) =>
  invoke<WeekEvent[]>('stats_get_week_events', { start, end, subject });

/** Crops `sourcePath` to a square app icon, applies it to the windows (and, in
 *  the everyday build, the Start Menu shortcut). Returns the stored PNG path. */
export const appIconSet = (sourcePath: string) => invoke<string>('app_icon_set', { sourcePath });

/** Back to the built-in icon. */
export const appIconReset = () => invoke<void>('app_icon_reset');

/** Completed focus sessions overlapping [start, end) (unix seconds, up to 400 days) — for charts. */
export const statsGetRangeEvents = (start: number, end: number, subject?: SubjectFilter) =>
  invoke<WeekEvent[]>('stats_get_range_events', { start, end, subject });

/** Daily + weekly data + streak in one call (Today and This Week tabs).
 *  `subject` filters all three; omitted or undefined means every session. */
export const statsGetDetailed = (subject?: SubjectFilter) =>
  invoke<DetailedStats>('stats_get_detailed', { subject });

/** Heatmap entries + lifetime totals (All Time tab). Same filter semantics as above. */
export const statsGetHeatmap = (subject?: SubjectFilter) =>
  invoke<HeatmapStats>('stats_get_heatmap', { subject });

/** Focus time per subject. `days` limits the window; omitted means all time. */
export const statsGetSubjectBreakdown = (days?: number) =>
  invoke<SubjectTotal[]>('stats_get_subject_breakdown', { days });

// --- Subject commands ---

/** All subjects, ordered for display. Archived ones excluded unless requested. */
export const subjectsList = (includeArchived?: boolean) =>
  invoke<Subject[]>('subjects_list', { includeArchived });

export const subjectsCreate = (name: string, color: string) =>
  invoke<Subject>('subjects_create', { name, color });

/** Patch a subject. Omitted fields are left untouched. */
export const subjectsUpdate = (id: number, name?: string, color?: string) =>
  invoke<Subject>('subjects_update', { id, name, color });

export const subjectsSetArchived = (id: number, archived: boolean) =>
  invoke<Subject>('subjects_set_archived', { id, archived });

/** Delete a subject. Its sessions are kept and become uncategorised. */
export const subjectsDelete = (id: number) => invoke<void>('subjects_delete', { id });

/** Persist a new display order — the full list of subject ids, in order. */
export const subjectsReorder = (ids: number[]) => invoke<void>('subjects_reorder', { ids });

/** Select the subject new work rounds are attributed to (`null` clears it). */
export const subjectsSetActive = (id: number | null) =>
  invoke<Settings>('subjects_set_active', { id });

// --- Task commands ---

/** All tasks. Done ones are excluded unless `includeDone` is true. */
export const tasksList = (includeDone?: boolean) => invoke<Task[]>('tasks_list', { includeDone });

export const tasksCreate = (title: string, subjectId: number | null, estMinutes: number | null) =>
  invoke<Task>('tasks_create', { title, subjectId, estMinutes });

export const tasksRename = (id: number, title: string) =>
  invoke<Task>('tasks_rename', { id, title });

/** `null` clears the estimate. */
export const tasksSetEstimate = (id: number, estMinutes: number | null) =>
  invoke<Task>('tasks_set_estimate', { id, estMinutes });

/** Move a task to a different section. `null` moves it to Uncategorized. */
export const tasksMove = (id: number, subjectId: number | null) =>
  invoke<Task>('tasks_move', { id, subjectId });

export const tasksSetDone = (id: number, done: boolean) =>
  invoke<Task>('tasks_set_done', { id, done });

/** Delete a task. Its logged sessions are kept and become unattributed. */
export const tasksDelete = (id: number) => invoke<void>('tasks_delete', { id });

/** Persist a new display order within one section (`subjectId: null` = Uncategorized). */
export const tasksReorder = (subjectId: number | null, ids: number[]) =>
  invoke<void>('tasks_reorder', { subjectId, ids });

/** Select the task new work rounds are attributed to (`null` clears it).
 *  Pins the task's subject as the active subject too. */
export const tasksSetActive = (id: number | null) => invoke<Settings>('tasks_set_active', { id });

// --- Jot commands (碎碎念) ---

/** Every jot: open ones newest first, then crossed-off / converted ones. */
export const jotsList = () => invoke<Jot[]>('jots_list');
/** Write one down; the backend records the active subject and whether a focus round is under way. */
export const jotsCreate = (body: string) => invoke<Jot>('jots_create', { body });
export const jotsEdit = (id: number, body: string) => invoke<Jot>('jots_edit', { id, body });
/** Cross off (or bring back). A converted jot is undone with `jotsUntask`. */
export const jotsSetDone = (id: number, done: boolean) => invoke<Jot>('jots_set_done', { id, done });
/** Turn into a task in `subjectId`'s section (`null` = Uncategorised). */
export const jotsToTask = (id: number, subjectId: number | null) =>
  invoke<Task>('jots_to_task', { id, subjectId });
/** Undo `jotsToTask`: deletes the task it made and reopens the jot. */
export const jotsUntask = (id: number) => invoke<Jot>('jots_untask', { id });
/** Delete; returns the jot so `jotsRestore` can put it back. */
export const jotsDelete = (id: number) => invoke<Jot>('jots_delete', { id });
export const jotsRestore = (jot: Jot) => invoke<Jot>('jots_restore', { jot });
/** Delete every crossed-off or converted jot; returns how many. */
export const jotsClearHandled = () => invoke<number>('jots_clear_handled');

// --- Platform commands ---

export const accessibilityTrusted = () => invoke<boolean>('accessibility_trusted');

/** Returns true if the system tray is usable on this platform/install.
 *  On Linux this probes for libayatana-appindicator3 / libappindicator3 at
 *  runtime; on macOS and Windows it always returns true. */
export const traySupported = () => invoke<boolean>('tray_supported');

// --- Updater commands ---

/** Check for an available update. Returns update info or null if already up to date. */
export const checkUpdate = () => invoke<UpdateInfo | null>('check_update');

/** Download, install, and immediately relaunch with the pending update. */
export const installUpdate = () => invoke<void>('install_update');

// --- Event listeners ---

export const onTimerTick = (
  cb: (payload: { elapsed_secs: number; total_secs: number }) => void
): Promise<UnlistenFn> =>
  listen<{ elapsed_secs: number; total_secs: number }>('timer:tick', (e) => cb(e.payload));

export const onTimerPaused = (
  cb: (payload: { elapsed_secs: number }) => void
): Promise<UnlistenFn> => listen<{ elapsed_secs: number }>('timer:paused', (e) => cb(e.payload));

export const onTimerResumed = (
  cb: (payload: { elapsed_secs: number }) => void
): Promise<UnlistenFn> => listen<{ elapsed_secs: number }>('timer:resumed', (e) => cb(e.payload));

export const onRoundChange = (cb: (state: TimerState) => void): Promise<UnlistenFn> =>
  listen<TimerState>('timer:round-change', (e) => cb(e.payload));

export const onTimerReset = (cb: (state: TimerState) => void): Promise<UnlistenFn> =>
  listen<TimerState>('timer:reset', (e) => cb(e.payload));

export const onSettingsChanged = (cb: (settings: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>('settings:changed', (e) => cb(e.payload));

export const onThemesChanged = (cb: (themes: Theme[]) => void): Promise<UnlistenFn> =>
  listen<Theme[]>('themes:changed', (e) => cb(e.payload));

export const onSessionsCleared = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('sessions:cleared', () => cb());

/** Fires after a focus record is added, edited or deleted by hand (week calendar). */
export const onSessionsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('sessions:changed', () => cb());

/** Fires after any subject create/rename/recolor/archive/delete/reorder. */
export const onSubjectsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('subjects:changed', () => cb());

// --- Google Calendar sync (src-tauri/src/gcal) ---

export const gcalStatus = () => invoke<GcalStatus>('gcal_status');
/** Import the Desktop-app OAuth client JSON downloaded from Google Cloud. */
export const gcalImportClient = (path: string) => invoke<GcalStatus>('gcal_import_client', { path });
/** Opens the browser to sign in; resolves once connected (or rejects). */
export const gcalConnect = () =>
  invoke<GcalStatus>('gcal_connect', { timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone ?? '' });
export const gcalCancelConnect = () => invoke<void>('gcal_cancel_connect');
export const gcalSyncNow = () => invoke<GcalStatus>('gcal_sync_now');
export const gcalDisconnect = () => invoke<GcalStatus>('gcal_disconnect');
export const gcalSetAuto = (enabled: boolean) => invoke<GcalStatus>('gcal_set_auto', { enabled });
export const onGcalStatus = (cb: (status: GcalStatus) => void): Promise<UnlistenFn> =>
  listen<GcalStatus>('gcal:status', (e) => cb(e.payload));
export const openJsonFilePicker = (): Promise<string | null> =>
  dialogOpen({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] }) as Promise<string | null>;

/** Fires after any task create/rename/estimate/move/done/delete/reorder. */
export const onTasksChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('tasks:changed', () => cb());

/** Fires after any jot is written, edited, crossed off, converted or deleted. */
export const onJotsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('jots:changed', () => cb());

/** The global jot shortcut brought the timer window forward (main window only). */
export const onJotCapture = (cb: (capture: JotCapture) => void): Promise<UnlistenFn> =>
  listen<JotCapture>('jots:capture', (e) => cb(e.payload));
