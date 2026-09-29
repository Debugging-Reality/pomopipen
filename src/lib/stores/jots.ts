// Jots (碎碎念) in the timer window: the list, and whether the pad is open.
// Loaded by JotPad and kept in sync through `jots:changed`, like stores/tasks.ts.

import { writable } from 'svelte/store';
import type { Jot, JotCapture } from '$lib/types';
import { jotsList, onJotsChanged } from '$lib/ipc';
import type { UnlistenFn } from '@tauri-apps/api/event';

export const jots = writable<Jot[]>([]);

/** `quick`: opened by the global shortcut from another app; after one jot the
 *  window goes back the way it was (`restore`). */
export const jotPad = writable<{ open: boolean; quick: JotCapture | null }>({ open: false, quick: null });

/** Counts up each time a jot lands in the task list (the Tasks button gulps). */
export const tasksBump = writable(0);

export function openJotPad(quick: JotCapture | null = null): void {
  jotPad.set({ open: true, quick });
}

export function closeJotPad(): void {
  jotPad.set({ open: false, quick: null });
}

export async function refreshJots(): Promise<void> {
  jots.set(await jotsList());
}

/** Load jots into the store and follow backend changes. Call from `onMount`. */
export async function watchJots(): Promise<UnlistenFn> {
  await refreshJots();
  return onJotsChanged(() => void refreshJots());
}
