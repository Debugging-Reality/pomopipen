<script lang="ts">
  import type { HeatmapStats, HeatmapEntry, SubjectTotal } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import SubjectBreakdown from './SubjectBreakdown.svelte';
  import { settings } from '$lib/stores/settings';
  import { dayOfWeek } from '$lib/utils/calendar';

  let { heatmap, breakdown }: { heatmap: HeatmapStats | null; breakdown: SubjectTotal[] | null } =
    $props();

  // Heatmap grid constants
  const CELL = 11;
  const GAP = 2;
  const STRIDE = CELL + GAP;
  const DAYS = 7;
  const GRID_H = DAYS * STRIDE - GAP; // 89px

  // Locale-aware month and day-of-week names — reactive to app language setting
  const monthFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'short' }));
  const dowFmt = $derived(new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }));
  const MONTH_NAMES: string[] = $derived(
    Array.from({ length: 12 }, (_, i) => monthFmt.format(new Date(2000, i, 1)))
  );
  // Rows follow the week-start setting (Sunday or Monday on top).
  const mondayFirst = $derived($settings.week_starts_monday);
  // Row labels: Mon, Wed, Fri — reference dates that land on those days
  const ROW_LABELS: Record<number, string> = $derived(Object.fromEntries(
    [new Date(2000, 0, 3), new Date(2000, 0, 5), new Date(2000, 0, 7)] // Monday, Wednesday, Friday
      .map((d) => [dayOfWeek(d, mondayFirst), dowFmt.format(d)])
  ));

  // Year navigation state
  const currentYear = new Date().getFullYear();
  let selectedYear = $state(currentYear);

  // Earliest year with recorded sessions (determines how far back the user can navigate)
  const firstYear = $derived.by(() => {
    if (!heatmap || heatmap.entries.length === 0) return currentYear;
    return Number(heatmap.entries[0].date.slice(0, 4));
  });

  interface GridCell {
    date: string;
    count: number;
    level: 0 | 1 | 2 | 3;
    dimmed: boolean; // future or out-of-year
  }

  interface MonthLabel {
    x: number;
    label: string;
  }

  function buildGridForYear(
    year: number,
    entries: HeatmapEntry[]
  ): { grid: GridCell[][]; months: MonthLabel[]; weekCount: number } {
    const byDate = new Map(entries.map((e) => [e.date, e.count]));

    const today = new Date();
    today.setHours(0, 0, 0, 0);

    // Grid start: first day of the week containing Jan 1 of the selected year
    const jan1 = new Date(year, 0, 1);
    const gridStart = new Date(jan1);
    gridStart.setDate(jan1.getDate() - dayOfWeek(jan1, mondayFirst));

    // Grid end: last day of the week containing Dec 31
    const dec31 = new Date(year, 11, 31);
    const gridEnd = new Date(dec31);
    gridEnd.setDate(dec31.getDate() + (6 - dayOfWeek(dec31, mondayFirst)));

    const msPerWeek = 7 * 24 * 60 * 60 * 1000;
    const weekCount = Math.round((gridEnd.getTime() - gridStart.getTime()) / msPerWeek) + 1;

    const grid: GridCell[][] = [];
    const months: MonthLabel[] = [];
    let lastMonth = -1;

    for (let w = 0; w < weekCount; w++) {
      const col: GridCell[] = [];
      for (let d = 0; d < DAYS; d++) {
        const dt = new Date(gridStart);
        dt.setDate(gridStart.getDate() + w * DAYS + d);
        const dateStr = fmtDate(dt);
        const count = byDate.get(dateStr) ?? 0;
        const dimmed = dt > today || dt.getFullYear() !== year;
        const level = dimmed || count === 0 ? 0 : count <= 3 ? 1 : count <= 7 ? 2 : 3;
        col.push({ date: dateStr, count, level: level as 0 | 1 | 2 | 3, dimmed });
      }

      // Month label when the first cell of the week belongs to the selected year and starts a new month
      const firstDate = new Date(col[0].date + 'T00:00:00');
      const month = firstDate.getMonth();
      if (firstDate.getFullYear() === year && month !== lastMonth) {
        months.push({ x: w * STRIDE, label: MONTH_NAMES[month] });
        lastMonth = month;
      }

      grid.push(col);
    }

    return { grid, months, weekCount };
  }

  function fmtDate(d: Date): string {
    return [
      d.getFullYear(),
      String(d.getMonth() + 1).padStart(2, '0'),
      String(d.getDate()).padStart(2, '0'),
    ].join('-');
  }

  function fmtHours(h: number): string {
    return h >= 1000 ? `${(h / 1000).toFixed(1)}k` : String(h);
  }

  const LEVEL_FILL = ['var(--heat-0)', 'var(--heat-1)', 'var(--heat-2)', 'var(--heat-3)'] as const;

  // Tooltip — viewport-fixed so it escapes SVG/overflow clipping.
  let tooltip = $state<{ x: number; y: number; text: string } | null>(null);
  let tooltipEl = $state<HTMLDivElement | undefined>(undefined);

  function showTooltip(event: MouseEvent, cell: GridCell) {
    if (cell.dimmed) {
      tooltip = null;
      return;
    }
    const cellRect = (event.currentTarget as SVGRectElement).getBoundingClientRect();
    const text =
      cell.count === 0
        ? `${cell.date}: ${m.stats_no_sessions_today().toLowerCase()}`
        : `${cell.date}: ${cell.count} ${cell.count === 1 ? m.stats_rounds().toLowerCase().replace(/s$/, '') : m.stats_rounds().toLowerCase()}`;
    tooltip = { x: cellRect.left + CELL / 2, y: cellRect.top - 8, text };
  }

  const { grid, months, weekCount } = $derived.by(() => {
    if (!heatmap) return { grid: [] as GridCell[][], months: [] as MonthLabel[], weekCount: 52 };
    return buildGridForYear(selectedYear, heatmap.entries);
  });

  const GRID_W = $derived(weekCount * STRIDE - GAP);
  const LEFT_OFFSET = 28;
  const MONTH_LABEL_H = 16;
  const SVG_W = $derived(LEFT_OFFSET + GRID_W);
  const SVG_H = MONTH_LABEL_H + GRID_H;

  const hasData = $derived(heatmap !== null && heatmap.total_rounds > 0);
</script>

<!-- Intensity scale CSS variables -->
<svelte:head>
  <style>
    :root {
      --heat-0: var(--ui-border);
      --heat-1: color-mix(in oklch, var(--color-focus-round) 30%, var(--ui-surface));
      --heat-2: color-mix(in oklch, var(--color-focus-round) 62%, var(--ui-surface));
      --heat-3: var(--color-focus-round);
    }
  </style>
</svelte:head>

<div class="view">
  {#if heatmap === null}
    <div class="loading"><span>{m.stats_loading()}</span></div>
  {:else}
    <!-- Heatmap section -->
    <div class="heatmap-section">
      <div class="heatmap-wrap">
        <!-- Year navigation -->
        <div class="year-nav">
          <button
            class="year-btn"
            onclick={() => {
              selectedYear -= 1;
              tooltip = null;
            }}
            disabled={selectedYear <= firstYear}
            aria-label={m.stats_prev_year()}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <polyline
                points="9,2 4,7 9,12"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
          <span class="year-label">{selectedYear}</span>
          <button
            class="year-btn"
            onclick={() => {
              selectedYear += 1;
              tooltip = null;
            }}
            disabled={selectedYear >= currentYear}
            aria-label={m.stats_next_year()}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <polyline
                points="5,2 10,7 5,12"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
        </div>

        <!-- Heatmap SVG -->
        <svg
          width={SVG_W}
          height={SVG_H}
          viewBox="0 0 {SVG_W} {SVG_H}"
          class="heatmap-svg"
          onmouseleave={() => {
            tooltip = null;
          }}
          role="img"
          aria-label="Annual activity heatmap"
        >
          <!-- Month labels -->
          {#each months as mo}
            <text x={LEFT_OFFSET + mo.x} y={10} class="month-label">{mo.label}</text>
          {/each}

          <!-- Day-of-week labels -->
          {#each Object.entries(ROW_LABELS) as [rowIdx, rowLabel]}
            <text
              x={LEFT_OFFSET - 4}
              y={MONTH_LABEL_H + Number(rowIdx) * STRIDE + CELL / 2 + 4}
              text-anchor="end"
              class="dow-label">{rowLabel}</text
            >
          {/each}

          <!-- Grid cells -->
          {#each grid as col, w}
            {#each col as cell, d}
              {@const cx = LEFT_OFFSET + w * STRIDE}
              {@const cy = MONTH_LABEL_H + d * STRIDE}
              <rect
                x={cx}
                y={cy}
                width={CELL}
                height={CELL}
                rx="2"
                style="fill: {LEVEL_FILL[cell.level]}"
                class="cell"
                class:cell-dimmed={cell.dimmed}
                role="img"
                aria-label="{cell.date}: {cell.count} {m.stats_rounds().toLowerCase()}"
                onmouseenter={(e) => showTooltip(e, cell)}
                onmouseleave={() => {
                  tooltip = null;
                }}
              />
            {/each}
          {/each}
        </svg>

        <!-- Cell tooltip (HTML so it matches Tooltip.svelte style and escapes SVG clipping) -->
        {#if tooltip}
          {@const pad = 8}
          {@const w = tooltipEl?.offsetWidth ?? 0}
          {@const rawLeft = tooltip.x - w / 2}
          {@const left = Math.max(pad, Math.min(rawLeft, window.innerWidth - w - pad))}
          {@const arrowLeft = tooltip.x - left}
          <div
            bind:this={tooltipEl}
            class="cell-tooltip"
            style="top:{tooltip.y}px;left:{left}px;--arrow-left:{arrowLeft}px"
          >
            {tooltip.text}
          </div>
        {/if}

        <!-- Legend -->
        <div class="legend">
          <span class="legend-label">{m.stats_legend_less()}</span>
          {#each [0, 1, 2, 3] as lvl}
            <div class="legend-cell" style="background: {LEVEL_FILL[lvl]}"></div>
          {/each}
          <span class="legend-label">{m.stats_legend_more()}</span>
        </div>
      </div>
    </div>

    <!-- Lifetime totals -->
    <div class="totals">
      <div class="total-card" style="--delay: 0ms">
        <span class="total-label">{m.stats_total_rounds()}</span>
        <span class="total-value">{heatmap.total_rounds.toLocaleString()}</span>
      </div>
      <div class="total-divider"></div>
      <div class="total-card" style="--delay: 60ms">
        <span class="total-label">{m.stats_focus_hours()}</span>
        <span class="total-value">{fmtHours(heatmap.total_hours)}</span>
      </div>
      <div class="total-divider"></div>
      <div class="total-card" style="--delay: 120ms">
        <span class="total-label">{m.stats_best_streak()}</span>
        <span class="total-value">
          {heatmap.longest_streak > 0 ? heatmap.longest_streak : '—'}
          {#if heatmap.longest_streak > 0}
            <span class="total-unit">{m.stats_days()}</span>
          {/if}
        </span>
      </div>
    </div>

    <SubjectBreakdown items={breakdown ?? []} />

    {#if !hasData}
      <div class="empty-overlay"><span>{m.stats_empty_history()}</span></div>
    {/if}
  {/if}
</div>

<style>
  .view {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: app-fade-in 0.2s ease;
  }

  .loading {
    padding: 40px;
    text-align: center;
    color: var(--ui-text-muted);
    font-size: 13px;
  }

  /* ── Heatmap card ─────────────────────────────────────────── */
  .heatmap-section {
    display: flex;
    padding: 16px 18px;
    border-radius: 14px;
    border: 1px solid var(--ui-border);
    background: var(--ui-surface);
    overflow-x: auto;
  }

  /* Auto margins center it without clipping the left edge when it overflows. */
  .heatmap-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
    margin: 0 auto;
  }

  .year-nav {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: 28px;
  }

  .year-label {
    min-width: 3.2em;
    text-align: center;
    font-size: 14px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--ui-text);
  }

  .year-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text-muted);
    cursor: pointer;
    transition:
      color 150ms ease,
      background 150ms ease;
  }

  .year-btn:hover:not(:disabled) {
    color: var(--ui-text);
    background: var(--ui-hover);
  }

  .year-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .heatmap-svg {
    display: block;
    overflow: visible;
  }

  .cell {
    cursor: default;
    transition: opacity 0.1s;
  }

  .cell:hover:not(.cell-dimmed) {
    opacity: 0.75;
  }

  .cell-dimmed {
    opacity: 0.35;
  }

  .month-label {
    fill: var(--ui-text-muted);
    font-size: 10px;
    font-weight: 600;
    cursor: default;
  }

  .dow-label {
    fill: var(--ui-text-muted);
    font-size: 9px;
    cursor: default;
  }

  .cell-tooltip {
    position: fixed;
    transform: translateY(-100%);
    width: max-content;
    max-width: 240px;
    padding: 5px 9px;
    border-radius: 8px;
    background: var(--ui-text);
    color: var(--ui-surface);
    font-size: 12px;
    font-weight: 600;
    line-height: 1.4;
    pointer-events: none;
    z-index: 9999;
  }

  .cell-tooltip::after {
    content: '';
    position: absolute;
    top: 100%;
    left: var(--arrow-left, 50%);
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-top-color: var(--ui-text);
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 3px;
    margin-left: 28px;
  }

  .legend-label {
    padding: 0 4px;
    font-size: 11px;
    color: var(--ui-text-muted);
  }

  .legend-cell {
    width: 11px;
    height: 11px;
    border-radius: 3px;
  }

  /* ── Lifetime totals ──────────────────────────────────────── */
  .totals {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .total-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border-radius: 14px;
    border: 1px solid var(--ui-border);
    background: var(--ui-surface);
    animation: card-rise 0.35s cubic-bezier(0.22, 1, 0.36, 1) both;
    animation-delay: var(--delay, 0ms);
  }

  @keyframes card-rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .total-label {
    font-size: 12.5px;
    color: var(--ui-text-muted);
  }

  .total-value {
    display: flex;
    align-items: baseline;
    gap: 4px;
    font-size: 26px;
    font-weight: 750;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    line-height: 1.1;
    color: var(--ui-text);
  }

  .total-unit {
    font-size: 13px;
    font-weight: 600;
    color: var(--ui-text-muted);
  }

  .total-divider {
    display: none;
  }

  .empty-overlay {
    position: absolute;
    top: 60px;
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
    font-size: 13px;
    color: var(--ui-text-muted);
    pointer-events: none;
  }
</style>
