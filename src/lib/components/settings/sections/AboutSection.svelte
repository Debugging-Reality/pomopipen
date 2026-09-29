<script lang="ts">
  import { onMount } from 'svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';

  import { info, warn, error as logError } from '@tauri-apps/plugin-log';
  import { openLogDir, appVersion, checkUpdate, installUpdate } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import type { UpdateInfo } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import { convertFileSrc } from '@tauri-apps/api/core';

  const BASE_VERSION = '1.0.0';
  const REPO = 'https://github.com/Splode/pomotroid';

  let version = $state('...');

  type UpdateState = 'idle' | 'checking' | 'up-to-date' | 'available' | 'installing' | 'error';
  let updateState = $state<UpdateState>('idle');
  let availableUpdate = $state<UpdateInfo | null>(null);
  let updateError = $state('');

  // Strip pre-release and build metadata to get the bare X.Y.Z for the release tag URL.
  function baseOnly(v: string): string {
    return v.split('-')[0].split('+')[0];
  }

  let releaseUrl = $derived(
    `${REPO}/releases/tag/v${baseOnly(version === '...' ? BASE_VERSION : version)}`
  );

  onMount(async () => {
    try {
      version = await appVersion();
    } catch {
      version = BASE_VERSION;
    }

    if ($settings.check_for_updates) {
      updateState = 'checking';
      await info('[about] checking for updates');
      try {
        const update = await checkUpdate();
        if (update) {
          availableUpdate = update;
          updateState = 'available';
          await info(`[about] update available: v${update.version}`);
        } else {
          updateState = 'up-to-date';
          await info('[about] already up to date');
        }
      } catch (e) {
        updateError = String(e);
        updateState = 'error';
        await warn(`[about] update check failed: ${e}`);
      }
    }
  });

  async function handleInstall() {
    updateState = 'installing';
    await info(`[about] installing update v${availableUpdate?.version}`);
    try {
      await installUpdate();
    } catch (e) {
      updateError = String(e);
      updateState = 'error';
      await logError(`[about] update install failed: ${e}`);
    }
  }
</script>

<div class="s-groups">
  <div class="s-card hero">
    <img class="logo" src={$settings.app_icon ? convertFileSrc($settings.app_icon) : '/app-icon.png'} alt="PomoPipen logo" />
    <div>
      <h2 class="name">PomoPipen</h2>
      <p class="version">Version {version}</p>
    </div>
  </div>

  <section class="s-group">
    <h3 class="s-group-title">{m.nav_about()}</h3>
    <div class="s-card">
      <button class="s-row link" onclick={() => openUrl(releaseUrl)}>
        <span class="s-label">{m.about_release_notes()}</span>
        <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 9L9 3M9 3H4.5M9 3v4.5" /></svg>
      </button>
      <button class="s-row link" onclick={() => openUrl(REPO)}>
        <span class="s-label">{m.about_source_code()}</span>
        <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true"><path d="M3 9L9 3M9 3H4.5M9 3v4.5" /></svg>
      </button>
      <button class="s-row link" onclick={openLogDir}>
        <span class="s-label">{m.about_open_log_folder()}</span>
        <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true"><path d="M1.5 3.5C1.5 2.9 2 2.5 2.5 2.5H5l1 1.2h3.5c.6 0 1 .4 1 1V9c0 .6-.4 1-1 1h-7c-.6 0-1-.4-1-1V3.5Z" /></svg>
      </button>
    </div>
  </section>

  {#if $settings.check_for_updates || updateState !== 'idle'}
    <section class="s-group">
      <div class="s-card">
        {#if updateState === 'available' && availableUpdate}
          <div class="s-row">
            <span class="s-label">{getLocale().startsWith('zh') ? '发现新版本' : 'Update available'} v{availableUpdate.version}</span>
            <button class="s-btn s-btn--primary" onclick={handleInstall}>{m.about_update_install({ version: availableUpdate.version })}</button>
          </div>
        {:else}
          <div class="s-row">
            <span class="s-desc status" class:checking={updateState === 'idle' || updateState === 'checking' || updateState === 'installing'}>
              {#if updateState === 'idle' || updateState === 'checking'}
                {m.about_update_checking()}
              {:else if updateState === 'up-to-date'}
                {m.about_update_up_to_date()}
              {:else if updateState === 'installing'}
                Installing…
              {:else}
                {m.about_update_error()}
              {/if}
            </span>
          </div>
        {/if}
      </div>
    </section>
  {/if}

  <p class="s-note">
    Built with Tauri, Svelte, and Rust.<br />
    MIT License — Copyright &copy; 2017–2026 Christopher Murphy. PomoPipen is a personal fork of Pomotroid.
  </p>
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 18px 20px;
  }

  .logo {
    width: 60px;
    height: 60px;
    border-radius: 14px;
    object-fit: cover;
    box-shadow: 0 2px 8px color-mix(in srgb, var(--ui-text) 18%, transparent);
  }

  .name {
    font-size: 20px;
    font-weight: 750;
    color: var(--ui-brand-strong);
    margin-bottom: 3px;
  }

  .version {
    font-size: 13px;
    color: var(--ui-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .link svg {
    flex-shrink: 0;
    fill: none;
    stroke: var(--ui-text-muted);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: stroke 150ms ease, transform 150ms ease;
  }

  .link:hover svg {
    stroke: var(--ui-brand);
    transform: translate(1px, -1px);
  }

  .status.checking {
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
</style>