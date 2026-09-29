<script lang="ts">
  // Compact button + popover for choosing which subject the current (or next)
  // work round is attributed to. Lives in the main timer window; selecting a
  // subject calls subjects_set_active, which re-tags an in-flight round too.
  import { onMount } from 'svelte';
  import { settings } from '$lib/stores/settings';
  import { subjects, watchSubjects } from '$lib/stores/subjects';
  import { tasks, watchTasks } from '$lib/stores/tasks';
  import { subjectsSetActive } from '$lib/ipc';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import * as m from '$paraglide/messages.js';

  let open = $state(false);

  let active = $derived($subjects.find((s) => s.id === $settings.active_subject_id) ?? null);
  // Feedback only — picking a specific task happens on the Tasks page, not
  // here. This window just shows what's currently pinned, if anything.
  let activeTask = $derived($tasks.find((t) => t.id === $settings.active_task_id) ?? null);

  function toggleOpen() {
    open = !open;
  }

  async function pick(id: number | null) {
    open = false;
    await subjectsSetActive(id);
  }

  function handleWindowClick(e: MouseEvent) {
    if (!(e.target instanceof HTMLElement) || !e.target.closest('.subject-picker')) {
      open = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];
    (async () => {
      cleanups.push(await watchSubjects());
      // Not filtered to pending tasks — the active task must still resolve
      // to a name even after it's marked done mid-round.
      cleanups.push(await watchTasks(true));
    })();
    window.addEventListener('click', handleWindowClick);
    window.addEventListener('keydown', handleKeydown);
    return () => {
      window.removeEventListener('click', handleWindowClick);
      window.removeEventListener('keydown', handleKeydown);
      for (const fn of cleanups) fn();
    };
  });
</script>

<div class="subject-picker">
  <button
    class="trigger"
    onclick={toggleOpen}
    aria-label={m.subject_picker_label()}
    aria-expanded={open}
  >
    <span class="dot" style="background: {active?.color ?? 'transparent'}" class:dot-none={!active}
    ></span>
    <span class="labels">
      <span class="name">{active?.name ?? m.subject_picker_none()}</span>
      {#if activeTask}
        <span class="task-name">{activeTask.title}</span>
      {/if}
    </span>
  </button>

  {#if open}
    <div class="menu" role="menu">
      <button
        class="menu-item"
        class:selected={active === null}
        role="menuitem"
        onclick={() => pick(null)}
      >
        <span class="dot dot-none"></span>
        <span>{m.subject_picker_none()}</span>
      </button>
      {#if $subjects.length === 0}
        <div class="menu-hint">{m.subject_picker_empty()}</div>
      {:else}
        {#each $subjects as subject (subject.id)}
          <button
            class="menu-item"
            class:selected={active?.id === subject.id}
            role="menuitem"
            onclick={() => pick(subject.id)}
          >
            <span class="dot" style="background: {subject.color}"></span>
            <span>{subject.name}</span>
          </button>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .subject-picker {
    position: relative;
    display: flex;
    justify-content: center;
    /* Collapse the gap above: sits right under the round-label with the
       same tight spacing convention used there. */
    margin-top: -4px;
  }

  .trigger {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    padding: 3px 9px;
    border-radius: 8px;
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.72rem;
    max-width: 180px;
    transition:
      color 0.15s,
      background 0.15s;
  }

  .trigger:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .labels {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
  }

  .task-name {
    font-size: 0.64rem;
    opacity: 0.7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 160px;
  }

  .dot {
    flex-shrink: 0;
    /* Nudged down to align with the (taller) first line's cap-height now
       that the trigger can be two lines tall. */
    margin-top: 4px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .dot-none {
    background: none;
    border: 1.5px dashed color-mix(in oklch, var(--color-foreground) 40%, transparent);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu {
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    margin-top: 4px;
    background: var(--color-background-light);
    border: 1px solid color-mix(in oklch, var(--color-foreground) 10%, transparent);
    border-radius: 6px;
    box-shadow: 0 4px 16px color-mix(in oklch, black 25%, transparent);
    padding: 4px;
    min-width: 160px;
    max-width: 220px;
    max-height: 200px;
    overflow-y: auto;
    z-index: 20;
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px 8px;
    border-radius: 4px;
    font-size: 0.78rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .menu-item:hover {
    background: var(--color-hover);
  }

  .menu-item.selected {
    background: color-mix(in oklch, var(--color-accent) 14%, transparent);
  }

  .menu-hint {
    padding: 8px;
    font-size: 0.72rem;
    color: var(--color-foreground-darker, var(--color-foreground));
    opacity: 0.7;
    max-width: 190px;
    white-space: normal;
  }
</style>
