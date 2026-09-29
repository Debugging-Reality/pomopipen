<script lang="ts">
  import { untrack } from 'svelte';
  import type { WeekEvent } from '$lib/types';
  import { addDays, dateKey, splitEvents, layoutSegments, visibleHours, minutesFreeAbove, mergeRounds, snapMinutes, atMinute, shiftStart, SNAP_MINUTES, type FocusBlock } from '$lib/utils/calendar';
  import { themedInk } from '$lib/utils/color';
  import { activeTheme } from '$lib/stores/theme';
  import ThemeBackground from '../ThemeBackground.svelte';
  import TomatoTop from '../classic/TomatoTop.svelte';
  import TomatoBasket from '../classic/TomatoBasket.svelte';
  import { collectionFor } from '$lib/themes/collection';
  import { varietyFor, type Variety } from '$lib/themes/varieties';
  import { settings } from '$lib/stores/settings';
  import { getLocale } from '$paraglide/runtime.js';
  import * as m from '$paraglide/messages.js';

  let { events, weekStart, loading = false, fullDay = false, varieties, editable = false, onmove, onresize, ondraw, onedit, ondelete }: {
    events: WeekEvent[]; weekStart: Date; loading?: boolean; fullDay?: boolean;
    /** Classic Tomato: the variety each subject is drawn as. */
    varieties?: Map<number, Variety>;
    /** Records can be dragged, drawn, edited and deleted (the handlers do the saving). */
    editable?: boolean;
    /** A block was dragged by whole days and minutes (snapped). */
    onmove?: (block: FocusBlock, days: number, minutes: number) => void;
    /** A block's lower edge was dragged by `minutes`. */
    onresize?: (block: FocusBlock, minutes: number) => void;
    /** Empty space was drawn over (or double-clicked): a new record from `start` to `end`, unix seconds. */
    ondraw?: (start: number, end: number) => void;
    onedit?: (round: WeekEvent) => void;
    ondelete?: (round: WeekEvent) => void;
  } = $props();
  // Hours stretch to fill the space the window gives the grid, like a desktop
  // calendar: the visible day fits without scrolling unless the window is tiny.
  const MIN_HOUR = 30;
  const MAX_HOUR = 64;
  let scrollHeight = $state(0);
  let headerHeight = $state(50);
  let scroller = $state<HTMLDivElement>();
  let chosen = $state<number | null>(null);
  let zh = $derived(getLocale().startsWith('zh'));
  // Consecutive rounds of one subject are one block (same rule as the Google Calendar sync).
  let blocks = $derived(mergeRounds(events));
  let segments = $derived(layoutSegments(splitEvents(blocks, weekStart)));
  let range = $derived(fullDay ? [0, 24] : visibleHours(segments));
  let startHour = $derived(range[0]);
  let endHour = $derived(range[1]);
  let hours = $derived(Array.from({ length: endHour - startHour + 1 }, (_, i) => i + startHour));
  let HOUR_HEIGHT = $derived(scrollHeight
    ? Math.max(MIN_HOUR, Math.min(MAX_HOUR, (scrollHeight - headerHeight - 14) / Math.max(1, endHour - startHour)))
    : 48);
  // Switching to 24 h starts at the first focus of the week instead of midnight.
  $effect(() => {
    if (!fullDay || !scroller) return;
    untrack(() => {
      const first = segments.length ? Math.min(...segments.map((s) => s.startMinute)) / 60 : 8;
      scroller!.scrollTop = Math.max(0, (Math.floor(first) - 1) * HOUR_HEIGHT);
    });
  });
  let days = $derived(Array.from({ length: 7 }, (_, i) => addDays(weekStart, i)));
  let selected = $derived(blocks.find(e => e.id === chosen));
  let timeFmt = $derived(new Intl.DateTimeFormat(getLocale(), { hour: '2-digit', minute: '2-digit', hour12: false }));
  let weekdayFmt = $derived(new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }));
  let today = dateKey(new Date());
  // Classic Tomato draws every block as a tomato of the subject's variety (§5.6).
  let tomatoes = $derived(collectionFor($activeTheme)?.id === 'classic-tomato');
  const title = (event: WeekEvent) => event.task_title || event.subject_name || m.subject_filter_uncategorized();
  /** Time range, rounds, focus minutes and (for several) the tasks of a block. */
  function facts(block: FocusBlock): string[] {
    const range = `${timeFmt.format(new Date(block.started_at * 1000))}–${timeFmt.format(new Date((block.started_at + block.duration_secs) * 1000))}`;
    const minutes = Math.round(block.focus_secs / 60);
    const unfinished = block.members.length - block.rounds;
    return [range,
      ...(block.rounds > 1 ? [zh ? `${block.rounds} 个番茄` : `${block.rounds} rounds`] : []),
      ...(unfinished ? [zh ? (block.members.length > 1 ? `${unfinished} 段未完成` : '未完成') : (block.members.length > 1 ? `${unfinished} unfinished` : 'Unfinished')] : []),
      zh ? `专注 ${minutes} 分钟` : `${minutes} min focus`,
      ...(block.tasks.length > 1 ? [(zh ? '任务：' : 'Tasks: ') + block.tasks.join(zh ? '、' : ', ')] : [])];
  }
  const description = (block: FocusBlock) => [title(block), ...facts(block)].join(' · ');
  const clock = (secs: number) => timeFmt.format(new Date(secs * 1000));
  const roundRange = (round: WeekEvent) => `${clock(round.started_at)}–${clock(round.started_at + round.duration_secs)}`;

  // ── Editing by pointer: drag a block to move it (to other days too), its
  // lower edge to change its length, or empty space to draw a new record.
  // Everything snaps to SNAP_MINUTES; nothing is saved until the pointer lifts.
  const DRAG_PX = 4;
  const GUTTER = 50;
  type Drag =
    | { kind: 'move' | 'resize'; block: FocusBlock; x: number; y: number; column: number; days: number; minutes: number; active: boolean }
    | { kind: 'draw'; day: number; from: number; to: number };
  let drag = $state<Drag | null>(null);
  let body = $state<HTMLDivElement>();
  // The click that ends a drag must not also select the block.
  let swallowClick = false;
  const columnWidth = () => ((body?.getBoundingClientRect().width ?? 0) - GUTTER) / 7 || 1;
  const minuteAt = (clientY: number) =>
    Math.max(0, Math.min(1440, (clientY - (body?.getBoundingClientRect().top ?? 0)) / HOUR_HEIGHT * 60 + startHour * 60));
  const defaultSecs = () => Math.max(SNAP_MINUTES * 60, $settings.time_work_secs || 1500);

  function grab(e: PointerEvent, block: FocusBlock) {
    if (!editable || e.button !== 0) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const kind = (e.target as Element).closest('.resize') ? 'resize' : 'move';
    drag = { kind, block, x: e.clientX, y: e.clientY, column: columnWidth(), days: 0, minutes: 0, active: false };
  }
  function startDraw(e: PointerEvent, day: number) {
    if (!editable || e.button !== 0 || e.target !== e.currentTarget) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const minute = snapMinutes(minuteAt(e.clientY));
    drag = { kind: 'draw', day, from: minute, to: minute };
  }
  function follow(e: PointerEvent) {
    if (!drag) return;
    if (drag.kind === 'draw') {
      drag.to = snapMinutes(minuteAt(e.clientY));
      return;
    }
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    if (!drag.active && Math.hypot(dx, dy) < DRAG_PX) return;
    drag.active = true;
    let minutes = snapMinutes(dy / HOUR_HEIGHT * 60);
    if (drag.kind === 'resize') {
      const last = drag.block.members[drag.block.members.length - 1];
      minutes = Math.max(minutes, SNAP_MINUTES - Math.round(last.duration_secs / 60));
    }
    drag.minutes = minutes;
    drag.days = drag.kind === 'move' ? Math.max(-dayOf(drag.block), Math.min(6 - dayOf(drag.block), Math.round(dx / drag.column))) : 0;
  }
  function drop() {
    const done = drag;
    drag = null;
    if (!done) return;
    if (done.kind === 'draw') {
      const from = Math.min(done.from, done.to);
      const to = Math.max(done.from, done.to);
      if (to - from >= SNAP_MINUTES) ondraw?.(atMinute(days[done.day], from), atMinute(days[done.day], to));
      return;
    }
    if (!done.active) return; // a plain click: onclick selects the block
    swallowClick = true;
    if (done.kind === 'move' && (done.days || done.minutes)) onmove?.(done.block, done.days, done.minutes);
    if (done.kind === 'resize' && done.minutes) onresize?.(done.block, done.minutes);
  }
  function drawAt(e: MouseEvent, day: number) {
    if (!editable || e.target !== e.currentTarget) return;
    const start = atMinute(days[day], snapMinutes(minuteAt(e.clientY), 15));
    ondraw?.(start, start + defaultSecs());
  }
  const dayOf = (block: FocusBlock) => Math.max(0, days.findIndex((d) => dateKey(d) === dateKey(new Date(block.started_at * 1000))));
  /** Where the dragged block lands, as "14:05–14:30" (shown on the block while dragging). */
  function landing(d: Extract<Drag, { block: FocusBlock }>): string {
    const start = d.kind === 'move' ? shiftStart(d.block.started_at, d.days, d.minutes) : d.block.started_at;
    const end = d.kind === 'move' ? start + d.block.duration_secs : d.block.started_at + d.block.duration_secs + d.minutes * 60;
    return `${clock(start)}–${clock(end)}`;
  }
  const draggedBy = (segment: { event: FocusBlock }) =>
    drag && drag.kind !== 'draw' && drag.active && drag.block.id === segment.event.id ? drag : null;
</script>

<div class="calendar-surface" class:basketed={tomatoes} style:--floor={tomatoes ? ($settings.basket_floor_opacity / 100) * 0.2 : 0}>
{#if tomatoes}<TomatoBasket opacity={$settings.basket_frame_opacity} />{:else}<ThemeBackground target="calendar" />{/if}
<div class="calendar-scroll" aria-busy={loading} bind:clientHeight={scrollHeight} bind:this={scroller}>
  <div class="calendar" class:loading style="--grid-height:{(endHour - startHour) * HOUR_HEIGHT}px; --hh:{HOUR_HEIGHT}px">
    <div class="day-header" bind:clientHeight={headerHeight}>
      <div class="timezone">{zh ? '本地' : 'Local'}</div>
      {#each days as day}
        <div class="day" class:today={dateKey(day) === today}>
          <span>{weekdayFmt.format(day)}</span><b>{day.getDate()}</b>
        </div>
      {/each}
    </div>
    <div class="calendar-body" bind:this={body}>
      <div class="time-gutter">
        {#each hours as hour}
          <span class="hour" style="top:{(hour - startHour) * HOUR_HEIGHT}px">{String(hour).padStart(2, '0')}:00</span>
        {/each}
      </div>
      {#each days as day, dayIndex}
        <!-- Drawing over empty space (or double-clicking it) adds a record; the editor dialog is the keyboard route. -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="day-column" class:today-column={dateKey(day) === today} class:editable
          onpointerdown={(e) => startDraw(e, dayIndex)} onpointermove={follow} onpointerup={drop}
          onpointercancel={() => { drag = null; }} ondblclick={(e) => drawAt(e, dayIndex)}>
          {#if drag?.kind === 'draw' && drag.day === dayIndex && drag.from !== drag.to}
            {@const from = Math.min(drag.from, drag.to)}
            {@const to = Math.max(drag.from, drag.to)}
            <div class="draft" style="top:{(from / 60 - startHour) * HOUR_HEIGHT}px; height:{(to - from) / 60 * HOUR_HEIGHT}px">
              <span>{clock(atMinute(day, from))}–{clock(atMinute(day, to))}</span>
            </div>
          {/if}
          {#each segments.filter(s => s.day === dayIndex) as segment (segment.key)}
            {@const color = /^#[\da-f]{6}$/i.test(segment.event.subject_color ?? '') ? segment.event.subject_color! : ($activeTheme?.colors['--pomo-alt-bg'] ?? '#7B7D87')}
            {@const height = Math.max(15, (segment.endMinute - segment.startMinute) / 60 * HOUR_HEIGHT - 2)}
            <!-- Classic Tomato: the subject's variety (its own, or the closest one for older colors). -->
            {@const variety = tomatoes && segment.event.subject_id !== null ? (varieties?.get(segment.event.subject_id) ?? varietyFor(segment.event.subject_color)) : undefined}
            {@const skin = tomatoes ? (segment.event.subject_id === null ? 'var(--pomo-soft)' : variety?.hex ?? color) : color}
            {@const top = variety?.top === 'stem' && minutesFreeAbove(segments, segment) / 60 * HOUR_HEIGHT >= (height >= 40 ? 11 : 8) ? 'stem' : 'calyx'}
            {@const moving = draggedBy(segment)}
            {@const lastPiece = !segments.some(s => s.event.id === segment.event.id && s.day > segment.day)}
            {@const shownHeight = moving?.kind === 'resize' && lastPiece ? Math.max(15, height + moving.minutes / 60 * HOUR_HEIGHT) : height}
            <button class="event tx-{variety?.texture ?? 'none'}" class:unclassified={!segment.event.subject_id}
              class:chosen={chosen === segment.event.id} class:dragging={!!moving} class:editable
              class:unfinished={segment.event.rounds === 0}
              class:tomato={tomatoes} class:tall={tomatoes && shownHeight > 38}
              style="top:{(segment.startMinute / 60 - startHour) * HOUR_HEIGHT}px; height:{shownHeight}px; left:calc({segment.column / segment.columns * 100}% + 2px); width:calc({100 / segment.columns}% - 4px); --event-bg:{skin}; --event-ink:{variety?.ink ?? themedInk(tomatoes && !segment.event.subject_id ? '#E8D8BD' : color, $activeTheme?.colors ?? null)}; --r:{Math.min(14, shownHeight / 2)}px{moving?.kind === 'move' ? `; transform:translate(${moving.days * moving.column}px, ${moving.minutes / 60 * HOUR_HEIGHT}px)` : ''}"
              title={tomatoes && variety ? `${description(segment.event)} · ${zh ? variety.nameZh : variety.name}` : description(segment.event)}
              aria-label={tomatoes && variety ? `${description(segment.event)} · ${zh ? variety.nameZh : variety.name}` : description(segment.event)}
              onpointerdown={(e) => grab(e, segment.event)} onpointermove={follow} onpointerup={drop}
              onpointercancel={() => { drag = null; }}
              onclick={() => {
                if (swallowClick) { swallowClick = false; return; }
                chosen = chosen === segment.event.id ? null : segment.event.id;
              }}>
              {#if moving}<span class="drag-time">{landing(moving)}</span>{/if}
              {#if editable && lastPiece}<span class="resize" aria-hidden="true"></span>{/if}
              {#if tomatoes}
                <span class="tomato-top" class:stem={top === 'stem'}>
                  <TomatoTop kind={top} width={top === 'stem' ? (height >= 40 ? 20 : 14) : height >= 40 ? 22 : 16}
                    onGreen={variety?.id === 'zebra' || variety?.id === 'green'} outline={!segment.event.subject_id} />
                </span>
              {/if}
              <span class="event-title">{title(segment.event)}</span>
              {#if height > 38}<span class="event-time">{timeFmt.format(new Date(segment.event.started_at * 1000))}{segment.event.rounds > 1 ? ` · ×${segment.event.rounds}` : ''}</span>{/if}
            </button>
          {/each}
        </div>
      {/each}
      {#if events.length === 0 && !loading}
        <div class="empty"><span class="empty-mark">◌</span><strong>{zh ? '这一周，还留着空白' : 'A quiet week, so far'}</strong>
          <p>{zh ? '开始专注后，学习足迹会出现在这里。' : 'Your focus will find its place here.'}</p>
          {#if editable}<p>{zh ? '也可以在空白处拖一下，补记一段学习。' : 'Or drag over empty space to add one by hand.'}</p>{/if}</div>
      {/if}
    </div>
  </div>
</div>
</div>
{#if selected}
  <div class="detail" aria-live="polite">
    <div class="detail-text"><strong>{title(selected)}</strong><span>{[selected.subject_name || m.subject_filter_uncategorized(), ...facts(selected)].join(' · ')}</span>
      {#if editable}
        <ul class="rounds" aria-label={zh ? '这一块里的番茄' : 'Rounds in this block'}>
          {#each selected.members as round (round.id)}
            <li>
              <span class="round-time">{roundRange(round)}</span>
              {#if round.completed === false}<span class="round-tag">{zh ? '未完成' : 'Unfinished'}</span>{/if}
              <span class="round-task">{round.task_title ?? ''}</span>
              <button class="s-btn s-btn--ghost small" onclick={() => onedit?.(round)}>{zh ? '编辑' : 'Edit'}</button>
              <button class="s-btn s-btn--ghost small danger" onclick={() => ondelete?.(round)}>{zh ? '删除' : 'Delete'}</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
    <button class="close" onclick={() => { chosen = null; }} aria-label={zh ? '关闭详情' : 'Close details'}>×</button>
  </div>
{:else if editable && events.length}
  <p class="edit-hint">{zh ? '拖动方块改时间（可拖到别的日子），拖下边缘改时长，在空白处拖动或双击可补记；点方块可编辑或删除。' : 'Drag a block to move it (also to another day), its lower edge to change its length; drag or double-click empty space to add a record; click a block to edit or delete.'}</p>
{/if}

<style>
  .calendar-surface { position: relative; isolation: isolate; background: var(--ui-surface); border: 1px solid var(--ui-border); border-radius: 14px; overflow: hidden; box-shadow: 0 1px 2px color-mix(in srgb, var(--ui-text) 5%, transparent); }
  .calendar-scroll { position: relative; z-index: 1; overflow: auto; max-height: 450px; scrollbar-width: thin; scrollbar-color: var(--ui-border-strong) transparent; }
  /* Classic Tomato: the basket rim and walls frame the calendar; the grid is its floor. */
  .calendar-surface.basketed { margin: 0 16px; padding: 29px; overflow: visible; border: 0; border-radius: 0; background: transparent; box-shadow: none; }
  .basketed .calendar-scroll { border-radius: 8px; background: var(--ui-surface); }
  /* The basket floor shows through the grid: square holes between thin red bars. */
  .basketed .calendar-body { background-image: repeating-linear-gradient(to bottom, rgb(201 48 44 / var(--floor)) 0 3px, transparent 3px 22px), repeating-linear-gradient(to right, rgb(201 48 44 / var(--floor)) 0 3px, transparent 3px 22px); }
  .basketed .day-header { background: color-mix(in srgb, var(--ui-surface) 96%, transparent); border-bottom-color: rgb(161 33 31 / 28%); }
  .basketed .day-column { border-left-color: rgb(161 33 31 / 20%); background-image: repeating-linear-gradient(to bottom, rgb(161 33 31 / 15%) 0px, rgb(161 33 31 / 15%) 1px, transparent 1px, transparent var(--hh)); }
  .calendar { min-width: 510px; transition: opacity 0.15s; }
  .loading { opacity: 0.4; }
  .day-header { display: grid; grid-template-columns: 50px repeat(7, minmax(0, 1fr)); position: sticky; top: 0; z-index: 4; background: color-mix(in srgb, var(--ui-surface) 94%, transparent); backdrop-filter: blur(6px); border-bottom: 1px solid var(--ui-border); }
  .timezone { align-self: end; padding: 0 4px 12px; text-align: center; font-size: 11px; color: var(--ui-text-muted); }
  .day { display: flex; align-items: center; flex-direction: column; gap: 2px; padding: 6px 0 5px; color: var(--ui-text-muted); }
  .day span { font-size: 12px; font-weight: 600; }
  .day b { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 50%; font-size: 16px; font-weight: 650; color: var(--ui-text); }
  .day.today span { color: var(--ui-brand-strong); }
  .day.today b { background: var(--ui-brand); color: var(--ui-on-brand); }
  .calendar-body { position: relative; display: grid; grid-template-columns: 50px repeat(7, minmax(0, 1fr)); height: var(--grid-height); margin: 8px 0 6px; }
  .time-gutter { position: relative; }
  .hour { position: absolute; right: 8px; transform: translateY(-50%); font-size: 11px; font-weight: 600; color: var(--ui-text-muted); font-variant-numeric: tabular-nums; }
  .day-column { position: relative; border-left: 1px solid var(--ui-border); background-image: repeating-linear-gradient(to bottom, var(--ui-border) 0px, var(--ui-border) 1px, transparent 1px, transparent var(--hh)); }
  .today-column { background-color: color-mix(in srgb, var(--ui-brand) 5%, transparent); }
  .event { position: absolute; display: flex; flex-direction: column; gap: 2px; min-height: 15px; padding: 2px 6px; border: 0; border-radius: 7px; background: var(--event-bg); color: var(--event-ink); font: inherit; text-align: left; overflow: hidden; cursor: pointer; box-shadow: 0 1px 2px rgb(0 0 0 / 12%); transition: filter 150ms ease, transform 150ms ease; }
  .event:hover { filter: brightness(0.96); transform: translateY(-1px); z-index: 2; }
  .event:focus-visible, .event.chosen { outline: 2px solid var(--ui-text); outline-offset: 1px; z-index: 3; }
  .unclassified { border: 1px dashed var(--event-ink); }
  /* Only unfinished rounds (had to leave part way): counted, drawn lighter. */
  .event.unfinished { opacity: 0.62; }
  .event.unfinished:is(:hover, .chosen, .dragging) { opacity: 0.9; }
  /* ── Classic Tomato: each block is a tomato; body edges = start and end ── */
  .event.tomato { overflow: visible; border-radius: var(--r); align-items: center; justify-content: center; text-align: center; gap: 1px; padding: 0 6px; box-shadow: inset 0 -3px 0 rgb(0 0 0 / 10%); }
  .event.tomato.tall { justify-content: flex-start; padding-top: 13px; }
  .event.tomato.unclassified { border: 0; box-shadow: inset 0 0 0 1.5px rgb(32 38 32 / 55%); }
  .event.tomato :is(.event-title, .event-time) { position: relative; z-index: 1; }
  .event.tomato .event-title { font-size: 11px; font-weight: 700; }
  .event.tomato .event-time { font-size: 10.5px; }
  .event.tomato:focus-visible, .event.tomato.chosen { outline: 2px solid var(--color-long-round); outline-offset: 2px; }
  .event.tomato:hover { filter: none; box-shadow: inset 0 -3px 0 rgb(0 0 0 / 16%); }
  .tomato-top { position: absolute; top: -3px; left: 50%; z-index: 1; transform: translateX(-50%); }
  .tomato-top.stem { top: auto; bottom: calc(100% - 5px); transform: translateX(-30%); }
  /* Skin patterns sit on the two sides and fade toward the middle, so the label stays on solid color. */
  .event.tomato::before, .event.tomato::after { content: ''; position: absolute; top: 0; bottom: 0; width: 18%; pointer-events: none; }
  .event.tomato::before { left: 0; border-radius: var(--r) 0 0 var(--r); }
  .event.tomato::after { right: 0; border-radius: 0 var(--r) var(--r) 0; }
  .tx-streak::before { background: linear-gradient(90deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(185 62 18 / 50%) 0 1.5px, transparent 1.5px 5px); }
  .tx-streak::after { background: linear-gradient(270deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(185 62 18 / 50%) 0 1.5px, transparent 1.5px 5px); }
  .tx-zebra::before { background: linear-gradient(90deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(196 214 96 / 55%) 0 2px, transparent 2px 6px); }
  .tx-zebra::after { background: linear-gradient(270deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(196 214 96 / 55%) 0 2px, transparent 2px 6px); }
  .tx-rib::before { background: linear-gradient(90deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(110 34 80 / 32%) 0 1px, transparent 1px 6px); }
  .tx-rib::after { background: linear-gradient(270deg, transparent 35%, var(--event-bg)), repeating-linear-gradient(90deg, rgb(110 34 80 / 32%) 0 1px, transparent 1px 6px); }
  .tx-speckle::before { background: linear-gradient(90deg, transparent 35%, var(--event-bg)), radial-gradient(circle, rgb(255 232 170 / 85%) 0 1px, transparent 1.6px) 0 0 / 5px 5px; }
  .tx-speckle::after { background: linear-gradient(270deg, transparent 35%, var(--event-bg)), radial-gradient(circle, rgb(255 232 170 / 85%) 0 1px, transparent 1.6px) 0 0 / 5px 5px; }
  .tx-dots::before { background: linear-gradient(90deg, transparent 35%, var(--event-bg)), radial-gradient(circle, rgb(40 80 24 / 60%) 0 .9px, transparent 1.5px) 0 0 / 5px 5px; }
  .tx-dots::after { background: linear-gradient(270deg, transparent 35%, var(--event-bg)), radial-gradient(circle, rgb(40 80 24 / 60%) 0 .9px, transparent 1.5px) 0 0 / 5px 5px; }
  .event-title { width: 100%; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; font-size: 12px; font-weight: 700; line-height: 1.25; }
  .event-time { font-size: 11px; font-variant-numeric: tabular-nums; opacity: 0.9; }
  .empty { position: absolute; top: 90px; left: 70px; right: 24px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 24px 10px; border-radius: 14px; background: color-mix(in srgb, var(--ui-surface) 94%, transparent); pointer-events: none; }
  .empty-mark { font-size: 28px; color: var(--ui-brand); }
  .empty strong { font-size: 14px; font-weight: 700; color: var(--ui-text); }
  .empty p { font-size: 12.5px; color: var(--ui-text-muted); }
  .detail { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; margin-top: 8px; padding: 12px 14px; border: 1px solid var(--ui-border); border-radius: 12px; background: var(--ui-surface); }
  .detail-text { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
  .detail strong { font-size: 14px; overflow-wrap: anywhere; color: var(--ui-text); }
  .detail-text > span { font-size: 12.5px; color: var(--ui-text-muted); }
  .detail .close { flex-shrink: 0; width: 30px; height: 30px; border: 0; border-radius: 8px; background: none; color: var(--ui-text-muted); font-size: 18px; cursor: pointer; }
  .detail .close:hover { background: var(--ui-hover); color: var(--ui-text); }
  .rounds { display: flex; flex-direction: column; margin: 6px 0 -4px; padding: 0; list-style: none; max-height: 132px; overflow-y: auto; }
  .rounds li { display: flex; align-items: center; gap: 8px; min-height: 32px; border-top: 1px solid var(--ui-border); font-size: 12.5px; }
  .round-time { color: var(--ui-text); font-weight: 650; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .round-tag { flex-shrink: 0; padding: 1px 6px; border: 1px dashed var(--ui-border-strong); border-radius: 6px; font-size: 11px; color: var(--ui-text-muted); }
  .round-task { flex: 1; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: var(--ui-text-muted); }
  .rounds .small { height: 26px; padding: 0 9px; font-size: 12px; }
  .rounds .danger:hover:not(:disabled) { background: var(--ui-danger-soft); color: var(--ui-danger); }
  .edit-hint { margin: 8px 2px 0; font-size: 12px; line-height: 1.5; color: var(--ui-text-muted); }
  /* ── Editing by pointer ── */
  .day-column.editable { cursor: crosshair; }
  .event.editable { cursor: grab; touch-action: none; }
  .event.dragging { z-index: 5; cursor: grabbing; opacity: 0.9; box-shadow: 0 8px 18px -6px rgb(0 0 0 / 35%); transition: none; }
  .event.dragging:hover { transform: none; }
  .resize { position: absolute; left: 0; right: 0; bottom: 0; z-index: 2; height: 7px; cursor: ns-resize; }
  .resize::after { content: ''; position: absolute; left: 50%; bottom: 2px; width: 18px; height: 3px; margin-left: -9px; border-radius: 2px; background: var(--event-ink); opacity: 0; transition: opacity 150ms ease; }
  .event:hover .resize::after, .event.dragging .resize::after { opacity: 0.55; }
  .drag-time { position: absolute; top: -20px; left: 50%; z-index: 6; transform: translateX(-50%); padding: 1px 6px; border-radius: 6px; background: var(--ui-text); color: var(--ui-surface); font-size: 11px; font-weight: 650; font-variant-numeric: tabular-nums; white-space: nowrap; pointer-events: none; }
  .draft { position: absolute; left: 2px; right: 2px; z-index: 4; display: flex; justify-content: center; padding-top: 2px; border: 1.5px dashed var(--ui-brand); border-radius: 7px; background: color-mix(in srgb, var(--ui-brand) 16%, transparent); pointer-events: none; }
  .draft span { font-size: 11px; font-weight: 700; color: var(--ui-brand-strong); font-variant-numeric: tabular-nums; }
</style>