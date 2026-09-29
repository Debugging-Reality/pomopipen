<script lang="ts" module>
  // Layers shared by every drum on the page (docs/design/classic-tomato §2.1, §06).
  // The shell, crown, pointer, gloss and the blank drum body never move; only
  // the printed scale is redrawn, projected onto the drum's cylinder.
  type Layers = { body: HTMLImageElement; mask: HTMLImageElement; gloss: HTMLImageElement };
  let layers: Promise<Layers> | null = null;
  function loadLayers(): Promise<Layers> {
    const load = (src: string) =>
      new Promise<HTMLImageElement>((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = reject;
        img.src = src;
      });
    layers ??= Promise.all([
      load('/classic-tomato/a-body.png'),
      load('/classic-tomato/a-drum-mask.png'),
      load('/classic-tomato/a-gloss.png'),
    ]).then(([body, mask, gloss]) => ({ body, mask, gloss }));
    return layers;
  }

  // Asset space (470 × 384) and the ellipse model fitted to the reference photo:
  // a horizontal circle on the drum projects to x = cx + R sin θ, y = y0 − k·R(1 − cos θ).
  const W = 470;
  const H = 384;
  const CX = 228.5; // under the pointer tip
  const K = 0.219; // camera elevation
  const TICKS = { R: 204, y: 235 };
  const NUMS = { R: 194, y: 272, size: 30 };
  /** Pixels of pointer travel per minute while twisting. */
  export const DRAG_PX_PER_MIN = 12;
</script>

<script lang="ts">
  import { onMount } from 'svelte';
  import { drumScale, drumAngle, isMultiple } from '$lib/utils/drumScale';

  interface Props {
    /** Round length and time left, in seconds (remaining may be fractional). */
    totalSecs: number;
    remainingSecs: number;
    /** CSS width; the height follows the artwork. */
    width: number;
    /** Extra backing-store scale for an ancestor CSS zoom. */
    zoom?: number;
    compact?: boolean;
    /** Idle: twisting the drum (or arrow keys) sets the duration. */
    adjustable?: boolean;
    reduced?: boolean;
    /** Completion press, 0.98…1. */
    press?: number;
    label: string;
    /** New duration in whole minutes (1–90). */
    onadjust?: (minutes: number) => void;
    /** Live candidate while twisting; null when the twist ends. */
    ondrag?: (minutes: number | null) => void;
  }

  let {
    totalSecs,
    remainingSecs,
    width,
    zoom = 1,
    compact = false,
    adjustable = false,
    reduced = false,
    press = 1,
    label,
    onadjust,
    ondrag,
  }: Props = $props();

  let canvas = $state<HTMLCanvasElement>();
  let images = $state<Layers | null>(null);
  let dpr = $state(typeof window === 'undefined' ? 1 : window.devicePixelRatio || 1);
  // Twist state: extra rotation of the printed scale and how visible its numbers are.
  let twist = $state(0);
  let numberAlpha = $state(1);
  let drag: { x0: number; minutes0: number; minutes: number; pointer: number } | null = null;
  let spring = 0;

  let scale = $derived(width / W);
  let height = $derived(H * scale);

  onMount(() => {
    loadLayers().then((l) => (images = l)).catch(() => (images = null));
    const media = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    const update = () => (dpr = window.devicePixelRatio || 1);
    media.addEventListener('change', update);
    return () => {
      media.removeEventListener('change', update);
      cancelAnimationFrame(spring);
    };
  });

  $effect(() => {
    if (!canvas || !images) return;
    draw(canvas, images, {
      T: Math.max(1, totalSecs) / 60,
      r: Math.max(0, remainingSecs) / 60,
      twist,
      numberAlpha,
      press,
      pixel: dpr * zoom,
    });
  });

  function draw(cv: HTMLCanvasElement, img: Layers, o: { T: number; r: number; twist: number; numberAlpha: number; press: number; pixel: number }) {
    const pw = Math.round(width * o.pixel);
    const ph = Math.round(height * o.pixel);
    if (cv.width !== pw || cv.height !== ph) {
      cv.width = pw;
      cv.height = ph;
    }
    const S = scale * o.pixel;
    const ctx = cv.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, pw, ph);
    // The press scales the whole object about its base; nothing else moves.
    const P = S * o.press;
    const ox = S * (1 - o.press) * CX;
    const oy = S * (1 - o.press) * 370;

    // Contact shadow: short, warm, shifted right for the upper-left light.
    ctx.setTransform(P, 0, 0, P, ox, oy);
    ctx.save();
    ctx.translate(240, 369);
    ctx.scale(1, 0.065);
    const shadow = ctx.createRadialGradient(0, 0, 0, 0, 0, 196);
    shadow.addColorStop(0, 'rgba(78,44,20,.34)');
    shadow.addColorStop(0.55, 'rgba(78,44,20,.13)');
    shadow.addColorStop(1, 'rgba(78,44,20,0)');
    ctx.fillStyle = shadow;
    ctx.beginPath();
    ctx.arc(0, 0, 196, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
    ctx.drawImage(img.body, 0, 0);

    // Printed scale on its own layer, clipped to the visible drum band.
    const off = document.createElement('canvas');
    off.width = pw;
    off.height = ph;
    const o2 = off.getContext('2d');
    if (!o2) return;
    const place = (theta: number, R: number, y0: number) => {
      const c = Math.cos(theta);
      const sn = Math.sin(theta);
      const x = CX + R * sn;
      const y = y0 - K * R * (1 - c);
      // Surface frame: horizontal follows the ellipse tangent, vertical stays vertical.
      o2.setTransform(P * c, P * -K * sn, 0, P, ox + P * x, oy + P * y);
      return c;
    };
    const ink = (c: number, a: number) =>
      `rgba(255,${Math.round(248 - 26 * (1 - c))},${Math.round(243 - 40 * (1 - c))},${a.toFixed(3)})`;
    const fade = (a: number, b: number, x: number) => {
      const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
      return t * t * (3 - 2 * t);
    };

    const spec = drumScale(o.T);
    const count = Math.round(o.T / spec.fine);
    for (let i = 0; i < count; i++) {
      const v = i * spec.fine;
      const major = isMultiple(v, spec.step);
      const mid = !major && isMultiple(v, spec.step / 2);
      if (compact && !major) continue;
      const theta = drumAngle(v, o.r, o.T, o.twist);
      const c = Math.cos(theta);
      if (c < 0.03) continue;
      place(theta, TICKS.R, TICKS.y);
      const len = major ? 19 : mid ? 13 : 9;
      const w = major ? 5.5 : 3;
      o2.fillStyle = ink(c, fade(0.03, 0.32, c) * 0.97);
      o2.fillRect(-w / 2, 0, w, len);
    }
    // Compact drums keep numbers readable (≥ 9.5 CSS px) and show only the front ones.
    const size = compact ? Math.max(NUMS.size, 9.5 / scale) : NUMS.size;
    const cMin = compact ? 0.55 : 0.05;
    const cFull = compact ? 0.8 : 0.4;
    o2.font = `600 ${size}px "Mona Sans", Arial, sans-serif`;
    o2.textAlign = 'center';
    o2.textBaseline = 'middle';
    for (const v of spec.labels) {
      const theta = drumAngle(v, o.r, o.T, o.twist);
      const c = Math.cos(theta);
      if (c < cMin) continue;
      place(theta, NUMS.R, NUMS.y + (compact ? 3 : 0));
      o2.fillStyle = ink(c, fade(cMin, cFull, c) * o.numberAlpha);
      o2.fillText(String(v), 0, 0);
    }
    o2.setTransform(P, 0, 0, P, ox, oy);
    o2.globalCompositeOperation = 'destination-in';
    o2.drawImage(img.mask, 0, 0);

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.drawImage(off, 0, 0);
    // Gloss stays put above the numbers: light from the upper left, never rotating.
    ctx.setTransform(P, 0, 0, P, ox, oy);
    ctx.globalCompositeOperation = 'screen';
    ctx.globalAlpha = 0.5;
    ctx.drawImage(img.gloss, 0, 0);
    ctx.globalAlpha = 1;
    ctx.globalCompositeOperation = 'source-over';
  }

  // ── Twist to set the duration (idle only) ───────────────────────────────
  const clampMinutes = (m: number) => Math.max(1, Math.min(90, m));

  function onDrumBand(ev: PointerEvent): boolean {
    if (!canvas) return false;
    const rect = canvas.getBoundingClientRect();
    const y = ((ev.clientY - rect.top) / rect.height) * H;
    return y > 222 && y < 360;
  }

  function pointerdown(ev: PointerEvent) {
    if (!adjustable || !onDrumBand(ev)) return;
    cancelAnimationFrame(spring);
    canvas?.setPointerCapture(ev.pointerId);
    const minutes = Math.round(totalSecs / 60);
    drag = { x0: ev.clientX, minutes0: minutes, minutes, pointer: ev.pointerId };
    numberAlpha = 0.35;
    ondrag?.(minutes);
  }

  function pointermove(ev: PointerEvent) {
    if (!canvas) return;
    if (!drag) {
      canvas.style.cursor = adjustable && onDrumBand(ev) ? 'ew-resize' : '';
      return;
    }
    // The drawing scale already includes any CSS zoom, so measure in screen pixels.
    const rect = canvas.getBoundingClientRect();
    const screenPerAsset = rect.width / W;
    const steps = Math.round((ev.clientX - drag.x0) / DRAG_PX_PER_MIN);
    const minutes = clampMinutes(drag.minutes0 + steps);
    if (minutes !== drag.minutes) {
      drag.minutes = minutes;
      ondrag?.(minutes);
    }
    // Ratchet: the surface moves one detent per minute, right = more time.
    twist = ((minutes - drag.minutes0) * DRAG_PX_PER_MIN) / (NUMS.R * screenPerAsset);
  }

  function pointerup() {
    if (!drag) return;
    const { minutes, minutes0 } = drag;
    drag = null;
    ondrag?.(null);
    if (minutes !== minutes0) onadjust?.(minutes);
    springBack();
  }

  /** Return to the seam and bring the (re-laid-out) numbers back. */
  function springBack() {
    const from = twist;
    if (reduced || from === 0) {
      twist = 0;
      numberAlpha = 1;
      return;
    }
    const start = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / 220);
      twist = from * (1 - (1 - (1 - t) ** 3));
      numberAlpha = 0.35 + 0.65 * Math.min(1, (now - start) / 150);
      if (t < 1) spring = requestAnimationFrame(step);
      else {
        twist = 0;
        numberAlpha = 1;
      }
    };
    spring = requestAnimationFrame(step);
  }

  function keydown(ev: KeyboardEvent) {
    if (!adjustable) return;
    const delta = ({ ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1, PageUp: 5, PageDown: -5 } as Record<string, number>)[ev.key];
    if (!delta) return;
    ev.preventDefault();
    ev.stopPropagation(); // arrow keys are also app shortcuts (reset/skip)
    const minutes = clampMinutes(Math.round(totalSecs / 60) + delta);
    if (minutes !== Math.round(totalSecs / 60)) onadjust?.(minutes);
  }
</script>

<canvas
  bind:this={canvas}
  class="drum"
  class:adjustable
  style:width="{width}px"
  style:height="{height}px"
  role={adjustable ? 'slider' : 'img'}
  tabindex={adjustable ? 0 : -1}
  aria-label={label}
  aria-valuemin={adjustable ? 1 : undefined}
  aria-valuemax={adjustable ? 90 : undefined}
  aria-valuenow={adjustable ? Math.round(totalSecs / 60) : undefined}
  onpointerdown={pointerdown}
  onpointermove={pointermove}
  onpointerup={pointerup}
  onpointercancel={pointerup}
  onkeydown={keydown}
></canvas>

<style>
  .drum {
    display: block;
    touch-action: none;
  }

  .drum:focus {
    outline: none;
  }

  .drum.adjustable:focus-visible {
    outline: 2px solid var(--navy, #203d65);
    outline-offset: 3px;
    border-radius: 6px;
  }
</style>
