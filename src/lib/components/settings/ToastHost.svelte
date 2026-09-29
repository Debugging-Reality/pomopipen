<script lang="ts">
  import { fly } from 'svelte/transition';
  import { toasts, dismiss } from '$lib/stores/toast';
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each $toasts as toast (toast.id)}
    <div class="toast" class:error={toast.tone === 'error'} transition:fly={{ y: 8, duration: 160 }}>
      <span class="mark" aria-hidden="true">{toast.tone === 'error' ? '!' : '✓'}</span>
      <span class="text">{toast.text}</span>
      {#if toast.action}
        <button onclick={() => { toast.action?.run(); dismiss(toast.id); }}>{toast.action.label}</button>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 18px;
    z-index: 500;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 9px;
    max-width: 360px;
    padding: 8px 10px 8px 9px;
    border-radius: 11px;
    background: var(--ui-text);
    color: var(--ui-surface);
    font-size: 13px;
    font-weight: 600;
    box-shadow: 0 6px 20px color-mix(in srgb, var(--ui-text) 28%, transparent);
  }

  .mark {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--ui-accent-2);
    color: var(--ui-text);
    font-size: 11px;
    font-weight: 800;
    flex-shrink: 0;
  }

  .toast.error .mark {
    background: var(--ui-danger);
    color: #fff;
  }

  .text {
    line-height: 1.4;
  }

  button {
    margin-left: 4px;
    padding: 3px 9px;
    border-radius: 7px;
    border: 1px solid color-mix(in srgb, var(--ui-surface) 45%, transparent);
    background: none;
    color: var(--ui-surface);
    font: inherit;
    font-size: 12.5px;
    cursor: pointer;
  }

  button:hover {
    background: color-mix(in srgb, var(--ui-surface) 16%, transparent);
  }
</style>
