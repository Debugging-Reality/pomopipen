<script lang="ts">
  // Week calendar: add or change one focus record by hand (date, start, end,
  // subject, task). Saving and deleting are done by the parent, which shows
  // the undo toast; errors from the backend are shown here.
  import { onMount, untrack } from 'svelte';
  import { tasksList } from '$lib/ipc';
  import type { FocusRecord, Task } from '$lib/types';
  import { subjects } from '$lib/stores/subjects';
  import { dateKey, MAX_RECORD_SECS } from '$lib/utils/calendar';
  import { getLocale } from '$paraglide/runtime.js';
  import * as m from '$paraglide/messages.js';

  let { record, editing = false, archivedSubject = null, onsave, ondelete, onclose }: {
    record: FocusRecord;
    /** Changing an existing record (shows Delete) rather than adding one. */
    editing?: boolean;
    /** Name of the record's subject when it is archived, i.e. not in the list. */
    archivedSubject?: string | null;
    onsave: (record: FocusRecord) => Promise<void>;
    ondelete?: () => Promise<void>;
    onclose: () => void;
  } = $props();

  let zh = $derived(getLocale().startsWith('zh'));
  const pad = (n: number) => String(n).padStart(2, '0');
  const hhmm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  // The form starts from the record it was opened with.
  const initial = untrack(() => record);
  const initialStart = new Date(initial.started_at * 1000);
  const initialEnd = new Date((initial.started_at + initial.duration_secs) * 1000);
  let date = $state(dateKey(initialStart));
  let start = $state(hhmm(initialStart));
  let end = $state(hhmm(initialEnd));
  let subject = $state(initial.subject_id === null ? '' : String(initial.subject_id));
  let task = $state(initial.task_id === null ? '' : String(initial.task_id));
  let tasks = $state<Task[]>([]);
  let busy = $state(false);
  let error = $state('');
  let startInput = $state<HTMLInputElement>();

  onMount(() => {
    startInput?.focus();
    tasksList(true).then((list) => {
      tasks = list;
      if (task && !list.some((t) => String(t.id) === task)) task = '';
    }).catch(() => { tasks = []; });
  });

  // Start and end on the wall clock; an end at or before the start is the next day.
  let times = $derived.by(() => {
    const [y, mo, d] = date.split('-').map(Number);
    const [sh, sm] = start.split(':').map(Number);
    const [eh, em] = end.split(':').map(Number);
    if (![y, mo, d, sh, sm, eh, em].every(Number.isFinite)) return null;
    const from = new Date(y, mo - 1, d, sh, sm);
    const to = new Date(y, mo - 1, d, eh, em);
    const nextDay = to <= from;
    if (nextDay) to.setDate(to.getDate() + 1);
    return { from: Math.floor(from.getTime() / 1000), to: Math.floor(to.getTime() / 1000), nextDay };
  });
  let problem = $derived.by(() => {
    if (!times) return zh ? '请填写日期、开始和结束时间' : 'Fill in the date, start and end';
    if (times.to - times.from > MAX_RECORD_SECS) return zh ? '一条记录最长 12 小时' : 'A record lasts at most 12 hours';
    if (times.to > Date.now() / 1000 + 60) return zh ? '记录不能结束在未来' : "A record can't end in the future";
    return '';
  });
  let minutes = $derived(times ? Math.round((times.to - times.from) / 60) : 0);
  let lengthLabel = $derived.by(() => {
    const h = Math.floor(minutes / 60);
    const rest = minutes % 60;
    const text = zh ? `${h ? `${h} 小时 ` : ''}${rest || !h ? `${rest} 分钟` : ''}`.trim() : `${h ? `${h} h ` : ''}${rest || !h ? `${rest} min` : ''}`.trim();
    return times?.nextDay ? (zh ? `${text}，结束于次日` : `${text}, ends the next day`) : text;
  });
  let subjectOptions = $derived($subjects.filter((s) => !s.archived));
  let taskOptions = $derived(
    tasks
      .filter((t) => (subject === '' ? t.subject_id === null : String(t.subject_id) === subject) || String(t.id) === task)
      .sort((a, b) => Number(a.done) - Number(b.done) || a.sort_order - b.sort_order),
  );

  function pickSubject(value: string) {
    subject = value;
    const current = tasks.find((t) => String(t.id) === task);
    if (current && String(current.subject_id ?? '') !== value) task = '';
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!times || problem || busy) return;
    busy = true;
    error = '';
    try {
      await onsave({
        started_at: times.from,
        duration_secs: times.to - times.from,
        subject_id: subject === '' ? null : Number(subject),
        task_id: task === '' ? null : Number(task),
      });
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!ondelete || busy) return;
    busy = true;
    error = '';
    try {
      await ondelete();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="scrim" onclick={(e) => { if (e.target === e.currentTarget && !busy) onclose(); }}
  onkeydown={(e) => { if (e.key === 'Escape' && !busy) onclose(); }}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="record-title">
  <form onsubmit={save}>
    <h3 id="record-title">{editing ? (zh ? '编辑学习记录' : 'Edit focus record') : (zh ? '补记一段学习' : 'Add a focus record')}</h3>
    <p class="sub">{zh ? '和计时器记下的番茄一样计入统计、任务用时和 Google 日历。' : 'Counts like a timed round: in stats, task totals and Google Calendar.'}</p>

    <div class="grid">
      <label class="field wide"><span>{zh ? '日期' : 'Date'}</span>
        <input class="s-input" type="date" bind:value={date} max={dateKey(new Date())} required /></label>
      <label class="field"><span>{zh ? '开始' : 'Start'}</span>
        <input class="s-input" type="time" bind:value={start} bind:this={startInput} required /></label>
      <label class="field"><span>{zh ? '结束' : 'End'}</span>
        <input class="s-input" type="time" bind:value={end} required /></label>
      <p class="length" class:invalid={!!problem} aria-live="polite">{problem || (zh ? `共 ${lengthLabel}` : lengthLabel)}</p>

      <label class="field wide"><span>{zh ? '科目' : 'Subject'}</span>
        <select class="s-input" value={subject} onchange={(e) => pickSubject(e.currentTarget.value)}>
          <option value="">{m.subject_filter_uncategorized()}</option>
          {#each subjectOptions as s (s.id)}<option value={String(s.id)}>{s.name}</option>{/each}
          {#if record.subject_id !== null && !subjectOptions.some((s) => s.id === record.subject_id)}
            <option value={String(record.subject_id)}>{archivedSubject ?? '…'}{zh ? '（已归档）' : ' (archived)'}</option>
          {/if}
        </select></label>
      <label class="field wide"><span>{zh ? '任务' : 'Task'}</span>
        <select class="s-input" bind:value={task}>
          <option value="">{zh ? '不指定任务' : 'No task'}</option>
          {#each taskOptions as t (t.id)}<option value={String(t.id)}>{t.title}{t.done ? (zh ? '（已完成）' : ' (done)') : ''}</option>{/each}
        </select></label>
    </div>

    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="foot">
      {#if editing && ondelete}
        <button type="button" class="s-btn s-btn--danger" disabled={busy} onclick={remove}>{zh ? '删除这条记录' : 'Delete'}</button>
      {/if}
      <span class="spacer"></span>
      <button type="button" class="s-btn" disabled={busy} onclick={onclose}>{zh ? '取消' : 'Cancel'}</button>
      <button type="submit" class="s-btn s-btn--primary" disabled={busy || !!problem}>{zh ? '保存' : 'Save'}</button>
    </div>
  </form>
  </div>
</div>

<style>
  .scrim { position: fixed; inset: 0; z-index: 400; display: grid; place-items: center; padding: 20px; background: color-mix(in srgb, var(--ui-text) 28%, transparent); }
  .dialog { width: min(440px, 100%); max-height: calc(100vh - 40px); overflow-y: auto; padding: 18px 20px 16px; border: 1px solid var(--ui-border-strong); border-radius: 14px; background: var(--ui-surface); color: var(--ui-text); box-shadow: 0 18px 40px -18px color-mix(in srgb, var(--ui-text) 55%, transparent); }
  h3 { font-size: 17px; font-weight: 750; letter-spacing: -0.01em; }
  .sub { margin: 3px 0 14px; font-size: 12.5px; line-height: 1.5; color: var(--ui-text-muted); }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px 12px; }
  .field { display: flex; flex-direction: column; gap: 5px; min-width: 0; font-size: 12.5px; font-weight: 600; color: var(--ui-text-muted); }
  .field.wide { grid-column: 1 / -1; }
  .field .s-input { width: 100%; min-width: 0; color-scheme: light dark; }
  .length { grid-column: 1 / -1; margin: -2px 0 2px; font-size: 12.5px; color: var(--ui-text-muted); font-variant-numeric: tabular-nums; }
  .length.invalid, .error { color: var(--ui-danger); font-weight: 600; }
  .error { margin-top: 10px; font-size: 12.5px; overflow-wrap: anywhere; }
  .foot { display: flex; align-items: center; gap: 8px; margin-top: 16px; }
  .spacer { flex: 1; }
</style>
