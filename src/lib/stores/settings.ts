// Reactive settings store.
// Loaded once on startup from Rust via settings_get; updated on save.

import { writable } from 'svelte/store';
import type { Settings } from '$lib/types';

const defaults: Settings = {
  time_work_secs: 1500,
  time_short_break_secs: 300,
  time_long_break_secs: 900,
  long_break_interval: 4,
  short_breaks_enabled: true,
  long_breaks_enabled: true,
  auto_start_work: false,
  auto_start_break: false,
  tray_icon_enabled: false,
  min_to_tray: false,
  min_to_tray_on_close: false,
  notifications_enabled: false,
  always_on_top: false,
  break_always_on_top: false,
  volume: 1.0,
  tick_sounds_during_work: false,
  click_sound_enabled: true,
  tick_sounds_during_break: false,
  shortcut_toggle: 'Control+F1',
  shortcut_reset: 'Control+F2',
  shortcut_skip: 'Control+F3',
  shortcut_restart: 'Control+F4',
  shortcut_jot: 'Control+Alt+N',
  websocket_enabled: false,
  websocket_port: 1314,
  theme_mode: 'auto',
  theme_light: 'Classic Tomato',
  theme_dark: 'Classic Tomato',
  theme_backgrounds: '{}',
  stats_zoom: 100,
  app_icon: '',
  timer_appearance: 'mechanical',
  basket_frame_opacity: 100,
  basket_floor_opacity: 40,
  classic_pattern: 'vine',
  week_starts_monday: false,
  dial_countdown: true,
  language: 'en',
  verbose_logging: false,
  check_for_updates: true,
  global_shortcuts_enabled: false,
  local_shortcut_toggle: ' ',
  local_shortcut_reset: 'ArrowLeft',
  local_shortcut_skip: 'ArrowRight',
  local_shortcut_volume_down: 'ArrowDown',
  local_shortcut_volume_up: 'ArrowUp',
  local_shortcut_mute: 'm',
  local_shortcut_fullscreen: 'F11',
  local_shortcut_jot: 'n',
  active_subject_id: null,
  active_task_id: null,
};

export const settings = writable<Settings>(defaults);

// Classic Tomato's page pattern is chosen in settings and applied by CSS
// (styles/classic-tomato.css reads html[data-classic-pattern]) in every window.
if (typeof document !== 'undefined') {
  settings.subscribe((s) => {
    document.documentElement.dataset.classicPattern = s.classic_pattern;
  });
}
