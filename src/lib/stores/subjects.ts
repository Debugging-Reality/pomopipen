// Reactive subjects store.
// Each window loads its own copy (Tauri windows do not share JS memory) and
// keeps it in sync via the `subjects:changed` event, mirroring how
// `stores/settings.ts` is hydrated and refreshed per-window.

import { writable } from 'svelte/store';
import type { Subject } from '$lib/types';
import { subjectsList, onSubjectsChanged } from '$lib/ipc';
import type { UnlistenFn } from '@tauri-apps/api/event';

export const subjects = writable<Subject[]>([]);

/**
 * Load subjects into the store and subscribe to backend changes.
 * Call from `onMount`; call the returned unlisten function on teardown.
 */
export async function watchSubjects(includeArchived = false): Promise<UnlistenFn> {
  subjects.set(await subjectsList(includeArchived));
  return onSubjectsChanged(async () => {
    subjects.set(await subjectsList(includeArchived));
  });
}
