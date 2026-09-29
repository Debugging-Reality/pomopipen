<script lang="ts">
  import { onMount } from 'svelte';
  import { statsGetRangeEvents, onRoundChange, onSessionsCleared, onSessionsChanged } from '$lib/ipc';
  import type { SubjectFilter } from '$lib/types';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { addDays, dailyTotals, movingAverage } from '$lib/utils/calendar';
  import { getLocale } from '$paraglide/runtime.js';

  let { subjectFilter = 'all' }: { subjectFilter?: SubjectFilter } = $props();

  const RANGES = [30, 90, 365] as const;
  const WINDOWS = [7, 14, 30] as const;
  const HEIGHT = 220;
  const M = { top: 14, right: 64, bottom: 26, left: 42 };

  let range = $state<number>(30);
  let windowDays = $state<number>(7);
  let refresh = $state(0);
  let dates = $state<Date[]>([]);
  let daily = $state<number[]>([]); // minutes, visible days only
  let average = $state<number[]>([]); // minutes, trailing mean incl. days before the range
  let loading = $state(true);
  let error = $state('');
  let width = $state(640);
  let hover = $state<number | null>(null);

  let zh = $derived(getLocale().startsWith('zh'));
  let dayFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'numeric', day: 'numeric' }));
  let longFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'short', day: 'numeric', weekday: 'short' }));
  let monthFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'short' }));

  function midnight(d = new Date()): Date { const m = new Date(d); m.setHours(0, 0, 0, 0); return m; }

  $effect(() => {
    void refresh;
    const days = range;
    const avgDays = windowDays;
    const filter = subjectFilter;
    const end = addDays(midnight(), 1);
    const from = addDays(end, -(days + avgDays - 1));
    let cancelled = false;
    loading = true;
    error = '';
    statsGetRangeEvents(Math.floor(from.getTime() / 1000), Math.floor(end.getTime() / 1000), filter)
      .then(events => {
        if (cancelled) return;
        const totals = dailyTotals(events, from, days + avgDays - 1);
        const minutes = totals.map(t => t.seconds / 60);
        dates = totals.slice(avgDays - 1).map(t => t.date);
        daily = minutes.slice(avgDays - 1);
        average = movingAverage(minutes, avgDays).slice(avgDays - 1);
        hover = null;
      })
      .catch(e => { if (!cancelled) error = String(e); })
      .finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  onMount(() => {
    let disposed = false;
    const cleanups: UnlistenFn[] = [];
    for (const listen of [onRoundChange, onSessionsCleared, onSessionsChanged]) {
      void listen(() => { refresh += 1; }).then(stop => { if (disposed) stop(); else cleanups.push(stop); });
    }
    return () => { disposed = true; cleanups.forEach(stop => stop()); };
  });

  function formatMinutes(min: number): string {
    const total = Math.round(min);
    const h = Math.floor(total / 60);
    const m = total % 60;
    if (h === 0) return `${m}m`;
    return m === 0 ? `${h}h` : `${h}h ${m}m`;
  }

  // Y axis in hours with a readable step.
  let scale = $derived.by(() => {
    const maxHours = Math.max(0, ...daily, ...average) / 60;
    const step = [0.25, 0.5, 1, 2, 3, 4, 6, 8].find(s => maxHours / s <= 4) ?? 12;
    const top = Math.max(step, Math.ceil(maxHours / step) * step);
    const ticks = Array.from({ length: Math.round(top / step) + 1 }, (_, i) => i * step);
    return { top, ticks };
  });
  let plotW = $derived(Math.max(80, width - M.left - M.right));
  let plotH = HEIGHT - M.top - M.bottom;
  let n = $derived(daily.length);
  const x = (i: number) => M.left + (n <= 1 ? plotW / 2 : (i * plotW) / (n - 1));
  const y = (minutes: number) => M.top + plotH - (minutes / 60 / scale.top) * plotH;
  const path = (values: number[]) => values.map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('');

  let xTicks = $derived.by(() => {
    if (range === 365) return dates.flatMap((d, i) => (d.getDate() === 1 ? [{ i, label: monthFmt.format(d) }] : []));
    const every = range === 30 ? 5 : 15;
    return dates.flatMap((d, i) => ((n - 1 - i) % every === 0 ? [{ i, label: dayFmt.format(d) }] : []));
  });

  let visibleTotal = $derived(daily.reduce((a, b) => a + b, 0));
  let bestDay = $derived(daily.length ? daily.indexOf(Math.max(...daily)) : -1);
  let studiedDays = $derived(daily.filter(v => v >= 1).length);
  let empty = $derived(!loading && !error && visibleTotal < 1);
  let avgLabel = $derived(zh ? `${windowDays}日平均` : `${windowDays}-day avg`);

  function pointer(e: PointerEvent) {
    const rect = (e.currentTarget as SVGRectElement).getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    hover = n ? Math.max(0, Math.min(n - 1, Math.round(ratio * (n - 1)))) : null;
  }
</script>

<section class="trend" aria-label={zh ? '每日学习时长趋势' : 'Daily study time trend'}>
  <header>
    <div>
      <span class="eyebrow">TREND</span>
      <h3>{zh ? '每日学习时长' : 'Daily study time'}</h3>
    </div>
    <div class="controls">
      <div class="seg" role="radiogroup" aria-label={zh ? '时间范围' : 'Range'}>
        {#each RANGES as r}
          <button role="radio" aria-checked={range === r} class:on={range === r} onclick={() => { range = r; }}>
            {r === 365 ? (zh ? '一年' : '1y') : zh ? `${r}天` : `${r}d`}</button>
        {/each}
      </div>
      <div class="seg" role="radiogroup" aria-label={zh ? '平均天数' : 'Average window'}>
        {#each WINDOWS as w}
          <button role="radio" aria-checked={windowDays === w} class:on={windowDays === w} onclick={() => { windowDays = w; }}>
            {zh ? `${w}日均` : `${w}d avg`}</button>
        {/each}
      </div>
    </div>
  </header>

  <div class="tiles">
    <div><span>{zh ? '合计' : 'Total'}</span><b>{formatMinutes(visibleTotal)}</b></div>
    <div><span>{zh ? '日均' : 'Per day'}</span><b>{formatMinutes(n ? visibleTotal / n : 0)}</b></div>
    <div><span>{zh ? '学习天数' : 'Days studied'}</span><b>{studiedDays}<small>/{n}</small></b></div>
    <div><span>{zh ? '最多一天' : 'Best day'}</span><b>{bestDay >= 0 && daily[bestDay] >= 1 ? formatMinutes(daily[bestDay]) : '—'}</b></div>
  </div>

  <div class="legend" aria-hidden="true">
    <span><i class="swatch daily"></i>{zh ? '每日学习' : 'Daily'}</span>
    <span><i class="swatch average"></i>{avgLabel}</span>
  </div>

  <div class="plot" bind:clientWidth={width}>
    {#if error}
      <p class="message" role="alert">{zh ? '趋势加载失败：' : 'Could not load the trend: '}{error}</p>
    {:else}
      <svg width={width} height={HEIGHT} role="img"
        aria-label={zh ? `近${range}天每日学习时长与${avgLabel}` : `Daily study time and ${avgLabel}, last ${range} days`}>
        {#each scale.ticks as t}
          <line class="grid" x1={M.left} x2={M.left + plotW} y1={y(t * 60)} y2={y(t * 60)} />
          <text class="axis" x={M.left - 8} y={y(t * 60) + 3.5} text-anchor="end">{t === 0 ? '0' : formatMinutes(t * 60)}</text>
        {/each}
        {#each xTicks as tick}
          <text class="axis" x={x(tick.i)} y={HEIGHT - 8} text-anchor="middle">{tick.label}</text>
        {/each}
        {#if n}
          <!-- Over long ranges the daily line is context; the average carries the trend. -->
          <path class="line daily" d={path(daily)} style:stroke-width={range > 90 ? 1 : range > 30 ? 1.5 : 2}
            style:opacity={range > 90 ? 0.45 : range > 30 ? 0.75 : 1} />
          <path class="line average" d={path(average)} />
          <text class="direct" x={x(n - 1) + 8} y={y(average[n - 1]) + 4}>{avgLabel}</text>
          {#if hover !== null}
            <line class="crosshair" x1={x(hover)} x2={x(hover)} y1={M.top} y2={M.top + plotH} />
            <circle class="dot daily" cx={x(hover)} cy={y(daily[hover])} r="4" />
            <circle class="dot average" cx={x(hover)} cy={y(average[hover])} r="4" />
          {/if}
        {/if}
        <rect class="hit" x={M.left} y={M.top} width={plotW} height={plotH} role="presentation"
          onpointermove={pointer} onpointerleave={() => { hover = null; }} />
      </svg>
      {#if hover !== null && dates[hover]}
        <div class="tooltip" style:left={`${Math.min(width - 150, Math.max(0, x(hover) - 75))}px`}>
          <strong>{longFmt.format(dates[hover])}</strong>
          <span><i class="swatch daily"></i>{zh ? '当日' : 'Day'} <b>{formatMinutes(daily[hover])}</b></span>
          <span><i class="swatch average"></i>{avgLabel} <b>{formatMinutes(average[hover])}</b></span>
        </div>
      {/if}
      {#if empty}<p class="message overlay">{zh ? '这段时间还没有专注记录' : 'No focus in this range yet'}</p>{/if}
      {#if loading}<p class="message overlay" role="status">{zh ? '正在加载…' : 'Loading…'}</p>{/if}
    {/if}
  </div>
</section>

<style>
  .trend { --chart-surface: var(--ui-surface); background: var(--chart-surface); border: 1px solid var(--ui-border); border-radius: 14px; padding: 16px 18px 12px; color: var(--ui-text); }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 14px; flex-wrap: wrap; }
  .eyebrow { font-size: 11px; letter-spacing: .14em; font-weight: 700; color: var(--ui-text-muted); }
  h3 { font-size: 15px; font-weight: 750; margin-top: 3px; }
  .controls { display: flex; gap: 8px; flex-wrap: wrap; }
  .seg { display: flex; padding: 3px; gap: 2px; border: 1px solid var(--ui-border); border-radius: 10px; background: var(--ui-page); }
  .seg button { border: 0; background: none; color: inherit; font-size: 12.5px; font-weight: 650; padding: 4px 10px; cursor: pointer; }
  .seg button { border-radius: 7px; color: var(--ui-text-muted); transition: background 150ms ease, color 150ms ease; } .seg button:hover { color: var(--ui-text); }
  .seg button.on { background: var(--ui-surface); color: var(--ui-brand-strong); box-shadow: 0 1px 3px color-mix(in srgb, var(--ui-text) 16%, transparent); }
  .tiles { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin: 14px 0 10px; }
  .tiles div { display: flex; flex-direction: column; gap: 3px; padding: 8px 10px; border-radius: 10px; background: var(--ui-page); }
  .tiles span { font-size: 12px; color: var(--ui-text-muted); }
  .tiles b { font-size: 19px; font-weight: 750; font-variant-numeric: tabular-nums; }
  .tiles small { font-size: 12px; font-weight: 600; color: var(--ui-text-muted); }
  .legend { display: flex; gap: 16px; font-size: 12px; font-weight: 600; color: var(--ui-text-muted); margin-bottom: 2px; }
  .legend span, .tooltip span { display: inline-flex; align-items: center; gap: 6px; }
  .swatch { display: inline-block; width: 14px; height: 3px; border-radius: 2px; }
  .swatch.daily { background: var(--chart-daily, var(--color-focus-round)); }
  .swatch.average { background: var(--chart-average, var(--color-long-round)); height: 4px; }
  .plot { position: relative; }
  svg { display: block; overflow: visible; }
  .grid { stroke: color-mix(in srgb, var(--ui-text) 12%, transparent); stroke-width: 1; }
  .axis { font-size: 9px; fill: var(--ui-text-muted); font-variant-numeric: tabular-nums; }
  .line { fill: none; stroke-linejoin: round; stroke-linecap: round; }
  .line.daily { stroke: var(--chart-daily, var(--color-focus-round)); stroke-width: 2; }
  .line.average { stroke: var(--chart-average, var(--color-long-round)); stroke-width: 2.5; }
  .direct { font-size: 10px; font-weight: 750; fill: var(--ui-text); }
  .crosshair { stroke: color-mix(in srgb, var(--ui-text) 35%, transparent); stroke-width: 1; }
  .dot { stroke: var(--chart-surface); stroke-width: 2; }
  .dot.daily { fill: var(--chart-daily, var(--color-focus-round)); }
  .dot.average { fill: var(--chart-average, var(--color-long-round)); }
  .hit { fill: transparent; cursor: crosshair; }
  .tooltip { position: absolute; top: 0; width: 150px; pointer-events: none; display: flex; flex-direction: column; gap: 4px; padding: 8px 10px; border-radius: 9px; background: var(--ui-surface); border: 1px solid color-mix(in srgb, var(--ui-text) 18%, transparent); box-shadow: 0 4px 14px color-mix(in srgb, black 14%, transparent); font-size: 12px; }
  .tooltip strong { font-size: 12.5px; }
  .tooltip b { margin-left: auto; font-variant-numeric: tabular-nums; }
  .message { font-size: 13px; color: var(--ui-text-muted); padding: 30px 0; text-align: center; }
  .message.overlay { position: absolute; inset: 40% 0 auto; padding: 0; pointer-events: none; }
  @media (max-width: 560px) { .tiles { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
</style>
