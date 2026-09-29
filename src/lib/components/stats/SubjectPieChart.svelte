<script lang="ts">
  import { onMount } from 'svelte';
  import { statsGetRangeEvents, onRoundChange, onSessionsCleared, onSessionsChanged, onSubjectsChanged } from '$lib/ipc';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { addDays, dailyTotals, startOfWeek } from '$lib/utils/calendar';
  import { settings } from '$lib/stores/settings';
  import { donutArc, subjectShares, type Share } from '$lib/utils/charts';
  import { getLocale } from '$paraglide/runtime.js';
  import * as m from '$paraglide/messages.js';

  type Period = 'day' | 'week' | 'month';
  const SIZE = 184;
  const OUTER = 88;
  const INNER = 56;

  let period = $state<Period>('day');
  let anchor = $state(midnight());
  let refresh = $state(0);
  let shares = $state<Share[]>([]);
  let loading = $state(true);
  let error = $state('');
  let hover = $state<number | null>(null);

  let zh = $derived(getLocale().startsWith('zh'));
  let dayFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'long', day: 'numeric', weekday: 'short' }));
  let shortFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'numeric', day: 'numeric' }));
  let monthFmt = $derived(new Intl.DateTimeFormat(getLocale(), { year: 'numeric', month: 'long' }));

  function midnight(d = new Date()): Date { const m = new Date(d); m.setHours(0, 0, 0, 0); return m; }

  let span = $derived.by(() => {
    if (period === 'day') return { start: anchor, days: 1 };
    if (period === 'week') return { start: startOfWeek(anchor, $settings.week_starts_monday), days: 7 };
    const start = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
    return { start, days: new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0).getDate() };
  });
  let label = $derived(period === 'day' ? dayFmt.format(span.start)
    : period === 'week' ? `${shortFmt.format(span.start)} – ${shortFmt.format(addDays(span.start, 6))}`
    : monthFmt.format(span.start));
  let isCurrent = $derived(midnight().getTime() >= span.start.getTime() && midnight().getTime() < addDays(span.start, span.days).getTime());

  $effect(() => {
    void refresh;
    const { start, days } = span;
    let cancelled = false;
    loading = true;
    error = '';
    statsGetRangeEvents(Math.floor(start.getTime() / 1000), Math.floor(addDays(start, days).getTime() / 1000), 'all')
      .then(events => { if (!cancelled) { shares = subjectShares(events, dailyTotals(events, start, days)); hover = null; } })
      .catch(e => { if (!cancelled) error = String(e); })
      .finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  onMount(() => {
    let disposed = false;
    const cleanups: UnlistenFn[] = [];
    for (const listen of [onRoundChange, onSessionsCleared, onSessionsChanged, onSubjectsChanged]) {
      void listen(() => { refresh += 1; }).then(stop => { if (disposed) stop(); else cleanups.push(stop); });
    }
    return () => { disposed = true; cleanups.forEach(stop => stop()); };
  });

  function step(direction: -1 | 1) {
    if (period === 'day') anchor = addDays(anchor, direction);
    else if (period === 'week') anchor = addDays(anchor, 7 * direction);
    else anchor = new Date(anchor.getFullYear(), anchor.getMonth() + direction, 1);
  }

  function formatSeconds(seconds: number): string {
    const total = Math.round(seconds / 60);
    const h = Math.floor(total / 60);
    const min = total % 60;
    if (h === 0) return `${min}m`;
    return min === 0 ? `${h}h` : `${h}h ${min}m`;
  }

  const NEUTRAL = 'color-mix(in srgb, var(--ui-text) 28%, var(--chart-surface))';
  const colorOf = (s: Share) => (s.color && /^#[\da-f]{6}$/i.test(s.color) ? s.color : NEUTRAL);
  const nameOf = (s: Share) => s.id === 'other' ? (zh ? '其他' : 'Other') : s.id === null ? m.subject_filter_uncategorized() : (s.name ?? '—');

  let arcs = $derived.by(() => {
    let from = 0;
    return shares.map(s => { const arc = { share: s, from, to: from + s.fraction }; from += s.fraction; return arc; });
  });
  let total = $derived(shares.reduce((a, s) => a + s.seconds, 0));
</script>

<section class="pie" aria-label={zh ? '各科学习时长占比' : 'Study time by subject'}>
  <header>
    <div>
      <span class="eyebrow">SUBJECTS</span>
      <h3>{zh ? '各科学习时长' : 'Time by subject'}</h3>
    </div>
    <div class="seg" role="radiogroup" aria-label={zh ? '统计周期' : 'Period'}>
      {#each [['day', zh ? '日' : 'Day'], ['week', zh ? '周' : 'Week'], ['month', zh ? '月' : 'Month']] as [value, text]}
        <button role="radio" aria-checked={period === value} class:on={period === value} onclick={() => { period = value as Period; }}>{text}</button>
      {/each}
    </div>
  </header>

  <div class="nav">
    <button onclick={() => step(-1)} aria-label={zh ? '上一段' : 'Previous'}>‹</button>
    <span>{label}</span>
    <button onclick={() => step(1)} aria-label={zh ? '下一段' : 'Next'}>›</button>
    <button class="today" disabled={isCurrent} onclick={() => { anchor = midnight(); }}>{zh ? '回到今天' : 'Today'}</button>
  </div>

  {#if error}
    <p class="message" role="alert">{zh ? '加载失败：' : 'Could not load: '}{error}</p>
  {:else if !loading && shares.length === 0}
    <p class="message">{zh ? '这段时间还没有专注记录' : 'No focus in this period'}</p>
  {:else}
    <div class="body" class:loading>
      <svg width={SIZE} height={SIZE} viewBox="0 0 {SIZE} {SIZE}" role="img"
        aria-label={shares.map(s => `${nameOf(s)} ${Math.round(s.fraction * 100)}%`).join(', ')}>
        {#each arcs as arc, i}
          <path d={donutArc(SIZE / 2, SIZE / 2, INNER, OUTER, arc.from, arc.to)} style:fill={colorOf(arc.share)}
            class="slice" class:dim={hover !== null && hover !== i} class:gap={arcs.length > 1}
            role="presentation" onpointerenter={() => { hover = i; }} onpointerleave={() => { hover = null; }} />
        {/each}
        <text class="center-value" x={SIZE / 2} y={SIZE / 2 + 2} text-anchor="middle">
          {formatSeconds(hover !== null ? shares[hover].seconds : total)}</text>
        <text class="center-label" x={SIZE / 2} y={SIZE / 2 + 20} text-anchor="middle">
          {hover !== null ? `${Math.round(shares[hover].fraction * 100)}%` : zh ? '合计' : 'total'}</text>
      </svg>
      <table class="legend">
        <tbody>
          {#each shares as share, i}
            <tr class:active={hover === i} onpointerenter={() => { hover = i; }} onpointerleave={() => { hover = null; }}>
              <td><i class="dot" style:background={colorOf(share)}></i>{nameOf(share)}</td>
              <td class="num">{formatSeconds(share.seconds)}</td>
              <td class="num pct">{Math.round(share.fraction * 100)}%</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .pie { --chart-surface: var(--ui-surface); background: var(--chart-surface); border: 1px solid var(--ui-border); border-radius: 14px; padding: 16px 18px; color: var(--ui-text); }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: 14px; }
  .eyebrow { font-size: 11px; letter-spacing: .14em; font-weight: 700; color: var(--ui-text-muted); }
  h3 { font-size: 15px; font-weight: 750; margin-top: 3px; }
  .seg { display: flex; padding: 3px; gap: 2px; border: 1px solid var(--ui-border); border-radius: 10px; background: var(--ui-page); }
  .seg button { border: 0; background: none; color: inherit; font-size: 12.5px; font-weight: 650; padding: 4px 12px; cursor: pointer; }
  .seg button { border-radius: 7px; color: var(--ui-text-muted); transition: background 150ms ease, color 150ms ease; } .seg button:hover { color: var(--ui-text); }
  .seg button.on { background: var(--ui-surface); color: var(--ui-brand-strong); box-shadow: 0 1px 3px color-mix(in srgb, var(--ui-text) 16%, transparent); }
  .nav { display: flex; align-items: center; gap: 8px; margin: 12px 0 6px; font-size: 13.5px; font-weight: 650; }
  .nav button { border: 1px solid var(--ui-border-strong); background: var(--ui-surface); color: inherit; border-radius: 8px; cursor: pointer; font-size: 1rem; line-height: 1; padding: 2px 8px; }
  .nav .today { font-size: 12.5px; padding: 4px 8px; margin-left: auto; }
  .nav button:disabled { opacity: .4; cursor: default; }
  .body { display: flex; align-items: center; gap: 22px; flex-wrap: wrap; transition: opacity .15s; }
  .body.loading { opacity: .45; }
  .slice { transition: opacity .12s; }
  .slice.gap { stroke: var(--chart-surface); stroke-width: 2; stroke-linejoin: round; }
  .slice.dim { opacity: .35; }
  .center-value { font-size: 17px; font-weight: 750; fill: var(--ui-text); font-variant-numeric: tabular-nums; }
  .center-label { font-size: 10px; fill: var(--ui-text-muted); }
  .legend { flex: 1; min-width: 190px; border-collapse: collapse; font-size: 13.5px; }
  .legend td { padding: 7px 4px; border-bottom: 1px solid color-mix(in srgb, var(--ui-text) 10%, transparent); }
  .legend tr.active td { font-weight: 800; }
  .legend .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .legend .pct { color: var(--ui-text-muted); width: 3.5em; }
  .dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin-right: 8px; vertical-align: -1px; }
  .message { font-size: 13px; color: var(--ui-text-muted); padding: 34px 0; text-align: center; }
</style>
