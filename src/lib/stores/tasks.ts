// Reactive tasks store.
// Each window loads its own copy and keeps it in sync via the
// `tasks:changed` event, mirroring `stores/subjects.ts`.

import { writable } from 'svelte/store';
import type { Task } from '$lib/types';
import { tasksList, onTasksChanged } from '$lib/ipc';
import type { UnlistenFn } from '@tauri-apps/api/event';

export const tasks = writable<Task[]>([]);

/**
 * Load tasks into the store and subscribe to backend changes.
 * Call from `onMount`; call the returned unlisten function on teardown.
 */
export async function watchTasks(includeDone = false): Promise<UnlistenFn> {
  tasks.set(await tasksList(includeDone));
  return onTasksChanged(async () => {
    tasks.set(await tasksList(includeDone));
  });
}
