<script lang="ts">
  // Classic Tomato: offer to move subjects whose color is not a tomato variety
  // onto the closest variety (docs/design/classic-tomato §5.7). Nothing changes
  // until the user confirms in the preview, and the change can be undone.
  import { tick } from 'svelte';
  import { subjects } from '$lib/stores/subjects';
  import { subjectsUpdate } from '$lib/ipc';
  import { notify } from '$lib/stores/toast';
  import { nearestVarieties, varietyFor, type Variety } from '$lib/themes/varieties';
  import TomatoTop from '../classic/TomatoTop.svelte';

  let { zh }: { zh: boolean } = $props();

  let open = $state(false);
  let busy = $state(false);
  let skipped = $state<Set<number>>(new Set());
  let applyButton = $state<HTMLButtonElement>();

  let pending = $derived($subjects.filter((s) => !varietyFor(s.color)));
  let plan = $derived.by(() => {
    const matches = nearestVarieties($subjects);
    return pending.map((s) => ({ subject: s, ...matches.get(s.id)! })).filter((row) => row.variety);
  });
  let chosen = $derived(plan.filter((row) => !skipped.has(row.subject.id)));

  const nameOf = (v: Variety) => (zh ? v.nameZh : v.name);

  async function show() {
    skipped = new Set();
    open = true;
    await tick();
    applyButton?.focus();
  }

  function toggle(id: number) {
    const next = new Set(skipped);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    skipped = next;
  }

  async function apply() {
    if (busy || chosen.length === 0) return;
    busy = true;
    const done: { id: number; color: string }[] = [];
    try {
      for (const row of chosen) {
        await subjectsUpdate(row.subject.id, undefined, row.variety.hex);
        done.push({ id: row.subject.id, color: row.subject.color });
      }
      open = false;
      notify(zh ? `已把 ${done.length} 个科目换成番茄品种` : `${done.length} subject${done.length === 1 ? '' : 's'} now use tomato varieties`, {
        ms: 10000,
        action: {
          label: zh ? '撤销' : 'Undo',
          run: () => void Promise.all(done.map((d) => subjectsUpdate(d.id, undefined, d.color))),
        },
      });
    } catch (e) {
      notify(zh ? `没有全部换完（已换 ${done.length} 个）：${e}` : `Stopped after ${done.length}: ${e}`, { tone: 'error' });
    } finally {
      busy = false;
    }
  }
</script>

{#if pending.length > 0}
  <div class="notice" role="status">
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1.5 15 14H1z" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" /><path d="M8 6v4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /><circle cx="8" cy="12" r=".9" fill="currentColor" /></svg>
    <span>{zh ? `${pending.length} 个科目的颜色还不是番茄品种色` : `${pending.length} subject${pending.length === 1 ? '' : 's'} not on a tomato variety yet`}</span>
    <button class="s-btn convert" onclick={show}>{zh ? '一键换成最接近的品种' : 'Switch to the closest varieties'}</button>
  </div>
{/if}

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" onclick={(e) => { if (e.target === e.currentTarget && !busy) open = false; }}
    onkeydown={(e) => { if (e.key === 'Escape' && !busy) open = false; }}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="variety-title">
      <h4 id="variety-title">{zh ? '换成最接近的番茄品种' : 'Switch to the closest tomato varieties'}</h4>
      <p class="sub">{zh ? '按色彩接近程度配对，每个品种尽量只给一个科目。可以取消勾选不想换的科目。' : 'Paired by closest color, one variety per subject where possible. Untick any you want to keep.'}</p>
      <div class="rows">
        {#each plan as row (row.subject.id)}
          {@const on = !skipped.has(row.subject.id)}
          <label class="row" class:off={!on}>
            <input type="checkbox" checked={on} onchange={() => toggle(row.subject.id)} />
            <span class="from">
              <i style:background={row.subject.color}></i>
              <span>{row.subject.name}<small>{row.subject.color}{row.subject.archived ? (zh ? ' · 已归档' : ' · archived') : ''}</small></span>
            </span>
            <span class="arrow" aria-hidden="true">→</span>
            <span class="to">
              <span class="mini" style:--skin={row.variety.hex}>
                <TomatoTop kind={row.variety.top} width={row.variety.top === 'stem' ? 12 : 11} onGreen={row.variety.id === 'zebra' || row.variety.id === 'green'} />
              </span>
              <span>{nameOf(row.variety)}<small>{row.variety.hex}{row.shared ? (zh ? ' · 与其他科目同色' : ' · shared with another subject') : ''}</small></span>
            </span>
          </label>
        {/each}
      </div>
      <div class="foot">
        <p>{zh ? '周历和统计里这些科目的历史记录也会一起换色；学习记录本身不变。' : 'Their past sessions in the calendar and charts change color too; the records themselves are untouched.'}</p>
        <button class="s-btn" disabled={busy} onclick={() => (open = false)}>{zh ? '取消' : 'Cancel'}</button>
        <button class="s-btn s-btn--primary" bind:this={applyButton} disabled={busy || chosen.length === 0} onclick={apply}>
          {zh ? `换 ${chosen.length} 个科目` : `Switch ${chosen.length}`}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 4px;
    background: var(--pomo-pop, #d5a438);
    color: var(--ui-text);
    font-size: 13px;
    font-weight: 600;
  }

  .notice svg {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
  }

  .convert {
    margin-left: auto;
    background: var(--ui-text);
    border-color: var(--ui-text);
    color: var(--ui-surface);
  }

  .convert:hover:not(:disabled) {
    background: color-mix(in srgb, var(--ui-text) 85%, var(--ui-surface));
    color: var(--ui-surface);
  }

  .scrim {
    position: fixed;
    inset: 0;
    z-index: 400;
    display: grid;
    place-items: center;
    padding: 20px;
    background: rgb(32 38 32 / 28%);
  }

  .dialog {
    width: min(480px, 100%);
    max-height: calc(100vh - 40px);
    overflow-y: auto;
    padding: 18px 20px 16px;
    border: 1.5px solid var(--ui-text);
    border-radius: 6px;
    background: var(--ui-surface);
    box-shadow: 0 18px 40px -18px rgb(32 38 32 / 55%);
  }

  h4 {
    font-family: var(--font-classic-display, serif);
    font-size: 18px;
    font-weight: 800;
    letter-spacing: 0.06em;
  }

  .sub {
    margin: 2px 0 10px;
    font-size: 12.5px;
    color: var(--ui-text-muted);
  }

  .row {
    display: grid;
    grid-template-columns: 22px 1fr 18px 1fr;
    gap: 10px;
    align-items: center;
    padding: 7px 0;
    border-bottom: 1px solid rgb(32 38 32 / 12%);
    font-size: 13.5px;
    cursor: pointer;
  }

  .row input {
    width: 16px;
    height: 16px;
    accent-color: var(--color-short-round);
  }

  .row.off {
    color: var(--ui-text-muted);
  }

  .row.off .to {
    opacity: 0.45;
  }

  .from,
  .to {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .from i {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    border-radius: 50%;
  }

  small {
    display: block;
    font-size: 11.5px;
    color: var(--ui-text-muted);
  }

  .mini {
    position: relative;
    flex-shrink: 0;
    display: grid;
    justify-items: center;
    width: 16px;
    height: 14px;
    border-radius: 50%;
    background: var(--skin);
    box-shadow: inset 0 -2px 0 rgb(0 0 0 / 12%);
  }

  .mini :global(.top) {
    position: absolute;
    top: -3px;
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }

  .foot p {
    flex: 1;
    font-size: 12px;
    color: var(--ui-text-muted);
  }
</style>
