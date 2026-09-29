// Classic Tomato mechanical timer: the printed scale on the rotating drum.
// One revolution = the round's duration T; the value under the fixed pointer
// is the time remaining. See docs/design/classic-tomato §2.2.

export interface DrumScale {
  /** Minutes between numbered (major) marks. */
  step: number;
  /** Numbered values, 0 first; T itself shares the 0 position (the seam). */
  labels: number[];
  /** Minutes between fine ticks — a visual division, not a promise of one minute each. */
  fine: number;
}

const STEPS = [1, 2, 3, 5, 10, 15, 20, 30];

/** Scale layout for a round of `T` minutes: at most 6 numbers and 60 fine ticks. */
export function drumScale(T: number): DrumScale {
  const step = STEPS.find((s) => Math.ceil(T / s - 1e-9) <= 6) ?? 30;
  const labels: number[] = [];
  for (let v = 0; v < T - 1e-9; v += step) labels.push(v);
  // A last number squeezed right up against the 0/T seam reads as noise.
  if (labels.length > 4 && T - labels[labels.length - 1] < step * 0.4) labels.pop();
  const fine = [step / 10, step / 5, step / 2, step].find((f) => T / f <= 60 + 1e-9) ?? step;
  return { step, labels, fine };
}

/** Angle of scale value `v` (minutes) relative to the pointer, in radians,
 *  wrapped to (-π, π]. Positive = right of the pointer; ±π/2 = the drum's edge. */
export function drumAngle(v: number, remainingMin: number, T: number, extra = 0): number {
  let a = (2 * Math.PI * (remainingMin - v)) / T + extra;
  a = (a + Math.PI) % (2 * Math.PI);
  if (a < 0) a += 2 * Math.PI;
  return a - Math.PI;
}

export const isMultiple = (v: number, m: number) => Math.abs(v / m - Math.round(v / m)) < 1e-6;

/** Settings key holding the duration of a round type. */
export function durationKey(roundType: string): 'time_work_secs' | 'time_short_break_secs' | 'time_long_break_secs' {
  return roundType === 'work' ? 'time_work_secs' : roundType === 'short-break' ? 'time_short_break_secs' : 'time_long_break_secs';
}

export const MIN_DURATION_SECS = 60;
export const MAX_DURATION_SECS = 90 * 60;

/** Parse "30", "30:00" or "7:30" into seconds, clamped to the app's 1–90 minute range. */
export function parseDuration(input: string): number | null {
  const text = input.trim();
  let secs: number | null = null;
  if (/^\d{1,3}$/.test(text)) secs = Number(text) * 60;
  else {
    const m = text.match(/^(\d{1,3})[:：](\d{1,2})$/);
    if (m && Number(m[2]) < 60) secs = Number(m[1]) * 60 + Number(m[2]);
  }
  if (secs === null || secs <= 0) return null;
  return Math.max(MIN_DURATION_SECS, Math.min(MAX_DURATION_SECS, secs));
}
