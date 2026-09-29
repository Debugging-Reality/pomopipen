<script lang="ts">
  // Subject management: create, rename, recolor, reorder, archive, delete.
  // Deleting keeps the subject's recorded sessions — they fall back to the
  // Uncategorised bucket in stats, they are never deleted.
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { subjects, watchSubjects } from '$lib/stores/subjects';
  import {
    subjectsCreate,
    subjectsUpdate,
    subjectsSetArchived,
    subjectsDelete,
    subjectsReorder,
  } from '$lib/ipc';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import type { Subject } from '$lib/types';
  import { activeTheme } from '$lib/stores/theme';
  import { collectionFor, subjectPaletteFor } from '$lib/themes/collection';
  import { settings } from '$lib/stores/settings';
  import { notify } from '$lib/stores/toast';
  import { readableInk } from '$lib/utils/color';
  import { varietyFor } from '$lib/themes/varieties';
  import VarietyConvert from '../VarietyConvert.svelte';
  import { getLocale } from '$paraglide/runtime.js';
  import * as m from '$paraglide/messages.js';

  let palette = $derived(subjectPaletteFor($activeTheme));
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let classic = $derived(collectionFor($activeTheme)?.id === 'classic-tomato');
  let themeName = $derived.by(() => {
    const c = collectionFor($activeTheme);
    return c ? (zh ? c.nameZh : c.name) : ($activeTheme?.name ?? '');
  });

  let showArchived = $state(false);
  let colorEditId = $state<number | null>(null);
  let renamingId = $state<number | null>(null);
  let confirmDeleteId = $state<number | null>(null);
  let formError = $state<string | null>(null);

  let newName = $state('');
  let chosenColor = $state<string | null>(null);

  // Svelte action: focuses + selects the element the instant it's mounted.
  // The rename `<input>` only exists while `renamingId` matches, so it never
  // already has focus the way an always-rendered input would — the HTML
  // `autofocus` attribute is unreliable for elements created by a conditional
  // block after initial page load, so this calls `.focus()` explicitly instead.
  function focusAndSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  let active = $derived($subjects.filter((s) => !s.archived));
  let archived = $derived($subjects.filter((s) => s.archived));
  // Cycle through the palette so a freshly created subject doesn't repeat
  // the previous one's color by default; the user can still pick another.
  let newColor = $derived(chosenColor ?? palette[active.length % palette.length]);

  function message(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
  }

  async function handleAdd() {
    const name = newName.trim();
    if (!name) return;
    try {
      await subjectsCreate(name, newColor);
      newName = '';
      chosenColor = null;
      formError = null;
      notify(zh ? `已添加「${name}」` : `Added “${name}”`);
    } catch (err) {
      formError = message(err);
    }
  }

  function startRename(subject: Subject) {
    colorEditId = null;
    renamingId = subject.id;
  }

  async function commitRename(subject: Subject, el: HTMLInputElement) {
    const value = el.value.trim();
    renamingId = null;
    if (!value || value === subject.name) return;
    try {
      await subjectsUpdate(subject.id, value);
      notify(zh ? `已改名为「${value}」` : `Renamed to “${value}”`);
    } catch (err) {
      notify(message(err), { tone: 'error' });
    }
  }

  async function pickColor(id: number, color: string) {
    colorEditId = null;
    try {
      await subjectsUpdate(id, undefined, color);
    } catch (err) {
      notify(message(err), { tone: 'error' });
    }
  }

  async function toggleArchived(subject: Subject, archive: boolean) {
    try {
      await subjectsSetArchived(subject.id, archive);
      if (archive) {
        notify(zh ? `已归档「${subject.name}」` : `Archived “${subject.name}”`, {
          action: { label: zh ? '撤销' : 'Undo', run: () => void subjectsSetArchived(subject.id, false) },
        });
      } else {
        notify(zh ? `「${subject.name}」已恢复` : `“${subject.name}” restored`);
      }
    } catch (err) {
      notify(message(err), { tone: 'error' });
    }
  }

  async function doDelete(subject: Subject) {
    confirmDeleteId = null;
    try {
      await subjectsDelete(subject.id);
      notify(zh ? `已删除「${subject.name}」，记录归入未分类` : `Deleted “${subject.name}”; its history is now uncategorized`);
    } catch (err) {
      notify(message(err), { tone: 'error' });
    }
  }

  async function move(id: number, direction: -1 | 1) {
    const ids = active.map((s) => s.id);
    const i = ids.indexOf(id);
    const j = i + direction;
    if (i === -1 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    try {
      await subjectsReorder(ids);
    } catch (err) {
      notify(message(err), { tone: 'error' });
    }
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];
    (async () => {
      cleanups.push(await watchSubjects(true));
    })();
    return () => {
      for (const fn of cleanups) fn();
    };
  });
</script>

{#snippet swatches(selected: string, onpick: (color: string) => void)}
  <div class="swatches" role="radiogroup" aria-label={m.subjects_color_label()}>
    {#each palette as color (color)}
      <button
        class="swatch"
        class:selected={selected === color}
        style:background={color}
        style:color={readableInk(color)}
        role="radio"
        aria-checked={selected === color}
        aria-label={varietyFor(color) ? (zh ? varietyFor(color)?.nameZh : varietyFor(color)?.name) : color}
        title={varietyFor(color) ? `${zh ? varietyFor(color)?.nameZh : varietyFor(color)?.name} · ${color}` : color}
        onclick={() => onpick(color)}
      >{#if selected === color}✓{/if}</button>
    {/each}
  </div>
{/snippet}

{#snippet subjectRow(subject: Subject, index: number, count: number)}
  {#if confirmDeleteId === subject.id}
    <div class="s-row stacked confirm" transition:slide={{ duration: 150 }}>
      <span class="s-desc">{m.subjects_delete_confirm({ name: subject.name })}</span>
      <div class="s-control end">
        <button class="s-btn" onclick={() => (confirmDeleteId = null)}>{m.subjects_cancel()}</button>
        <button class="s-btn s-btn--danger-solid" onclick={() => doDelete(subject)}>{m.subjects_delete()}</button>
      </div>
    </div>
  {:else}
    <div class="subject">
      <div class="s-row">
        <button
          class="dot"
          style:background={subject.color}
          onclick={() => {
            renamingId = null;
            colorEditId = colorEditId === subject.id ? null : subject.id;
          }}
          aria-label={m.subjects_color_label()}
          title={zh ? '更换颜色' : 'Change color'}
          aria-expanded={colorEditId === subject.id}
        ></button>

        {#if renamingId === subject.id}
          <input
            class="s-input name-input"
            type="text"
            value={subject.name}
            maxlength="40"
            use:focusAndSelect
            onblur={(e) => commitRename(subject, e.target as HTMLInputElement)}
            onkeydown={(e) => {
              if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
              else if (e.key === 'Escape') renamingId = null;
            }}
          />
        {:else}
          <button class="name" onclick={() => startRename(subject)} title={zh ? '点击改名' : 'Click to rename'}>
            <span>{subject.name}</span>
            <svg class="pencil" width="13" height="13" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 20h4L19 9l-4-4L4 16v4ZM13.5 6.5l4 4" />
            </svg>
          </button>
        {/if}

        <div class="s-control actions">
          {#if !subject.archived}
            <button class="s-icon-btn" disabled={index === 0} onclick={() => move(subject.id, -1)}
              aria-label={m.subjects_move_up()} title={m.subjects_move_up()}>
              <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true"><polyline points="2.5,7.5 6,4 9.5,7.5" /></svg>
            </button>
            <button class="s-icon-btn" disabled={index === count - 1} onclick={() => move(subject.id, 1)}
              aria-label={m.subjects_move_down()} title={m.subjects_move_down()}>
              <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true"><polyline points="2.5,4.5 6,8 9.5,4.5" /></svg>
            </button>
            <button class="s-btn s-btn--ghost small" onclick={() => toggleArchived(subject, true)}>{m.subjects_archive()}</button>
          {:else}
            <button class="s-btn small" onclick={() => toggleArchived(subject, false)}>{m.subjects_unarchive()}</button>
          {/if}
          <button class="s-icon-btn danger" onclick={() => (confirmDeleteId = subject.id)}
            aria-label={m.subjects_delete()} title={m.subjects_delete()}>
            <svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 7h16M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13M10 11v6M14 11v6" />
            </svg>
          </button>
        </div>
      </div>
      {#if colorEditId === subject.id}
        <div class="recolor" transition:slide={{ duration: 150 }}>
          {@render swatches(subject.color, (color) => pickColor(subject.id, color))}
        </div>
      {/if}
    </div>
  {/if}
{/snippet}

<div class="s-groups">
  {#if classic}<VarietyConvert {zh} />{/if}

  <section class="s-group">
    <h3 class="s-group-title">{m.subjects_group_active()}</h3>
    <div class="s-card">
      {#if active.length === 0}
        <div class="s-row"><span class="s-desc">{m.subjects_empty()}</span></div>
      {:else}
        {#each active as subject, i (subject.id)}
          {@render subjectRow(subject, i, active.length)}
        {/each}
      {/if}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '新增科目' : 'New subject'}</h3>
    <div class="s-card">
      <div class="s-row stacked">
        <div class="s-text">
          <span class="s-label">{zh ? '颜色' : 'Color'}</span>
          <span class="s-desc">{zh ? `来自「${themeName}」配色；换主题不会改掉已有科目的颜色。` : `From the ${themeName} palette; switching themes never recolors existing subjects.`}</span>
        </div>
        {@render swatches(newColor, (color) => (chosenColor = color))}
        <div class="add">
          <span class="preview" style:background={newColor} aria-hidden="true"></span>
          <input
            class="s-input"
            class:invalid={!!formError}
            type="text"
            placeholder={m.subjects_name_placeholder()}
            aria-label={m.subjects_name_placeholder()}
            bind:value={newName}
            maxlength="40"
            oninput={() => (formError = null)}
            onkeydown={(e) => {
              if (e.key === 'Enter') handleAdd();
            }}
          />
          <button class="s-btn s-btn--primary" onclick={handleAdd} disabled={!newName.trim()}>
            <span aria-hidden="true">＋</span>{m.subjects_add()}
          </button>
        </div>
        {#if formError}<span class="s-hint error" role="alert">{formError}</span>{/if}
      </div>
    </div>
  </section>

  {#if archived.length > 0}
    <section class="s-group">
      <button class="archived-toggle" aria-expanded={showArchived} onclick={() => (showArchived = !showArchived)}>
        <svg class:open={showArchived} width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><polyline points="3,1.5 7,5 3,8.5" /></svg>
        {showArchived ? m.subjects_hide_archived() : m.subjects_show_archived({ n: archived.length })}
      </button>
      {#if showArchived}
        <div class="s-card" transition:slide={{ duration: 150 }}>
          {#each archived as subject (subject.id)}
            {@render subjectRow(subject, 0, 1)}
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</div>

<style>
  .subject .s-row {
    gap: 12px;
    min-height: 52px;
  }

  .dot {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    box-shadow: 0 0 0 2px var(--ui-surface), 0 0 0 3px transparent;
    transition: box-shadow 150ms ease;
  }

  .dot:hover,
  .dot[aria-expanded='true'] {
    box-shadow: 0 0 0 2px var(--ui-surface), 0 0 0 4px var(--ui-border-strong);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    text-align: left;
    cursor: text;
    transition: background 150ms ease;
  }

  .name span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pencil {
    flex-shrink: 0;
    fill: none;
    stroke: var(--ui-text-muted);
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0;
    transition: opacity 150ms ease;
  }

  .name:hover {
    background: var(--ui-hover);
  }

  .name:hover .pencil,
  .name:focus-visible .pencil {
    opacity: 1;
  }

  .name-input {
    flex: 1;
    min-width: 0;
    height: 32px;
  }

  .actions {
    gap: 2px;
  }

  .actions svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .small {
    height: 28px;
    padding: 0 10px;
    font-size: 12.5px;
  }

  .confirm {
    background: var(--ui-danger-soft);
  }

  .end {
    justify-content: flex-end;
  }

  .recolor {
    padding: 0 16px 14px 46px;
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .swatch {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    font-size: 13px;
    font-weight: 800;
    cursor: pointer;
    box-shadow: 0 0 0 2px var(--ui-surface), 0 0 0 3px transparent;
    transition:
      box-shadow 150ms ease,
      transform 150ms ease;
  }

  .swatch:hover {
    transform: scale(1.08);
  }

  .swatch.selected {
    box-shadow: 0 0 0 2px var(--ui-surface), 0 0 0 4px var(--ui-text);
  }

  .add {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .add .s-input {
    flex: 1;
    min-width: 0;
  }

  .preview {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .archived-toggle {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    align-self: flex-start;
    padding: 6px 10px 6px 4px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text-muted);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition:
      color 150ms ease,
      background 150ms ease;
  }

  .archived-toggle:hover {
    color: var(--ui-text);
    background: var(--ui-hover);
  }

  .archived-toggle svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 150ms ease;
  }

  .archived-toggle svg.open {
    transform: rotate(90deg);
  }
</style>
