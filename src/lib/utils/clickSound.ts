// A short sound whenever a red primary button is pressed (mouse or keyboard).
// The sound itself — built-in or the user's own file — is played by the Rust
// audio engine; this only decides which clicks count.
import { get } from 'svelte/store';
import { settings } from '$lib/stores/settings';
import { audioPlayClick } from '$lib/ipc';

/** The red primary buttons across the timer themes and the settings-style windows. */
const PRIMARY = '.bp, .s-btn--primary, .play-pause, .controls .primary, .jot-send';

/** Listen in this window; returns the cleanup. */
export function installClickSound(): () => void {
  const onClick = (e: MouseEvent) => {
    const button = (e.target as Element | null)?.closest?.(PRIMARY);
    if (!button || (button as HTMLButtonElement).disabled) return;
    if (!get(settings).click_sound_enabled) return;
    audioPlayClick().catch(() => {});
  };
  document.addEventListener('click', onClick, true);
  return () => document.removeEventListener('click', onClick, true);
}
