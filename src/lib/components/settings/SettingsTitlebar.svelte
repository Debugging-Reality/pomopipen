<script lang="ts">
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { isMac } from '$lib/utils/platform';
  import * as m from '$paraglide/messages.js';
  import type { Snippet } from 'svelte';

  // Shared by the settings and tasks windows; `left` holds optional controls.
  let {
    title = m.settings_title(),
    left,
    maximizable = false,
  }: { title?: string; left?: Snippet; maximizable?: boolean } = $props();

  function close() {
    getCurrentWebviewWindow().close();
  }
</script>

<nav class="titlebar" class:macos={isMac} data-tauri-drag-region>
  {#if left}<div class="left">{@render left()}</div>{/if}
  <h1 class="title"><span class="spark" aria-hidden="true">✦</span>{title}</h1>
  <!-- Hidden on macOS; the native traffic light close button handles this. -->
  {#if !isMac}
    {#if maximizable}
      <button class="btn-close btn-max" onclick={() => getCurrentWebviewWindow().toggleMaximize()} aria-label="Maximize" title="Maximize">
        <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
          <rect x="1" y="1" width="9" height="9" rx="2" fill="none" stroke="currentColor" stroke-width="1.4" />
        </svg>
      </button>
    {/if}
    <button class="btn-close" onclick={close} aria-label="Close" title="Close">
      <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
        <path d="M1.5 1.5l9 9M10.5 1.5l-9 9" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
    </button>
  {/if}
</nav>

<style>
  .titlebar {
    height: 44px;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    flex-shrink: 0;
    background: var(--ui-brand);
    border-bottom: 1px solid color-mix(in srgb, var(--ui-brand-strong) 60%, transparent);
  }

  /* Shift the centered title right of the traffic lights on macOS. */
  .macos {
    padding-left: 72px;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-ui);
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--ui-on-brand);
    pointer-events: none;
  }

  .left {
    position: absolute;
    left: 8px;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .macos .left {
    left: 80px;
  }

  .spark {
    font-size: 11px;
    letter-spacing: 0;
    color: var(--ui-accent-2);
  }

  .btn-close {
    position: absolute;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--ui-on-brand);
    cursor: pointer;
    transition: background 150ms ease;
  }

  .btn-max {
    right: 44px;
  }

  .btn-close:hover {
    background: color-mix(in srgb, var(--ui-on-brand) 18%, transparent);
  }

  .btn-close:active {
    background: color-mix(in srgb, var(--ui-on-brand) 28%, transparent);
  }

  .btn-close:focus-visible {
    outline: 2px solid var(--ui-on-brand);
    outline-offset: -2px;
  }
</style>
