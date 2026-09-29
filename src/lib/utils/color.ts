/** Relative sRGB luminance. Invalid custom colors fall back to a readable neutral. */
export function luminance(hex: string): number {
  if (!/^#[\da-f]{6}$/i.test(hex)) return 0.5;
  const channels = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
    .map(c => c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
}

export function readableInk(hex: string): string {
  const value = luminance(hex);
  if ((value + 0.05) / (luminance('#17191D') + 0.05) >= 4.5) return '#17191D';
  if (1.05 / (value + 0.05) >= 4.5) return '#FFFFFF';
  return '#000000';
}

/** Preserve hue while ensuring at least 4.5:1 on the actual solid surface. */
export function readableAccent(hex: string, background: string): string {
  if (!/^#[\da-f]{6}$/i.test(hex) || !/^#[\da-f]{6}$/i.test(background)) return hex;
  const bg = luminance(background);
  const channels = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
  const target = bg > 0.179 ? 0 : 255;
  for (let step = 0; step <= 100; step++) {
    const color = '#' + channels.map(c => Math.round(c + (target - c) * step / 100).toString(16).padStart(2, '0')).join('');
    const lum = luminance(color);
    if ((Math.max(lum, bg) + 0.05) / (Math.min(lum, bg) + 0.05) >= 4.5) return color;
  }
  return readableInk(background);
}

/** Keep event text within its selected theme when contrast allows. */
export function themedInk(background: string, colors: Record<string, string> | null): string {
  const candidates = [colors?.['--pomo-ink'], colors?.['--pomo-on'], colors?.['--color-foreground']];
  const bg = luminance(background);
  for (const color of candidates) {
    if (!color || !/^#[\da-f]{6}$/i.test(color)) continue;
    const fg = luminance(color);
    if ((Math.max(bg, fg) + .05) / (Math.min(bg, fg) + .05) >= 4.5) return color;
  }
  return colors?.['--pomo-ink'] ? readableAccent(colors['--pomo-ink'], background) : readableInk(background);
}
