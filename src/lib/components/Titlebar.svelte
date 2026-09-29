<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebviewWindow, WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { setWindowVisibility } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import { isMac } from '$lib/utils/platform';
  import Tooltip from './Tooltip.svelte';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import { jots, jotPad, tasksBump, openJotPad, closeJotPad } from '$lib/stores/jots';
  import { badgeText } from '$lib/utils/jots';

  let maximized = $state(false);

  // 碎碎念: the bubble's badge counts open jots and bumps when one is added;
  // the Tasks button gulps when a jot flies into it.
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let openJots = $derived($jots.filter((j) => j.done_at === null).length);
  let badgeBump = $state(0);
  let lastOpen = -1;
  $effect(() => {
    const n = openJots;
    if (lastOpen >= 0 && n > lastOpen) badgeBump++;
    lastOpen = n;
  });
  let gulp = $state(0);
  $effect(() => {
    if ($tasksBump > 0) gulp = $tasksBump;
  });

  function toggleJots() {
    if ($jotPad.open) closeJotPad();
    else openJotPad();
  }
  let suppressTitlebarHover = $state(false);

  function blurTitlebarControl() {
    const active = document.activeElement;
    if (active instanceof HTMLElement && active.closest('.titlebar')) active.blur();
  }

  function suppressRestoredTitlebarState() {
    suppressTitlebarHover = true;
    blurTitlebarControl();
  }

  onMount(() => {
    const win = getCurrentWebviewWindow();
    win.isMaximized().then((v) => {
      maximized = v;
    });
    const unlisten = win.onResized(async () => {
      maximized = await win.isMaximized();
    });
    const clearRestoredTitlebarFocus = () => {
      if (suppressTitlebarHover) requestAnimationFrame(blurTitlebarControl);
    };
    const clearSuppressedTitlebarHover = () => {
      suppressTitlebarHover = false;
    };
    window.addEventListener('focus', clearRestoredTitlebarFocus);
    document.addEventListener('pointermove', clearSuppressedTitlebarHover);
    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener('focus', clearRestoredTitlebarFocus);
      document.removeEventListener('pointermove', clearSuppressedTitlebarHover);
    };
  });

  async function openSettings() {
    const existing = await WebviewWindow.getByLabel('settings');
    if (existing) {
      await existing.show();
      await existing.setFocus();
      return;
    }
    new WebviewWindow('settings', {
      url: '/settings',
      title: 'PomoPipen — Settings',
      width: 760,
      height: 560,
      minWidth: 640,
      minHeight: 460,
      // On macOS: native decorations + overlay titlebar for rounded corners and
      // traffic light buttons. On other platforms: custom decorations-free window.
      decorations: isMac,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      titleBarStyle: isMac ? ('Overlay' as any) : undefined,
      hiddenTitle: isMac ? true : undefined,
      resizable: true,
      visible: false,
    });
  }

  async function openTasks() {
    const existing = await WebviewWindow.getByLabel('tasks');
    if (existing) {
      await existing.show();
      await existing.setFocus();
      return;
    }
    new WebviewWindow('tasks', {
      url: '/tasks',
      title: 'PomoPipen — Tasks',
      width: 520,
      height: 660,
      minWidth: 420,
      minHeight: 420,
      decorations: isMac,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      titleBarStyle: isMac ? ('Overlay' as any) : undefined,
      hiddenTitle: isMac ? true : undefined,
      resizable: true,
      visible: false,
    });
  }

  async function openStats() {
    const existing = await WebviewWindow.getByLabel('stats');
    if (existing) {
      await existing.show();
      await existing.setFocus();
      return;
    }
    new WebviewWindow('stats', {
      url: '/stats',
      title: 'PomoPipen — Statistics',
      width: 900,
      height: 680,
      minWidth: 560,
      minHeight: 420,
      decorations: isMac,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      titleBarStyle: isMac ? ('Overlay' as any) : undefined,
      hiddenTitle: isMac ? true : undefined,
      resizable: true,
      visible: false,
    });
  }

  async function minimize() {
    suppressRestoredTitlebarState();
    if ($settings.min_to_tray) {
      await setWindowVisibility(false);
    } else {
      await getCurrentWebviewWindow().minimize();
    }
  }

  function toggleMaximize() {
    getCurrentWebviewWindow().toggleMaximize();
  }

  async function close() {
    suppressRestoredTitlebarState();
    await getCurrentWebviewWindow().close();
  }
</script>

{#snippet settingsBtn()}
  <Tooltip text={m.tooltip_settings()}>
    <button class="btn-icon" onclick={openSettings} aria-label="Settings">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <line
          x1="2"
          y1="4"
          x2="14"
          y2="4"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
        <circle
          cx="5"
          cy="4"
          r="1.8"
          fill="var(--color-background)"
          stroke="currentColor"
          stroke-width="1.3"
        />
        <line
          x1="2"
          y1="8"
          x2="14"
          y2="8"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
        <circle
          cx="11"
          cy="8"
          r="1.8"
          fill="var(--color-background)"
          stroke="currentColor"
          stroke-width="1.3"
        />
        <line
          x1="2"
          y1="12"
          x2="14"
          y2="12"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
        <circle
          cx="7"
          cy="12"
          r="1.8"
          fill="var(--color-background)"
          stroke="currentColor"
          stroke-width="1.3"
        />
      </svg>
    </button>
  </Tooltip>
{/snippet}

{#snippet statsBtn()}
  <Tooltip text={m.tooltip_statistics()}>
    <button class="btn-icon" onclick={openStats} aria-label="Statistics">
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <rect x="2" y="9" width="3" height="5" rx="0.5" fill="currentColor" opacity="0.6" />
        <rect x="6.5" y="5" width="3" height="9" rx="0.5" fill="currentColor" opacity="0.8" />
        <rect x="11" y="2" width="3" height="12" rx="0.5" fill="currentColor" />
      </svg>
    </button>
  </Tooltip>
{/snippet}

{#snippet tasksBtn()}
  <Tooltip text={m.tooltip_tasks()}>
    <button class="btn-icon" onclick={openTasks} aria-label="Tasks" data-jot-target="tasks">
      {#key gulp}<svg class:gulp={gulp > 0} width="16" height="16" viewBox="0 0 16 16" fill="none">
        <rect
          x="2"
          y="2.5"
          width="3"
          height="3"
          rx="0.5"
          stroke="currentColor"
          stroke-width="1.3"
        />
        <path
          d="M2.6 4 L3.3 4.7 L4.6 3.2"
          stroke="currentColor"
          stroke-width="1"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <line
          x1="7"
          y1="4"
          x2="14"
          y2="4"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
        />
        <rect
          x="2"
          y="7.5"
          width="3"
          height="3"
          rx="0.5"
          stroke="currentColor"
          stroke-width="1.3"
          opacity="0.7"
        />
        <line
          x1="7"
          y1="9"
          x2="14"
          y2="9"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
          opacity="0.7"
        />
        <rect
          x="2"
          y="12.5"
          width="3"
          height="3"
          rx="0.5"
          stroke="currentColor"
          stroke-width="1.3"
          opacity="0.5"
        />
        <line
          x1="7"
          y1="14"
          x2="14"
          y2="14"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linecap="round"
          opacity="0.5"
        />
      </svg>{/key}
    </button>
  </Tooltip>
{/snippet}

{#snippet jotsBtn()}
  <Tooltip text={zh ? '碎碎念（N）' : 'Jots (N)'}>
    <button
      class="btn-icon jots"
      class:on={$jotPad.open}
      onclick={toggleJots}
      aria-label={zh ? `碎碎念，${openJots} 条未处理` : `Jots, ${openJots} open`}
      aria-pressed={$jotPad.open}
    >
      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
        <path
          d="M3.2 2.8h9.6a1.6 1.6 0 0 1 1.6 1.6v5.4a1.6 1.6 0 0 1-1.6 1.6H8.4L5.2 14v-2.6h-2A1.6 1.6 0 0 1 1.6 9.8V4.4a1.6 1.6 0 0 1 1.6-1.6Z"
          stroke="currentColor"
          stroke-width="1.3"
          stroke-linejoin="round"
        />
        <circle cx="5.2" cy="7.1" r="0.95" fill="currentColor" />
        <circle cx="8" cy="7.1" r="0.95" fill="currentColor" />
        <circle cx="10.8" cy="7.1" r="0.95" fill="currentColor" />
      </svg>
      {#if openJots > 0}
        {#key badgeBump}<span class="badge" class:bump={badgeBump > 0}>{badgeText(openJots)}</span>{/key}
      {/if}
    </button>
  </Tooltip>
{/snippet}

<nav class="titlebar" class:suppress-hover={suppressTitlebarHover} data-tauri-drag-region>
  <!-- Left: settings + stats buttons on Linux/Windows. On macOS the traffic
       lights live here; the action buttons move to the right side instead. -->
  {#if !isMac}
    {@render tasksBtn()}
    {@render jotsBtn()}
    {@render settingsBtn()}
    {@render statsBtn()}
  {/if}

  <!-- Right: settings + stats buttons on macOS, window controls on Linux/Windows. -->
  <div class="controls">
    {#if isMac}
      {@render statsBtn()}
      {@render settingsBtn()}
      {@render jotsBtn()}
      {@render tasksBtn()}
    {:else}
      <button class="btn-icon" onclick={minimize} aria-label="Minimize">
        <svg width="12" height="12" viewBox="0 0 12 12">
          <line
            x1="1"
            y1="6"
            x2="11"
            y2="6"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
      <button
        class="btn-icon"
        onclick={toggleMaximize}
        aria-label={maximized ? 'Restore' : 'Maximize'}
      >
        {#if maximized}
          <svg width="12" height="12" viewBox="0 0 12 12">
            <rect
              x="3"
              y="1"
              width="8"
              height="8"
              rx="1"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
            />
            <path
              d="M1 4 L1 11 L8 11"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        {:else}
          <svg width="12" height="12" viewBox="0 0 12 12">
            <rect
              x="1"
              y="1"
              width="10"
              height="10"
              rx="1"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
            />
          </svg>
        {/if}
      </button>
      <button class="btn-icon close" onclick={close} aria-label="Close">
        <svg width="12" height="12" viewBox="0 0 12 12">
          <line
            x1="1"
            y1="1"
            x2="11"
            y2="11"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
          <line
            x1="11"
            y1="1"
            x2="1"
            y2="11"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
    {/if}
  </div>
</nav>

<style>
  .titlebar {
    height: 40px;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    position: relative;
    flex-shrink: 0;
  }

  .controls {
    display: flex;
    gap: 4px;
    margin-left: auto;
  }

  .btn-icon {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--color-foreground-darker, var(--color-foreground));
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 4px;
    transition:
      color 0.15s,
      background 0.15s;
  }

  .btn-icon:focus {
    outline: none;
  }

  .btn-icon:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }

  .titlebar:not(.suppress-hover) .btn-icon:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  /* 碎碎念 button: the open count sits on the bubble's shoulder. */
  .btn-icon.jots {
    position: relative;
  }

  .btn-icon.jots.on {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .badge {
    position: absolute;
    top: 1px;
    right: -1px;
    min-width: 14px;
    height: 14px;
    padding: 0 3.5px;
    border-radius: 7px;
    background: var(--color-focus-round);
    color: var(--ui-on-brand, #fff);
    box-shadow: 0 0 0 1.5px var(--color-background);
    font-family: var(--font-ui);
    font-size: 9.5px;
    font-weight: 800;
    line-height: 14px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }

  /* The bundled themes paint the titlebar in their main color: a pop-colored badge. */
  :global(html[data-pomo-theme]) .badge {
    background: var(--pomo-pop);
    color: var(--pomo-ink);
    box-shadow: 0 0 0 1.5px var(--pomo-main);
  }

  /* Classic Tomato's titlebar is paper: a tomato badge. */
  :global(html[data-pomo-theme='classic-tomato']) .badge {
    background: var(--pomo-main);
    color: var(--pomo-light);
    box-shadow: 0 0 0 1.5px var(--pomo-base);
  }

  .badge.bump {
    animation: badge-bump 380ms cubic-bezier(0.3, 0.7, 0.4, 1.4);
  }

  @keyframes badge-bump {
    0% { transform: scale(1); }
    35% { transform: scale(1.4); }
    70% { transform: scale(0.92); }
    100% { transform: scale(1); }
  }

  /* A jot landing in Tasks: the list icon swallows it. */
  svg.gulp {
    animation: gulp 420ms cubic-bezier(0.3, 0.7, 0.4, 1);
  }

  @keyframes gulp {
    0% { transform: scale(1); }
    30% { transform: scale(1.28, 0.86); }
    60% { transform: scale(0.92, 1.08); }
    100% { transform: scale(1); }
  }

  @media (prefers-reduced-motion: reduce) {
    .badge.bump,
    svg.gulp {
      animation: none;
    }
  }

  .titlebar:not(.suppress-hover) .btn-icon.close:hover {
    color: var(--color-background);
    background: var(--color-focus-round);
  }
</style>
