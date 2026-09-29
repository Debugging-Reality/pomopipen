<script lang="ts">
  import { settings } from '$lib/stores/settings';
  import { setSetting } from '$lib/ipc';
  import SettingsToggle from '$lib/components/settings/SettingsToggle.svelte';
  import { getLocale } from '$paraglide/runtime.js';
  import * as m from '$paraglide/messages.js';

  type DurationKey = 'time_work_secs' | 'time_short_break_secs' | 'time_long_break_secs';

  const MIN_SECS = 60; // 1:00
  const MAX_SECS = 5400; // 90:00
  const MAX_ROUNDS = 12;

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let rounds = $derived($settings.long_break_interval);

  // Per-row edit state: the raw text the user is currently typing.
  let edits = $state<Partial<Record<DurationKey, string>>>({});

  /** Parse MM:SS or bare integer minutes. Returns total seconds, or null on failure. */
  function parseMMSS(input: string): number | null {
    const trimmed = input.trim();
    const colonIdx = trimmed.indexOf(':');
    if (colonIdx === -1) {
      const mins = parseInt(trimmed, 10);
      if (isNaN(mins) || trimmed === '') return null;
      return mins * 60;
    }
    const mm = parseInt(trimmed.slice(0, colonIdx), 10);
    const ss = parseInt(trimmed.slice(colonIdx + 1), 10);
    if (isNaN(mm) || isNaN(ss) || ss < 0 || ss > 59) return null;
    return mm * 60 + ss;
  }

  /** Format total seconds as M:SS or MM:SS. */
  function formatMMSS(totalSecs: number): string {
    const mins = Math.floor(totalSecs / 60);
    const secs = totalSecs % 60;
    return `${mins}:${String(secs).padStart(2, '0')}`;
  }

  async function handleChange(dbKey: string, rawValue: number) {
    const updated = await setSetting(dbKey, String(rawValue));
    settings.set(updated);
  }

  async function toggle(dbKey: string, current: boolean) {
    const updated = await setSetting(dbKey, current ? 'false' : 'true');
    settings.set(updated);
  }

  /** Commit an edited value: parse, clamp, save. Reverts on invalid input. */
  async function commit(key: DurationKey, el: HTMLInputElement) {
    const raw = edits[key];
    edits[key] = undefined;
    const parsed = raw === undefined ? null : parseMMSS(raw);
    if (parsed === null) {
      el.value = formatMMSS($settings[key]);
      return;
    }
    const clamped = Math.max(MIN_SECS, Math.min(MAX_SECS, parsed));
    await handleChange(key, clamped);
    el.value = formatMMSS(clamped);
  }
</script>

{#snippet duration(label: string, key: DurationKey, color: string, disabled: boolean)}
  <div class="s-row stacked" class:is-disabled={disabled}>
    <div class="meta">
      <span class="s-label">{label}</span>
      <input
        class="s-value"
        type="text"
        aria-label={`${label} (MM:SS)`}
        title={zh ? '可直接输入分钟或 分:秒' : 'Type minutes or MM:SS'}
        value={edits[key] ?? formatMMSS($settings[key])}
        {disabled}
        onfocus={(e) => {
          edits[key] = e.currentTarget.value;
          e.currentTarget.select();
        }}
        oninput={(e) => (edits[key] = e.currentTarget.value)}
        onblur={(e) => commit(key, e.currentTarget)}
        onkeydown={(e) => {
          if (e.key === 'Enter') e.currentTarget.blur();
          else if (e.key === 'Escape') {
            edits[key] = undefined;
            e.currentTarget.value = formatMMSS($settings[key]);
            e.currentTarget.blur();
          }
        }}
      />
    </div>
    <input
      type="range"
      min="1"
      max="90"
      step="1"
      class="s-range"
      aria-label={label}
      {disabled}
      style:--fill={color}
      style:--frac={(Math.round($settings[key] / 60) - 1) / 89}
      value={Math.round($settings[key] / 60)}
      oninput={(e) => handleChange(key, e.currentTarget.valueAsNumber * 60)}
    />
  </div>
{/snippet}

<div class="s-groups">
  <section class="s-group">
    <h3 class="s-group-title">{zh ? '专注' : 'Focus'}</h3>
    <div class="s-card">
      {@render duration(m.timer_slider_focus(), 'time_work_secs', 'var(--color-focus-round)', false)}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '短休息' : 'Short break'}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.timer_toggle_short_breaks()}
        description={m.timer_toggle_short_breaks_desc()}
        checked={!$settings.short_breaks_enabled}
        onclick={() => toggle('short_breaks_enabled', $settings.short_breaks_enabled)}
      />
      {@render duration(m.timer_slider_short_break(), 'time_short_break_secs', 'var(--color-short-round)', !$settings.short_breaks_enabled)}
    </div>
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '长休息' : 'Long break'}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.timer_toggle_long_breaks()}
        description={m.timer_toggle_long_breaks_desc()}
        checked={!$settings.long_breaks_enabled}
        onclick={() => toggle('long_breaks_enabled', $settings.long_breaks_enabled)}
      />
      {@render duration(m.timer_slider_long_break(), 'time_long_break_secs', 'var(--color-long-round)', !$settings.long_breaks_enabled)}
      <div class="s-row stacked" class:is-disabled={!$settings.long_breaks_enabled}>
        <div class="meta">
          <span class="s-label">{m.timer_slider_rounds()}</span>
          <span class="s-value">{rounds}</span>
        </div>
        <input
          type="range"
          min="1"
          max={MAX_ROUNDS}
          step="1"
          class="s-range"
          aria-label={m.timer_slider_rounds()}
          disabled={!$settings.long_breaks_enabled}
          style:--frac={(rounds - 1) / (MAX_ROUNDS - 1)}
          value={rounds}
          oninput={(e) => handleChange('work_rounds', e.currentTarget.valueAsNumber)}
        />
      </div>
    </div>
    {#if !$settings.long_breaks_enabled || !$settings.short_breaks_enabled}
      <p class="s-hint">{zh ? '灰色的时长在对应休息被禁用时不起作用。' : 'Greyed-out durations have no effect while that break is disabled.'}</p>
    {/if}
  </section>

  <section class="s-group">
    <h3 class="s-group-title">{zh ? '自动开始与表盘' : 'Auto-start and dial'}</h3>
    <div class="s-card">
      <SettingsToggle
        label={m.timer_toggle_auto_start_work()}
        description={m.timer_toggle_auto_start_work_desc()}
        checked={$settings.auto_start_work}
        onclick={() => toggle('auto_start_work', $settings.auto_start_work)}
      />
      <SettingsToggle
        label={m.timer_toggle_auto_start_break()}
        description={m.timer_toggle_auto_start_break_desc()}
        checked={$settings.auto_start_break}
        onclick={() => toggle('auto_start_break', $settings.auto_start_break)}
      />
      <SettingsToggle
        label={m.timer_toggle_countdown()}
        description={m.timer_toggle_countdown_desc()}
        checked={$settings.dial_countdown}
        onclick={() => toggle('dial_countdown', $settings.dial_countdown)}
      />
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

  .s-range {
    margin: 4px 0 2px;
  }
</style>
