// Shared TypeScript types mirroring Rust structs (must stay in sync with Rust serde output).

export type RoundType = 'work' | 'short-break' | 'long-break';

/** Mirrors Rust `TimerSnapshot` — emitted via timer:tick / timer:round-change events
 *  and returned by the `timer_get_state` IPC command. */
export interface TimerState {
  round_type: RoundType;
  previous_round_type: string; // round type before this one; "" on first round
  elapsed_secs: number;
  total_secs: number;
  is_running: boolean;
  is_paused: boolean;
  work_round_number: number; // current work round (1-based)
  work_rounds_total: number; // total work rounds before long break
  session_work_count: number; // monotonic focus round count since last reset
}

/** Mirrors Rust `Settings` struct returned by `settings_get`. */
export interface Settings {
  time_work_secs: number;
  time_short_break_secs: number;
  time_long_break_secs: number;
  long_break_interval: number;
  short_breaks_enabled: boolean;
  long_breaks_enabled: boolean;
  auto_start_work: boolean;
  auto_start_break: boolean;
  tray_icon_enabled: boolean;
  min_to_tray: boolean;
  min_to_tray_on_close: boolean;
  notifications_enabled: boolean;
  always_on_top: boolean;
  break_always_on_top: boolean;
  volume: number; // 0.0–1.0
  tick_sounds_during_work: boolean;
  click_sound_enabled: boolean; // sound on red primary buttons
  tick_sounds_during_break: boolean;
  shortcut_toggle: string;
  shortcut_reset: string;
  shortcut_skip: string;
  shortcut_restart: string;
  shortcut_jot: string; // global: timer forward with the jot pad open
  websocket_enabled: boolean;
  websocket_port: number;
  theme_mode: string; // 'auto' | 'light' | 'dark'
  theme_light: string;
  theme_dark: string;
  theme_backgrounds: string; // JSON, see $lib/themes/backgrounds.ts
  stats_zoom: number; // percent, 50–200
  app_icon: string; // custom icon PNG path; '' = built-in icon
  timer_appearance: 'mechanical' | 'ring'; // Classic Tomato only
  basket_frame_opacity: number; // 0–100, Classic Tomato calendar basket frame
  basket_floor_opacity: number; // 0–100, the basket floor grid inside the calendar
  classic_pattern: 'vine' | 'varieties' | 'none'; // Classic Tomato page backgrounds
  week_starts_monday: boolean; // stats weeks start on Monday (else Sunday)
  dial_countdown: boolean;
  language: string; // 'auto' | 'en' | 'es' | 'fr' | 'de' | 'ja'
  verbose_logging: boolean;
  check_for_updates: boolean;
  global_shortcuts_enabled: boolean;
  local_shortcut_toggle: string;
  local_shortcut_reset: string;
  local_shortcut_skip: string;
  local_shortcut_volume_down: string;
  local_shortcut_volume_up: string;
  local_shortcut_mute: string;
  local_shortcut_fullscreen: string;
  local_shortcut_jot: string; // opens the jot pad (碎碎念)
  /** Subject new work rounds are attributed to. `null` = uncategorised. */
  active_subject_id: number | null;
  /** Task new work rounds are attributed to. `null` = no specific task. */
  active_task_id: number | null;
}

/** Mirrors Rust `gcal::GcalStatus` — Google Calendar sync (Settings → Calendar sync). */
export interface GcalStatus {
  /** OAuth client in use (imported, or embedded in this build). */
  client_id: string | null;
  client_embedded: boolean;
  connected: boolean;
  account: string | null;
  auto_sync: boolean;
  syncing: boolean;
  /** Waiting for the browser sign-in. */
  connecting: boolean;
  /** Unix seconds of the last successful sync. */
  last_sync: number | null;
  last_error: string | null;
  last_result: { blocks: number; created: number; updated: number; deleted: number } | null;
  /** Proxy Google requests go through now (e.g. `http://127.0.0.1:7897`); null = direct. */
  proxy: string | null;
}

/** Returned by `check_update` — describes an available update. */
export interface UpdateInfo {
  version: string;
  body: string | null;
  date: string | null;
}

/** Mirrors Rust `CustomAudioInfo` — null means the built-in sound is active. */
export interface CustomAudioInfo {
  work_alert: string | null;
  short_break_alert: string | null;
  long_break_alert: string | null;
  button_click: string | null;
}

/** Mirrors Rust `Theme` struct. Color keys include the `--` CSS var prefix. */
export interface Theme {
  name: string;
  colors: Record<string, string>; // keys like "--color-background", "--color-focus-round"
  is_custom: boolean;
}

// ---------------------------------------------------------------------------
// Stats types — mirror Rust structs in commands.rs / queries.rs
// ---------------------------------------------------------------------------

export interface DailyStats {
  rounds: number;
  focus_mins: number;
  completion_rate: number | null; // null when no sessions started today
  by_hour: number[]; // 24 entries, index = hour of day
}

export interface DayStat {
  date: string; // "YYYY-MM-DD"
  rounds: number;
}

export interface WeekEvent {
  id: number;
  started_at: number;
  duration_secs: number;
  subject_id: number | null;
  subject_name: string | null;
  subject_color: string | null;
  task_id: number | null;
  task_title: string | null;
  /** False for a round skipped or reset part way; `duration_secs` is then the time actually focused. */
  completed: boolean;
}

/** Mirrors Rust `db::calendar::FocusRecord` — a focus record edited by hand. */
export interface FocusRecord {
  started_at: number; // unix seconds
  duration_secs: number;
  subject_id: number | null;
  task_id: number | null;
}

export interface HeatmapEntry {
  date: string; // "YYYY-MM-DD"
  count: number;
}

export interface StreakInfo {
  current: number;
  longest: number;
}

/** Returned by stats_get_detailed — Today + This Week + streak in one call. */
export interface DetailedStats {
  today: DailyStats;
  week: DayStat[];
  streak: StreakInfo;
}

/** Returned by stats_get_heatmap — heatmap entries + lifetime totals. */
export interface HeatmapStats {
  entries: HeatmapEntry[];
  total_rounds: number;
  total_hours: number;
  longest_streak: number;
}

// ---------------------------------------------------------------------------
// Subject types — mirror Rust structs in subjects/mod.rs / db/queries.rs
// ---------------------------------------------------------------------------

/** Mirrors Rust `Subject` struct. */
export interface Subject {
  id: number;
  name: string;
  color: string; // "#rrggbb"
  archived: boolean;
  created_at: number;
  sort_order: number;
}

/**
 * Mirrors Rust `SubjectFilter` enum (serde externally-tagged, camelCase).
 * `'all'` — every session. `'uncategorized'` — only sessions with no subject.
 * `{ subject: id }` — only that subject's sessions.
 */
export type SubjectFilter = 'all' | 'uncategorized' | { subject: number };

/** Mirrors Rust `SubjectTotal` — one row of the per-subject breakdown. */
export interface SubjectTotal {
  subject_id: number | null; // null = the uncategorised bucket
  name: string; // empty string for the uncategorised bucket
  color: string; // empty string for the uncategorised bucket
  rounds: number;
  focus_secs: number;
}

// ---------------------------------------------------------------------------
// Task types — mirror Rust structs in tasks/mod.rs
// ---------------------------------------------------------------------------

/** Mirrors Rust `Task` struct. A task's "section" in the UI is its subject —
 *  there is no separate sections table; `subject_id: null` is Uncategorised. */
export interface Task {
  id: number;
  title: string;
  subject_id: number | null;
  done: boolean;
  est_minutes: number | null;
  created_at: number;
  completed_at: number | null;
  sort_order: number;
  /** Seconds focused in work sessions logged against this task, unfinished rounds included. */
  actual_secs: number;
}

// ---------------------------------------------------------------------------
// Jot types — mirror Rust structs in jots/mod.rs
// ---------------------------------------------------------------------------

/** Mirrors Rust `Jot`: a stray thought or to-do written down mid-round (碎碎念). */
export interface Jot {
  id: number;
  body: string;
  created_at: number;
  /** When it was crossed off or turned into a task; `null` = still open. */
  done_at: number | null;
  /** The task it became, while that task exists. */
  task_id: number | null;
  /** Subject active when it was written (context only). */
  subject_id: number | null;
  /** Written while a focus round was running or paused. */
  in_focus: boolean;
}

/** Payload of `jots:capture` (the global jot shortcut): how to put the window back afterwards. */
export interface JotCapture {
  restore: 'hide' | 'minimize' | '';
}
