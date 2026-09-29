// Per-theme background images, stored as JSON in the `theme_backgrounds`
// setting: { "<theme name>": { timer?: BackgroundChoice, calendar?: BackgroundChoice } }.
// Each color scheme keeps its own pictures; switching themes never shares them.

export type BackgroundTarget = 'timer' | 'calendar';
/** `tile` repeats the picture at a fixed size (patterns); `cover` fills and crops (photos). */
export type BackgroundFit = 'tile' | 'cover';

export interface BackgroundChoice {
  path: string;
  opacity: number; // 0–100
  fit: BackgroundFit;
  scale: number; // tile size in percent; 100 % covers a default-size timer window once
}

export type ThemeBackgrounds = Record<string, Partial<Record<BackgroundTarget, BackgroundChoice>>>;

export const SCALE_RANGE = [25, 300] as const;

const DEFAULTS: Omit<BackgroundChoice, 'path'> = { opacity: 60, fit: 'tile', scale: 100 };

function clamp(value: unknown, min: number, max: number, fallback: number): number {
  const n = Number(value);
  return Number.isFinite(n) ? Math.min(max, Math.max(min, Math.round(n))) : fallback;
}

export function parseBackgrounds(json: string | null | undefined): ThemeBackgrounds {
  try {
    const value = JSON.parse(json || '{}');
    return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
  } catch {
    return {};
  }
}

/** The normalized choice for one theme and target, or null when it has no picture. */
export function backgroundFor(
  all: ThemeBackgrounds,
  theme: string | null | undefined,
  target: BackgroundTarget
): BackgroundChoice | null {
  const raw = theme ? all[theme]?.[target] : undefined;
  if (!raw || typeof raw.path !== 'string' || !raw.path) return null;
  return {
    path: raw.path,
    opacity: clamp(raw.opacity, 0, 100, DEFAULTS.opacity),
    fit: raw.fit === 'cover' ? 'cover' : 'tile',
    scale: clamp(raw.scale, SCALE_RANGE[0], SCALE_RANGE[1], DEFAULTS.scale),
  };
}

/** Returns a copy with one theme/target changed. `null` removes that picture. */
export function withBackground(
  all: ThemeBackgrounds,
  theme: string,
  target: BackgroundTarget,
  patch: Partial<BackgroundChoice> | null
): ThemeBackgrounds {
  const next: ThemeBackgrounds = { ...all, [theme]: { ...all[theme] } };
  if (patch === null) {
    delete next[theme][target];
  } else {
    const current = backgroundFor(all, theme, target) ?? { path: '', ...DEFAULTS };
    next[theme][target] = { ...current, ...patch };
  }
  if (Object.keys(next[theme]).length === 0) delete next[theme];
  return next;
}
