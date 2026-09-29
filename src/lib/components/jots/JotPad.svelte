<script lang="ts">
  // 碎碎念 — the jot pad in the timer window. Whatever comes to mind mid-round
  // (a to-do, an idea, something to say) goes in with one key and Enter, and
  // focus carries on; the pile is handled in the break.
  //
  //   open      N (local), the titlebar bubble, or the global shortcut (which
  //             sends the window back afterwards: "quick" mode)
  //   focus     while a focus round runs, the list folds into a stack of slips
  //             so writing never turns into reading
  //   break     a peek slip offers the pile; each jot can be crossed off (the
  //             tomato stamp), turned into a task (it flies into Tasks), edited
  //             or deleted, all undoable
  import { onMount, tick, untrack } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { backOut, cubicOut } from 'svelte/easing';
  import { SvelteSet } from 'svelte/reactivity';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import type { Jot, TimerState } from '$lib/types';
  import {
    jotsCreate, jotsEdit, jotsSetDone, jotsToTask, jotsUntask, jotsDelete, jotsRestore,
    jotsClearHandled, onJotCapture, setWindowVisibility,
  } from '$lib/ipc';
  import { jots, jotPad, tasksBump, watchJots, openJotPad, closeJotPad, refreshJots } from '$lib/stores/jots';
  import { timerState } from '$lib/stores/timer';
  import { settings } from '$lib/stores/settings';
  import { subjects, watchSubjects } from '$lib/stores/subjects';
  import { tasks, watchTasks } from '$lib/stores/tasks';
  import { notify } from '$lib/stores/toast';
  import { didAdvanceRound } from '$lib/utils/background';
  import { foldsInFocus, isSendKey, splitJots } from '$lib/utils/jots';
  import ToastHost from '../settings/ToastHost.svelte';
  import JotRow from './JotRow.svelte';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';

  /** The review harness passes its own timer state; the app uses the store. */
  let { snap }: { snap?: TimerState } = $props();
  let timer = $derived(snap ?? $timerState);
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  // ── Reduced motion follows the system setting ───────────────────────────
  let reduced = $state(false);
  onMount(() => {
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    reduced = mq.matches;
    const change = (e: MediaQueryListEvent) => (reduced = e.matches);
    mq.addEventListener('change', change);
    return () => mq.removeEventListener('change', change);
  });

  // ── Data ─────────────────────────────────────────────────────────────────
  /** Crossed off a moment ago: still shown in place while the stamp plays. */
  const settling = new SvelteSet<number>();
  const popping = new SvelteSet<number>();
  const settleTimers = new Map<number, ReturnType<typeof setTimeout>>();
  /** How a row leaves the open list, read by the `leave` transition. */
  let leaving = $state<Record<number, 'done' | 'task' | 'delete'>>({});

  let split = $derived(splitJots($jots, settling));
  let open = $derived(split.open);
  let handled = $derived(split.handled);
  let openCount = $derived(open.filter((j) => j.done_at === null).length);

  let now = $state(new Date());
  onMount(() => {
    const stops: (() => void)[] = [];
    let disposed = false;
    const keep = (p: Promise<() => void>) => void p.then((fn) => (disposed ? fn() : stops.push(fn)));
    keep(watchJots());
    // Compact timers don't mount the subject picker, so load what the rows show here too.
    keep(watchSubjects());
    keep(watchTasks(true));
    keep(onJotCapture((capture) => void show(capture)));
    const clock = setInterval(() => (now = new Date()), 30_000);
    return () => {
      disposed = true;
      stops.forEach((fn) => fn());
      clearInterval(clock);
      settleTimers.forEach(clearTimeout);
    };
  });

  // ── Open / close ─────────────────────────────────────────────────────────
  let field = $state<HTMLTextAreaElement | null>(null);
  let draft = $state('');
  let sending = $state(false);
  let unfolded = $state(false);
  let showHandled = $state(false);
  let confirmClear = $state(false);

  async function show(capture: Parameters<typeof openJotPad>[0] = null) {
    openJotPad(capture);
    peek = false;
    await tick();
    field?.focus();
  }

  // Opened from elsewhere (titlebar, N): put the cursor in the box.
  $effect(() => {
    if ($jotPad.open) void tick().then(() => field?.focus());
  });

  async function restoreWindow(restore: 'hide' | 'minimize' | '') {
    try {
      if (restore === 'hide') await setWindowVisibility(false);
      else if (restore === 'minimize') await getCurrentWebviewWindow().minimize();
    } catch {
      // The window stays where it is; nothing is lost.
    }
  }

  /** `back`: a quick jot from another app returns the window to how it was. */
  function close(back = false) {
    const quick = $jotPad.quick;
    closeJotPad();
    confirmClear = false;
    (document.activeElement as HTMLElement | null)?.blur?.();
    if (back && quick) void restoreWindow(quick.restore);
  }

  /** Anything beyond writing one jot means the user is here to stay. */
  function stay() {
    if ($jotPad.quick) jotPad.update((p) => ({ ...p, quick: null }));
  }

  function onWindowKey(e: KeyboardEvent) {
    if (e.key === 'Escape' && $jotPad.open) {
      e.preventDefault();
      close(true);
    }
  }

  // ── Writing ──────────────────────────────────────────────────────────────
  function grow() {
    if (!field) return;
    field.style.height = 'auto';
    field.style.height = `${Math.min(field.scrollHeight, 112)}px`;
    field.style.overflowY = field.scrollHeight > 112 ? 'auto' : 'hidden';
  }

  async function send() {
    const body = draft.trim();
    if (!body || sending) return;
    sending = true;
    try {
      await jotsCreate(body);
      draft = '';
      await tick();
      grow();
      await refreshJots();
      const quick = $jotPad.quick;
      // Let the slip land, then go back — unless another jot is already being typed.
      if (quick) setTimeout(() => { if (!draft.trim() && $jotPad.quick) close(true); }, reduced ? 250 : 800);
    } catch (e) {
      notify(zh ? `没记上：${e}` : `Not saved: ${e}`, { tone: 'error' });
    } finally {
      sending = false;
    }
  }

  function onFieldKey(e: KeyboardEvent) {
    if (isSendKey(e) || (e.key === 'Enter' && (e.ctrlKey || e.metaKey))) {
      e.preventDefault();
      void send();
    }
  }

  // ── Handling ─────────────────────────────────────────────────────────────
  async function cross(jot: Jot) {
    stay();
    if (jot.task_id !== null) return;
    if (jot.done_at === null && !popping.has(jot.id)) {
      leaving[jot.id] = 'done';
      popping.add(jot.id);
      settling.add(jot.id);
      try {
        await jotsSetDone(jot.id, true);
        await refreshJots();
      } catch (e) {
        popping.delete(jot.id);
        settling.delete(jot.id);
        notify(String(e), { tone: 'error' });
        return;
      }
      settleTimers.set(jot.id, setTimeout(() => {
        settleTimers.delete(jot.id);
        popping.delete(jot.id);
        settling.delete(jot.id);
      }, reduced ? 400 : 1100));
    } else {
      // Clicked again (during the stamp, or in the handled list): bring it back.
      clearTimeout(settleTimers.get(jot.id));
      settleTimers.delete(jot.id);
      popping.delete(jot.id);
      settling.delete(jot.id);
      delete leaving[jot.id];
      await jotsSetDone(jot.id, false).catch((e) => notify(String(e), { tone: 'error' }));
      await refreshJots();
    }
  }

  async function toTask(jot: Jot, subjectId: number | null) {
    stay();
    leaving[jot.id] = 'task';
    await tick();
    try {
      await jotsToTask(jot.id, subjectId);
      await refreshJots();
    } catch (e) {
      delete leaving[jot.id];
      notify(zh ? `没转成：${e}` : `Couldn't add it: ${e}`, { tone: 'error' });
      return;
    }
    const list = subjectId === null ? m.subject_filter_uncategorized() : ($subjects.find((s) => s.id === subjectId)?.name ?? '');
    notify(zh ? `已放进任务 · ${list}` : `Added to Tasks · ${list}`, {
      action: { label: zh ? '撤销' : 'Undo', run: () => void jotsUntask(jot.id).then(refreshJots) },
    });
  }

  async function untask(jot: Jot) {
    stay();
    await jotsUntask(jot.id).catch((e) => notify(String(e), { tone: 'error' }));
    await refreshJots();
  }

  async function remove(jot: Jot) {
    stay();
    leaving[jot.id] = 'delete';
    await tick();
    try {
      const gone = await jotsDelete(jot.id);
      await refreshJots();
      notify(zh ? '已删除' : 'Deleted', {
        action: { label: zh ? '撤销' : 'Undo', run: () => void jotsRestore(gone).then(refreshJots) },
      });
    } catch (e) {
      delete leaving[jot.id];
      notify(String(e), { tone: 'error' });
    }
  }

  async function edit(jot: Jot, body: string) {
    await jotsEdit(jot.id, body).catch((e) => notify(String(e), { tone: 'error' }));
    await refreshJots();
  }

  async function clearHandled() {
    confirmClear = false;
    try {
      const n = await jotsClearHandled();
      await refreshJots();
      notify(zh ? `已清空 ${n} 条` : `Cleared ${n}`);
    } catch (e) {
      notify(String(e), { tone: 'error' });
    }
  }

  // ── Focus rounds fold the list; a break offers it ───────────────────────
  let fold = $derived(foldsInFocus(timer) && !unfolded);
  let stack = $derived(open.filter((j) => j.done_at === null).slice(0, 3));

  let peek = $state(false);
  let peekTimer: ReturnType<typeof setTimeout> | undefined;
  let before: TimerState | null = null;
  const mountedAt = Date.now();
  $effect(() => {
    const next = timer;
    const prev = before;
    before = next;
    if (!prev || !didAdvanceRound(prev, next)) return;
    unfolded = false;
    // The first snapshot after start-up is hydration, not a finished round.
    if (Date.now() - mountedAt < 2500) return;
    if (prev.round_type === 'work' && next.round_type !== 'work' && untrack(() => openCount) > 0 && !untrack(() => $jotPad.open)) {
      peek = true;
      clearTimeout(peekTimer);
      peekTimer = setTimeout(() => (peek = false), 12_000);
    }
    if (next.round_type === 'work') peek = false;
  });
  onMount(() => () => clearTimeout(peekTimer));

  // ── Header clock ─────────────────────────────────────────────────────────
  const two = (n: number) => String(n).padStart(2, '0');
  let remaining = $derived(Math.max(0, timer.total_secs - timer.elapsed_secs));
  let clock = $derived(`${two(Math.floor(remaining / 60))}:${two(remaining % 60)}`);
  let phase = $derived(timer.round_type === 'work' ? m.round_label_work() : timer.round_type === 'short-break' ? m.round_label_short_break() : m.round_label_long_break());
  let phaseColor = $derived(timer.round_type === 'work' ? 'var(--color-focus-round)' : timer.round_type === 'short-break' ? 'var(--color-short-round)' : 'var(--color-long-round)');

  let placeholder = $derived(
    foldsInFocus(timer)
      ? (zh ? '冒出什么念头？记下就好，接着专注' : "What's on your mind? Jot it down")
      : (zh ? '想做的、想说的，都先写在这儿' : 'Things to do, things to say — put them here'),
  );

  // ── Motion ───────────────────────────────────────────────────────────────
  /** A new jot drops in like a slip laid on the pad; the rows below make room. */
  function dropIn(node: HTMLElement) {
    if (reduced) return { duration: 0 };
    const h = node.offsetHeight;
    return {
      duration: 420,
      css: (t: number) => {
        const room = cubicOut(Math.min(1, t / 0.45));
        const e = 1 - backOut(t);
        return `height:${h * room}px;opacity:${Math.min(1, t * 2.4)};transform-origin:24px 50%;` +
          `transform:translateY(${e * -14}px) rotate(${e * -2.6}deg) scale(${1 + e * 0.03})`;
      },
    };
  }

  /** Crossed off: fold away. Deleted: slide out left. Task: a copy flies into the Tasks button. */
  function leave(node: HTMLElement) {
    if (reduced) return { duration: 0 };
    const kind = node.dataset.leave;
    const h = node.offsetHeight;
    if (kind === 'task') flyToTasks(node);
    if (kind === 'delete') {
      return {
        duration: 260,
        css: (t: number) => `overflow:hidden;height:${h * Math.min(1, t * 1.7)}px;opacity:${t};transform:translateX(${(1 - t) * -30}px)`,
      };
    }
    return {
      duration: kind === 'task' ? 320 : 280,
      css: (t: number) => `overflow:hidden;height:${h * cubicOut(t)}px;opacity:${kind === 'task' ? 0 : t}`,
    };
  }

  function flyToTasks(node: HTMLElement) {
    const target = node.closest('.app, .timer-window')?.querySelector<HTMLElement>('[data-jot-target="tasks"]');
    const row = node.firstElementChild as HTMLElement | null;
    if (!target || !row) return;
    const from = row.getBoundingClientRect();
    const to = target.getBoundingClientRect();
    const ghost = row.cloneNode(true) as HTMLElement;
    ghost.classList.add('jot-ghost');
    // Inline, so the row's own scoped `position: relative` cannot win over it.
    Object.assign(ghost.style, {
      position: 'fixed', margin: '0', zIndex: '1000', pointerEvents: 'none',
      left: `${from.left}px`, top: `${from.top}px`, width: `${from.width}px`, height: `${from.height}px`,
    });
    document.body.appendChild(ghost);
    const dx = to.left + to.width / 2 - (from.left + from.width / 2);
    const dy = to.top + to.height / 2 - (from.top + from.height / 2);
    const flight = ghost.animate(
      [
        { transform: 'translate(0, 0) scale(1) rotate(0)', opacity: 1 },
        { transform: `translate(${dx * 0.18}px, 6px) scale(0.78) rotate(-2deg)`, opacity: 1, offset: 0.3 },
        { transform: `translate(${dx}px, ${dy}px) scale(0.06) rotate(-9deg)`, opacity: 0.25 },
      ],
      { duration: 560, easing: 'cubic-bezier(0.55, 0, 0.75, 0.45)', fill: 'forwards' },
    );
    flight.onfinish = () => {
      ghost.remove();
      tasksBump.update((n) => n + 1);
    };
  }

  /** In the focus stack a new slip lands on top of the pile. */
  function slipIn(node: HTMLElement) {
    if (reduced) return { duration: 0 };
    return {
      duration: 440,
      css: (t: number) => {
        const e = 1 - backOut(t);
        return `opacity:${Math.min(1, t * 2.5)};transform:translateY(${e * -26}px) rotate(${-1.2 + e * -5}deg) scale(${1 + e * 0.05})`;
      },
    };
  }
</script>

<svelte:window onkeydown={onWindowKey} />

{#if $jotPad.open}
  <div class="layer">
    <button class="veil" aria-label={zh ? '收起碎碎念' : 'Close jots'} tabindex="-1" onclick={() => close()}
      transition:fade={{ duration: reduced ? 0 : 160 }}></button>

    <div class="pad" role="dialog" aria-modal="true" aria-label={zh ? '碎碎念' : 'Jots'}
      in:fly={{ y: reduced ? 0 : 56, duration: reduced ? 0 : 280, easing: cubicOut, opacity: 0.2 }}
      out:fly={{ y: reduced ? 0 : 40, duration: reduced ? 0 : 170, opacity: 0 }}>
      <div class="binding" aria-hidden="true"></div>

      <header class="head">
        <h2>{zh ? '碎碎念' : 'Jots'}{#if openCount > 0}<span class="count">{openCount}</span>{/if}</h2>
        <span class="clock" style:color={phaseColor} title={zh ? '计时器照常在走' : 'The timer keeps going'}>
          {#if timer.is_running}<i></i>{:else if timer.is_paused}<b>Ⅱ</b>{/if}<span class="phase">{phase}</span> {clock}
        </span>
        <button class="x" onclick={() => close(true)} aria-label={zh ? '收起' : 'Close'} title={zh ? '收起（Esc）' : 'Close (Esc)'}>
          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 2l8 8M10 2l-8 8" /></svg>
        </button>
      </header>

      <form class="composer" onsubmit={(e) => { e.preventDefault(); void send(); }}>
        <textarea
          bind:this={field}
          bind:value={draft}
          rows="1"
          maxlength="500"
          {placeholder}
          aria-label={zh ? '写一条碎碎念' : 'Write a jot'}
          oninput={grow}
          onkeydown={onFieldKey}
        ></textarea>
        <div class="composer-foot">
          <span class="keys">
            {#if draft.length > 400}<span class="left" class:over={draft.length >= 500}>{draft.length}/500</span>
            {:else}<kbd>Enter</kbd> {zh ? '记下' : 'to jot'} · <kbd>Shift</kbd>+<kbd>Enter</kbd> {zh ? '换行' : 'new line'}{/if}
          </span>
          <button class="send jot-send" type="submit" disabled={!draft.trim() || sending}>
            <svg viewBox="0 0 14 14" aria-hidden="true"><path d="M11.5 3v4.2a1.8 1.8 0 0 1-1.8 1.8H3M5.6 6.2 2.8 9l2.8 2.8" /></svg>
            {zh ? '记下' : 'Jot'}
          </button>
        </div>
      </form>

      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="scroll" onpointerdown={stay}>
        {#if fold}
          <div class="fold">
            {#if stack.length}
              <div class="stack" style:height="{44 + (stack.length - 1) * 6}px" aria-label={zh ? '最新的碎碎念' : 'Latest jots'}>
                {#each stack as jot, i (jot.id)}
                  <div class="slip" style:--i={i} in:slipIn out:fade={{ duration: reduced ? 0 : 140 }}>{jot.body}</div>
                {/each}
              </div>
            {/if}
            <p class="fold-note">
              <b>{zh ? '专注中' : 'Focusing'}</b>
              {#if openCount === 0}
                · {zh ? '冒出念头就记在这里，记完按 Esc 回去' : 'jot whatever comes up, then Esc to get back'}
              {:else}
                · {zh ? `这 ${openCount} 条先放着，休息时再处理` : `${openCount} set aside for the break`}
              {/if}
            </p>
            {#if openCount > 0}
              <button class="ghost" onclick={() => (unfolded = true)}>{zh ? '现在就看' : 'Show them now'}</button>
            {/if}
          </div>
        {:else}
          {#if open.length}
            <ul class="list">
              {#each open as jot (jot.id)}
                <li in:dropIn out:leave data-leave={leaving[jot.id] ?? 'done'}>
                  <JotRow
                    {jot} {zh} {now}
                    subjects={$subjects}
                    popping={popping.has(jot.id)}
                    oncross={() => cross(jot)}
                    ontask={(subjectId) => toTask(jot, subjectId)}
                    onuntask={() => untask(jot)}
                    ondelete={() => remove(jot)}
                    onedit={(body) => edit(jot, body)}
                  />
                </li>
              {/each}
            </ul>
          {:else if !handled.length}
            <div class="empty">
              <svg viewBox="0 0 72 52" aria-hidden="true">
                <g transform="rotate(-6 30 30)"><rect class="e-slip" x="8" y="12" width="40" height="32" rx="2" /><path class="e-line" d="M15 23h24M15 30h18M15 37h21" /></g>
                <path class="e-bubble" d="M44 6h20a4 4 0 0 1 4 4v10a4 4 0 0 1-4 4h-9l-5 5v-5h-6a4 4 0 0 1-4-4V10a4 4 0 0 1 4-4Z" />
                <circle class="e-dot" cx="48.5" cy="15" r="1.6" /><circle class="e-dot" cx="54" cy="15" r="1.6" /><circle class="e-dot" cx="59.5" cy="15" r="1.6" />
              </svg>
              <p><b>{zh ? '脑子里冒出的事，都可以先丢在这里。' : 'Anything that pops into your head can wait here.'}</b></p>
              <p>{zh ? '专注时按 N 随手记一句，休息时再一件件划掉。' : 'Press N mid-focus to jot it down; cross things off in the break.'}</p>
            </div>
          {:else}
            <p class="all-done">{zh ? '都处理完了 ✓' : 'All handled ✓'}</p>
          {/if}

          {#if handled.length}
            <div class="handled-head">
              <button class="fold-btn" aria-expanded={showHandled} onclick={() => (showHandled = !showHandled)}>
                <svg class:turned={showHandled} viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 2 7 5 3.5 8" /></svg>
                {zh ? '已处理' : 'Handled'} · {handled.length}
              </button>
              {#if !confirmClear}
                <button class="ghost" onclick={() => (confirmClear = true)}>{zh ? '清空' : 'Clear'}</button>
              {/if}
            </div>
            {#if confirmClear}
              <div class="confirm" role="alertdialog" aria-label={zh ? '确认清空' : 'Confirm clearing'}>
                <span>{zh ? `删掉这 ${handled.length} 条已处理的？转成的任务会留着。` : `Delete these ${handled.length}? Tasks made from them stay.`}</span>
                <button class="ghost" onclick={() => (confirmClear = false)}>{zh ? '取消' : 'Cancel'}</button>
                <button class="ghost danger" onclick={clearHandled}>{zh ? '清空' : 'Clear'}</button>
              </div>
            {/if}
            {#if showHandled}
              <ul class="list handled" transition:fade={{ duration: reduced ? 0 : 140 }}>
                {#each handled as jot (jot.id)}
                  {@const task = jot.task_id === null ? undefined : $tasks.find((t) => t.id === jot.task_id)}
                  <li in:fade={{ duration: reduced ? 0 : 160 }} out:leave data-leave={leaving[jot.id] ?? 'done'}>
                    <JotRow
                      {jot} {zh} {now}
                      subjects={$subjects}
                      taskSubject={task === undefined ? undefined : task.subject_id === null ? null : $subjects.find((s) => s.id === task.subject_id)}
                      oncross={() => cross(jot)}
                      ontask={() => {}}
                      onuntask={() => untask(jot)}
                      ondelete={() => remove(jot)}
                      onedit={() => {}}
                    />
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if peek && !$jotPad.open}
  <div class="peek" role="status" transition:fly={{ y: reduced ? 0 : 16, duration: reduced ? 0 : 220 }}>
    <span class="peek-mark" aria-hidden="true">
      <svg viewBox="0 0 16 16"><path d="M3 3.5h10a1.5 1.5 0 0 1 1.5 1.5v5a1.5 1.5 0 0 1-1.5 1.5H8l-3 2.5v-2.5H3A1.5 1.5 0 0 1 1.5 10V5A1.5 1.5 0 0 1 3 3.5Z" /></svg>
    </span>
    <span class="peek-text">{zh ? `休息一下 · ${openCount} 条碎碎念待处理` : `Break time · ${openCount} jot${openCount === 1 ? '' : 's'} to handle`}</span>
    <button class="peek-go" onclick={() => void show()}>{zh ? '看看' : 'Open'}</button>
    <button class="peek-x" onclick={() => (peek = false)} aria-label={zh ? '不用了' : 'Dismiss'}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3l6 6M9 3l-6 6" /></svg>
    </button>
  </div>
{/if}

<ToastHost />

<style>
  /* ── Layer: covers the timer below the titlebar (the titlebar stays usable) ── */
  .layer {
    position: absolute;
    inset: 40px 0 0;
    z-index: 30;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    pointer-events: none;
    font-family: var(--font-ui);
  }

  .veil {
    position: absolute;
    inset: 0;
    padding: 0;
    border: 0;
    background: color-mix(in srgb, var(--color-background) 60%, transparent);
    pointer-events: auto;
    cursor: default;
  }

  .pad {
    --jot-row-radius: 10px;
    container-type: inline-size;
    position: relative;
    display: flex;
    flex-direction: column;
    width: min(100% - 16px, 440px);
    max-height: calc(100% - 10px);
    overflow: hidden;
    border-radius: 16px 16px 0 0;
    background: var(--ui-surface);
    color: var(--ui-text);
    box-shadow: 0 -12px 32px -14px color-mix(in srgb, var(--ui-text) 38%, transparent);
    pointer-events: auto;
  }

  .binding {
    flex-shrink: 0;
    height: 5px;
    background: var(--ui-brand);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 8px 6px 16px;
  }

  h2 {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    gap: 7px;
    margin: 0;
    white-space: nowrap;
    font-size: 15px;
    font-weight: 800;
    letter-spacing: 0.06em;
  }

  .count {
    display: inline-grid;
    place-items: center;
    min-width: 19px;
    height: 19px;
    padding: 0 6px;
    border-radius: 10px;
    background: var(--ui-selected);
    color: var(--ui-brand-strong);
    font-family: var(--font-ui);
    font-size: 11.5px;
    font-weight: 800;
    letter-spacing: 0;
    font-variant-numeric: tabular-nums;
  }

  .clock {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-left: auto;
    font-size: 12px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .clock i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .clock b {
    font-size: 10px;
  }

  .x {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--ui-text-muted);
    cursor: pointer;
  }

  .x svg {
    width: 11px;
    height: 11px;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .x:hover {
    background: var(--ui-hover);
    color: var(--ui-text);
  }

  /* Compact windows (≈240 px): time only, no key hints. */
  @container (max-width: 280px) {
    .head {
      padding-left: 12px;
    }

    .clock .phase,
    .keys {
      display: none;
    }

    .composer {
      margin: 0 8px 6px;
    }
  }

  /* ── Composer ── */
  .composer {
    flex-shrink: 0;
    margin: 0 12px 8px;
    border: 1.5px solid var(--ui-border-strong);
    border-radius: 12px;
    background: var(--ui-page);
    transition: border-color 150ms ease;
  }

  .composer:focus-within {
    border-color: var(--ui-brand);
  }

  textarea {
    display: block;
    width: 100%;
    max-height: 112px;
    overflow-y: hidden;
    padding: 9px 12px 3px;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 14px;
    line-height: 1.45;
    resize: none;
    outline: none;
  }

  textarea::placeholder {
    color: var(--ui-text-muted);
  }

  .composer-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 6px 6px 12px;
  }

  .keys {
    min-width: 0;
    overflow: hidden;
    font-size: 11px;
    color: var(--ui-text-muted);
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  kbd {
    padding: 0 4px;
    border: 1px solid var(--ui-border-strong);
    border-radius: 3px;
    font: inherit;
    font-size: 10.5px;
  }

  .left {
    font-variant-numeric: tabular-nums;
  }

  .left.over {
    color: var(--ui-danger);
    font-weight: 700;
  }

  .send {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    margin-left: auto;
    height: 28px;
    padding: 0 12px 0 10px;
    border: 0;
    border-radius: 8px;
    background: var(--ui-brand);
    color: var(--ui-on-brand);
    font: inherit;
    font-size: 13px;
    font-weight: 750;
    cursor: pointer;
    transition:
      background 120ms ease,
      opacity 120ms ease,
      transform 80ms ease;
  }

  .send svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .send:hover:not(:disabled) {
    background: var(--ui-brand-strong);
  }

  .send:active:not(:disabled) {
    transform: translateY(1px);
  }

  .send:disabled {
    opacity: 0.42;
    cursor: default;
  }

  /* ── List ── */
  .scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 0 4px 10px;
    scrollbar-width: thin;
    scrollbar-color: var(--ui-border-strong) transparent;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .list li + li {
    border-top: 1px solid var(--ui-border);
  }

  .all-done {
    margin: 6px 0 4px;
    padding: 0 12px;
    font-size: 13px;
    font-weight: 650;
    color: var(--jot-leaf, var(--color-short-round));
  }

  .handled-head {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 8px 2px 8px;
    padding-top: 8px;
    border-top: 1px dashed var(--ui-border-strong);
    font-size: 12px;
  }

  .fold-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-right: auto;
    padding: 3px 6px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--ui-text-muted);
    font: inherit;
    font-weight: 700;
    cursor: pointer;
  }

  .fold-btn:hover {
    background: var(--ui-hover);
    color: var(--ui-text);
  }

  .fold-btn svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 150ms ease;
  }

  .fold-btn svg.turned {
    transform: rotate(90deg);
  }

  .confirm {
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 4px 8px 2px;
    padding: 6px 6px 6px 10px;
    border-radius: 8px;
    background: var(--ui-danger-soft);
    font-size: 12px;
    line-height: 1.4;
    color: var(--ui-text);
  }

  .confirm span {
    flex: 1;
    min-width: 0;
  }

  .ghost {
    flex-shrink: 0;
    white-space: nowrap;
    height: 26px;
    padding: 0 9px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--ui-brand-strong);
    font: inherit;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
  }

  .ghost:hover {
    background: var(--ui-hover);
  }

  .ghost.danger {
    color: var(--ui-danger);
  }

  /* ── Focus: the stack of slips ── */
  .fold {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 4px 12px 2px;
  }

  .stack {
    position: relative;
    width: 100%;
    margin: 4px 0 2px;
  }

  .slip {
    position: absolute;
    left: calc(var(--i) * 5px);
    right: calc(var(--i) * -3px + 6px);
    top: calc(var(--i) * 6px);
    z-index: calc(3 - var(--i));
    height: 38px;
    padding: 0 12px;
    overflow: hidden;
    border: 1px solid var(--ui-border-strong);
    border-radius: 8px;
    background: var(--ui-page);
    box-shadow: 0 2px 6px -3px color-mix(in srgb, var(--ui-text) 30%, transparent);
    color: var(--ui-text);
    font-size: 13.5px;
    line-height: 36px;
    white-space: nowrap;
    text-overflow: ellipsis;
    transform: rotate(calc(-1.2deg + var(--i) * 1.3deg));
    transition:
      left 260ms ease,
      right 260ms ease,
      top 260ms ease,
      transform 260ms ease,
      color 200ms ease;
  }

  .slip:not(:first-child) {
    color: transparent;
  }

  .fold-note {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    text-align: center;
    color: var(--ui-text-muted);
  }

  .fold-note b {
    color: var(--color-focus-round);
  }

  /* ── Empty ── */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 20px 10px;
    text-align: center;
  }

  .empty svg {
    width: 72px;
    height: 52px;
    margin-bottom: 4px;
  }

  .e-slip {
    fill: var(--ui-page);
    stroke: var(--ui-text);
    stroke-width: 1.4;
  }

  .e-line {
    stroke: var(--ui-border-strong);
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .e-bubble {
    fill: var(--ui-brand);
  }

  .e-dot {
    fill: var(--ui-on-brand);
  }

  .empty p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--ui-text-muted);
  }

  .empty p b {
    color: var(--ui-text);
    font-weight: 650;
  }

  /* ── The flying copy of a jot on its way into Tasks ── */
  :global(.jot-ghost) {
    border-radius: 10px;
    background: var(--ui-surface);
    box-shadow: 0 6px 18px -6px color-mix(in srgb, var(--ui-text) 45%, transparent);
    transform-origin: 50% 50%;
  }

  /* ── Break peek ── */
  .peek {
    position: absolute;
    left: 50%;
    bottom: 12px;
    z-index: 25;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: calc(100% - 20px);
    padding: 6px 6px 6px 10px;
    translate: -50% 0;
    border-radius: 12px;
    background: var(--ui-surface);
    color: var(--ui-text);
    box-shadow: 0 8px 22px -8px color-mix(in srgb, var(--ui-text) 40%, transparent);
    font-family: var(--font-ui);
    font-size: 12.5px;
    white-space: nowrap;
  }

  .peek-mark {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border-radius: 6px;
    background: var(--ui-brand);
    color: var(--ui-on-brand);
  }

  .peek-mark svg {
    width: 13px;
    height: 13px;
    fill: currentColor;
  }

  .peek-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 600;
  }

  .peek-go {
    flex-shrink: 0;
    height: 24px;
    padding: 0 10px;
    border: 0;
    border-radius: 7px;
    background: var(--ui-brand);
    color: var(--ui-on-brand);
    font: inherit;
    font-weight: 750;
    cursor: pointer;
  }

  .peek-x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--ui-text-muted);
    cursor: pointer;
  }

  .peek-x svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .pad button:focus-visible {
    outline: 2px solid var(--ui-brand);
    outline-offset: 1px;
  }

  /* ── Classic Tomato: a paper memo pad with a red glued binding ────────── */
  :global(html[data-pomo-theme='classic-tomato']) .pad {
    --jot-row-radius: 3px;
    --jot-ink: var(--pomo-ink);
    --jot-stamp-edge: var(--pomo-ink);
    --jot-leaf: var(--color-short-round);
    border: 1.5px solid var(--pomo-ink);
    border-bottom: 0;
    border-radius: 6px 6px 0 0;
    background: var(--pomo-light) var(--classic-grain);
    box-shadow: 0 -10px 26px -16px rgb(32 38 32 / 45%);
  }

  :global(html[data-pomo-theme='classic-tomato']) .binding {
    height: 7px;
    background: var(--pomo-main);
    border-bottom: 1.5px solid var(--pomo-ink);
    /* The perforation where a page tears off. */
    box-shadow: 0 4px 0 -2.5px var(--pomo-light), 0 5px 0 -2.5px rgb(32 38 32 / 0%);
  }

  :global(html[data-pomo-theme='classic-tomato']) .head {
    padding-top: 10px;
    border-top: 1.5px dashed rgb(32 38 32 / 22%);
    margin-top: 3px;
  }

  :global(html[data-pomo-theme='classic-tomato']) h2 {
    font-family: var(--font-classic-display);
    font-size: 17px;
    letter-spacing: 0.16em;
  }

  :global(html[data-pomo-theme='classic-tomato']) .count {
    border-radius: 3px;
    background: var(--pomo-ink);
    color: var(--pomo-light);
  }

  :global(html[data-pomo-theme='classic-tomato']) .composer {
    border-color: var(--pomo-ink);
    border-radius: 4px;
    background: var(--pomo-light);
  }

  /* The box is focused whenever the pad is open, so a quiet navy edge, not a ring. */
  :global(html[data-pomo-theme='classic-tomato']) .composer:focus-within {
    border-color: var(--pomo-alt);
    box-shadow: inset 0 0 0 0.5px var(--pomo-alt);
  }

  :global(html[data-pomo-theme='classic-tomato']) .send {
    border: 1.5px solid var(--pomo-ink);
    border-radius: 4px;
    box-shadow: 2px 2px 0 var(--pomo-ink);
    letter-spacing: 0.06em;
  }

  :global(html[data-pomo-theme='classic-tomato']) .send:active:not(:disabled) {
    box-shadow: none;
    transform: translate(2px, 2px);
  }

  :global(html[data-pomo-theme='classic-tomato']) .list li + li {
    border-top: 1px dashed rgb(32 38 32 / 18%);
  }

  :global(html[data-pomo-theme='classic-tomato']) .slip {
    border: 1.5px solid var(--pomo-ink);
    border-radius: 3px;
    background: var(--pomo-light);
    box-shadow: 2px 2px 0 rgb(32 38 32 / 85%);
  }

  :global(html[data-pomo-theme='classic-tomato']) .fold-note b {
    font-family: var(--font-classic-display);
    letter-spacing: 0.08em;
  }

  :global(html[data-pomo-theme='classic-tomato']) .e-bubble {
    fill: var(--pomo-main);
  }

  :global(html[data-pomo-theme='classic-tomato']) .e-line {
    stroke: var(--color-short-round);
  }

  :global(html[data-pomo-theme='classic-tomato']) .pad textarea:focus-visible {
    outline: none;
  }

  :global(html[data-pomo-theme='classic-tomato']) :is(.pad, .peek) button:focus-visible {
    outline: 2px solid var(--pomo-alt);
    outline-offset: 2px;
  }

  :global(html[data-pomo-theme='classic-tomato']) .peek {
    border: 1.5px solid var(--pomo-ink);
    border-radius: 4px;
    background: var(--pomo-light);
    box-shadow: 2px 2px 0 var(--pomo-ink);
  }

  :global(html[data-pomo-theme='classic-tomato']) :is(.peek-mark, .peek-go) {
    border-radius: 3px;
    background: var(--pomo-main);
  }

  :global(html[data-pomo-theme='classic-tomato'] .jot-ghost) {
    border: 1.5px solid var(--pomo-ink);
    border-radius: 3px;
    background: var(--pomo-light);
    box-shadow: 2px 2px 0 var(--pomo-ink);
  }

  /* Berry Planet's "short round" is magenta; its tomato leaves stay green-gold. */
  :global(html[data-pomo-theme='berry-planet']) .pad {
    --jot-leaf: var(--ui-accent-2);
  }
</style>
