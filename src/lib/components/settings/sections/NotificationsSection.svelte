<script lang="ts">
  import { onMount } from 'svelte';
  import { settings } from '$lib/stores/settings';
  import {
    setSetting,
    getCustomAudioInfo,
    setCustomAudio,
    clearCustomAudio,
    openAudioFilePicker,
    onSettingsChanged,
  } from '$lib/ipc';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import type { CustomAudioInfo } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { warn, error as logError } from '@tauri-apps/plugin-log';
  import { getLocale } from '$paraglide/runtime.js';

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  type CueKey = keyof CustomAudioInfo;

  const CUE_LIST: { id: CueKey; label: () => string }[] = [
    { id: 'work_alert', label: m.notif_alert_work },
    { id: 'short_break_alert', label: m.notif_alert_short_break },
    { id: 'long_break_alert', label: m.notif_alert_long_break },
  ];

  let localVolume = $state($settings.volume);
  $effect(() => {
    localVolume = $settings.volume;
  });

  let workAlert = $state<string | null>(null);
  let shortBreakAlert = $state<string | null>(null);
  let longBreakAlert = $state<string | null>(null);
  let buttonClick = $state<string | null>(null);

  function getFileName(id: CueKey): string | null {
    if (id === 'work_alert') return workAlert;
    if (id === 'short_break_alert') return shortBreakAlert;
    if (id === 'button_click') return buttonClick;
    return longBreakAlert;
  }

  function setFileName(id: CueKey, val: string | null) {
    if (id === 'work_alert') workAlert = val;
    else if (id === 'short_break_alert') shortBreakAlert = val;
    else if (id === 'button_click') buttonClick = val;
    else longBreakAlert = val;
  }

  let buttonClickError = $state<string | null>(null);
  let workAlertError = $state<string | null>(null);
  let shortBreakAlertError = $state<string | null>(null);
  let longBreakAlertError = $state<string | null>(null);

  function getError(id: CueKey): string | null {
    if (id === 'work_alert') return workAlertError;
    if (id === 'short_break_alert') return shortBreakAlertError;
    if (id === 'button_click') return buttonClickError;
    return longBreakAlertError;
  }

  function setError(id: CueKey, val: string | null) {
    if (id === 'work_alert') workAlertError = val;
    else if (id === 'short_break_alert') shortBreakAlertError = val;
    else if (id === 'button_click') buttonClickError = val;
    else longBreakAlertError = val;
  }

  async function refreshAudioInfo() {
    try {
      const info: CustomAudioInfo = await getCustomAudioInfo();
      workAlert = info.work_alert;
      shortBreakAlert = info.short_break_alert;
      longBreakAlert = info.long_break_alert;
      buttonClick = info.button_click;
    } catch (err) {
      await warn(`[audio] getCustomAudioInfo failed (audio unavailable?): ${err}`);
    }
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      await refreshAudioInfo();
      unlisten = await onSettingsChanged(refreshAudioInfo);
    })();
    return () => unlisten?.();
  });

  async function toggle(dbKey: string, current: boolean) {
    const updated = await setSetting(dbKey, current ? 'false' : 'true');
    settings.set(updated);
  }

  async function handleVolumeInput(e: Event) {
    const val = (e.target as HTMLInputElement).valueAsNumber;
    localVolume = val;
    const updated = await setSetting('volume', String(Math.round(val * 100)));
    settings.set(updated);
  }

  async function pickAudio(id: CueKey) {
    setError(id, null);
    let path: string | null = null;
    try {
      path = await openAudioFilePicker();
    } catch (err) {
      await logError(`[audio] file picker error: ${err}`);
      return;
    }
    if (!path) return;
    try {
      const displayName = await setCustomAudio(id, path);
      setFileName(id, displayName);
      setError(id, null);
    } catch (err) {
      await logError(`[audio] setCustomAudio failed: ${err}`);
      setError(id, String(err));
    }
  }

  async function restoreAudio(id: CueKey) {
    try {
      await clearCustomAudio(id);
      setFileName(id, null);
      setError(id, null);
    } catch (err) {
      await logError(`[audio] clearCustomAudio failed: ${err}`);
    }
  }
</script>

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">{m.notif_group_alert()}</h3>
    <div class="s-card">
      {#each CUE_LIST as { id, label } (id)}
        <div class="s-row">
          <div class="s-text">
            <span class="s-label">{label()}</span>
            {#if getError(id)}
              <span class="s-hint error" role="alert">{getError(id)}</span>
            {:else}
              <span class="file-name" class:custom={getFileName(id) !== null}>
                {getFileName(id) ?? m.notif_audio_default()}
              </span>
            {/if}
          </div>
          <div class="s-control">
            {#if getFileName(id) !== null}
              <button class="s-btn s-btn--ghost" onclick={() => restoreAudio(id)}>{m.notif_btn_restore()}</button>
            {/if}
            <button class="s-btn" onclick={() => pickAudio(id)}>{m.notif_btn_choose()}</button>
          </div>
        </div>
      {/each}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '按钮音效' : 'Button sound'}</h3>
    <div class="s-card">
      <SettingsToggle
        label={zh ? '点击红色按钮时发声' : 'Play a sound on red buttons'}
        description={zh ? '开始、暂停、继续和确认类按钮按下时，发出一声轻轻的“嗒”。音量跟随下方的音量设置。' : 'A soft “tok” when you press start, pause, resume and other primary buttons. Uses the volume below.'}
        checked={$settings.click_sound_enabled}
        onclick={() => toggle('click_sound_enabled', $settings.click_sound_enabled)}
      />
      <div class="s-row" class:is-disabled={!$settings.click_sound_enabled}>
        <div class="s-text">
          <span class="s-label">{zh ? '按钮音效' : 'Button sound'}</span>
          {#if getError('button_click')}
            <span class="s-hint error" role="alert">{getError('button_click')}</span>
          {:else}
            <span class="file-name" class:custom={buttonClick !== null}>{buttonClick ?? (zh ? '默认 · 木质“嗒”' : 'Default · wooden tok')}</span>
          {/if}
        </div>
        <div class="s-control">
          {#if buttonClick !== null}
            <button class="s-btn s-btn--ghost" onclick={() => restoreAudio('button_click')}>{m.notif_btn_restore()}</button>
          {/if}
          <button class="s-btn" onclick={() => pickAudio('button_click')}>{m.notif_btn_choose()}</button>
        </div>
      </div>
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.notif_group_desktop()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.notif_toggle_desktop()}
        description={m.notif_toggle_desktop_desc()}
        checked={$settings.notifications_enabled}
        onclick={() => toggle('notifications', $settings.notifications_enabled)}
      />
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.notif_group_tick()}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.notif_toggle_tick_work()}
        description={m.notif_toggle_tick_work_desc()}
        checked={$settings.tick_sounds_during_work}
        onclick={() => toggle('tick_sounds_work', $settings.tick_sounds_during_work)}
      />
      <SettingsToggle
        label={m.notif_toggle_tick_break()}
        description={m.notif_toggle_tick_break_desc()}
        checked={$settings.tick_sounds_during_break}
        onclick={() => toggle('tick_sounds_break', $settings.tick_sounds_during_break)}
      />
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.notif_group_volume()}</h3>
    <div class="s-card">
      <div class="s-row stacked">
        <div class="meta">
          <span class="s-label">{m.notif_label_volume()}</span>
          <span class="s-value">{Math.round(localVolume * 100)}%</span>
        </div>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={localVolume}
          class="s-range"
          aria-label={m.notif_label_volume()}
          style:--frac={localVolume}
          oninput={handleVolumeInput}
        />
      </div>
    </div>
  </section>
</div>

<style>
  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .file-name {
    font-size: 12.5px;
    font-family: 'Mona Sans Mono', ui-monospace, monospace;
    color: var(--ui-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file-name.custom {
    color: var(--ui-brand-strong);
    font-weight: 600;
  }
</style>