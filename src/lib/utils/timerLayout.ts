// Sizing of the main timer window content, shared by the app and the preview.

/** Default window size; the full layout is designed for it at scale 1. */
export const BASE_W = 360;
export const BASE_H = 478;
export const TITLEBAR_H = 40;
/** Below this full-layout scale, only the dial and a mini control row are shown. */
export const COMPACT_SCALE = 0.7;
/** Space reserved under the compact dial for the mini controls. */
const COMPACT_BOTTOM_PAD = 48;

export interface TimerLayout {
  compact: boolean;
  /** Zoom for the rendered timer (the whole layout, or the dial alone when compact). */
  uiScale: number;
}

/** Width at which a picture of this aspect ratio exactly covers the content
 *  area of a default-size window — the 100 % size of a tiled background. */
export function coverTileWidth(naturalWidth: number, naturalHeight: number): number {
  if (!(naturalWidth > 0 && naturalHeight > 0)) return BASE_W;
  return Math.max(BASE_W, ((BASE_H - TITLEBAR_H) * naturalWidth) / naturalHeight);
}

export function timerLayout(width: number, height: number): TimerLayout {
  const fullScale = Math.min(width / BASE_W, (height - TITLEBAR_H) / (BASE_H - TITLEBAR_H));
  if (fullScale >= COMPACT_SCALE) {
    return { compact: false, uiScale: Math.max(0.5, Math.min(fullScale, 4)) };
  }
  // The dial box is 220px plus its 8px offset shadow.
  const available = Math.min(width - 24, height - TITLEBAR_H - 16 - COMPACT_BOTTOM_PAD);
  return { compact: true, uiScale: Math.max(0.4, Math.min(available / 228, 4)) };
}
