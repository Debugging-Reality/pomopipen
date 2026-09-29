// Short confirmation messages ("已添加", "已保存") shown at the bottom of a window.
import { writable } from 'svelte/store';

export interface Toast {
  id: number;
  text: string;
  tone: 'ok' | 'error';
  /** Optional one-click follow-up, e.g. undo. */
  action?: { label: string; run: () => void };
}

export const toasts = writable<Toast[]>([]);
let next = 1;

export function notify(text: string, options: { tone?: Toast['tone']; action?: Toast['action']; ms?: number } = {}): void {
  const toast: Toast = { id: next++, text, tone: options.tone ?? 'ok', action: options.action };
  toasts.update(list => [...list.slice(-2), toast]);
  setTimeout(() => dismiss(toast.id), options.ms ?? (options.action ? 5000 : 2200));
}

export function dismiss(id: number): void {
  toasts.update(list => list.filter(t => t.id !== id));
}
