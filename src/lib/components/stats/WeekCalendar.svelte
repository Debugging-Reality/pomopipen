<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { setSetting, statsGetWeekEvents, onRoundChange, onSessionsCleared, onSessionsChanged, onSubjectsChanged, onTasksChanged, sessionsCreate, sessionsUpdate, sessionsDelete } from '$lib/ipc';
  import type { FocusRecord, SubjectFilter, WeekEvent } from '$lib/types';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { startOfWeek, addDays, dateKey, splitEvents, movedRecords, resizedRecord, toRecord, atMinute, type FocusBlock } from '$lib/utils/calendar';
  import RecordEditor from './RecordEditor.svelte';
  import { notify } from '$lib/stores/toast';
  import { settings } from '$lib/stores/settings';
  import { getLocale } from '$paraglide/runtime.js';
  import WeekCalendarGrid from './WeekCalendarGrid.svelte';
  import TomatoTop from '../classic/TomatoTop.svelte';
  import { subjects } from '$lib/stores/subjects';
  import { activeTheme } from '$lib/stores/theme';
  import { collectionFor } from '$lib/themes/collection';
  import { subjectVarieties } from '$lib/themes/varieties';

  let { subjectFilter = 'all', editable = true }: { subjectFilter?: SubjectFilter; editable?: boolean } = $props();
  let mondayFirst = $derived($settings.week_starts_monday);
  let weekStart = $state(startOfWeek(new Date(), $settings.week_starts_monday));
  // Switching the first weekday keeps showing (roughly) the same week.
  let shownRule = $settings.week_starts_monday;
  $effect(() => {
    const monday = mondayFirst;
    if (monday === shownRule) return;
    shownRule = monday;
    untrack(() => {
      const wasCurrent = dateKey(weekStart) === dateKey(startOfWeek(new Date(), !monday));
      weekStart = startOfWeek(wasCurrent ? new Date() : addDays(weekStart, 3), monday);
    });
  });
  let events = $state<WeekEvent[]>([]);
  let loading = $state(true);
  let error = $state('');
  let fullDay = $state(false);
  let refresh = $state(0);
  let zh = $derived(getLocale().startsWith('zh'));
  let current = $derived(dateKey(weekStart) === dateKey(startOfWeek(new Date(), mondayFirst)));
  let dateFmt = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'short', day: 'numeric' }));
  let totalMinutes = $derived(Math.round(splitEvents(events, weekStart).reduce((sum,s) => sum + s.seconds, 0) / 60));

  // Classic Tomato: every subject is a tomato variety. All known subjects take
  // part in the matching, so a subject keeps the same tomato from week to week.
  let classic = $derived(collectionFor($activeTheme)?.id === 'classic-tomato');
  let varieties = $derived.by(() => {
    if (!classic) return undefined;
    const known = new Map<number, { id: number; color: string; name: string }>();
    for (const s of $subjects) known.set(s.id, { id: s.id, color: s.color, name: s.name });
    for (const e of events) {
      if (e.subject_id !== null && e.subject_color && !known.has(e.subject_id)) {
        known.set(e.subject_id, { id: e.subject_id, color: e.subject_color, name: e.subject_name ?? '' });
      }
    }
    return subjectVarieties([...known.values()]);
  });
  let legend = $derived.by(() => {
    if (!varieties) return [];
    const seen = new Map<number, string>();
    for (const e of events) if (e.subject_id !== null && !seen.has(e.subject_id)) seen.set(e.subject_id, e.subject_name ?? '');
    return [...seen].flatMap(([id, name]) => {
      const variety = varieties.get(id);
      return variety ? [{ id, name, variety }] : [];
    });
  });

  // Another week or filter starts empty; a refresh after an edit keeps the
  // blocks on screen until the new ones arrive (no flash).
  let shownKey = '';
  $effect(() => {
    const start = Math.floor(weekStart.getTime() / 1000);
    const end = Math.floor(addDays(weekStart, 7).getTime() / 1000);
    const filter = subjectFilter;
    void refresh;
    let cancelled = false;
    const key = `${start}|${JSON.stringify(filter)}`;
    error = '';
    if (key !== shownKey) {
      shownKey = key;
      loading = true;
      events = [];
    }
    statsGetWeekEvents(start, end, filter).then(result => {
      if (!cancelled) events = result;
    }).catch(e => { if (!cancelled) error = String(e); })
      .finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  onMount(() => {
    let disposed = false;
    const cleanups: UnlistenFn[] = [];
    const reload = () => { refresh += 1; };
    for (const listen of [onRoundChange, onSessionsCleared, onSessionsChanged, onSubjectsChanged, onTasksChanged]) {
      void listen(reload).then(stop => { if (disposed) stop(); else cleanups.push(stop); })
        .catch(e => console.error('Calendar subscription failed', e));
    }
    return () => { disposed = true; cleanups.forEach(stop => stop()); };
  });

  // ── Editing by hand ──
  // Every change is saved at once and can be undone from the toast. The
  // backend emits `sessions:changed`, which reloads this calendar.
  let editor = $state<{ id: number | null; record: FocusRecord; archivedSubject: string | null } | null>(null);
  const undoLabel = () => (zh ? '撤销' : 'Undo');
  const failed = (e: unknown) => notify(String(e), { tone: 'error', ms: 5000 });
  const inFuture = (record: FocusRecord) => record.started_at + record.duration_secs > Date.now() / 1000 + 60;

  /** Save several records; on failure, put back the ones already saved. */
  async function saveAll(changes: [WeekEvent, FocusRecord][], done: string) {
    if (changes.some(([, r]) => inFuture(r))) {
      notify(zh ? '记录不能结束在未来' : "A record can't end in the future", { tone: 'error' });
      return;
    }
    const saved: [WeekEvent, FocusRecord][] = [];
    try {
      for (const [round, record] of changes) {
        await sessionsUpdate(round.id, record);
        saved.push([round, record]);
      }
    } catch (e) {
      for (const [round] of saved.reverse()) await sessionsUpdate(round.id, toRecord(round)).catch(() => {});
      failed(e);
      return;
    }
    notify(done, { action: { label: undoLabel(), run: () => {
      void (async () => { for (const [round] of [...changes].reverse()) await sessionsUpdate(round.id, toRecord(round)); })().catch(failed);
    } } });
  }

  const moveBlock = (block: FocusBlock, days: number, minutes: number) =>
    saveAll(movedRecords(block, days, minutes), zh ? '已移动' : 'Moved');
  const resizeBlock = (block: FocusBlock, minutes: number) =>
    saveAll([resizedRecord(block, minutes)], zh ? '已调整时长' : 'Length changed');

  function newRecord(start: number, end: number) {
    const s = $settings;
    const subject = $subjects.some((x) => x.id === s.active_subject_id && !x.archived) ? s.active_subject_id : null;
    editor = { id: null, archivedSubject: null,
      record: { started_at: start, duration_secs: end - start, subject_id: subject, task_id: subject === null ? null : s.active_task_id } };
  }
  async function toggleWeekStart() {
    try {
      settings.set(await setSetting('week_starts_monday', String(!mondayFirst)));
    } catch (e) {
      failed(e);
    }
  }
  /** Toolbar button: the latest focus-length slot that has already ended (this
   *  week), or 09:00 on the second day of another week. */
  function addRecord() {
    const length = Math.max(300, $settings.time_work_secs || 1500);
    if (current) {
      const end = Math.floor(Date.now() / 1000 / 300) * 300;
      newRecord(end - length, end);
    } else {
      const start = atMinute(addDays(weekStart, 1), 9 * 60);
      newRecord(start, start + length);
    }
  }
  function editRound(round: WeekEvent) {
    const archived = round.subject_id !== null && !$subjects.some((s) => s.id === round.subject_id);
    editor = { id: round.id, record: toRecord(round), archivedSubject: archived ? round.subject_name : null };
  }
  async function deleteRound(round: WeekEvent) {
    try {
      const removed = await sessionsDelete(round.id);
      notify(zh ? '已删除这条记录' : 'Record deleted', { action: { label: undoLabel(), run: () => { sessionsCreate(removed).catch(failed); } } });
    } catch (e) {
      failed(e);
    }
  }
  async function saveEditor(record: FocusRecord) {
    const open = editor;
    if (!open) return;
    if (open.id === null) {
      const id = await sessionsCreate(record);
      notify(zh ? '已补记' : 'Record added', { action: { label: undoLabel(), run: () => { sessionsDelete(id).catch(failed); } } });
    } else {
      const id = open.id;
      await sessionsUpdate(id, record);
      notify(zh ? '已保存' : 'Saved', { action: { label: undoLabel(), run: () => { sessionsUpdate(id, open.record).catch(failed); } } });
    }
    editor = null;
  }
  async function deleteFromEditor() {
    const open = editor;
    if (!open || open.id === null) return;
    const removed = await sessionsDelete(open.id);
    editor = null;
    notify(zh ? '已删除这条记录' : 'Record deleted', { action: { label: undoLabel(), run: () => { sessionsCreate(removed).catch(failed); } } });
  }
</script>

<section class="week-calendar" aria-label={zh ? '学习周历' : 'Study calendar'}>
  <!-- One compact row, like a desktop calendar: the grid gets the height. -->
  <div class="toolbar">
    <button class="today" disabled={current} onclick={() => { weekStart = startOfWeek(new Date(), mondayFirst); }}>{zh ? '本周' : 'This week'}</button>
    <div class="navigation">
      <button class="arrow" onclick={() => { weekStart = addDays(weekStart, -7); }} aria-label={zh ? '上一周' : 'Previous week'}>‹</button>
      <button class="arrow" onclick={() => { weekStart = addDays(weekStart, 7); }} aria-label={zh ? '下一周' : 'Next week'}>›</button>
    </div>
    <h2 class="range">{dateFmt.format(weekStart)} – {dateFmt.format(addDays(weekStart, 6))}<small>{weekStart.getFullYear()}</small></h2>
    {#if legend.length}
      <div class="legend" aria-label={zh ? '科目与番茄品种' : 'Subjects and their tomatoes'}>
        {#each legend as item (item.id)}
          <span title={`${item.name} · ${zh ? item.variety.nameZh : item.variety.name}`}>
            <i style:background={item.variety.hex}><TomatoTop kind={item.variety.top} width={item.variety.top === 'stem' ? 11 : 10} onGreen={item.variety.id === 'zebra' || item.variety.id === 'green'} /></i>{item.name}
          </span>
        {/each}
      </div>
    {:else}
      <span class="spacer"></span>
    {/if}
    {#if loading}<span class="loading-label" role="status">{zh ? '加载中…' : 'Loading…'}</span>{/if}
    <span class="total" title={zh ? '按科目着色 · 中途离开的未完成番茄颜色较淡 · 位置按开始时间，长度按专注时长（不含暂停）' : 'Subject colors · Unfinished rounds drawn lighter · Start time + focus duration, excluding pauses'}>
      {zh ? '本周' : 'Week'} <b>{Math.floor(totalMinutes / 60)}<small>h</small> {totalMinutes % 60}<small>m</small></b>
    </span>
    {#if editable}
      <button onclick={addRecord} title={zh ? '补记一段没用计时器的学习（也可以在周历空白处拖动）' : 'Add study time you did without the timer (or drag over empty space)'}>＋ {zh ? '补记' : 'Add'}</button>
    {/if}
    <button class="week-start" onclick={toggleWeekStart}
      title={zh ? `每周从${mondayFirst ? '周一' : '周日'}开始，点一下改为从${mondayFirst ? '周日' : '周一'}开始` : `Weeks start on ${mondayFirst ? 'Monday' : 'Sunday'}; click to start on ${mondayFirst ? 'Sunday' : 'Monday'}`}>
      {zh ? (mondayFirst ? '周一起' : '周日起') : (mondayFirst ? 'Mon first' : 'Sun first')}</button>
    <button class:active={fullDay} aria-pressed={fullDay} onclick={() => { fullDay = !fullDay; }} title={zh ? '显示全天 24 小时' : 'Show all 24 hours'}>24h</button>
  </div>
  {#if error}
    <div class="load-error" role="alert"><span>{zh ? '周历加载失败' : 'Could not load the calendar'}</span>
      <button onclick={() => { refresh += 1; }}>{zh ? '重试' : 'Retry'}</button><small>{error}</small></div>
  {:else}
    <WeekCalendarGrid {events} {weekStart} {loading} {fullDay} {varieties} {editable}
      onmove={moveBlock} onresize={resizeBlock} ondraw={newRecord} onedit={editRound} ondelete={deleteRound} />
  {/if}
</section>
{#if editor}
  <RecordEditor record={editor.record} editing={editor.id !== null} archivedSubject={editor.archivedSubject}
    onsave={saveEditor} ondelete={editor.id !== null ? deleteFromEditor : undefined} onclose={() => { editor = null; }} />
{/if}

<style>
  .week-calendar { display: flex; flex-direction: column; min-height: 0; padding: 10px 16px 12px; }
  .toolbar { display: flex; align-items: center; gap: 10px; min-height: 36px; margin-bottom: 8px; }
  .navigation { display: flex; gap: 2px; }
  .range { display: flex; align-items: baseline; gap: 8px; margin: 0 4px; font-size: 18px; font-weight: 700; letter-spacing: -0.01em; color: var(--ui-text); white-space: nowrap; }
  .range small { font-size: 13px; font-weight: 500; color: var(--ui-text-muted); }
  .spacer { flex: 1; }
  .legend { flex: 1; display: flex; gap: 4px 14px; min-width: 0; overflow-x: auto; scrollbar-width: none; font-size: 12.5px; font-weight: 600; color: var(--ui-text); }
  .legend span { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .legend i { position: relative; display: grid; justify-items: center; width: 14px; height: 12px; border-radius: 50%; flex-shrink: 0; box-shadow: inset 0 -2px 0 rgb(0 0 0 / 12%); }
  .legend i :global(.top) { position: absolute; top: -3px; }
  .total { font-size: 12.5px; color: var(--ui-text-muted); white-space: nowrap; }
  .total b { margin-left: 2px; font-size: 16px; font-weight: 750; color: var(--ui-text); font-variant-numeric: tabular-nums; }
  .total small { margin: 0 1px; font-size: 11px; font-weight: 600; }
  button { height: 30px; min-width: 30px; padding: 0 12px; border: 1px solid var(--ui-border-strong); border-radius: 9px; background: var(--ui-surface); color: var(--ui-text); font: inherit; font-size: 12.5px; font-weight: 650; cursor: pointer; transition: background 150ms ease, border-color 150ms ease; }
  .arrow { width: 30px; padding: 0; border-color: transparent; background: none; font-size: 20px; line-height: 1; }
  button:hover:not(:disabled) { background: var(--ui-hover); border-color: color-mix(in srgb, var(--ui-brand) 45%, var(--ui-border-strong)); }
  .arrow:hover:not(:disabled) { border-color: transparent; }
  button.active { background: var(--ui-brand); border-color: var(--ui-brand); color: var(--ui-on-brand); }
  button:disabled { opacity: 0.4; cursor: default; }
  .loading-label { font-size: 12px; color: var(--ui-text-muted); }
  .load-error { display: flex; gap: 12px; flex-wrap: wrap; align-items: center; padding: 25px; font-size: 13px; }
  .load-error small { flex-basis: 100%; overflow-wrap: anywhere; }
  @media (max-width: 620px) { .week-calendar { padding: 8px 10px; } .range small, .legend { display: none; } .range { font-size: 15px; } }
</style>
