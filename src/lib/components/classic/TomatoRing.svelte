<script lang="ts">
  // Classic Tomato, appearance B: a still, real tomato inside a countdown ring.
  // The ring starts at 12 o'clock; used time opens clockwise and the colored
  // remainder shrinks. The fruit never moves or changes color (§2.5).

  interface Props {
    /** Share of the round already used, 0…1. */
    elapsed: number;
    /** Phase ink for the arc. */
    color: string;
    /** Stage size and fruit silhouette (fruit + calyx tips) in CSS px. */
    size: number;
    fruit: number;
    gap?: number;
    stroke?: number;
    ticks?: number;
    /** false = the existing "count up" dial setting: draw the used arc instead. */
    countdown?: boolean;
    label: string;
  }

  let { elapsed, color, size, fruit, gap = 12, stroke = 4.5, ticks = 12, countdown = true, label }: Props = $props();

  // The artwork is 404 px square with a 392 px silhouette.
  const ART = 404 / 392;
  let c = $derived(size / 2);
  let r = $derived(fruit / 2 + gap + stroke / 2);
  let e = $derived(Math.min(1, Math.max(0, elapsed)));
  // pathLength = 1 keeps the dash maths independent of the radius.
  let arc = $derived(countdown ? { length: 1 - e, offset: -e } : { length: e, offset: 0 });
  // Big jumps (a new round) snap instead of sweeping back around the dial.
  let lastE = 0;
  let animate = $state(true);
  $effect.pre(() => {
    animate = Math.abs(e - lastE) < 0.05;
    lastE = e;
  });
  // The marker sits where used time meets the rest, whichever part is drawn.
  let markerAngle = $derived(e * 360);
  let marks = $derived(
    Array.from({ length: ticks }, (_, i) => {
      const a = (i / ticks) * 2 * Math.PI - Math.PI / 2;
      const r1 = r + stroke / 2 + 2.5;
      const r2 = r1 + (i % (ticks / 4) === 0 ? 5 : 3);
      return { x1: c + r1 * Math.cos(a), y1: c + r1 * Math.sin(a), x2: c + r2 * Math.cos(a), y2: c + r2 * Math.sin(a) };
    }),
  );
</script>

<svg class="ring" class:animate width={size} height={size} viewBox="0 0 {size} {size}" role="img" aria-label={label}>
  {#each marks as m}
    <line x1={m.x1} y1={m.y1} x2={m.x2} y2={m.y2} class="mark" />
  {/each}
  <circle cx={c} cy={c} {r} class="track" stroke-width={stroke} />
  {#if arc.length > 0}
    <circle
      cx={c}
      cy={c}
      {r}
      class="arc"
      pathLength="1"
      stroke={color}
      stroke-width={stroke}
      stroke-dasharray="{arc.length} 1"
      stroke-dashoffset={arc.offset}
      transform="rotate(-90 {c} {c})"
    />
  {/if}
  <image
    href="/classic-tomato/b-fruit.png"
    x={c - (fruit * ART) / 2}
    y={c - (fruit * ART) / 2}
    width={fruit * ART}
    height={fruit * ART}
    class="fruit"
  />
  {#if e > 0 && e < 1}
    <g class="marker" style:transform="rotate({markerAngle}deg)" style:transform-origin="{c}px {c}px">
      <circle cx={c} cy={c - r} r={stroke * 0.85 + 1.6} fill={color} class="dot" />
    </g>
  {:else if e === 0}
    <circle cx={c} cy={c - r} r={stroke * 0.85 + 1.6} fill={color} class="dot" />
  {/if}
</svg>

<style>
  .ring {
    display: block;
    overflow: visible;
  }

  .mark {
    stroke: rgb(32 38 32 / 30%);
    stroke-width: 1;
  }

  .track {
    fill: none;
    stroke: var(--pomo-soft, #e8d8bd);
  }

  .arc {
    fill: none;
  }

  .fruit {
    filter: drop-shadow(1.5px 2.5px 2.5px rgb(70 35 15 / 24%));
  }

  .dot {
    stroke: var(--pomo-light, #fff8ea);
    stroke-width: 1.5;
  }

  /* One-second ticks glide instead of stepping; reduced motion drops this via app.css. */
  .animate .arc {
    transition:
      stroke-dasharray 1s linear,
      stroke-dashoffset 1s linear;
  }

  .animate .marker {
    transition: transform 1s linear;
  }
</style>
