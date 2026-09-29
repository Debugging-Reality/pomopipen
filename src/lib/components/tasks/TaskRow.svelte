<script lang="ts">
  // One todolist item: checkbox, inline-editable title, estimate vs actual
  // time, "set as current" pin, and delete (with inline confirm).
  import type { Task } from '$lib/types';
  import {
    tasksSetDone,
    tasksRename,
    tasksSetEstimate,
    tasksDelete,
    tasksSetActive,
  } from '$lib/ipc';
  import * as m from '$paraglide/messages.js';
  import { notify } from '$lib/stores/toast';
  import { getLocale } from '$paraglide/runtime.js';

  let { task, isActive }: { task: Task; isActive: boolean } = $props();
  let zh = $derived(getLocale().startsWith('zh'));

  let renaming = $state(false);
  let editingEstimate = $state(false);
  let confirmingDelete = $state(false);

  // Svelte action: focuses + selects on mount. The HTML `autofocus` attribute
  // is unreliable for elements a conditional block inserts after initial
  // page load — see the note in SubjectsSection.svelte for the full reasoning.
  function focusAndSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  function fmtMinutes(totalMin: number): string {
    const h = Math.floor(totalMin / 60);
    const min = Math.round(totalMin % 60);
    if (h === 0) return `${min}m`;
    return `${h}h ${String(min).padStart(2, '0')}m`;
  }

  async function toggleDone() {
    const { id, title, done } = task;
    await tasksSetDone(id, !done);
    if (!done) {
      notify(zh ? `完成「${title}」` : `Done: ${title}`, {
        action: { label: zh ? '撤销' : 'Undo', run: () => void tasksSetDone(id, false) },
      });
    }
  }

  async function commitRename(el: HTMLInputElement) {
    const value = el.value.trim();
    renaming = false;
    if (!value || value === task.title) return;
    await tasksRename(task.id, value);
  }

  async function commitEstimate(el: HTMLInputElement) {
    editingEstimate = false;
    const raw = el.value.trim();
    if (raw === '') {
      if (task.est_minutes !== null) await tasksSetEstimate(task.id, null);
      return;
    }
    const parsed = Math.round(Number(raw));
    if (!Number.isFinite(parsed) || parsed <= 0) return;
    await tasksSetEstimate(task.id, Math.min(parsed, 1440));
  }

  async function setActive() {
    await tasksSetActive(isActive ? null : task.id);
  }

  async function doDelete() {
    confirmingDelete = false;
    await tasksDelete(task.id);
  }
</script>

{#if confirmingDelete}
  <div class="confirm-row">
    <span class="confirm-label">{m.tasks_delete_confirm({ title: task.title })}</span>
    <div class="confirm-actions">
      <button class="s-btn" onclick={() => (confirmingDelete = false)}>{m.tasks_cancel()}</button>
      <button class="s-btn s-btn--danger-solid" onclick={doDelete}>{m.tasks_delete()}</button>
    </div>
  </div>
{:else}
  <div class="task-row" class:done={task.done} class:active={isActive}>
    <button
      class="checkbox"
      class:checked={task.done}
      onclick={toggleDone}
      aria-label={m.tasks_checkbox_label()}
      role="checkbox"
      aria-checked={task.done}
    >
      <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
        <path d="M2 5.8 L4.4 8.1 L9 2.9" />
      </svg>
    </button>

    {#if renaming}
      <input
        class="s-input title-input"
        type="text"
        value={task.title}
        maxlength="200"
        use:focusAndSelect
        onblur={(e) => commitRename(e.target as HTMLInputElement)}
        onkeydown={(e) => {
          if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
          else if (e.key === 'Escape') renaming = false;
        }}
      />
    {:else}
      <button class="title" onclick={() => (renaming = true)} aria-label={m.tasks_rename_label()} title={zh ? '点击改名' : 'Click to rename'}>
        {task.title}
      </button>
    {/if}

    {#if isActive}
      <span class="current-badge">{m.tasks_current_badge()}</span>
    {/if}

    <div class="chips">
      {#if editingEstimate}
        <input
          class="s-input est-input"
          type="number"
          min="1"
          max="1440"
          placeholder={m.tasks_est_placeholder()}
          value={task.est_minutes ?? ''}
          use:focusAndSelect
          onblur={(e) => commitEstimate(e.target as HTMLInputElement)}
          onkeydown={(e) => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
            else if (e.key === 'Escape') editingEstimate = false;
          }}
        />
      {:else}
        <button class="chip est" class:empty={task.est_minutes === null} onclick={() => (editingEstimate = true)}
          title={zh ? '点击设置预计时长' : 'Click to set an estimate'}>
          {m.tasks_estimate_label()}
          {task.est_minutes !== null ? fmtMinutes(task.est_minutes) : '—'}
        </button>
      {/if}
      {#if task.actual_secs > 0}
        <span class="chip actual" class:over={task.est_minutes !== null && task.actual_secs / 60 > task.est_minutes}>
          {m.tasks_actual_label()}
          {fmtMinutes(Math.round(task.actual_secs / 60))}
        </span>
      {/if}
    </div>

    <button
      class="icon pin"
      class:pinned={isActive}
      onclick={setActive}
      aria-label={m.tasks_set_current()}
      title={m.tasks_set_current()}
      aria-pressed={isActive}
    >
      <svg width="16" height="16" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.8l-5.2 2.8 1-5.8-4.3-4.1 5.9-.9L12 3.5Z" />
      </svg>
    </button>
    <button
      class="icon danger"
      onclick={() => (confirmingDelete = true)}
      aria-label={m.tasks_delete()}
      title={m.tasks_delete()}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v6M14 11v6" />
      </svg>
    </button>
  </div>
{/if}

<style>
  .task-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    padding: 8px 10px 8px 14px;
    transition: background 150ms ease;
  }

  .task-row:hover {
    background: var(--ui-hover);
  }

  .task-row.active {
    background: color-mix(in srgb, var(--ui-selected) 55%, var(--ui-surface));
  }

  .task-row.active::before {
    content: '';
    position: absolute;
    left: 0;
    top: 10px;
    bottom: 10px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--ui-brand);
  }

  .checkbox {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid var(--ui-border-strong);
    background: var(--ui-surface);
    cursor: pointer;
    transition:
      background 150ms ease,
      border-color 150ms ease,
      transform 150ms ease;
  }

  .checkbox svg {
    fill: none;
    stroke: var(--ui-on-brand);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0;
    transition: opacity 150ms ease;
  }

  .checkbox:hover {
    border-color: var(--ui-brand);
  }

  .checkbox:hover svg {
    opacity: 0.35;
    stroke: var(--ui-brand);
  }

  .checkbox:active {
    transform: scale(0.9);
  }

  .checkbox.checked {
    background: var(--ui-brand);
    border-color: var(--ui-brand);
  }

  .checkbox.checked svg {
    opacity: 1;
    stroke: var(--ui-on-brand);
  }

  .title {
    flex: 1;
    min-width: 0;
    padding: 3px 6px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    text-align: left;
    cursor: text;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: background 150ms ease;
  }

  .title:hover {
    background: var(--ui-surface);
  }

  .done .title {
    color: var(--ui-text-muted);
    text-decoration: line-through;
  }

  .title-input {
    flex: 1;
    min-width: 0;
    height: 30px;
  }

  .current-badge {
    flex-shrink: 0;
    padding: 2px 8px;
    border-radius: 999px;
    background: var(--ui-brand);
    color: var(--ui-on-brand);
    font-size: 11.5px;
    font-weight: 700;
  }

  .chips {
    display: flex;
    gap: 5px;
    flex-shrink: 0;
  }

  .chip {
    height: 24px;
    padding: 0 9px;
    border-radius: 999px;
    border: 1px solid var(--ui-border);
    background: var(--ui-page);
    color: var(--ui-text-muted);
    font: inherit;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  button.chip {
    cursor: pointer;
    transition:
      border-color 150ms ease,
      color 150ms ease;
  }

  button.chip:hover {
    border-color: var(--ui-brand);
    color: var(--ui-text);
  }

  .chip.est.empty {
    border-style: dashed;
  }

  .chip.actual {
    background: var(--ui-selected);
    border-color: transparent;
    color: var(--ui-brand-strong);
    font-weight: 600;
  }

  .chip.actual.over {
    background: var(--ui-danger-soft);
    color: var(--ui-danger);
  }

  .est-input {
    width: 72px;
    height: 26px;
    padding: 0 8px;
    font-size: 12px;
  }

  .icon {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text-muted);
    cursor: pointer;
    opacity: 0.6;
    transition:
      opacity 150ms ease,
      background 150ms ease,
      color 150ms ease;
  }

  .icon svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .task-row:hover .icon,
  .icon:focus-visible,
  .icon.pinned {
    opacity: 1;
  }

  .icon:hover {
    background: var(--ui-surface);
    color: var(--ui-text);
  }

  .icon.pinned {
    color: var(--ui-brand);
  }

  .icon.pinned svg {
    fill: var(--ui-accent-2);
    stroke: var(--ui-brand);
  }

  .icon.danger:hover {
    background: var(--ui-danger-soft);
    color: var(--ui-danger);
  }

  .confirm-row {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    background: var(--ui-danger-soft);
  }

  .confirm-label {
    font-size: 13px;
    line-height: 1.5;
    color: var(--ui-text);
  }

  .confirm-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
</style>
