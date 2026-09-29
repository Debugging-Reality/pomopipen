<script lang="ts">
  // Classic Tomato week calendar frame: a red plastic produce basket seen from
  // above, lying on its side (handles at the left and right ends). The rim and
  // the slotted walls ARE the calendar's border; the grid sits on the basket's
  // floor, so the week's tomatoes look packed inside. Built in: only its
  // opacity can change (Settings → Appearance).
  //
  // The parent reserves room for it: 29 px of padding inside (rim + wall) and
  // 16 px outside on the left and right for the handles.

  let { opacity = 100 }: { opacity?: number } = $props();

  let width = $state(0);
  let height = $state(0);

  const RED = '#C9302C';
  const RED_DARK = '#A1211F';
  const RED_LIGHT = '#E4574B';
  const HANDLE = '#EDD58E';
  const HANDLE_EDGE = '#BF9F4F';
  const SIDE = 16; // handle room outside the rim, left and right
  const RIM = 9;
  const WALL = 20;

  let g = $derived.by(() => {
    const W = width;
    const H = height;
    const body = { x: SIDE, y: 0, w: Math.max(0, W - 2 * SIDE), h: H };
    const inset = RIM + WALL;
    const win = { x: body.x + inset, y: inset, w: Math.max(0, body.w - 2 * inset), h: Math.max(0, H - 2 * inset) };
    const grip = Math.max(60, Math.min(H * 0.34, 200));
    // Slots along each wall, clear of the corners.
    const across: number[] = [];
    for (let x = win.x + 8; x <= win.x + win.w - 13; x += 11) across.push(x);
    const down: number[] = [];
    for (let y = win.y + 8; y <= win.y + win.h - 13; y += 11) down.push(y);
    return { W, H, body, win, grip, gripY: H / 2 - grip / 2, across, down };
  });

  const rrect = (x: number, y: number, w: number, h: number, r: number) =>
    `M${x + r} ${y}H${x + w - r}A${r} ${r} 0 0 1 ${x + w} ${y + r}V${y + h - r}A${r} ${r} 0 0 1 ${x + w - r} ${y + h}H${x + r}A${r} ${r} 0 0 1 ${x} ${y + h - r}V${y + r}A${r} ${r} 0 0 1 ${x + r} ${y}Z`;
</script>

<div class="basket" bind:clientWidth={width} bind:clientHeight={height} style:opacity={Math.max(0, Math.min(100, opacity)) / 100} aria-hidden="true">
  {#if width > 120 && height > 120}
    {@const { body, win } = g}
    <svg {width} {height} viewBox="0 0 {width} {height}">
      <!-- Handles, tucked under the rim at both ends. -->
      {#each [0, 1] as side}
        {@const x = side === 0 ? 1 : g.W - SIDE - 5}
        <rect x={x} y={g.gripY} width={SIDE + 4} height={g.grip} rx="7" fill={HANDLE} stroke={HANDLE_EDGE} stroke-width="1.5" />
        <rect x={side === 0 ? x + 4 : x + SIDE - 3} y={g.gripY + 12} width="3" height={g.grip - 24} rx="1.5" fill={HANDLE_EDGE} opacity="0.45" />
      {/each}

      <!-- Rim (outer band) and sloping wall (inner band); the floor window stays open. -->
      <path d="{rrect(body.x, body.y, body.w, body.h, 22)} {rrect(win.x, win.y, win.w, win.h, 8)}" fill={RED} fill-rule="evenodd" />
      <path d="{rrect(body.x + RIM, body.y + RIM, body.w - 2 * RIM, body.h - 2 * RIM, 14)} {rrect(win.x, win.y, win.w, win.h, 8)}" fill={RED_DARK} fill-rule="evenodd" />

      <!-- Slots through the walls show the page behind, like the real basket. -->
      <g class="holes">
        {#each g.across as x}
          <rect x={x} y={RIM + 3.5} width="5" height={WALL - 7} rx="1.5" />
          <rect x={x} y={g.H - RIM - WALL + 3.5} width="5" height={WALL - 7} rx="1.5" />
        {/each}
        {#each g.down as y}
          <rect x={body.x + RIM + 3.5} y={y} width={WALL - 7} height="5" rx="1.5" />
          <rect x={body.x + body.w - RIM - WALL + 3.5} y={y} width={WALL - 7} height="5" rx="1.5" />
        {/each}
      </g>

      <!-- Highlight on the rim from the upper left, a crisp inner edge at the floor. -->
      <path d={rrect(body.x + 2, body.y + 2, body.w - 4, body.h - 4, 20)} fill="none" stroke={RED_LIGHT} stroke-width="1.5" opacity="0.9" />
      <path d={rrect(win.x - 1, win.y - 1, win.w + 2, win.h + 2, 9)} fill="none" stroke={RED_DARK} stroke-width="2" />

      <!-- Hinges where the handles meet the rim. -->
      {#each [0, 1] as side}
        {@const hx = side === 0 ? SIDE - 3 : g.W - SIDE - 3}
        <rect x={hx} y={g.gripY + 8} width="6" height="18" rx="2" fill={RED_DARK} />
        <rect x={hx} y={g.gripY + g.grip - 26} width="6" height="18" rx="2" fill={RED_DARK} />
      {/each}
    </svg>
  {/if}
</div>

<style>
  .basket {
    position: absolute;
    inset: 0 -16px;
    z-index: 2;
    pointer-events: none;
  }

  svg {
    display: block;
  }

  .holes {
    fill: var(--ui-page, #f5ebd8);
  }
</style>
