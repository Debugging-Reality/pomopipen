<script lang="ts">
  // One jot in the pad: tomato stamp, the text (click to edit), where it came
  // up, and — while open — "→ Task" (pick a list) and delete. Handled jots show
  // when they were crossed off or which list they went to, with a way back.
  import type { Jot, Subject } from '$lib/types';
  import JotStamp from './JotStamp.svelte';
  import { isSendKey, jotWhen } from '$lib/utils/jots';
  import * as m from '$paraglide/messages.js';

  let {
    jot,
    zh,
    subjects,
    taskSubject,
    popping = false,
    now,
    oncross,
    ontask,
    onuntask,
    ondelete,
    onedit,
  }: {
    jot: Jot;
    zh: boolean;
    subjects: Subject[];
    /** For a converted jot: the list its task is in (`null` = Uncategorised, `undefined` = unknown). */
    taskSubject?: Subject | null;
    popping?: boolean;
    now: Date;
    oncross: () => void;
    ontask: (subjectId: number | null) => void;
    onuntask: () => void;
    ondelete: () => void;
    onedit: (body: string) => void;
  } = $props();

  let editing = $state(false);
  let choosing = $state(false);
  // Crossed off (struck through). A jot turned into a task moved on instead: muted, not struck.
  let done = $derived((jot.done_at !== null && jot.task_id === null) || popping);
  let tasked = $derived(jot.task_id !== null);
  let context = $derived(jot.subject_id === null ? null : (subjects.find((s) => s.id === jot.subject_id) ?? null));

  function focusSelect(el: HTMLTextAreaElement) {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
    grow(el);
  }

  function grow(el: HTMLTextAreaElement) {
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 120)}px`;
  }

  function commit(el: HTMLTextAreaElement) {
    if (!editing) return;
    editing = false;
    const body = el.value.trim();
    if (body && body !== jot.body) onedit(body);
  }

  function focusFirst(el: HTMLElement) {
    el.querySelector<HTMLButtonElement>('button')?.focus();
  }
</script>

<div class="jot" class:done class:tasked>
  {#if tasked}
    <span class="tasked-mark" title={zh ? '已转为任务' : 'Now a task'} aria-hidden="true">
      <svg viewBox="0 0 16 16"><path d="M3 8h8M8 4.5 11.5 8 8 11.5" /></svg>
    </span>
  {:else}
    <JotStamp
      checked={done}
      pop={popping}
      label={done ? (zh ? '恢复成未完成' : 'Bring back') : (zh ? '划掉' : 'Cross off')}
      onclick={oncross}
    />
  {/if}

  <div class="main">
    {#if editing}
      <textarea
        class="edit"
        value={jot.body}
        maxlength="500"
        aria-label={zh ? '修改这条碎碎念' : 'Edit this jot'}
        use:focusSelect
        oninput={(e) => grow(e.currentTarget)}
        onblur={(e) => commit(e.currentTarget)}
        onkeydown={(e) => {
          e.stopPropagation();
          if (isSendKey(e)) {
            e.preventDefault();
            e.currentTarget.blur();
          } else if (e.key === 'Escape') {
            editing = false;
          }
        }}
      ></textarea>
    {:else}
      <button class="body" disabled={done || tasked} onclick={() => (editing = true)} title={done || tasked ? undefined : zh ? '点击修改' : 'Click to edit'}>
        <span class="txt">{jot.body}</span>
      </button>
    {/if}

    {#if choosing}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div class="choose" role="group" aria-label={zh ? '放进哪个任务栏？' : 'Add to which list?'} use:focusFirst
        onkeydown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); choosing = false; } }}>
        <span class="choose-q">{zh ? '放进哪一栏？' : 'Add to'}</span>
        <button class="chip" onclick={() => { choosing = false; ontask(null); }}>
          <i class="dot none"></i>{m.subject_filter_uncategorized()}
        </button>
        {#each subjects as s (s.id)}
          <button class="chip" class:was={s.id === jot.subject_id} onclick={() => { choosing = false; ontask(s.id); }}
            title={s.id === jot.subject_id ? (zh ? '写下时正在学这一科' : 'You were studying this when you wrote it') : undefined}>
            <i class="dot" style:background={s.color}></i>{s.name}
          </button>
        {/each}
        <button class="chip ghost" onclick={() => (choosing = false)}>{zh ? '取消' : 'Cancel'}</button>
      </div>
    {:else}
      <div class="meta">
        {#if tasked}
          <span class="went">→ {zh ? '已转为任务' : 'Now a task'}{taskSubject !== undefined ? ` · ${taskSubject?.name ?? m.subject_filter_uncategorized()}` : ''}</span>
          <button class="link" onclick={onuntask}>{zh ? '撤回' : 'Take back'}</button>
        {:else if jot.done_at !== null && !popping}
          <span>{zh ? '划掉于' : 'Crossed off'} {jotWhen(jot.done_at, now, zh)}</span>
        {:else}
          <span>{jotWhen(jot.created_at, now, zh)}</span>
          {#if jot.in_focus}<span class="focus-tag">{zh ? '专注时' : 'mid-focus'}</span>{/if}
          {#if context}<span class="ctx"><i class="dot" style:background={context.color}></i>{context.name}</span>{/if}
        {/if}
      </div>
    {/if}
  </div>

  {#if !done && !tasked && !editing && !choosing}
    <div class="acts">
      <button class="act to-task" onclick={() => (choosing = true)} title={zh ? '转成任务' : 'Make it a task'}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.2" y="3" width="3" height="3" rx=".5" /><path d="M7.4 4.5h6.4M2.2 10.2h6M10.4 7.6l2.6 2.6-2.6 2.6" /></svg>
        <span>{zh ? '任务' : 'Task'}</span>
      </button>
      <button class="act danger" onclick={ondelete} aria-label={zh ? '删除' : 'Delete'} title={zh ? '删除' : 'Delete'}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13" /></svg>
      </button>
    </div>
  {:else if jot.done_at !== null && !popping && !tasked}
    <div class="acts">
      <button class="act danger" onclick={ondelete} aria-label={zh ? '删除' : 'Delete'} title={zh ? '删除' : 'Delete'}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13" /></svg>
      </button>
    </div>
  {/if}
</div>

<style>
  .jot {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 9px 10px 8px 12px;
    border-radius: var(--jot-row-radius, 10px);
    transition: background 150ms ease;
  }

  .jot:hover {
    background: var(--ui-hover);
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-top: 1px;
  }

  .body {
    display: block;
    width: 100%;
    padding: 1px 4px;
    margin: 0 -4px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    line-height: 1.45;
    text-align: left;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    cursor: text;
  }

  .body:disabled {
    cursor: default;
  }

  .body:not(:disabled):hover {
    background: color-mix(in srgb, var(--ui-surface) 70%, transparent);
  }

  /* A pen stroke drawn left to right through every line of the text. */
  .txt {
    background: linear-gradient(currentColor, currentColor) no-repeat 0 58% / 0 1.5px;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
    transition:
      background-size 340ms cubic-bezier(0.4, 0, 0.2, 1) 120ms,
      color 300ms ease 200ms;
  }

  .done .txt {
    background-size: 100% 1.5px;
    color: var(--ui-text-muted);
  }

  .tasked .txt {
    color: var(--ui-text-muted);
  }

  .edit {
    width: 100%;
    min-height: 30px;
    padding: 5px 7px;
    margin: -3px -4px 0;
    border: 1.5px solid var(--ui-brand);
    border-radius: 6px;
    background: var(--ui-surface);
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    line-height: 1.45;
    resize: none;
    outline: none;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px 8px;
    font-size: 11.5px;
    line-height: 16px;
    color: var(--ui-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .focus-tag {
    padding: 0 5px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--color-focus-round) 12%, transparent);
    color: var(--jot-focus-ink, var(--ui-brand-strong));
    font-weight: 650;
  }

  .ctx {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    display: inline-block;
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .dot.none {
    border: 1.2px dashed var(--ui-text-muted);
  }

  .went {
    color: var(--ui-text);
    font-weight: 600;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--ui-brand-strong);
    font: inherit;
    font-weight: 650;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .tasked-mark {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 5px;
    background: var(--ui-selected);
    color: var(--ui-brand-strong);
  }

  .tasked-mark svg,
  .act svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tasked-mark svg {
    width: 14px;
    height: 14px;
  }

  /* ── Actions: float over the row's end on hover, so text keeps the full width ── */
  .acts {
    position: absolute;
    top: 6px;
    right: 8px;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--ui-surface);
    box-shadow: 0 0 0 1px var(--ui-border), 0 4px 10px -6px color-mix(in srgb, var(--ui-text) 40%, transparent);
    opacity: 0;
    pointer-events: none;
    transition: opacity 120ms ease;
  }

  .jot:hover .acts,
  .jot:focus-within .acts {
    opacity: 1;
    pointer-events: auto;
  }

  .act {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 24px;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--ui-text-muted);
    font: inherit;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
  }

  .act svg {
    width: 14px;
    height: 14px;
  }

  .act:hover {
    background: var(--ui-hover);
    color: var(--ui-text);
  }

  .act.danger:hover {
    background: var(--ui-danger-soft);
    color: var(--ui-danger);
  }

  /* ── "→ Task": pick the list ── */
  .choose {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 5px;
    margin-top: 4px;
  }

  .choose-q {
    font-size: 12px;
    font-weight: 650;
    color: var(--ui-text-muted);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    max-width: 150px;
    height: 24px;
    padding: 0 9px;
    border: 1px solid var(--ui-border-strong);
    border-radius: 999px;
    background: var(--ui-surface);
    color: var(--ui-text);
    font: inherit;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .chip:hover {
    border-color: var(--ui-brand);
  }

  .chip.was {
    border-style: dashed;
  }

  .chip.ghost {
    border-color: transparent;
    background: none;
    color: var(--ui-text-muted);
  }

  /* Classic Tomato: printed-label chips and an inked action tab. */
  :global(html[data-pomo-theme='classic-tomato']) .acts {
    border-radius: 4px;
    background: var(--pomo-light);
    box-shadow: 0 0 0 1.5px var(--pomo-ink), 2px 2px 0 var(--pomo-ink);
  }

  :global(html[data-pomo-theme='classic-tomato']) :is(.act, .tasked-mark, .body, .edit) {
    border-radius: 3px;
  }

  :global(html[data-pomo-theme='classic-tomato']) .chip {
    border: 1.5px solid color-mix(in srgb, var(--pomo-ink) 55%, transparent);
    border-radius: 3px;
    background: var(--pomo-light);
  }

  :global(html[data-pomo-theme='classic-tomato']) .chip:hover {
    border-color: var(--pomo-ink);
  }

  :global(html[data-pomo-theme='classic-tomato']) .chip.ghost {
    border-color: transparent;
    background: none;
  }

  :global(html[data-pomo-theme='classic-tomato']) .edit {
    border-color: var(--pomo-ink);
    outline: 2px solid var(--pomo-alt);
    outline-offset: 2px;
  }

  @media (prefers-reduced-motion: reduce) {
    .txt {
      transition: none;
    }
  }
</style>
