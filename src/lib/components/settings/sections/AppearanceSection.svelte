<script lang="ts">
  import { onMount } from 'svelte';
  import type { Theme } from '$lib/types';
  import { settings } from '$lib/stores/settings';
  import { applyTheme } from '$lib/stores/theme';
  import { getThemes, setSetting, onThemesChanged, appIconSet, appIconReset, getSettings } from '$lib/ipc';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import { notify } from '$lib/stores/toast';
  import { resolveThemeName } from '$lib/utils/theme';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import * as m from '$paraglide/messages.js';
  import ThemeCollection from '../ThemeCollection.svelte';
  import TimerAppearance from '../TimerAppearance.svelte';
  import PageBackdrop from '../PageBackdrop.svelte';
  import { collectionFor } from '$lib/themes/collection';
  import { getLocale } from '$paraglide/runtime.js';

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  let themes = $state<Theme[]>([]);
  let osDark = $state(window.matchMedia('(prefers-color-scheme: dark)').matches);
  // Accordion: at most one picker open at a time.
  let openPicker = $state<'light' | 'dark' | null>(null);

  // Light picker is active when mode='light', or mode='auto' and OS is light.
  let lightIsActive = $derived(
    $settings.theme_mode === 'light' || ($settings.theme_mode === 'auto' && !osDark)
  );
  // Dark picker is active when mode='dark', or mode='auto' and OS is dark.
  let darkIsActive = $derived(
    $settings.theme_mode === 'dark' || ($settings.theme_mode === 'auto' && osDark)
  );

  let classicActive = $derived(collectionFor(resolveThemeName($settings, osDark))?.id === 'classic-tomato');

  let selectedLightTheme = $derived(themes.find((t) => t.name === $settings.theme_light));
  let selectedDarkTheme = $derived(themes.find((t) => t.name === $settings.theme_dark));

  function togglePicker(picker: 'light' | 'dark') {
    openPicker = openPicker === picker ? null : picker;
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];

    // Track live OS color scheme changes to update picker highlights.
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const mqListener = (e: MediaQueryListEvent) => {
      osDark = e.matches;
    };
    mq.addEventListener('change', mqListener);
    cleanups.push(() => mq.removeEventListener('change', mqListener));

    (async () => {
      themes = await getThemes();
      cleanups.push(
        await onThemesChanged((updated) => {
          themes = updated;
        })
      );
    })();

    return () => {
      for (const fn of cleanups) fn();
    };
  });

  // Mode selector: save + immediately apply the resolved theme.
  async function setMode(mode: string) {
    const resolved = resolveThemeName({ ...$settings, theme_mode: mode }, osDark);
    const t = themes.find((th) => th.name === resolved);
    if (t) applyTheme(t);
    await setSetting('theme_mode', mode);
  }

  // Light picker: save + apply only if light picker is currently active.
  async function selectLight(theme: Theme) {
    if (lightIsActive) applyTheme(theme);
    await setSetting('theme_light', theme.name);
  }

  // Dark picker: save + apply only if dark picker is currently active.
  async function selectDark(theme: Theme) {
    if (darkIsActive) applyTheme(theme);
    await setSetting('theme_dark', theme.name);
  }

  // App icon: the stored PNG keeps one file name, so bust the webview cache on change.
  let iconVersion = $state(Date.now());
  let iconBusy = $state(false);
  let iconSrc = $derived($settings.app_icon ? `${convertFileSrc($settings.app_icon)}?v=${iconVersion}` : '/app-icon.png');

  async function chooseIcon() {
    if (iconBusy) return;
    const selected = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] });
    if (typeof selected !== 'string') return;
    iconBusy = true;
    try {
      await appIconSet(selected);
      settings.set(await getSettings());
      iconVersion = Date.now();
      notify(zh ? '图标已更换' : 'Icon updated');
    } catch (e) {
      notify(zh ? `换图标失败：${e}` : `Could not change the icon: ${e}`, { tone: 'error' });
    } finally {
      iconBusy = false;
    }
  }

  async function resetIcon() {
    try {
      await appIconReset();
      settings.set(await getSettings());
      notify(zh ? '已恢复默认图标' : 'Default icon restored');
    } catch (e) {
      notify(String(e), { tone: 'error' });
    }
  }
</script>

{#snippet picker(kind: 'light' | 'dark', label: string, isActive: boolean, selectedName: string, selected: Theme | undefined, choose: (theme: Theme) => void)}
  <div class="picker">
    <button class="s-row" aria-expanded={openPicker === kind} onclick={() => togglePicker(kind)}>
      <span class="s-text">
        <span class="s-label">{label}{#if isActive}<span class="badge">{m.appearance_badge_active()}</span>{/if}</span>
      </span>
      <span class="s-control">
        <span class="preview-name">{selectedName}</span>
        {#if selected}
          <span class="dots" aria-hidden="true">
            {#each ['--color-focus-round', '--color-short-round', '--color-long-round'] as key}
              <i style:background={selected.colors[key]}></i>
            {/each}
          </span>
        {/if}
        <svg class="chevron" class:rotated={openPicker === kind} width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
          <polyline points="2.5,4.5 6,8 9.5,4.5" />
        </svg>
      </span>
    </button>
    {#if openPicker === kind}
      <div class="theme-list" role="listbox" aria-label={label}>
        {#each themes as theme (theme.name)}
          {@const isSelected = theme.name === selectedName}
          <button
            class="option"
            class:selected={isSelected}
            role="option"
            aria-selected={isSelected}
            style="--card-bg:{theme.colors['--pomo-base'] ?? theme.colors['--color-background']}; --card-fg:{theme.colors['--pomo-ink'] ?? theme.colors['--color-foreground']};"
            onclick={() => choose(theme)}
          >
            <span class="dots" aria-hidden="true">
              {#each ['--color-focus-round', '--color-short-round', '--color-long-round'] as key}
                <i style:background={theme.colors[key]}></i>
              {/each}
            </span>
            <span class="option-name">{theme.name}</span>
            {#if theme.is_custom}<span class="custom">{m.appearance_badge_custom()}</span>{/if}
            {#if isSelected}<span class="check" aria-hidden="true">✓</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/snippet}

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">{m.appearance_group_mode()}</h3>
    <div class="s-card">
      <div class="s-row">
        <div class="s-text">
          <span class="s-label">{zh ? '明暗模式' : 'Light / dark'}</span>
          <span class="s-desc">{zh ? '“自动”会跟随系统，在下方的浅色和深色主题之间切换。' : '“Auto” follows the system, switching between the light and dark themes below.'}</span>
        </div>
        <div class="s-segment" role="radiogroup" aria-label={m.appearance_group_mode()}>
          {#each [['auto', m.appearance_mode_auto()], ['light', m.appearance_mode_light()], ['dark', m.appearance_mode_dark()]] as [value, label] (value)}
            <button role="radio" aria-checked={$settings.theme_mode === value} class:on={$settings.theme_mode === value} onclick={() => setMode(value)}>{label}</button>
          {/each}
        </div>
      </div>
    </div>
  </section>

  <ThemeCollection {themes} {osDark} />

  {#if classicActive}<TimerAppearance /><PageBackdrop />{/if}

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '应用图标' : 'App icon'}</h3>
    <div class="s-card">
      <div class="s-row">
        <img class="icon-preview" src={iconSrc} alt={zh ? '当前图标' : 'Current icon'} />
        <div class="s-text">
          <span class="s-label">{$settings.app_icon ? (zh ? '自定义图标' : 'Custom icon') : (zh ? '默认图标 · 番茄' : 'Default icon · tomato')}</span>
          <span class="s-desc">{zh ? '选一张图（JPG、PNG、WebP），会裁成正方形，换掉窗口、任务栏和开始菜单快捷方式的图标。' : 'Pick a JPG, PNG or WebP; it is cropped square and used for the windows, taskbar and Start Menu shortcut.'}</span>
        </div>
        <div class="s-control">
          {#if $settings.app_icon}<button class="s-btn s-btn--ghost" onclick={resetIcon}>{zh ? '恢复默认' : 'Default'}</button>{/if}
          <button class="s-btn" disabled={iconBusy} onclick={chooseIcon}>{iconBusy ? (zh ? '处理中…' : 'Working…') : (zh ? '选择图片…' : 'Choose…')}</button>
        </div>
      </div>
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '浅色与深色主题' : 'Light and dark themes'}</h3>
    <div class="s-card">
      {@render picker('light', m.appearance_group_light_theme(), lightIsActive, $settings.theme_light, selectedLightTheme, selectLight)}
      {@render picker('dark', m.appearance_group_dark_theme(), darkIsActive, $settings.theme_dark, selectedDarkTheme, selectDark)}
    </div>
  </section>
</div>

<style>
  .icon-preview {
    width: 48px;
    height: 48px;
    flex-shrink: 0;
    border-radius: 12px;
    object-fit: cover;
    box-shadow: 0 1px 4px color-mix(in srgb, var(--ui-text) 18%, transparent);
  }

  .picker .s-row {
    cursor: pointer;
  }

  .badge {
    margin-left: 8px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--ui-selected);
    color: var(--ui-brand-strong);
    font-size: 11.5px;
    font-weight: 700;
  }

  .preview-name {
    font-size: 13px;
    color: var(--ui-text-muted);
    white-space: nowrap;
  }

  .dots {
    display: inline-flex;
    gap: 3px;
  }

  .dots i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--ui-text) 12%, transparent);
  }

  .chevron {
    fill: none;
    stroke: var(--ui-text-muted);
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 150ms ease;
  }

  .chevron.rotated {
    transform: rotate(180deg);
  }

  .theme-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 12px 12px;
  }

  .option {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 42px;
    padding: 0 14px;
    border-radius: 10px;
    border: 2px solid transparent;
    background: var(--card-bg);
    color: var(--card-fg);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
    transition:
      border-color 150ms ease,
      transform 150ms ease;
  }

  .option:hover {
    transform: translateX(2px);
  }

  .option.selected {
    border-color: var(--card-fg);
  }

  .option-name {
    flex: 1;
  }

  .custom {
    font-size: 11.5px;
    opacity: 0.8;
  }

  .check {
    font-weight: 800;
  }
</style>