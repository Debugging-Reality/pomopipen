import type { WeekEvent } from '../types';
import type { DayTotal } from './calendar';

export interface Share {
  /** Subject id, `null` for uncategorized, `'other'` for the folded tail. */
  id: number | null | 'other';
  name: string | null;
  color: string | null;
  seconds: number;
  fraction: number;
}

/** Focus time per subject summed over `days` (from `dailyTotals`), largest first;
 *  `events` supply names and colors. Beyond `maxSlices` the smallest subjects
 *  fold into one `'other'` slice. */
export function subjectShares(events: WeekEvent[], days: DayTotal[], maxSlices = 6): Share[] {
  const info = new Map<number | null, { name: string | null; color: string | null }>();
  for (const e of events) info.set(e.subject_id ?? null, { name: e.subject_name, color: e.subject_color });
  const totals = new Map<number | null, number>();
  for (const day of days) {
    for (const [id, seconds] of day.bySubject) totals.set(id, (totals.get(id) ?? 0) + seconds);
  }
  const sum = [...totals.values()].reduce((a, b) => a + b, 0);
  const rows: Share[] = [...totals]
    .filter(([, seconds]) => seconds > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([id, seconds]) => ({ id, ...(info.get(id) ?? { name: null, color: null }), seconds, fraction: seconds / sum }));
  if (rows.length <= maxSlices) return rows;
  const tail = rows.slice(maxSlices - 1);
  const seconds = tail.reduce((a, r) => a + r.seconds, 0);
  return [...rows.slice(0, maxSlices - 1), { id: 'other', name: null, color: null, seconds, fraction: seconds / sum }];
}

/** SVG path for a donut segment; `from`/`to` are fractions of a turn, clockwise from 12 o'clock. */
export function donutArc(cx: number, cy: number, inner: number, outer: number, from: number, to: number): string {
  if (to - from >= 0.9999) {
    return donutArc(cx, cy, inner, outer, from, from + 0.5) + donutArc(cx, cy, inner, outer, from + 0.5, to);
  }
  const point = (r: number, t: number) =>
    `${(cx + r * Math.sin(2 * Math.PI * t)).toFixed(2)},${(cy - r * Math.cos(2 * Math.PI * t)).toFixed(2)}`;
  const large = to - from > 0.5 ? 1 : 0;
  return `M${point(outer, from)}A${outer},${outer} 0 ${large} 1 ${point(outer, to)}` +
    `L${point(inner, to)}A${inner},${inner} 0 ${large} 0 ${point(inner, from)}Z`;
}
