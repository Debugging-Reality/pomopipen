<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { settings } from '$lib/stores/settings';
  import { setSetting, resetSettings, clearSessionHistory, traySupported } from '$lib/ipc';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import { notify } from '$lib/stores/toast';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import { setLocale } from '$lib/locale.svelte.js';
  import { isMac, isLinux } from '$lib/utils/platform';

  // Language options: value stored in DB, label shown in native language.
  const LANGUAGES = [
    { value: 'auto', label: 'Auto' },
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Español' },
    { value: 'fr', label: 'Français' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '日本語' },
    { value: 'zh', label: '中文' },
    { value: 'pt', label: 'Português' },
    { value: 'tr', label: 'Türkçe' },
  ];

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  // On Linux, probe for libayatana-appindicator3 at runtime.  The tray section
  // is hidden entirely when the library is absent so users can't enable a
  // feature that would crash the app.  Non-Linux platforms always support tray.
  let trayAvailable = $state(!isLinux);
  onMount(async () => {
    if (isLinux) trayAvailable = await traySupported();
  });

  let localPort = $state(String($settings.websocket_port));
  let portInvalid = $state(false);
  $effect(() => {
    localPort = String($settings.websocket_port);
  });

  let langOpen = $state(false);
  let langEl: HTMLElement | undefined;
  let triggerEl: HTMLButtonElement | undefined;

  const selectedLabel = $derived(
    LANGUAGES.find((l) => l.value === $settings.language)?.label ?? 'Auto'
  );

  async function openLanguages() {
    langOpen = true;
    await tick();
    langEl?.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
  }

  function closeLanguages() {
    langOpen = false;
    triggerEl?.focus();
  }

  function onMenuKey(e: KeyboardEvent) {
    const options = [...(langEl?.querySelectorAll<HTMLButtonElement>('.option') ?? [])];
    const index = options.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const step = e.key === 'ArrowDown' ? 1 : -1;
      options[(index + step + options.length) % options.length]?.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeLanguages();
    }
  }

  async function selectLanguage(value: string) {
    closeLanguages();
    const updated = await setSetting('language', value);
    settings.set(updated);
    setLocale(value);
  }

  $effect(() => {
    if (!langOpen) return;
    function onOutside(e: MouseEvent) {
      if (langEl && !langEl.contains(e.target as Node)) langOpen = false;
    }
    window.addEventListener('mousedown', onOutside);
    return () => window.removeEventListener('mousedown', onOutside);
  });

  let confirmingReset = $state(false);
  let confirmingClear = $state(false);

  async function handleReset() {
    const updated = await resetSettings();
    settings.set(updated);
    confirmingReset = false;
    notify(zh ? '所有设置已恢复默认' : 'All settings were reset');
  }

  async function handleClear() {
    await clearSessionHistory();
    confirmingClear = false;
    notify(zh ? '学习记录已清除' : 'Session history cleared');
  }

  async function toggle(dbKey: string, current: boolean) {
    const updated = await setSetting(dbKey, current ? 'false' : 'true');
    settings.set(updated);
  }

  async function handlePortBlur() {
    const port = parseInt(localPort, 10);
    if (!isNaN(port) && port >= 1024 && port <= 65535) {
      portInvalid = false;
      if (port === $settings.websocket_port) return;
      const updated = await setSetting('websocket_port', String(port));
      settings.set(updated);
      notify(zh ? `端口已改为 ${port}` : `Port set to ${port}`);
    } else {
      portInvalid = true;
    }
  }
</script>

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">{m.system_group_integrations()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.system_toggle_websocket()}
        description={m.system_toggle_websocket_desc({ port: $settings.websocket_port })}
        tooltip={m.tooltip_websocket()}
        checked={$settings.websocket_enabled}
        onclick={() => toggle('websocket_enabled', $settings.websocket_enabled)}
      />
      {#if $settings.websocket_enabled}
        <div class="s-row">
          <div class="s-text">
            <label class="s-label" for="ws-port">{m.system_label_port()}</label>
            {#if portInvalid}
              <span class="s-hint error" role="alert">{zh ? '请输入 1024–65535 之间的端口号' : 'Use a port between 1024 and 65535'}</span>
            {:else}
              <span class="s-desc">
                ws://127.0.0.1:{$settings.websocket_port}/ws · <code>{'{ "type": "getState" }'}</code>
              </span>
            {/if}
          </div>
          <input
            id="ws-port"
            class="s-input port"
            class:invalid={portInvalid}
            type="number"
            min="1024"
            max="65535"
            bind:value={localPort}
            oninput={() => (portInvalid = false)}
            onblur={handlePortBlur}
          />
        </div>
      {/if}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.system_group_language()}</h3>
    <div class="s-card">
      <div class="s-row">
        <div class="s-text">
          <span class="s-label">{zh ? '界面语言' : 'Interface language'}</span>
          <span class="s-desc">{zh ? '“Auto” 跟随系统语言。' : '“Auto” follows the system language.'}</span>
        </div>
        <div class="dropdown" bind:this={langEl}>
          <button
            class="s-input trigger"
            class:open={langOpen}
            bind:this={triggerEl}
            aria-haspopup="listbox"
            aria-expanded={langOpen}
            onclick={() => (langOpen ? closeLanguages() : openLanguages())}
            onkeydown={(e) => {
              if (e.key === 'ArrowDown' && !langOpen) {
                e.preventDefault();
                openLanguages();
              }
            }}
          >
            <span>{selectedLabel}</span>
            <svg class="chevron" width="10" height="6" viewBox="0 0 10 6" aria-hidden="true">
              <polyline points="0.5,0.5 5,5 9.5,0.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
          {#if langOpen}
            <div class="menu" role="listbox" tabindex="-1" onkeydown={onMenuKey}>
              {#each LANGUAGES as lang (lang.value)}
                <button
                  class="option"
                  role="option"
                  aria-selected={$settings.language === lang.value}
                  onclick={() => selectLanguage(lang.value)}
                >
                  <span>{lang.label}</span>
                  {#if $settings.language === lang.value}<span class="check" aria-hidden="true">✓</span>{/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.system_group_updates()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.system_toggle_check_updates()}
        description={m.system_toggle_check_updates_desc()}
        checked={$settings.check_for_updates}
        onclick={() => toggle('check_for_updates', $settings.check_for_updates)}
      />
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.advanced_group_logging()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.advanced_toggle_verbose_logging()}
        description={m.advanced_toggle_verbose_logging_desc()}
        tooltip={m.tooltip_verbose_logging()}
        checked={$settings.verbose_logging}
        onclick={() => toggle('verbose_logging', $settings.verbose_logging)}
      />
    </div>
  </section>

  {#if trayAvailable}
    <section class="s-group">
      <h3 class="s-group-title">{m.system_group_tray()}</h3>
      <div class="s-card">
        <!-- Show in System Tray: available on all platforms. -->
        <SettingsToggle
          label={m.system_toggle_show_tray()}
          description={m.system_toggle_show_tray_desc()}
          tooltip={isLinux ? m.system_tray_gnome_hint() : undefined}
          checked={$settings.tray_icon_enabled}
          onclick={() => toggle('tray_icon_enabled', $settings.tray_icon_enabled)}
        />
        <!-- Minimize to Tray is Windows/Linux only: the macOS yellow traffic-light
             button routes to the Dock and cannot be intercepted by the app. -->
        {#if !isMac}
          <SettingsToggle
            label={m.system_toggle_min_tray()}
            description={$settings.tray_icon_enabled
              ? m.system_toggle_min_tray_desc()
              : zh ? '需要先打开“显示在系统托盘”。' : 'Turn on “Show in tray” first.'}
            disabled={!$settings.tray_icon_enabled}
            checked={$settings.min_to_tray}
            onclick={() => toggle('min_to_tray', $settings.min_to_tray)}
          />
        {/if}
        <!-- Close to Tray is available on all platforms: the CloseRequested event
             fires on macOS (red button / Cmd+W) and can be intercepted. -->
        <SettingsToggle
          label={m.system_toggle_close_tray()}
          description={$settings.tray_icon_enabled
            ? m.system_toggle_close_tray_desc()
            : zh ? '需要先打开“显示在系统托盘”。' : 'Turn on “Show in tray” first.'}
          disabled={!$settings.tray_icon_enabled}
          checked={$settings.min_to_tray_on_close}
          onclick={() => toggle('min_to_tray_on_close', $settings.min_to_tray_on_close)}
        />
      </div>
    </section>
  {/if}

  <section class="s-group">
    <h3 class="s-group-title">{m.system_group_window()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.system_toggle_aot()}
        description={m.system_toggle_aot_desc()}
        checked={$settings.always_on_top}
        onclick={() => toggle('always_on_top', $settings.always_on_top)}
      />
      <SettingsToggle
        label={m.system_toggle_break_aot()}
        description={$settings.always_on_top
          ? m.system_toggle_break_aot_desc()
          : zh ? '需要先打开上一项“窗口置顶”。' : 'Turn on “Always on top” first.'}
        disabled={!$settings.always_on_top}
        checked={$settings.break_always_on_top}
        onclick={() => toggle('break_always_on_top', $settings.break_always_on_top)}
      />
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.system_group_data()}</h3>
    <div class="s-card">
      <div class="s-row" class:stacked={confirmingClear}>
        <div class="s-text">
          <span class="s-label">{zh ? '清除学习记录' : 'Clear session history'}</span>
          <span class="s-desc">
            {confirmingClear
              ? zh ? '所有已完成的番茄钟都会被永久删除，无法撤销。确定吗？' : 'This permanently deletes all session history. It cannot be undone.'
              : zh ? '删除所有番茄钟记录；科目、任务和设置不受影响。' : 'Deletes every recorded round; subjects, tasks and settings stay.'}
          </span>
        </div>
        <div class="s-control">
          {#if confirmingClear}
            <button class="s-btn" onclick={() => (confirmingClear = false)}>{zh ? '取消' : 'Cancel'}</button>
            <button class="s-btn s-btn--danger-solid" onclick={handleClear}>{zh ? '永久清除' : 'Clear forever'}</button>
          {:else}
            <button class="s-btn s-btn--danger" onclick={() => (confirmingClear = true)}>{zh ? '清除…' : 'Clear…'}</button>
          {/if}
        </div>
      </div>
      <div class="s-row" class:stacked={confirmingReset}>
        <div class="s-text">
          <span class="s-label">{m.about_reset_all()}</span>
          <span class="s-desc">
            {confirmingReset
              ? m.about_reset_confirm()
              : zh ? '所有设置回到默认值；学习记录会保留。' : 'Every setting returns to its default; your history is kept.'}
          </span>
        </div>
        <div class="s-control">
          {#if confirmingReset}
            <button class="s-btn" onclick={() => (confirmingReset = false)}>{zh ? '取消' : 'Cancel'}</button>
            <button class="s-btn s-btn--danger-solid" onclick={handleReset}>{zh ? '确认重置' : 'Reset'}</button>
          {:else}
            <button class="s-btn s-btn--danger" onclick={() => (confirmingReset = true)}>{zh ? '重置…' : 'Reset…'}</button>
          {/if}
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  .port {
    width: 104px;
    text-align: right;
    font-family: 'Mona Sans Mono', ui-monospace, monospace;
  }

  code {
    font-family: 'Mona Sans Mono', ui-monospace, monospace;
    font-size: 0.92em;
  }

  .stacked .s-control {
    justify-content: flex-end;
  }

  .dropdown {
    position: relative;
    flex-shrink: 0;
  }

  .trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 180px;
    cursor: pointer;
    text-align: left;
  }

  .trigger.open {
    border-color: var(--ui-brand);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ui-brand) 22%, transparent);
  }

  .chevron {
    color: var(--ui-text-muted);
    transition: transform 150ms ease;
  }

  .trigger.open .chevron {
    transform: rotate(180deg);
  }

  .menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    width: 200px;
    max-height: 260px;
    overflow-y: auto;
    z-index: 200;
    padding: 5px;
    border-radius: 12px;
    border: 1px solid var(--ui-border);
    background: var(--ui-surface);
    box-shadow: 0 10px 28px color-mix(in srgb, var(--ui-text) 16%, transparent);
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-text);
    font: inherit;
    font-size: 14px;
    text-align: left;
    cursor: pointer;
  }

  .option:hover,
  .option:focus-visible {
    background: var(--ui-hover);
    outline: none;
  }

  .option[aria-selected='true'] {
    color: var(--ui-brand-strong);
    font-weight: 700;
  }

  .check {
    font-size: 13px;
  }
</style>
