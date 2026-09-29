<script lang="ts">
  import { installClickSound } from '$lib/utils/clickSound';
  import '../../app.css';
  import '$lib/styles/ui.css';
  import '$lib/styles/classic-tomato.css';
  import { onMount } from 'svelte';
  import {
    getSettings,
    getThemes,
    onSettingsChanged,
    onThemesChanged,
    onTasksChanged,
    onSessionsChanged,
    tasksList,
    tasksCreate,
  } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import { subjects, watchSubjects } from '$lib/stores/subjects';
  import { tasks } from '$lib/stores/tasks';
  import { applyTheme } from '$lib/stores/theme';
  import { setLocale } from '$lib/locale.svelte.js';
  import { resolveThemeName } from '$lib/utils/theme';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import type { Task } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { info, error as logError } from '@tauri-apps/plugin-log';
  import TaskRow from '$lib/components/tasks/TaskRow.svelte';
  import SettingsTitlebar from '$lib/components/settings/SettingsTitlebar.svelte';
  import ToastHost from '$lib/components/settings/ToastHost.svelte';
  import ResizeHandles from '$lib/components/ResizeHandles.svelte';
  import { notify } from '$lib/stores/toast';
  import { getLocale } from '$paraglide/runtime.js';

  // A short sound on the red primary buttons (Settings → Notifications).
  onMount(installClickSound);

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  let showDone = $state(false);
  let formError = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  interface SectionView {
    subjectId: number | null;
    name: string;
    color: string | null; // null renders the Uncategorized section's dashed dot
    items: Task[];
  }

  // Every active subject gets a section, always — even with zero tasks, so
  // there is somewhere to add the first one. Uncategorized is the same: a
  // fixed section rather than one that appears only once it has an orphan,
  // otherwise there would be no way to create an unassigned task at all.
  let sectionViews = $derived.by((): SectionView[] => {
    const bySubject = new Map<number, Task[]>();
    const uncategorized: Task[] = [];
    for (const t of $tasks) {
      if (t.subject_id === null) {
        uncategorized.push(t);
      } else {
        const arr = bySubject.get(t.subject_id) ?? [];
        arr.push(t);
        bySubject.set(t.subject_id, arr);
      }
    }
    const byOrder = (a: Task, b: Task) => a.sort_order - b.sort_order;
    const subjectViews: SectionView[] = $subjects.map((s) => ({
      subjectId: s.id,
      name: s.name,
      color: s.color,
      items: (bySubject.get(s.id) ?? []).slice().sort(byOrder),
    }));
    return [
      ...subjectViews,
      {
        subjectId: null,
        name: m.subject_filter_uncategorized(),
        color: null,
        items: uncategorized.slice().sort(byOrder),
      },
    ];
  });

  let doneCount = $derived($tasks.filter((t) => t.done).length);
  let openCount = $derived($tasks.filter((t) => !t.done).length);

  function flashError(err: unknown) {
    formError = err instanceof Error ? err.message : String(err);
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (formError = null), 4000);
  }

  async function refreshTasks() {
    try {
      tasks.set(await tasksList(showDone));
    } catch (e) {
      await logError(`[tasks] failed to refresh task list: ${e}`);
    }
  }

  async function toggleShowDone() {
    showDone = !showDone;
    await refreshTasks();
  }

  async function handleAdd(e: SubmitEvent, subjectId: number | null) {
    e.preventDefault();
    const form = e.currentTarget as HTMLFormElement;
    const titleInput = form.elements.namedItem('title') as HTMLInputElement;
    const estInput = form.elements.namedItem('est') as HTMLInputElement;
    const title = titleInput.value.trim();
    if (!title) return;
    const estRaw = estInput.value.trim();
    const est = estRaw ? Math.min(1440, Math.max(1, Math.round(Number(estRaw)))) : null;
    try {
      await tasksCreate(title, subjectId, est);
      form.reset();
      notify(zh ? `已添加「${title}」` : `Added “${title}”`);
      titleInput.focus();
    } catch (err) {
      flashError(err);
    }
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];

    (async () => {
      try {
        const s = await getSettings();
        settings.set(s);
        setLocale(s.language);
        await info(`[tasks] settings loaded, locale=${s.language}`);

        const themes = await getThemes();
        const osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const activeTheme = themes.find((t) => t.name === resolveThemeName(s, osDark)) ?? themes[0];
        if (activeTheme) applyTheme(activeTheme);

        await getCurrentWebviewWindow().show();

        cleanups.push(await watchSubjects());
        await refreshTasks();
        cleanups.push(await onTasksChanged(refreshTasks));
        // Task totals include records edited in the week calendar.
        cleanups.push(await onSessionsChanged(refreshTasks));
        await info(`[tasks] initialized, theme=${activeTheme?.name ?? 'none'}`);
      } catch (e) {
        await logError(`[tasks] initialization failed: ${e}`);
        throw e;
      }

      cleanups.push(
        await onSettingsChanged(async (updated) => {
          const prev = {
            mode: $settings.theme_mode,
            light: $settings.theme_light,
            dark: $settings.theme_dark,
            language: $settings.language,
          };
          settings.set(updated);
          if (updated.language !== prev.language) {
            setLocale(updated.language);
          }
          if (
            updated.theme_mode !== prev.mode ||
            updated.theme_light !== prev.light ||
            updated.theme_dark !== prev.dark
          ) {
            const allThemes = await getThemes();
            const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            const t = allThemes.find((th) => th.name === resolveThemeName(updated, dark));
            if (t) applyTheme(t);
          }
        }),
        await onThemesChanged((updated) => {
          const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
          const current =
            updated.find((t) => t.name === resolveThemeName($settings, dark)) ?? updated[0];
          if (current) applyTheme(current);
        })
      );
    })();

    return () => {
      clearTimeout(errorTimer);
      for (const fn of cleanups) fn();
    };
  });
</script>

<ResizeHandles />

<div class="window pomo-ui">
  <SettingsTitlebar title={m.tasks_title()} />

  <div class="content">
    <div class="s-page">
      <header class="s-page-head">
        <h2>{m.tasks_title()}</h2>
        <p>
          {zh ? `${openCount} 项待办` : `${openCount} to do`}{doneCount > 0 ? (zh ? ` · 已完成 ${doneCount}` : ` · ${doneCount} done`) : ''}
          · {zh ? '点 ☆ 设为当前任务，专注时长会记到它名下。' : 'Star a task to make it current; focus time is logged to it.'}
        </p>
      </header>

      {#if formError}
        <p class="s-hint error" role="alert">{formError}</p>
      {/if}

      {#each sectionViews as section (section.subjectId ?? 'uncategorized')}
        <section class="s-group">
          <h3 class="section-title">
            <span class="section-dot" class:dot-none={section.color === null} style:background={section.color ?? undefined}></span>
            <span>{section.name}</span>
            {#if section.items.some((t) => !t.done)}<span class="count">{section.items.filter((t) => !t.done).length}</span>{/if}
          </h3>
          <div class="s-card">
            {#if section.items.length === 0}
              <div class="empty">{m.tasks_section_empty_hint()}</div>
            {:else}
              {#each section.items as task (task.id)}
                <TaskRow {task} isActive={task.id === $settings.active_task_id} />
              {/each}
            {/if}

            <form class="add-row" onsubmit={(e) => handleAdd(e, section.subjectId)}>
              <span class="plus" aria-hidden="true">＋</span>
              <input
                class="add-input"
                name="title"
                type="text"
                placeholder={m.tasks_add_placeholder()}
                aria-label={`${section.name} · ${m.tasks_add_placeholder()}`}
                maxlength="200"
              />
              <input
                class="s-input add-est"
                name="est"
                type="number"
                min="1"
                max="1440"
                placeholder={m.tasks_est_placeholder()}
                aria-label={m.tasks_est_placeholder()}
                title={zh ? '预计分钟数（可不填）' : 'Estimated minutes (optional)'}
              />
              <button class="s-btn s-btn--primary add-btn" type="submit">{m.tasks_add()}</button>
            </form>
          </div>
        </section>
      {/each}

      {#if doneCount > 0 || showDone}
        <button class="s-btn s-btn--ghost done-toggle" onclick={toggleShowDone}>
          {showDone ? m.tasks_hide_done() : m.tasks_show_done({ n: doneCount })}
        </button>
      {/if}
    </div>
  </div>
  <ToastHost />
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    animation: app-fade-in 0.18s ease;
    overflow: hidden;
    cursor: default;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--ui-border-strong) transparent;
  }

  .s-page {
    padding: 18px 20px 28px;
    gap: 18px;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 2px;
    font-size: 14px;
    font-weight: 700;
    color: var(--ui-text);
  }

  .section-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
    box-shadow:
      0 0 0 2px var(--ui-page),
      0 0 0 3px color-mix(in srgb, var(--ui-text) 12%, transparent);
  }

  .section-dot.dot-none {
    background: none;
    border: 1.5px dashed var(--ui-text-muted);
    box-shadow: none;
  }

  .count {
    display: inline-grid;
    place-items: center;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--ui-selected);
    color: var(--ui-brand-strong);
    font-size: 12px;
    font-weight: 700;
  }

  .empty {
    padding: 14px 16px;
    font-size: 13px;
    color: var(--ui-text-muted);
  }

  .add-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 14px;
    background: color-mix(in srgb, var(--ui-page) 60%, var(--ui-surface));
  }

  .plus {
    flex-shrink: 0;
    width: 18px;
    text-align: center;
    font-size: 14px;
    color: var(--ui-text-muted);
  }

  .add-input {
    flex: 1;
    min-width: 0;
    height: 32px;
    padding: 0 6px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    outline: none;
    transition: background 150ms ease;
  }

  .add-input::placeholder {
    color: var(--ui-text-muted);
  }

  .add-input:hover,
  .add-input:focus {
    background: var(--ui-surface);
  }

  .add-input:focus-visible {
    outline: 2px solid var(--ui-brand);
    outline-offset: 0;
  }

  .add-est {
    width: 68px;
    height: 32px;
    padding: 0 8px;
    font-size: 13px;
  }

  .add-btn {
    height: 32px;
    padding: 0 12px;
  }

  .done-toggle {
    align-self: center;
  }
</style>
