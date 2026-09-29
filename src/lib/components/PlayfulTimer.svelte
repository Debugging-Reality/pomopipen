<script lang="ts">
  import type { TimerState } from '$lib/types';
  import { settings } from '$lib/stores/settings';
  import { timerRestartRound, timerSkip, timerToggle } from '$lib/ipc';
  import SubjectPicker from './SubjectPicker.svelte';
  import TimerFooter from './TimerFooter.svelte';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';

  // `compact`: small windows keep the theme's own dial but drop everything
  // around it except a mini control row. `uiScale` then sizes the dial alone.
  let { state, uiScale, compact = false }: { state: TimerState; uiScale: number; compact?: boolean } = $props();
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let remaining = $derived(Math.max(0, state.total_secs - state.elapsed_secs));
  let display = $derived(`${String(Math.floor(remaining / 60)).padStart(2, '0')}:${String(remaining % 60).padStart(2, '0')}`);
  let fraction = $derived(state.total_secs ? Math.min(1, state.elapsed_secs / state.total_secs) : 0);
  let progress = $derived(($settings.dial_countdown ? 1 - fraction : fraction) * 100);
  let roundName = $derived(state.round_type === 'work' ? m.round_label_work() : state.round_type === 'short-break' ? m.round_label_short_break() : m.round_label_long_break());
</script>

<div class="playful" class:compact class:break={state.round_type !== 'work'} style:zoom={compact ? undefined : uiScale}>
  {#if !compact}<div class="topline"><span>✦ POMOPIPEN</span><span>{zh ? '专注有颜色' : 'FOCUS IN COLOR'} ✦</span></div>{/if}
  <div class="dial-box" style:zoom={compact ? uiScale : undefined}>
    <div class="dial" aria-label={`${roundName} ${display}`}>
      <div class="dial-core"><span class="round-tag">{roundName}</span><strong class="digits">{display}</strong>
        {#if !compact || uiScale >= 0.9}<span class="dial-caption">{zh ? '慢慢来，也很了不起' : 'ONE LITTLE STEP AT A TIME'}</span>{/if}</div>
      <svg class="ring" viewBox="0 0 220 220" aria-hidden="true">
        <circle cx="110" cy="110" r="101" class="ring-track" />
        <circle cx="110" cy="110" r="101" class="ring-progress" stroke-dasharray="634.6" stroke-dashoffset={634.6 * (1 - progress / 100)} />
      </svg>
      <div class="progress-bar"><i style:width={`${progress}%`}></i></div>
    </div>
  </div>
  {#if compact}
    <div class="controls mini">
      <button class="secondary" onclick={timerRestartRound} aria-label={m.tooltip_restart_round()} title={m.tooltip_restart_round()}>↺</button>
      <button class="primary" onclick={timerToggle} aria-label={state.is_running ? 'Pause' : 'Play'}>{state.is_running ? 'Ⅱ' : '▶'}</button>
      <button class="secondary" onclick={timerSkip} aria-label={m.tooltip_skip()} title={m.tooltip_skip()}>↠</button>
    </div>
  {:else}
    <div class="round-state">{state.round_type === 'work' ? (zh ? '现在，专心做一件事' : 'One thing at a time') : (zh ? '休息一下，等会儿再出发' : 'Take a bright little break')}</div>
    <div class="subject"><SubjectPicker /></div>
    <div class="controls">
      <button class="secondary" onclick={timerRestartRound} aria-label={m.tooltip_restart_round()} title={m.tooltip_restart_round()}>↺</button>
      <button class="primary" onclick={timerToggle} aria-label={state.is_running ? 'Pause' : 'Play'}>{state.is_running ? (zh ? '暂停' : 'PAUSE') : (zh ? '开始' : 'START')} <span>{state.is_running ? 'Ⅱ' : '▶'}</span></button>
      <button class="secondary" onclick={timerSkip} aria-label={m.tooltip_skip()} title={m.tooltip_skip()}>↠</button>
    </div>
    <div class="footer"><TimerFooter snap={state} /></div>
  {/if}
</div>

<style>
  .playful { --dial-bg: var(--pomo-pop); --dial-ink: var(--pomo-ink); width: 300px; min-height: 391px; display: flex; align-items: center; flex-direction: column; color: var(--pomo-ink); font-family: 'Mona Sans', system-ui, sans-serif; }
  .playful.compact { width: auto; min-height: 0; gap: 10px; }
  .topline { width: 100%; display: flex; justify-content: space-between; gap: 8px; font-size: .62rem; font-weight: 850; letter-spacing: .09em; color: var(--pomo-ink); padding: 3px 7px 6px; }
  .dial { width: 220px; height: 220px; flex: 0 0 220px; position: relative; border: 10px solid var(--pomo-main); background: var(--dial-bg); border-radius: 50%; display: grid; place-items: center; box-shadow: 8px 9px 0 color-mix(in srgb, var(--pomo-main) 23%, transparent); }
  .dial-core { position: relative; z-index: 1; display: flex; align-items: center; flex-direction: column; transform: translateY(2px); }
  .round-tag { background: var(--pomo-main); color: var(--pomo-on); border-radius: 30px; font-size: .68rem; font-weight: 850; padding: 4px 12px; letter-spacing: .06em; }
  .digits { color: var(--dial-ink); font-family: 'Mona Sans Mono', monospace; font-variant-numeric: tabular-nums; font-size: 3.15rem; font-weight: 800; letter-spacing: -.02em; line-height: 1.24; white-space: nowrap; }
  .dial-caption { color: var(--dial-ink); font-size: .62rem; font-weight: 800; letter-spacing: .03em; }
  .ring { position: absolute; inset: -10px; width: 220px; height: 220px; transform: rotate(-90deg); }
  .ring circle { fill: none; stroke-width: 5; }
  .ring-track { stroke: color-mix(in srgb, var(--pomo-main) 25%, transparent); }
  .ring-progress { stroke: var(--pomo-main); stroke-linecap: round; transition: stroke-dashoffset .6s ease; }
  .progress-bar { display: none; }
  .round-state { margin-top: 17px; font-size: .86rem; font-weight: 850; color: var(--pomo-ink); background: var(--pomo-light); padding: 6px 13px; border-radius: 10px; }
  .subject { margin-top: 10px; background: var(--pomo-soft); border-radius: 12px; border: 2px solid var(--pomo-main); min-width: 145px; min-height: 29px; display: grid; place-items: center; position: relative; z-index: 10; }
  .controls { display: flex; gap: 9px; align-items: center; margin-top: 13px; }
  button { cursor: pointer; font-family: inherit; }
  .controls button { height: 42px; border: 3px solid var(--pomo-main); color: var(--pomo-on); background: var(--pomo-main); font-weight: 850; }
  .controls .secondary { width: 45px; border-radius: 50%; font-size: 1.45rem; line-height: 1; }
  .controls .primary { min-width: 116px; border-radius: 22px; font-size: .88rem; }
  .primary span { font-size: .7rem; margin-left: 4px; }
  .controls.mini { margin-top: 0; gap: 8px; }
  .controls.mini button { height: 32px; border-width: 2px; }
  .controls.mini .secondary { width: 32px; font-size: 1.05rem; }
  .controls.mini .primary { min-width: 0; width: 46px; border-radius: 16px; font-size: .8rem; }
  .controls button:hover { transform: translateY(-2px); box-shadow: 0 3px 0 var(--pomo-ink); }
  .controls button:focus-visible { outline: 3px solid var(--pomo-pop); outline-offset: 2px; }
  .footer { display: flex; justify-content: space-around; align-items: center; gap: 8px; width: 240px; background: var(--pomo-soft); border-radius: 10px; margin-top: 12px; padding: 4px 8px; }
  .footer :global(*) { color: var(--pomo-ink); }
  :global(html[data-pomo-theme='cherry-soda']) .playful { --dial-bg: var(--pomo-pop); }
  :global(html[data-pomo-theme='citrus-club']) .playful { --dial-bg: var(--pomo-main); --dial-ink: var(--pomo-on); }
  :global(html[data-pomo-theme='citrus-club']) .dial { border-radius: 110px 110px 19px 19px; border-color: var(--pomo-pop); box-shadow: 8px 9px 0 color-mix(in srgb, var(--pomo-main) 20%, transparent); }
  :global(html[data-pomo-theme='citrus-club']) .round-tag { color: var(--pomo-ink); background: var(--pomo-pop); }
  :global(html[data-pomo-theme='citrus-club']) .ring { display: none; }
  :global(html[data-pomo-theme='citrus-club']) .progress-bar { display: block; position: absolute; bottom: 14px; left: 15px; right: 15px; height: 8px; background: color-mix(in srgb, var(--pomo-on) 35%, transparent); border-radius: 8px; overflow: hidden; }
  :global(html[data-pomo-theme='citrus-club']) .progress-bar i { display: block; height: 100%; background: var(--pomo-pop); border-radius: inherit; transition: width .6s ease; }
  :global(html[data-pomo-theme='citrus-club']) .controls .secondary { border-radius: 12px; color: var(--pomo-ink); background: var(--pomo-soft); }
  :global(html[data-pomo-theme='citrus-club']) .controls .primary { border-radius: 12px; }
  :global(html[data-pomo-theme='berry-planet']) .playful { --dial-bg: var(--pomo-soft); }
  :global(html[data-pomo-theme='berry-planet']) .dial { border-radius: 44px; transform: rotate(-4deg); }
  :global(html[data-pomo-theme='berry-planet']) .dial-core { transform: rotate(4deg); }
  :global(html[data-pomo-theme='berry-planet']) .ring { transform: rotate(-94deg); }
  :global(html[data-pomo-theme='berry-planet']) .controls .secondary { border-radius: 14px; color: var(--pomo-ink); background: var(--pomo-pop); }
  :global(html[data-pomo-theme='berry-planet']) .controls .primary { border-radius: 15px; }
</style>
