<script lang="ts" module>
  // A flattened five-sepal calyx seen from the side, in a 24 × 12 box.
  const SEPALS = [-165, -15, 30, 90, 150];
  const points: string[] = [];
  SEPALS.forEach((a, i) => {
    const next = SEPALS[(i + 1) % SEPALS.length] + (i === SEPALS.length - 1 ? 360 : 0);
    const mid = (((a + next) / 2) * Math.PI) / 180;
    const r = (a * Math.PI) / 180;
    points.push(`${(12 + 11.5 * Math.cos(r)).toFixed(2)} ${(5 + 6 * Math.sin(r)).toFixed(2)}`);
    points.push(`${(12 + 2.6 * Math.cos(mid)).toFixed(2)} ${(5 + 1.6 * Math.sin(mid)).toFixed(2)}`);
  });
  const CALYX = `M${points.join(' L')}Z`;
</script>

<script lang="ts">
  // The top of a Classic Tomato calendar block (or a small legend tomato):
  // a star calyx sitting on the edge, or a stem with one leaf rising above it.
  interface Props {
    kind: 'calyx' | 'stem';
    /** Width in px; the calyx is half as tall, the stem 0.7×. */
    width: number;
    /** Green skins get a dark calyx with a light edge so it still shows. */
    onGreen?: boolean;
    /** Outline-only calyx for uncategorised blocks. */
    outline?: boolean;
  }

  let { kind, width, onGreen = false, outline = false }: Props = $props();
</script>

{#if kind === 'calyx'}
  <svg class="top calyx" width={width} height={width / 2} viewBox="0 0 24 12" aria-hidden="true">
    <path
      d={CALYX}
      fill={outline ? 'none' : onGreen ? '#1C3A1F' : '#2D5A2E'}
      stroke={outline ? '#202620' : onGreen ? 'rgb(255 248 234 / 75%)' : 'none'}
      stroke-width={outline ? 1.2 : 0.9}
      stroke-linejoin="round"
    />
    <rect x="10.9" y="0" width="2.2" height="4.6" rx="1" fill={outline ? '#202620' : '#4B5A2A'} />
  </svg>
{:else}
  <svg class="top stem" width={width} height={width * 0.7} viewBox="0 0 20 14" aria-hidden="true">
    <path d="M7 14 C 7 9, 7.5 6, 9 2.5" fill="none" stroke="#5A4A26" stroke-width="2.2" stroke-linecap="round" />
    <path d="M8.6 6.4 C 11 1.8, 15.5 0.6, 19.4 1.2 C 17.6 5, 13.2 7.6, 8.6 6.4 Z" fill="#3F7A3A" />
    <path d="M9.5 5.9 C 12.5 4.2, 15 3, 18 1.8" fill="none" stroke="#2A5A2A" stroke-width=".7" />
  </svg>
{/if}

<style>
  .top {
    display: block;
    pointer-events: none;
  }
</style>
