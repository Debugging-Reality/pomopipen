<script lang="ts">
  // Classic Tomato main timer (docs/design/classic-tomato §01–§04).
  // Top to bottom: phase heading + subject → the tomato object → exact MM:SS →
  // status line → actions → round / today summary. Appearance A is the
  // mechanical kitchen timer, B the real tomato in a countdown ring; both read
  // the same timer state, so switching never pauses or resets a round.
  import { onMount, untrack } from 'svelte';
  import { fade } from 'svelte/transition';
  import type { RoundType, TimerState } from '$lib/types';
  import { settings } from '$lib/stores/settings';
  import { activeTheme } from '$lib/stores/theme';
  import { timerRestartRound, timerSkip, timerToggle, setSetting, statsGetDetailed, onSessionsCleared, onSessionsChanged } from '$lib/ipc';
  import { didAdvanceRound } from '$lib/utils/background';
  import { durationKey, parseDuration } from '$lib/utils/drumScale';
  import SubjectPicker from '../SubjectPicker.svelte';
  import TimerFooter from '../TimerFooter.svelte';
  import MechanicalDrum from './MechanicalDrum.svelte';
  import TomatoRing from './TomatoRing.svelte';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';

  // `state` is the prop name Timer.svelte passes; locally it is `snap` so it cannot clash with the $state rune.
  let { state: snap, uiScale, compact = false, viewWidth, viewHeight }: {
    state: TimerState; uiScale: number; compact?: boolean;
    /** Window size; defaults to this webview's window (the review harness passes its fake windows). */
    viewWidth?: number; viewHeight?: number;
  } = $props();

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let appearance = $derived($settings.timer_appearance === 'ring' ? 'ring' : 'mechanical');
  const phaseKey: Record<RoundType, string> = { work: '--color-focus-round', 'short-break': '--color-short-round', 'long-break': '--color-long-round' };
  const phaseFallback: Record<RoundType, string> = { work: '#C83224', 'short-break': '#244B36', 'long-break': '#203D65' };
  const phaseColor = (round: RoundType) => $activeTheme?.colors[phaseKey[round]] ?? phaseFallback[round];
  const roundName = (round: RoundType) =>
    round === 'work' ? m.round_label_work() : round === 'short-break' ? m.round_label_short_break() : m.round_label_long_break();
  const fmt = (secs: number) => {
    const s = Math.max(0, Math.ceil(secs - 1e-6));
    return `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;
  };

  // ── Reduced motion follows the system setting (no in-app switch). ────────
  let reduced = $state(false);
  onMount(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    reduced = mq.matches;
    const change = (e: MediaQueryListEvent) => (reduced = e.matches);
    mq.addEventListener('change', change);
    return () => mq.removeEventListener('change', change);
  });

  // ── Today's tomatoes (existing daily stats; no new backend command). ─────
  let today = $state<number | null>(null);
  async function refreshToday() {
    try {
      today = (await statsGetDetailed()).today.rounds;
    } catch {
      today = null;
    }
  }
  onMount(() => {
    void refreshToday();
    const stops: (() => void)[] = [];
    let disposed = false;
    for (const listen of [onSessionsCleared, onSessionsChanged]) {
      void listen(refreshToday).then((fn) => (disposed ? fn() : stops.push(fn)));
    }
    // Also after the window comes back (covers midnight and edits in the stats window).
    const focus = () => void refreshToday();
    window.addEventListener('focus', focus);
    return () => {
      disposed = true;
      stops.forEach((fn) => fn());
      window.removeEventListener('focus', focus);
    };
  });

  // ── Completion: a purely visual 1.2 s hold on 00:00 ──────────────────────
  // The engine has already switched to the next round when this runs; the hold
  // only delays what is drawn, it never changes timer rules (decision 4).
  let hold = $state<{ round: RoundType; total: number } | null>(null);
  let doneText = $state('');
  let press = $state(1);
  let prev: TimerState | null = null;
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let doneTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const next = snap;
    const before = prev;
    prev = next;
    if (!before || !didAdvanceRound(before, next)) return;
    setTimeout(refreshToday, 400); // the finished session is saved just before the event
    const natural = before.is_running && before.total_secs - before.elapsed_secs <= 2;
    if (!natural) return;
    const done = before.round_type === 'work' ? (zh ? '✓ 专注完成' : '✓ Focus done') : (zh ? '✓ 休息结束' : '✓ Break over');
    clearTimeout(holdTimer);
    clearTimeout(doneTimer);
    if (next.is_running) {
      // Auto-start already began the next round: just say so for 2 s.
      doneText = done;
      doneTimer = setTimeout(() => (doneText = ''), 2000);
      return;
    }
    hold = { round: before.round_type, total: before.total_secs };
    doneText = `${done} · ${zh ? '下一轮' : 'next'} ${roundName(next.round_type)} ${fmt(next.total_secs)}`;
    if (!untrack(() => reduced)) pressOnce();
    holdTimer = setTimeout(() => {
      hold = null;
      doneText = '';
    }, 1200);
  });
  function pressOnce() {
    const start = performance.now();
    const step = (now: number) => {
      const t = now - start;
      press = t < 120 ? 1 - 0.02 * (t / 120) : t < 280 ? 0.98 + 0.02 * ((t - 120) / 160) : 1;
      if (t < 280) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  // ── What is drawn ─────────────────────────────────────────────────────────
  let round = $derived(hold?.round ?? snap.round_type);
  let total = $derived(hold?.total ?? snap.total_secs);
  let remaining = $derived(hold ? 0 : Math.max(0, snap.total_secs - snap.elapsed_secs));
  let idle = $derived(!hold && !snap.is_running && !snap.is_paused && snap.elapsed_secs === 0);
  let paused = $derived(!hold && snap.is_paused);
  let color = $derived(phaseColor(round));

  // Between one-second ticks the drum and ring glide instead of stepping.
  let smooth = $state(0);
  let glide = 0;
  $effect(() => {
    const target = remaining;
    const moving = snap.is_running && !reduced;
    cancelAnimationFrame(glide);
    const from = untrack(() => smooth);
    if (!moving || Math.abs(from - target) > 2) {
      smooth = target;
      return;
    }
    const start = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - start) / 1000);
      smooth = from + (target - from) * k;
      if (k < 1) glide = requestAnimationFrame(step);
    };
    glide = requestAnimationFrame(step);
  });
  onMount(() => () => cancelAnimationFrame(glide));

  // ── Setting the duration: twist the drum, arrow keys, or type ────────────
  let dragMinutes = $state<number | null>(null);
  let editing = $state(false);
  let draft = $state('');
  let editError = $state('');

  async function setDuration(secs: number) {
    editError = '';
    try {
      settings.set(await setSetting(durationKey(snap.round_type), String(secs)));
    } catch (e) {
      editError = String(e);
    }
  }

  function startEdit() {
    if (!idle) return;
    editError = '';
    draft = String(Math.round(snap.total_secs / 60));
    editing = true;
  }

  function finishEdit(commit: boolean) {
    if (!editing) return;
    editing = false;
    if (!commit) return;
    const secs = parseDuration(draft);
    if (secs === null) {
      editError = zh ? '没有保存：请输入 1–90 分钟，或 分:秒' : 'Not saved: enter 1–90 minutes, or MM:SS';
      return;
    }
    if (secs !== snap.total_secs) void setDuration(secs);
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  let readout = $derived(dragMinutes !== null ? fmt(dragMinutes * 60) : fmt(remaining));

  let status = $derived.by((): { text: string; tone: '' | 'paused' | 'done' | 'error' } => {
    if (editError) return { text: `! ${editError}`, tone: 'error' };
    if (editing) return { text: zh ? '输入分钟或 分:秒 · Enter 确认 · Esc 取消' : 'Minutes or MM:SS · Enter to save · Esc to cancel', tone: '' };
    if (dragMinutes !== null) return { text: zh ? '松手确认 · 每格 1 分钟' : 'Release to set · one minute per notch', tone: '' };
    if (doneText) return { text: doneText, tone: 'done' };
    if (paused) return { text: zh ? 'Ⅱ 已暂停 · 进度已保留' : 'Ⅱ Paused · progress kept', tone: 'paused' };
    if (idle)
      return {
        text: appearance === 'mechanical'
          ? (zh ? '左右拖动刻度，或点击时间修改时长' : 'Drag the scale or click the time to change it')
          : (zh ? '点击时间修改时长' : 'Click the time to change it'),
        tone: '',
      };
    return { text: '', tone: '' };
  });

  let mainLabel = $derived(
    snap.is_running ? (zh ? '暂停' : 'Pause')
      : snap.is_paused ? (zh ? '继续' : 'Resume')
      : snap.round_type === 'work' ? (zh ? '开始专注' : 'Start focus') : (zh ? '开始休息' : 'Start break'),
  );
  // Compact windows keep the verb only.
  let shortLabel = $derived(
    snap.is_running ? (zh ? '暂停' : 'Pause') : snap.is_paused ? (zh ? '继续' : 'Resume') : (zh ? '开始' : 'Start'),
  );
  let objectLabel = $derived(`${roundName(round)} · ${zh ? '剩余' : 'remaining'} ${fmt(remaining)}`);

  // Compact windows lay out a 240 × 200 content area and zoom it to fit.
  let innerWidth = $state(240);
  let innerHeight = $state(240);
  let compactZoom = $derived(Math.max(0.6, Math.min((viewWidth ?? innerWidth) / 240, ((viewHeight ?? innerHeight) - 40) / 200, 4)));
  let zoom = $derived(compact ? compactZoom : uiScale);
</script>

<svelte:window bind:innerWidth bind:innerHeight />

<div class="classic" class:compact style:zoom style:--pc={color}>
  {#if compact}
    <span class="ctag phase-tag">{roundName(round)}</span>
    {#if paused}<span class="ctag paused-tag">{zh ? 'Ⅱ 已暂停' : 'Ⅱ Paused'}</span>{/if}
  {:else}
    <div class="hd">
      <div class="phase"><i></i><b>{roundName(round)}</b><i></i></div>
      <div class="subject"><SubjectPicker /></div>
    </div>
  {/if}

  <div class="stage">
    {#key appearance}<div class="object" in:fade={{ duration: reduced ? 0 : 180 }}>
    {#if appearance === 'mechanical'}
      <MechanicalDrum
        totalSecs={total}
        remainingSecs={dragMinutes !== null ? dragMinutes * 60 : smooth}
        width={compact ? 126 : 230}
        zoom={zoom}
        {compact}
        adjustable={idle && !editing}
        {reduced}
        {press}
        label={objectLabel}
        ondrag={(minutes) => (dragMinutes = minutes)}
        onadjust={(minutes) => setDuration(minutes * 60)}
      />
    {:else}
      <TomatoRing
        elapsed={total ? 1 - smooth / total : 0}
        {color}
        size={compact ? 104 : 196}
        fruit={compact ? 70 : 150}
        stroke={compact ? 3.5 : 4.5}
        ticks={compact ? 4 : 12}
        countdown={$settings.dial_countdown}
        label={objectLabel}
      />
    {/if}
    </div>{/key}
  </div>

  {#if editing}
    <div class="ro">
      <input
        class="ro-input"
        type="text"
        inputmode="numeric"
        aria-label={zh ? '时长（分钟或 分:秒）' : 'Duration (minutes or MM:SS)'}
        bind:value={draft}
        use:focusSelect
        onkeydown={(e) => {
          e.stopPropagation(); // keep app shortcuts (space, arrows) out of the field
          if (e.key === 'Enter') finishEdit(true);
          else if (e.key === 'Escape') finishEdit(false);
        }}
        onblur={() => finishEdit(true)}
      />
    </div>
  {:else if idle}
    <button class="ro editable" onclick={startEdit} title={zh ? '点击修改时长' : 'Click to change the duration'}>{readout}</button>
  {:else}
    <div class="ro" role="timer" aria-live="off">{readout}</div>
  {/if}

  {#if !compact}
    <div class="st" class:paused={status.tone === 'paused'} class:done={status.tone === 'done'} class:error={status.tone === 'error'} aria-live="polite">{status.text}</div>
  {/if}

  <div class="acts">
    <button class="bs" onclick={timerRestartRound} aria-label={m.tooltip_restart_round()} title={m.tooltip_restart_round()}>
      <svg viewBox="0 0 18 18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3.5 9a5.5 5.5 0 1 0 1.8-4.1" /><path d="M3.2 2.6v3.2h3.2" /></svg>
    </button>
    <button class="bp" onclick={timerToggle}>
      {#if snap.is_running}
        <svg viewBox="0 0 12 12" aria-hidden="true"><rect x="2" y="1.2" width="2.8" height="9.6" rx=".5" fill="currentColor" /><rect x="7.2" y="1.2" width="2.8" height="9.6" rx=".5" fill="currentColor" /></svg>
      {:else}
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 1.2 10.5 6l-8 4.8z" fill="currentColor" /></svg>
      {/if}
      <span>{compact ? shortLabel : mainLabel}</span>
    </button>
    <button class="bs" onclick={timerSkip} aria-label={m.tooltip_skip()} title={m.tooltip_skip()}>
      <svg viewBox="0 0 18 18" aria-hidden="true"><path d="M3.5 3.5 11 9l-7.5 5.5z" fill="currentColor" /><rect x="12.6" y="3.5" width="2.2" height="11" rx=".6" fill="currentColor" /></svg>
    </button>
  </div>

  {#if !compact}
    <div class="ft">
      <span class="today">{today === null ? '' : zh ? `今日 ${today} 个番茄` : `${today} today`}</span>
      <TimerFooter {snap} />
    </div>
  {/if}
</div>

<style>
  .classic {
    --paper-light: var(--pomo-light, #fff8ea);
    --paper-shade: var(--pomo-soft, #e8d8bd);
    --ink: var(--pomo-ink, #202620);
    --ink-2: var(--color-foreground-darker, #626456);
    --tomato: var(--pomo-main, #c83224);
    --tomato-deep: var(--ui-brand-strong, #a52820);
    --navy: var(--color-long-round, #203d65);
    --leaf: var(--color-short-round, #244b36);
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 360px;
    height: 438px;
    flex-shrink: 0;
    color: var(--ink);
    font-family: var(--font-ui);
  }

  /* ── Heading: the screen's one decorative motif ── */
  .hd {
    width: 100%;
    padding: 10px 20px 0;
    text-align: center;
  }

  .phase {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 30px;
    color: var(--pc);
  }

  .phase b {
    font-family: var(--font-classic-display);
    font-weight: 800;
    font-size: 22px;
    line-height: 30px;
    letter-spacing: 0.16em;
    margin-right: -0.16em;
  }

  .phase i {
    width: 26px;
    height: 5px;
    border-top: 1.5px solid currentColor;
    border-bottom: 1px solid currentColor;
  }

  .subject {
    display: flex;
    justify-content: center;
    height: 26px;
  }

  .subject :global(.subject-picker) {
    margin-top: 0;
  }

  .subject :global(.trigger) {
    align-items: center;
    max-width: 300px;
    padding: 3px 8px;
    border-radius: 4px;
    color: var(--ink);
    font-size: 14px;
    font-weight: 600;
  }

  .subject :global(.labels) {
    flex-direction: row;
    gap: 4px;
    align-items: baseline;
  }

  .subject :global(.task-name) {
    font-size: 13px;
    font-weight: 400;
    opacity: 1;
    color: var(--ink-2);
    max-width: 150px;
  }

  .subject :global(.task-name)::before {
    content: '· ';
  }

  .subject :global(.dot) {
    width: 8px;
    height: 8px;
    margin-top: 0;
    box-shadow: 0 0 0 1.5px var(--pomo-base, #f5ebd8), 0 0 0 2.5px rgb(32 38 32 / 22%);
  }

  .subject :global(.menu) {
    background: var(--paper-light);
    border: 1.5px solid var(--ink);
    border-radius: 5px;
    box-shadow: 0 10px 24px -12px rgb(32 38 32 / 45%);
  }

  /* ── Object, reading, status ── */
  .stage {
    display: grid;
    place-items: center;
    height: 196px;
    margin-top: 2px;
  }

  .object {
    grid-area: 1 / 1;
  }

  .ro {
    height: 56px;
    font-family: 'Source Serif Display', Georgia, serif;
    font-weight: 700;
    font-size: 54px;
    line-height: 56px;
    font-variant-numeric: lining-nums tabular-nums;
    letter-spacing: 0.01em;
    text-align: center;
    color: var(--ink);
  }

  button.ro {
    padding: 0 6px;
    border: 0;
    border-radius: 5px;
    background: none;
    cursor: text;
  }

  button.ro:hover {
    background: color-mix(in srgb, var(--paper-shade) 55%, transparent);
  }

  .ro-input {
    width: 150px;
    height: 50px;
    margin-top: 3px;
    border: 1.5px solid var(--ink);
    border-radius: 5px;
    background: var(--paper-light);
    color: var(--ink);
    font: inherit;
    font-size: 44px;
    line-height: 1;
    text-align: center;
    outline: 2px solid var(--navy);
    outline-offset: 2px;
  }

  .st {
    height: 20px;
    max-width: 100%;
    padding: 0 16px;
    overflow: hidden;
    font-size: 12.5px;
    line-height: 20px;
    white-space: nowrap;
    text-overflow: ellipsis;
    color: var(--ink-2);
  }

  .st.paused {
    color: var(--navy);
    font-weight: 650;
  }

  .st.done {
    color: var(--leaf);
    font-weight: 650;
  }

  .st.error {
    color: var(--tomato-deep);
    font-weight: 650;
  }

  /* ── Actions ── */
  .acts {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
  }

  .bs {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 1.5px solid var(--ink);
    border-radius: 5px;
    background: var(--paper-light);
    color: var(--ink);
    cursor: pointer;
    transition: background 120ms ease;
  }

  .bs:hover {
    background: var(--paper-shade);
  }

  .bs svg {
    width: 18px;
    height: 18px;
  }

  .bp {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-width: 150px;
    height: 40px;
    padding: 0 18px;
    border: 1.5px solid var(--ink);
    border-radius: 5px;
    background: var(--tomato);
    color: var(--paper-light);
    font: inherit;
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.06em;
    box-shadow: 2px 2px 0 var(--ink);
    cursor: pointer;
    transition:
      background 120ms ease,
      transform 80ms ease,
      box-shadow 80ms ease;
  }

  .bp svg {
    width: 13px;
    height: 13px;
  }

  .bp:hover {
    background: color-mix(in srgb, var(--tomato) 88%, var(--ink));
  }

  .bp:active {
    background: var(--tomato-deep);
    box-shadow: none;
    transform: translate(2px, 2px);
  }

  .classic :is(button, input):focus-visible {
    outline: 2px solid var(--navy);
    outline-offset: 2px;
  }

  /* ── Round / today summary ── */
  .ft {
    position: absolute;
    left: 20px;
    right: 20px;
    bottom: 6px;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 34px;
    border-top: 1px solid rgb(32 38 32 / 16%);
    font-size: 12.5px;
    color: var(--ink-2);
  }

  /* Order: rounds · today … reset, volume (TimerFooter renders the three controls). */
  .ft > :global(:nth-child(2)) {
    order: -1;
  }

  .ft > :global(:nth-child(3)) {
    margin-left: auto;
  }

  .today:not(:empty)::before {
    content: '· ';
  }

  .ft :global(.rounds) {
    min-width: 0;
    font-size: 12.5px;
    font-weight: 650;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }

  /* ── Compact 240 × 200 ── */
  .classic.compact {
    width: 240px;
    height: 200px;
  }

  .ctag {
    position: absolute;
    top: 8px;
    z-index: 2;
    height: 20px;
    padding: 0 6px;
    border-radius: 3px;
    font-size: 12px;
    font-weight: 700;
    line-height: 20px;
    letter-spacing: 0.06em;
  }

  .phase-tag {
    left: 12px;
    background: var(--pc);
    color: var(--paper-light);
  }

  .paused-tag {
    right: 12px;
    border: 1.5px solid var(--navy);
    background: var(--paper-light);
    color: var(--navy);
    line-height: 17px;
  }

  .compact .stage {
    height: 104px;
    margin-top: 8px;
  }

  .compact .ro {
    height: 40px;
    font-size: 36px;
    line-height: 40px;
  }

  .compact .ro-input {
    width: 104px;
    height: 36px;
    margin-top: 2px;
    font-size: 28px;
  }

  .compact .acts {
    gap: 8px;
    margin-top: 4px;
  }

  .compact .bs {
    width: 32px;
    height: 32px;
  }

  .compact .bs svg {
    width: 15px;
    height: 15px;
  }

  .compact .bp {
    min-width: 84px;
    height: 32px;
    padding: 0 12px;
    font-size: 13px;
    letter-spacing: 0.04em;
  }
</style>
