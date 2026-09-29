// Pattern pictures saved from the web are cut at arbitrary points, so tiling
// them as-is shows seams. This finds the pattern's own repeat distance and cuts
// exactly one repeat out of the picture, which then tiles without seams.

export interface PatternTile {
  url: string;
  /** Tile size in picture pixels. */
  width: number;
  height: number;
  naturalWidth: number;
  naturalHeight: number;
  /** False when no repeat was found and the whole picture is tiled. */
  seamless: boolean;
}

/**
 * Smallest shift along `axis` that maps a luminance image onto itself, or null
 * when the picture does not repeat that way. The error curve of a repeating
 * picture rises from zero shift and dips back near zero at each repeat; the
 * first clear dip is the period.
 */
export function findPeriod(lum: ArrayLike<number>, w: number, h: number, axis: 'x' | 'y', range?: [number, number]): number | null {
  const length = axis === 'x' ? w : h;
  const across = axis === 'x' ? h : w;
  // Index of pixel i along the axis in line j across it: j * lineStride + i * pixelStride.
  const pixelStride = axis === 'x' ? 1 : w;
  const lineStride = axis === 'x' ? w : 1;
  let mean = 0;
  for (let k = 0; k < w * h; k++) mean += lum[k];
  mean /= w * h;
  let spread = 0;
  for (let k = 0; k < w * h; k++) spread += Math.abs(lum[k] - mean);
  spread /= w * h;
  if (spread < 1e-3) return null; // flat picture

  const lo = range?.[0] ?? Math.max(3, Math.round(length * 0.03));
  const hi = range?.[1] ?? length - Math.max(8, Math.round(length * 0.08));
  const step = Math.max(1, Math.round(across / 120));
  const errors: number[] = [];
  for (let d = lo; d <= hi; d++) {
    let sum = 0;
    let count = 0;
    const offset = d * pixelStride;
    for (let j = 0; j < across; j += step) {
      const line = j * lineStride;
      for (let i = 0, k = line; i + d < length; i++, k += pixelStride) sum += Math.abs(lum[k] - lum[k + offset]);
      count += length - d;
    }
    errors.push(count ? sum / count / spread : Infinity);
  }
  if (range) {
    // Refinement: the best shift inside the given window.
    let best = 0;
    for (let k = 1; k < errors.length; k++) if (errors[k] < errors[best]) best = k;
    return errors[best] < 0.5 ? lo + best : null;
  }
  const floor = Math.min(...errors);
  if (!(floor < 0.4)) return null;
  const limit = floor * 1.3 + 0.05;
  let peak = 0;
  for (let k = 1; k < errors.length - 1; k++) {
    peak = Math.max(peak, errors[k - 1]);
    const e = errors[k];
    // A real repeat is a dip after the error has clearly risen, not the
    // smooth start of the curve that any soft picture has.
    if (e <= limit && e <= errors[k - 1] && e <= errors[k + 1] && peak - e >= 0.25) return lo + k;
  }
  return null;
}

function luminance(img: HTMLImageElement, maxSide: number) {
  const scale = Math.min(1, maxSide / Math.max(img.naturalWidth, img.naturalHeight));
  const w = Math.max(1, Math.round(img.naturalWidth * scale));
  const h = Math.max(1, Math.round(img.naturalHeight * scale));
  const canvas = document.createElement('canvas');
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
  ctx.drawImage(img, 0, 0, w, h);
  const rgba = ctx.getImageData(0, 0, w, h).data; // throws on a tainted canvas
  const lum = new Float32Array(w * h);
  for (let k = 0; k < w * h; k++) lum[k] = (0.299 * rgba[4 * k] + 0.587 * rgba[4 * k + 1] + 0.114 * rgba[4 * k + 2]) / 255;
  return { lum, w, h, scale };
}

type Luminance = ReturnType<typeof luminance>;

function period(coarse: Luminance, fine: Luminance, axis: 'x' | 'y'): number | null {
  const rough = findPeriod(coarse.lum, coarse.w, coarse.h, axis);
  if (rough === null) return null;
  const estimate = (rough / coarse.scale) * fine.scale;
  const slack = Math.ceil(fine.scale / coarse.scale) + 2;
  const exact = findPeriod(fine.lum, fine.w, fine.h, axis, [Math.max(2, Math.floor(estimate - slack)), Math.ceil(estimate + slack)]);
  return exact === null ? null : exact / fine.scale;
}

function load(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.crossOrigin = 'anonymous'; // the asset protocol answers with CORS headers
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Cannot load ${source}`));
    img.src = source;
  });
}

async function build(source: string): Promise<PatternTile> {
  const img = await load(source);
  const naturalWidth = img.naturalWidth;
  const naturalHeight = img.naturalHeight;
  const whole = { url: source, width: naturalWidth, height: naturalHeight, naturalWidth, naturalHeight, seamless: false };
  try {
    const coarse = luminance(img, 280);
    const fine = luminance(img, 1600);
    const px = period(coarse, fine, 'x');
    const py = px === null ? null : period(coarse, fine, 'y');
    if (px === null || py === null) return whole;
    const width = Math.round(px);
    const height = Math.round(py);
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    // Cut from the middle, away from edge artifacts.
    canvas.getContext('2d')!.drawImage(img, (naturalWidth - px) / 2, (naturalHeight - py) / 2, px, py, 0, 0, width, height);
    const blob = await new Promise<Blob | null>(resolve => canvas.toBlob(resolve, 'image/png'));
    if (!blob) return whole;
    return { url: URL.createObjectURL(blob), width, height, naturalWidth, naturalHeight, seamless: true };
  } catch {
    return whole;
  }
}

const cache = new Map<string, Promise<PatternTile>>();

/** One repeat of the pattern in `source` (cached per picture). */
export function patternTile(source: string): Promise<PatternTile> {
  let tile = cache.get(source);
  if (!tile) {
    tile = build(source);
    cache.set(source, tile);
    tile.catch(() => cache.delete(source));
  }
  return tile;
}
