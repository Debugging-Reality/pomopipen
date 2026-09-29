<script lang="ts">
  import { installClickSound } from '$lib/utils/clickSound';
  import '../../app.css';
  import '$lib/styles/ui.css';
  import '$lib/styles/classic-tomato.css';
  import { onMount } from 'svelte';
  import { getLocale } from '$paraglide/runtime.js';
  import { activeTheme } from '$lib/stores/theme';
  import { collectionFor } from '$lib/themes/collection';
  import ResizeHandles from '$lib/components/ResizeHandles.svelte';
  import ToastHost from '$lib/components/settings/ToastHost.svelte';
  import { getSettings, getThemes, onSettingsChanged, onThemesChanged } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import { applyTheme } from '$lib/stores/theme';
  import { resolveThemeName } from '$lib/utils/theme';
  import { setLocale } from '$lib/locale.svelte.js';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { info, error as logError } from '@tauri-apps/plugin-log';
  import { createLocalShortcutHandler } from '$lib/utils/localShortcuts';

  import SettingsTitlebar from '$lib/components/settings/SettingsTitlebar.svelte';
  import TimerSection from '$lib/components/settings/sections/TimerSection.svelte';
  import AppearanceSection from '$lib/components/settings/sections/AppearanceSection.svelte';
  import SubjectsSection from '$lib/components/settings/sections/SubjectsSection.svelte';
  import NotificationsSection from '$lib/components/settings/sections/NotificationsSection.svelte';
  import ShortcutsSection from '$lib/components/settings/sections/ShortcutsSection.svelte';
  import SystemSection from '$lib/components/settings/sections/SystemSection.svelte';
  import CalendarSyncSection from '$lib/components/settings/sections/CalendarSyncSection.svelte';
  import AboutSection from '$lib/components/settings/sections/AboutSection.svelte';

  import * as m from '$paraglide/messages.js';

  // A short sound on the red primary buttons (Settings → Notifications).
  onMount(installClickSound);

  type Section =
    | 'timer'
    | 'appearance'
    | 'subjects'
    | 'notifications'
    | 'shortcuts'
    | 'sync'
    | 'system'
    | 'about';

  // Line icons (24px grid, drawn with a 1.8px stroke) and a one-line page intro.
  const SECTIONS: { id: Section; label: () => string; icon: string; intro: [string, string] }[] = [
    { id: 'timer', label: m.nav_timer, icon: 'M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16ZM12 9v4l2.5 2M9.5 2.5h5',
      intro: ['专注与休息的时长，以及轮次如何自动衔接。', 'Round lengths and how rounds follow each other.'] },
    { id: 'appearance', label: m.nav_appearance, icon: 'M12 3a9 9 0 1 0 0 18c1 0 1.6-.8 1.6-1.6 0-.5-.2-.8-.5-1.2-.3-.3-.5-.7-.5-1.1 0-.9.7-1.6 1.6-1.6H16a5 5 0 0 0 5-5C21 6.3 17 3 12 3ZM7.5 11.5h.01M10 7.5h.01M14.5 7.5h.01',
      intro: ['主题配色、背景图和明暗模式。', 'Theme colors, background pictures and light/dark mode.'] },
    { id: 'subjects', label: m.nav_subjects, icon: 'M5 4.5A1.5 1.5 0 0 1 6.5 3H19v15H6.5A1.5 1.5 0 0 0 5 19.5v-15ZM5 19.5A1.5 1.5 0 0 0 6.5 21H19v-3M9 7h6',
      intro: ['学习时长按科目归类；删除科目不会删除记录。', 'Study time is grouped by subject; deleting one keeps its history.'] },
    { id: 'notifications', label: m.nav_notifications, icon: 'M6 16v-5a6 6 0 1 1 12 0v5l1.5 2h-15L6 16ZM10 20.5a2 2 0 0 0 4 0',
      intro: ['提醒音、桌面通知和音量。', 'Alert sounds, desktop notifications and volume.'] },
    { id: 'shortcuts', label: m.nav_shortcuts, icon: 'M5.5 6h13A2.5 2.5 0 0 1 21 8.5v7a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 15.5v-7A2.5 2.5 0 0 1 5.5 6ZM7 10h.01M10.5 10h.01M14 10h.01M17 10h.01M8 14h8',
      intro: ['点击任意按键即可重新设置，Esc 取消。', 'Click a key to change it; Esc cancels.'] },
    { id: 'sync', label: () => (getLocale().startsWith('zh') ? '日历同步' : 'Calendar sync'), icon: 'M4 6.5A1.5 1.5 0 0 1 5.5 5h13A1.5 1.5 0 0 1 20 6.5v12a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5v-12ZM4 10h16M8 3v4M16 3v4M9.5 15l2 2 3.5-4',
      intro: ['把专注记录同步到 Google 日历。', 'Send your focus sessions to Google Calendar.'] },
    { id: 'system', label: m.nav_system, icon: 'M4 7h9M17 7h3M4 17h3M11 17h9M15 5v4M9 15v4',
      intro: ['集成、语言、更新、托盘、窗口和数据。', 'Integrations, language, updates, tray, window and data.'] },
    { id: 'about', label: m.nav_about, icon: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM12 11v5M12 8h.01',
      intro: ['版本、更新和日志文件夹。', 'Version, updates and the log folder.'] },
  ];

  let active = $state<Section>('timer');
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let current = $derived(SECTIONS.find((s) => s.id === active)!);
  let themeInfo = $derived(collectionFor($activeTheme));

  // Local shortcut state for the settings window.
  let localVolume = $state(1.0);
  let preMuteVolume = $state(0.5);
  let isFullscreen = $state(false);

  onMount(() => {
    const cleanups: UnlistenFn[] = [];

    // Mount local keyboard shortcut handler.
    const shortcutHandler = createLocalShortcutHandler({
      getSettings: () => $settings,
      getVolume: () => localVolume,
      setVolume: (v) => {
        localVolume = v;
      },
      getPreMuteVolume: () => preMuteVolume,
      setPreMuteVolume: (v) => {
        preMuteVolume = v;
      },
      getFullscreen: () => isFullscreen,
      setFullscreen: (v) => {
        isFullscreen = v;
      },
    });
    document.addEventListener('keydown', shortcutHandler);
    cleanups.push(() => document.removeEventListener('keydown', shortcutHandler));

    (async () => {
      try {
        const s = await getSettings();
        settings.set(s);
        localVolume = s.volume;

        // Apply the stored locale on mount.
        setLocale(s.language);
        await info(`[settings] settings loaded, locale=${s.language}`);

        const themes = await getThemes();
        const osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const activeTheme = themes.find((t) => t.name === resolveThemeName(s, osDark)) ?? themes[0];
        if (activeTheme) applyTheme(activeTheme);
        await info(`[settings] initialized, theme=${activeTheme?.name ?? 'none'}`);

        // Show the window now that the theme is applied (avoids white flash)
        await getCurrentWebviewWindow().show();
      } catch (e) {
        await logError(`[settings] initialization failed: ${e}`);
        throw e;
      }

      // Live OS color scheme changes — re-resolve only in auto mode.
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const mqListener = async (e: MediaQueryListEvent) => {
        if ($settings.theme_mode !== 'auto') return;
        const allThemes = await getThemes();
        const t = allThemes.find((th) => th.name === resolveThemeName($settings, e.matches));
        if (t) applyTheme(t);
      };
      mq.addEventListener('change', mqListener);
      cleanups.push(() => mq.removeEventListener('change', mqListener));

      cleanups.push(
        await onSettingsChanged(async (updated) => {
          const prevMode = $settings.theme_mode;
          const prevLight = $settings.theme_light;
          const prevDark = $settings.theme_dark;
          const prevLanguage = $settings.language;
          settings.set(updated);
          localVolume = updated.volume;
          if (updated.language !== prevLanguage) {
            setLocale(updated.language);
          }
          if (
            updated.theme_mode !== prevMode ||
            updated.theme_light !== prevLight ||
            updated.theme_dark !== prevDark
          ) {
            const allThemes = await getThemes();
            const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            const t = allThemes.find((th) => th.name === resolveThemeName(updated, dark));
            if (t) applyTheme(t);
          }
        }),
        await onThemesChanged((updated) => {
          const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
          const current =
            updated.find((t) => t.name === resolveThemeName($settings, dark)) ?? updated[0];
          if (current) applyTheme(current);
        })
      );
    })();

    return () => {
      for (const fn of cleanups) fn();
    };
  });
</script>

<ResizeHandles />

<div class="window pomo-ui">
  <SettingsTitlebar />

  <div class="body">
    <!-- Left sidebar navigation -->
    <aside class="sidebar">
      <nav aria-label={m.settings_title()}>
        {#each SECTIONS as section}
          <button
            class="nav-item"
            class:active={active === section.id}
            aria-current={active === section.id ? 'page' : undefined}
            onclick={() => {
              active = section.id;
            }}
          >
            <svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true"><path d={section.icon} /></svg>
            <span>{section.label()}</span>
          </button>
        {/each}
      </nav>
      {#if themeInfo}
        <div class="theme-tag">
          <span class="dots" aria-hidden="true">{#each themeInfo.colors as color}<i style:background={color}></i>{/each}</span>
          <span>{zh ? themeInfo.nameZh : themeInfo.name}</span>
        </div>
      {/if}
    </aside>

    <!-- Right content area -->
    <main class="content">
      <div class="s-page">
      <header class="s-page-head">
        <h2>{current.label()}</h2>
        <p>{zh ? current.intro[0] : current.intro[1]}</p>
      </header>
      {#if active === 'timer'}
        <TimerSection />
      {:else if active === 'appearance'}
        <AppearanceSection />
      {:else if active === 'subjects'}
        <SubjectsSection />
      {:else if active === 'notifications'}
        <NotificationsSection />
      {:else if active === 'shortcuts'}
        <ShortcutsSection />
      {:else if active === 'sync'}
        <CalendarSyncSection />
      {:else if active === 'system'}
        <SystemSection />
      {:else if active === 'about'}
        <AboutSection />
      {/if}
      </div>
    </main>
  </div>
  <ToastHost />
</div>

<style>
  .window {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: app-fade-in 0.2s ease both;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  /* Sidebar */
  .sidebar {
    width: 196px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 16px;
    border-right: 1px solid var(--ui-border);
    background: var(--ui-sidebar);
    overflow-y: auto;
    padding: 14px 10px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 38px;
    padding: 0 12px;
    background: none;
    border: none;
    border-radius: 10px;
    text-align: left;
    font: inherit;
    font-size: 14px;
    font-weight: 500;
    color: var(--ui-text-muted);
    cursor: pointer;
    transition:
      color 150ms ease,
      background 150ms ease;
  }

  .nav-icon {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-item:hover {
    color: var(--ui-text);
    background: var(--ui-hover);
  }

  .nav-item:active {
    background: var(--ui-selected);
  }

  .nav-item.active {
    color: var(--ui-brand-strong);
    background: var(--ui-selected);
    font-weight: 700;
  }

  /* Small indicator bar: selection is never shown by color alone. */
  .nav-item.active::before {
    content: '';
    position: absolute;
    left: 3px;
    top: 11px;
    bottom: 11px;
    width: 3px;
    border-radius: 2px;
    background: var(--ui-brand);
  }

  .theme-tag {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    font-size: 12px;
    color: var(--ui-text-muted);
  }

  .dots {
    display: flex;
  }

  .dots i {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px solid var(--ui-sidebar);
    margin-left: -3px;
  }

  .dots i:first-child {
    margin-left: 0;
  }

  /* Content */
  .content {
    flex: 1;
    overflow-y: auto;
    min-width: 0;
    background: var(--ui-page);
    scrollbar-width: thin;
    scrollbar-color: var(--ui-border-strong) transparent;
  }
</style>
