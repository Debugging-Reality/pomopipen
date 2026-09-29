<script lang="ts">
  // Compact "time by subject" list. Always all-subjects/all-time — see the
  // comment on `breakdown` in stats/+page.svelte for why it ignores the
  // subject filter that narrows the tabs above it.
  import type { SubjectTotal } from '$lib/types';
  import * as m from '$paraglide/messages.js';

  let { items }: { items: SubjectTotal[] } = $props();

  function fmtDuration(secs: number): string {
    const h = Math.floor(secs / 3600);
    const min = Math.round((secs % 3600) / 60);
    if (h === 0) return `${min}m`;
    return `${h}h ${String(min).padStart(2, '0')}m`;
  }
</script>

{#if items.length > 0}
  <div class="breakdown">
    <div class="breakdown-heading">{m.stats_by_subject()}</div>
    <div class="breakdown-list">
      {#each items as item (item.subject_id ?? 'uncategorized')}
        <div class="breakdown-row">
          <span
            class="dot"
            class:dot-none={item.subject_id === null}
            style={item.subject_id === null ? '' : `background: ${item.color}`}
          ></span>
          <span class="name"
            >{item.subject_id === null ? m.subject_filter_uncategorized() : item.name}</span
          >
          <span class="time">{fmtDuration(item.focus_secs)}</span>
          <span class="rounds">({item.rounds})</span>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .breakdown {
    padding: 14px 18px 16px;
    border-radius: 14px;
    border: 1px solid var(--ui-border);
    background: var(--ui-surface);
  }

  .breakdown-heading {
    margin-bottom: 10px;
    font-size: 14px;
    font-weight: 700;
    color: var(--ui-text);
  }

  .breakdown-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .breakdown-row {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 12px;
    border-radius: 999px;
    background: var(--ui-page);
    border: 1px solid var(--ui-border);
    font-size: 13px;
    color: var(--ui-text);
  }

  .dot {
    flex-shrink: 0;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }

  .dot-none {
    background: none;
    border: 1.5px dashed var(--ui-text-muted);
  }

  .name {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }

  .rounds {
    font-size: 12px;
    color: var(--ui-text-muted);
  }
</style>