// Classic Tomato: every subject color is a tomato variety. The same eight colors
// are the theme's subject palette (see collection.ts) and drive the week
// calendar's tomato blocks. Order is fixed and was checked for color-vision
// separation (docs/design/classic-tomato, §5.6); never reorder or append.

export type VarietyTexture = 'none' | 'streak' | 'zebra' | 'rib' | 'speckle' | 'dots';

export interface Variety {
  id: string;
  name: string;
  nameZh: string;
  hex: string;
  /** Text color on the skin (≥ 4.5 : 1). */
  ink: string;
  /** What sits on top of a calendar block: a star calyx or a stem with one leaf. */
  top: 'calyx' | 'stem';
  texture: VarietyTexture;
}

export const INK = '#202620';
export const PAPER_LIGHT = '#FFF8EA';

export const VARIETIES: Variety[] = [
  { id: 'red', name: 'Red', nameZh: '大红番茄', hex: '#D33526', ink: PAPER_LIGHT, top: 'calyx', texture: 'none' },
  { id: 'yellow', name: 'Golden', nameZh: '黄番茄', hex: '#E9A60C', ink: INK, top: 'calyx', texture: 'streak' },
  { id: 'indigo', name: 'Indigo', nameZh: '靛蓝番茄', hex: '#4E4AA8', ink: PAPER_LIGHT, top: 'calyx', texture: 'none' },
  { id: 'orange', name: 'Orange', nameZh: '橙番茄', hex: '#F07A22', ink: INK, top: 'stem', texture: 'speckle' },
  { id: 'zebra', name: 'Green zebra', nameZh: '绿斑马', hex: '#2B7F3E', ink: PAPER_LIGHT, top: 'calyx', texture: 'zebra' },
  { id: 'pink', name: 'Pink', nameZh: '粉番茄', hex: '#F27C8E', ink: INK, top: 'stem', texture: 'rib' },
  { id: 'green', name: 'Lime', nameZh: '青番茄', hex: '#8CC63F', ink: INK, top: 'stem', texture: 'dots' },
  { id: 'purple', name: 'Plum', nameZh: '紫番茄', hex: '#9C2F55', ink: PAPER_LIGHT, top: 'calyx', texture: 'none' },
];

export function varietyFor(hex: string | null | undefined): Variety | undefined {
  if (!hex) return undefined;
  const key = hex.toLowerCase();
  return VARIETIES.find((v) => v.hex.toLowerCase() === key);
}

/** sRGB hex → OKLab [L, a, b]. */
export function hexToOklab(hex: string): [number, number, number] {
  const [r, g, b] = [1, 3, 5]
    .map((i) => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

/** OKLab distance with lightness counted at 0.4×: variety colors have fixed
 *  lightness, and people judge "the closest tomato" by hue family first. */
export function varietyDistance(a: string, b: string): number {
  const p = hexToOklab(a);
  const q = hexToOklab(b);
  return Math.hypot(0.4 * (p[0] - q[0]), p[1] - q[1], p[2] - q[2]);
}

export interface VarietyMatch {
  variety: Variety;
  /** True when there were more subjects than free varieties and this one repeats a color. */
  shared: boolean;
}

/**
 * The closest variety for every subject whose color is not already a variety.
 * Subjects already on a variety keep it and reserve it. While enough free
 * varieties remain, the pairing with the smallest total distance wins (not a
 * greedy grab, which could push blue onto green); extra subjects reuse their
 * nearest variety.
 */
export function nearestVarieties(subjects: { id: number; color: string }[]): Map<number, VarietyMatch> {
  const valid = subjects.filter((s) => /^#[\da-f]{6}$/i.test(s.color));
  const taken = new Set(valid.filter((s) => varietyFor(s.color)).map((s) => s.color.toLowerCase()));
  const free = VARIETIES.filter((v) => !taken.has(v.hex.toLowerCase()));
  const todo = valid.filter((s) => !varietyFor(s.color));
  const nearest = (color: string, pool: Variety[]) =>
    pool.reduce((best, v) => (varietyDistance(color, v.hex) < varietyDistance(color, best.hex) ? v : best));

  const uniq =
    todo.length <= free.length
      ? todo
      : [...todo]
          .sort((a, b) => varietyDistance(a.color, nearest(a.color, free).hex) - varietyDistance(b.color, nearest(b.color, free).hex))
          .slice(0, free.length);

  // Exhaustive search: at most 8 subjects × 8 varieties.
  const cost = uniq.map((s) => free.map((v) => varietyDistance(s.color, v.hex) ** 2));
  let best: number[] = [];
  let bestCost = Infinity;
  const used = free.map(() => false);
  const pick: number[] = [];
  const search = (i: number, total: number) => {
    if (total >= bestCost) return;
    if (i === uniq.length) {
      bestCost = total;
      best = [...pick];
      return;
    }
    for (let j = 0; j < free.length; j++) {
      if (used[j]) continue;
      used[j] = true;
      pick.push(j);
      search(i + 1, total + cost[i][j]);
      pick.pop();
      used[j] = false;
    }
  };
  search(0, 0);

  const out = new Map<number, VarietyMatch>();
  uniq.forEach((s, i) => out.set(s.id, { variety: free[best[i]], shared: false }));
  for (const s of todo) {
    if (!out.has(s.id)) out.set(s.id, { variety: nearest(s.color, VARIETIES), shared: true });
  }
  return out;
}

/**
 * The tomato every subject is drawn as in the Classic Tomato week calendar:
 * its own variety when its color is one, otherwise the variety it would get
 * from "switch to the closest varieties" — so the calendar always shows real
 * variety colors, and converting later changes nothing on screen.
 */
export function subjectVarieties(subjects: { id: number; color: string }[]): Map<number, Variety> {
  const out = new Map<number, Variety>();
  const matches = nearestVarieties(subjects);
  for (const s of subjects) {
    const variety = varietyFor(s.color) ?? matches.get(s.id)?.variety;
    if (variety) out.set(s.id, variety);
  }
  return out;
}
