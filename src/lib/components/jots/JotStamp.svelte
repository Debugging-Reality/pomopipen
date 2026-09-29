<script lang="ts">
  // The jot checkbox. Crossing a jot off stamps a little tomato: the ring
  // squashes and fills, the calyx springs on top, the tick draws itself and a
  // few seeds pop out — one small emoji-sized moment (~0.6 s), played once.
  // `pop` plays it; reduced motion shows the end state straight away.
  let {
    checked,
    pop = false,
    label,
    onclick,
  }: { checked: boolean; pop?: boolean; label: string; onclick: () => void } = $props();

  const SEEDS = [0, 60, 120, 180, 240, 300].map((deg, i) => ({ deg: deg + (i % 2 ? 12 : -8), far: i % 2 ? 16 : 20 }));
</script>

<button class="stamp" class:checked class:pop role="checkbox" aria-checked={checked} aria-label={label} title={label} {onclick}>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <ellipse class="fruit" cx="12" cy="13.8" rx="8.1" ry="7.4" />
    <g class="calyx">
      <path d="M12 8.2Q9.1 6 6.6 7.8Q9.5 9.4 12 8.2ZM12 8.2Q14.9 6 17.4 7.8Q14.5 9.4 12 8.2ZM12 8.2Q11.1 10 12 11Q12.9 10 12 8.2Z" />
      <path class="stem" d="M12 8.3Q12.1 6 13.5 4.7" />
    </g>
    <path class="tick" d="M8.5 14l2.4 2.4 4.7-5" pathLength="1" />
  </svg>
  {#if pop}
    <span class="seeds" aria-hidden="true">
      {#each SEEDS as seed, i (i)}<i style:--a="{seed.deg}deg" style:--d="{seed.far}px"></i>{/each}
    </span>
  {/if}
</button>

<style>
  .stamp {
    --stamp-ink: var(--jot-ink, var(--ui-border-strong));
    --stamp-fill: var(--ui-brand);
    --stamp-leaf: var(--jot-leaf, var(--color-short-round));
    --stamp-paper: var(--ui-on-brand);
    position: relative;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    margin: -2px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }

  svg {
    width: 24px;
    height: 24px;
    overflow: visible;
    transform-origin: 50% 62%;
  }

  .fruit {
    fill: var(--ui-surface);
    stroke: var(--stamp-ink);
    stroke-width: 1.6;
    transition:
      fill 140ms ease,
      stroke 140ms ease;
  }

  .calyx {
    fill: var(--stamp-leaf);
    opacity: 0;
    transform-origin: 12px 8.4px;
    transform: scale(0.4);
    transition:
      opacity 140ms ease,
      transform 180ms ease;
  }

  .stem {
    fill: none;
    stroke: var(--stamp-leaf);
    stroke-width: 1.4;
    stroke-linecap: round;
  }

  .tick {
    fill: none;
    stroke: var(--stamp-ink);
    stroke-width: 1.9;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    opacity: 0;
  }

  /* Hover hints at what a click does: a faint tomato top and tick. */
  .stamp:hover:not(.checked) .calyx {
    opacity: 0.4;
    transform: scale(0.85);
  }

  .stamp:hover:not(.checked) .fruit {
    stroke: var(--stamp-fill);
  }

  .stamp:hover:not(.checked) .tick {
    opacity: 0.45;
    stroke: var(--stamp-fill);
    stroke-dashoffset: 0;
  }

  .stamp:active:not(.checked) svg {
    transform: scale(0.88);
  }

  .checked .fruit {
    fill: var(--stamp-fill);
    stroke: var(--jot-stamp-edge, var(--stamp-fill));
  }

  .checked .calyx {
    opacity: 1;
    transform: none;
  }

  .checked .tick {
    opacity: 1;
    stroke: var(--stamp-paper);
    stroke-dashoffset: 0;
  }

  /* ── The pop ─────────────────────────────────────────────────────────── */
  .pop svg {
    animation: squash 560ms cubic-bezier(0.3, 0.7, 0.4, 1) both;
  }

  .pop .calyx {
    animation: spring 600ms 70ms cubic-bezier(0.3, 0.7, 0.4, 1) both;
  }

  .pop .tick {
    animation: draw 240ms 150ms ease-out both;
  }

  @keyframes squash {
    0% { transform: scale(1); }
    14% { transform: scale(0.78); }
    38% { transform: scale(1.24, 0.88); }
    58% { transform: scale(0.9, 1.12); }
    78% { transform: scale(1.04, 0.97); }
    100% { transform: scale(1); }
  }

  @keyframes spring {
    0% { opacity: 0; transform: translateY(3px) scale(0) rotate(-40deg); }
    55% { opacity: 1; transform: translateY(-1.5px) scale(1.3) rotate(12deg); }
    80% { transform: scale(0.95) rotate(-4deg); }
    100% { opacity: 1; transform: none; }
  }

  @keyframes draw {
    from { stroke-dashoffset: 1; }
    to { stroke-dashoffset: 0; }
  }

  .seeds {
    position: absolute;
    left: 50%;
    top: 55%;
    pointer-events: none;
  }

  .seeds i {
    position: absolute;
    left: -2px;
    top: -2.8px;
    width: 4px;
    height: 5.6px;
    border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%;
    background: var(--jot-seed, var(--pomo-pop, var(--ui-accent-2)));
    opacity: 0;
    animation: burst 540ms 90ms cubic-bezier(0.2, 0.7, 0.3, 1) forwards;
  }

  .seeds i:nth-child(3n + 2) {
    background: var(--stamp-leaf);
  }

  .seeds i:nth-child(3n) {
    background: var(--stamp-fill);
  }

  @keyframes burst {
    0% { opacity: 0; transform: rotate(var(--a)) translateY(-5px) scale(0.7); }
    22% { opacity: 1; }
    100% { opacity: 0; transform: rotate(var(--a)) translateY(calc(-1 * var(--d))) scale(0.35); }
  }

  @media (prefers-reduced-motion: reduce) {
    .pop svg,
    .pop .calyx,
    .pop .tick {
      animation: none;
    }

    .seeds {
      display: none;
    }

    .fruit,
    .calyx {
      transition: none;
    }
  }
</style>
