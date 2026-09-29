// Pure helpers for the jot pad (碎碎念). Kept free of Svelte so tests can import them.
import type { Jot, TimerState } from '$lib/types';

/** Open jots (plus ones still playing their crossed-off moment), newest first;
 *  then handled ones, most recently handled first. */
export function splitJots(list: Jot[], settling: ReadonlySet<number>): { open: Jot[]; handled: Jot[] } {
  const open = list
    .filter((j) => j.done_at === null || settling.has(j.id))
    .sort((a, b) => b.created_at - a.created_at || b.id - a.id);
  const handled = list
    .filter((j) => j.done_at !== null && !settling.has(j.id))
    .sort((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0) || b.id - a.id);
  return { open, handled };
}

/** While a focus round is running the list folds into a stack: write things
 *  down, but don't start reading the pile. Paused or on a break, it opens. */
export function foldsInFocus(state: Pick<TimerState, 'round_type' | 'is_running'>): boolean {
  return state.round_type === 'work' && state.is_running;
}

const pad = (n: number) => String(n).padStart(2, '0');
const EN_MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "14:32" today, "昨天 14:32" / "Yesterday 14:32", "9月26日" / "Sep 26" this year, else with the year. */
export function jotWhen(unixSecs: number, now: Date, zh: boolean): string {
  const at = new Date(unixSecs * 1000);
  const time = `${pad(at.getHours())}:${pad(at.getMinutes())}`;
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const daysAgo = Math.round((day(now) - day(at)) / 86_400_000);
  if (daysAgo <= 0) return time;
  if (daysAgo === 1) return zh ? `昨天 ${time}` : `Yesterday ${time}`;
  const sameYear = at.getFullYear() === now.getFullYear();
  if (zh) return `${sameYear ? '' : `${at.getFullYear()}年`}${at.getMonth() + 1}月${at.getDate()}日`;
  return `${EN_MONTHS[at.getMonth()]} ${at.getDate()}${sameYear ? '' : `, ${at.getFullYear()}`}`;
}

/** Titlebar badge: 1–9, then "9+". Empty when there is nothing open. */
export function badgeText(openCount: number): string {
  return openCount <= 0 ? '' : openCount > 9 ? '9+' : String(openCount);
}

/** Enter writes the jot; Shift+Enter is a new line; Enter that confirms an IME
 *  candidate (Chinese input) must not send half-typed text. */
export function isSendKey(e: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'isComposing' | 'keyCode'>): boolean {
  return e.key === 'Enter' && !e.shiftKey && !e.isComposing && e.keyCode !== 229;
}
