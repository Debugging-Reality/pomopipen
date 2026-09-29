// Theme store.
// Applies theme colors to CSS custom properties on :root.

import type { Theme } from '$lib/types';
import { writable } from 'svelte/store';
import { readableAccent, readableInk } from '$lib/utils/color';
import { collectionFor } from '$lib/themes/collection';

export const activeTheme = writable<Theme | null>(null);
let appliedKeys: string[] = [];

/** Apply a theme's colors to the document root CSS custom properties.
 *  Theme keys already include the `--` prefix (e.g. "--color-background"). */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  for (const key of appliedKeys) root.style.removeProperty(key);
  for (const [key, value] of Object.entries(theme.colors)) {
    root.style.setProperty(key, value);
  }
  appliedKeys = Object.keys(theme.colors);
  const collectionTheme = collectionFor(theme);
  if (collectionTheme) root.dataset.pomoTheme = collectionTheme.id;
  else delete root.dataset.pomoTheme;
  for (const round of ['focus', 'short', 'long']) {
    const color = theme.colors[`--color-${round}-round`] ?? '#72251F';
    root.style.setProperty(`--ink-${round}-round`, readableInk(color));
    root.style.setProperty(`--text-${round}-round`, readableAccent(color, theme.colors['--color-background'] ?? '#FFFFFF'));
  }
  activeTheme.set(theme);
}
