<script lang="ts">
  import { onMount } from 'svelte';
  import { settings } from '$lib/stores/settings';
  import { setSetting, reloadShortcuts, accessibilityTrusted } from '$lib/ipc';
  import ShortcutInput from '$lib/components/ShortcutInput.svelte';
  import LocalShortcutInput from '$lib/components/LocalShortcutInput.svelte';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import { formatLocalKey } from '$lib/utils/localShortcuts';
  import { notify } from '$lib/stores/toast';
  import type { Settings } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import { isMac } from '$lib/utils/platform';
  import { openUrl } from '@tauri-apps/plugin-opener';

  const ACCESSIBILITY_URL =
    'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility';

  type ShortcutKey = keyof Settings & (`local_shortcut_${string}` | `shortcut_${string}`);
  const LOCAL: { key: ShortcutKey; label: () => string }[] = [
    { key: 'local_shortcut_toggle', label: m.shortcuts_local_toggle_timer },
    { key: 'local_shortcut_reset', label: m.shortcuts_local_reset_round },
    { key: 'local_shortcut_skip', label: m.shortcuts_local_skip_round },
    { key: 'local_shortcut_volume_down', label: m.shortcuts_local_volume_down },
    { key: 'local_shortcut_volume_up', label: m.shortcuts_local_volume_up },
    { key: 'local_shortcut_mute', label: m.shortcuts_local_mute },
    { key: 'local_shortcut_fullscreen', label: m.shortcuts_local_fullscreen },
    { key: 'local_shortcut_jot', label: () => (zh ? '记一笔碎碎念' : 'Jot something down') },
  ];
  const GLOBAL: { key: ShortcutKey; label: () => string }[] = [
    { key: 'shortcut_toggle', label: m.shortcuts_toggle_timer },
    { key: 'shortcut_reset', label: m.shortcuts_reset_timer },
    { key: 'shortcut_skip', label: m.shortcuts_skip_round },
    { key: 'shortcut_restart', label: m.shortcuts_restart_round },
    { key: 'shortcut_jot', label: () => (zh ? '记一笔碎碎念（在任何软件里）' : 'Jot something down (from any app)') },
  ];

  let trusted = $state(true);
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  // A rejected binding, shown in place under the row that tried to take it.
  let conflict = $state<{ key: ShortcutKey; text: string } | null>(null);

  async function checkTrusted() {
    trusted = await accessibilityTrusted();
  }

  onMount(() => {
    if (!isMac) return;

    checkTrusted();

    function onFocus() {
      if (!trusted) checkTrusted();
    }

    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  });

  async function toggle(dbKey: string, current: boolean) {
    const updated = await setSetting(dbKey, current ? 'false' : 'true');
    settings.set(updated);
  }

  /** Saves a binding unless another action in the same list already uses it. */
  async function assign(
    list: typeof LOCAL,
    item: (typeof LOCAL)[number],
    value: string,
    shown: string,
    global: boolean
  ) {
    const clash = list.find((other) => other.key !== item.key && $settings[other.key] === value);
    if (clash) {
      conflict = {
        key: item.key,
        text: zh
          ? `「${shown}」已用于「${clash.label()}」，没有保存。换一个按键试试。`
          : `${shown} is already used by “${clash.label()}” — not saved.`,
      };
      return;
    }
    conflict = null;
    if ($settings[item.key] === value) return;
    try {
      settings.set(await setSetting(item.key, value));
      if (global) await reloadShortcuts();
      notify(zh ? `已保存：${item.label()} → ${shown}` : `Saved: ${item.label()} → ${shown}`);
    } catch (e) {
      notify(zh ? `保存失败：${e}` : `Could not save: ${e}`, { tone: 'error' });
    }
  }
</script>

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">{m.shortcuts_local_heading()}</h3>
    <p class="s-note">{m.shortcuts_local_note()}</p>
    <div class="s-card">
      {#each LOCAL as item (item.key)}
        <div class="s-row">
          <div class="s-text">
            <span class="s-label">{item.label()}</span>
            {#if conflict?.key === item.key}<span class="s-hint error" role="alert">{conflict.text}</span>{/if}
          </div>
          <div class="s-control">
            <LocalShortcutInput
              value={String($settings[item.key])}
              label={item.label()}
              onchange={(v) => assign(LOCAL, item, v, formatLocalKey(v), false)}
            />
          </div>
        </div>
      {/each}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{m.shortcuts_global_heading()}</h3>
    <p class="s-note">{m.shortcuts_note()}</p>
    <div class="s-card">
      <SettingsToggle
        label={m.shortcuts_toggle_enabled()}
        description={m.shortcuts_toggle_enabled_desc()}
        checked={$settings.global_shortcuts_enabled}
        onclick={() => toggle('global_shortcuts_enabled', $settings.global_shortcuts_enabled)}
      />

      {#if isMac && !trusted}
        <div class="s-row">
          <span class="s-desc">{m.shortcuts_accessibility_notice()}</span>
          <button class="s-btn" onclick={() => openUrl(ACCESSIBILITY_URL)}>
            {m.shortcuts_accessibility_open()}
          </button>
        </div>
      {/if}

      {#each GLOBAL as item (item.key)}
        <div class="s-row" class:is-disabled={!$settings.global_shortcuts_enabled}>
          <div class="s-text">
            <span class="s-label">{item.label()}</span>
            {#if item.key === 'shortcut_jot'}
              <span class="s-desc">{zh
                ? '在别的软件里按下，计时器带着碎碎念出现；Enter 记下后自动退回原样。'
                : 'Press it in any app: the timer comes up with the jot pad, and goes back once you press Enter.'}</span>
            {/if}
            {#if conflict?.key === item.key}<span class="s-hint error" role="alert">{conflict.text}</span>{/if}
          </div>
          <div class="s-control">
            <ShortcutInput
              value={String($settings[item.key])}
              label={item.label()}
              onchange={(v) => assign(GLOBAL, item, v, v.replace('Control', 'Ctrl'), true)}
            />
          </div>
        </div>
      {/each}
    </div>
    {#if !$settings.global_shortcuts_enabled}
      <p class="s-hint">
        {zh ? '全局快捷键已关闭，打开上方开关后才能修改和使用。' : 'Global shortcuts are off — turn on the switch above to edit and use them.'}
      </p>
    {/if}
  </section>
</div>
