<script lang="ts">
  // Today and the last seven days on one page: headline numbers, then when
  // today's focus happened and how the week went.
  import type { DailyStats, DayStat, StreakInfo } from '$lib/types';
  import { settings } from '$lib/stores/settings';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';

  let { today, week, streak }: { today: DailyStats | null; week: DayStat[] | null; streak: StreakInfo | null } =
    $props();

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  const shortFmt = $derived(new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }));
  const dateFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'numeric', day: 'numeric' }));

  function fmtTime(mins: number): string {
    if (mins < 60) return `${mins}m`;
    const h = Math.floor(mins / 60);
    const rest = mins % 60;
    return rest === 0 ? `${h}h` : `${h}h ${rest}m`;
  }

  const byHour = $derived(today?.by_hour ?? Array(24).fill(0));
  const maxHour = $derived(Math.max(1, ...byHour));
  const days = $derived.by(() => {
    const countByDate = new Map((week ?? []).map((d) => [d.date, d.rounds]));
    const midnight = new Date();
    midnight.setHours(0, 0, 0, 0);
    return Array.from({ length: 7 }, (_, i) => {
      const d = new Date(midnight);
      d.setDate(midnight.getDate() - (6 - i));
      const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
      return { date: d, rounds: countByDate.get(key) ?? 0, isToday: i === 6 };
    });
  });
  const maxDay = $derived(Math.max(1, ...days.map((d) => d.rounds)));
  const weekTotal = $derived(days.reduce((s, d) => s + d.rounds, 0));

  // Charts fill their card; width comes from the container.
  let hourWidth = $state(420);
  let dayWidth = $state(320);
  const H = 132;
  let hoverHour = $state<number | null>(null);
  let hoverDay = $state<number | null>(null);
</script>

<div class="overview">
  <div class="tiles">
    <div class="tile accent">
      <span class="label">{zh ? '今日番茄' : 'Rounds today'}</span>
      <span class="value">{today?.rounds ?? '—'}</span>
    </div>
    <div class="tile">
      <span class="label">{m.stats_focus_time()}</span>
      <span class="value">{today ? fmtTime(today.focus_mins) : '—'}</span>
    </div>
    <div class="tile">
      <span class="label">{m.stats_completion()}</span>
      <span class="value">{today?.completion_rate != null ? `${Math.round(today.completion_rate * 100)}%` : '—'}</span>
    </div>
    <div class="tile">
      <span class="label">{zh ? '近 7 天番茄' : 'Last 7 days'}</span>
      <span class="value">{weekTotal}</span>
    </div>
    <div class="tile">
      <span class="label">{m.stats_current_streak()}</span>
      <span class="value">
        {#if streak && streak.current > 0}{streak.current}<small>{zh ? '天' : streak.current === 1 ? m.stats_day() : m.stats_days()}</small>
        {:else}<small class="none">{m.stats_no_active_streak()}</small>{/if}
      </span>
    </div>
  </div>

  <div class="panels">
    <section class="panel">
      <header>
        <h3>{zh ? '今天的专注时段' : m.stats_sessions_by_hour()}</h3>
        {#if !today || today.rounds === 0}<span class="hint">{zh ? '今天还没有完成的番茄' : m.stats_no_sessions_today()}</span>{/if}
      </header>
      <div class="chart" bind:clientWidth={hourWidth}>
        {#if hourWidth > 0}
          {@const step = hourWidth / 24}
          {@const barW = Math.max(3, step * 0.62)}
          <svg width={hourWidth} height={H + 22} role="img" aria-label={m.stats_sessions_by_hour()}>
            <line class="base" x1="0" x2={hourWidth} y1={H} y2={H} />
            {#each byHour as count, h}
              {@const barH = count ? Math.max(4, (count / maxHour) * (H - 16)) : 3}
              <rect class="bar" class:empty={!count} class:hover={hoverHour === h}
                x={h * step + (step - barW) / 2} y={H - barH} width={barW} height={barH} rx={Math.min(4, barW / 2)} />
              <rect class="hit" x={h * step} y="0" width={step} height={H} role="presentation"
                onpointerenter={() => (hoverHour = h)} onpointerleave={() => (hoverHour = null)} />
              {#if h % 6 === 0}
                <text class="axis" x={h * step + step / 2} y={H + 16} text-anchor="middle">{h}:00</text>
              {/if}
            {/each}
          </svg>
          {#if hoverHour !== null}
            <div class="tip" style:left={`${Math.min(hourWidth - 110, Math.max(0, hoverHour * step - 40))}px`}>
              {hoverHour}:00–{hoverHour + 1}:00 · {byHour[hoverHour]} {zh ? '轮' : 'rounds'}
            </div>
          {/if}
        {/if}
      </div>
    </section>

    <section class="panel">
      <header>
        <h3>{zh ? '最近 7 天' : 'Last 7 days'}</h3>
        {#if weekTotal === 0}<span class="hint">{m.stats_no_sessions_week()}</span>{/if}
      </header>
      <div class="chart" bind:clientWidth={dayWidth}>
        {#if dayWidth > 0}
          {@const step = dayWidth / 7}
          {@const barW = Math.min(44, step * 0.6)}
          <svg width={dayWidth} height={H + 22} role="img" aria-label={zh ? '最近 7 天番茄数' : 'Rounds in the last 7 days'}>
            <line class="base" x1="0" x2={dayWidth} y1={H} y2={H} />
            {#each days as day, i}
              {@const barH = day.rounds ? Math.max(4, (day.rounds / maxDay) * (H - 22)) : 3}
              {@const x = i * step + (step - barW) / 2}
              <rect class="bar day" class:today={day.isToday} class:empty={!day.rounds} class:hover={hoverDay === i}
                {x} y={H - barH} width={barW} height={barH} rx="5" />
              {#if day.rounds}
                <text class="count" x={x + barW / 2} y={H - barH - 6} text-anchor="middle">{day.rounds}</text>
              {/if}
              <rect class="hit" x={i * step} y="0" width={step} height={H} role="presentation"
                onpointerenter={() => (hoverDay = i)} onpointerleave={() => (hoverDay = null)} />
              <text class="axis" class:today={day.isToday} x={i * step + step / 2} y={H + 16} text-anchor="middle">
                {day.isToday ? (zh ? '今天' : 'Today') : shortFmt.format(day.date)}
              </text>
            {/each}
          </svg>
          {#if hoverDay !== null}
            <div class="tip" style:left={`${Math.min(dayWidth - 110, Math.max(0, hoverDay * step - 20))}px`}>
              {dateFmt.format(days[hoverDay].date)} · {days[hoverDay].rounds} {zh ? '轮' : 'rounds'}
            </div>
          {/if}
        {/if}
      </div>
    </section>
  </div>
</div>

<style>
  .overview {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(128px, 1fr));
    gap: 10px;
  }

  .tile {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
  }

  .tile.accent {
    background: var(--ui-brand);
    border-color: var(--ui-brand);
    color: var(--ui-on-brand);
  }

  .label {
    font-size: 12.5px;
    color: var(--ui-text-muted);
  }

  .accent .label {
    color: color-mix(in srgb, var(--ui-on-brand) 85%, transparent);
  }

  .value {
    display: flex;
    align-items: baseline;
    gap: 4px;
    font-size: 26px;
    font-weight: 750;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .value small {
    font-size: 13px;
    font-weight: 600;
    color: var(--ui-text-muted);
  }

  .value small.none {
    font-size: 13px;
    font-weight: 500;
  }

  .panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }

  .panel {
    padding: 16px 18px 12px;
    border-radius: 14px;
    background: var(--ui-surface);
    border: 1px solid var(--ui-border);
    min-width: 0;
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 10px;
  }

  h3 {
    font-size: 14px;
    font-weight: 700;
    color: var(--ui-text);
  }

  .hint {
    font-size: 12.5px;
    color: var(--ui-text-muted);
  }

  .chart {
    position: relative;
  }

  svg {
    display: block;
    overflow: visible;
  }

  .base {
    stroke: var(--ui-border);
  }

  .bar {
    fill: var(--color-focus-round);
    transition: opacity 150ms ease;
  }

  .bar.day {
    fill: color-mix(in srgb, var(--color-focus-round) 45%, var(--ui-surface));
  }

  .bar.day.today {
    fill: var(--color-focus-round);
  }

  .bar.empty {
    fill: var(--ui-border);
  }

  .bar.hover {
    opacity: 0.75;
  }

  .hit {
    fill: transparent;
  }

  .axis {
    font-size: 11px;
    fill: var(--ui-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .axis.today {
    fill: var(--ui-brand-strong);
    font-weight: 700;
  }

  .count {
    font-size: 11px;
    font-weight: 700;
    fill: var(--ui-text);
    font-variant-numeric: tabular-nums;
  }

  .tip {
    position: absolute;
    top: -6px;
    padding: 5px 9px;
    border-radius: 8px;
    background: var(--ui-text);
    color: var(--ui-surface);
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    pointer-events: none;
  }
</style>
