<script lang="ts">
  import { installClickSound } from '$lib/utils/clickSound';
  import '../../app.css';
  import '$lib/styles/ui.css';
  import '$lib/styles/classic-tomato.css';
  import { onMount } from 'svelte';
  import {
    getSettings,
    getThemes,
    onSettingsChanged,
    onThemesChanged,
    onRoundChange,
    onSessionsCleared,
    onSessionsChanged,
    setSetting,
    statsGetDetailed,
    statsGetHeatmap,
    statsGetSubjectBreakdown,
  } from '$lib/ipc';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { getLocale } from '$paraglide/runtime.js';
  import ResizeHandles from '$lib/components/ResizeHandles.svelte';
  import StudyTrendChart from '$lib/components/stats/StudyTrendChart.svelte';
  import SubjectPieChart from '$lib/components/stats/SubjectPieChart.svelte';
  import { settings } from '$lib/stores/settings';
  import { subjects, watchSubjects } from '$lib/stores/subjects';
  import { applyTheme } from '$lib/stores/theme';
  import { setLocale } from '$lib/locale.svelte.js';
  import { resolveThemeName } from '$lib/utils/theme';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import type { DetailedStats, HeatmapStats, SubjectFilter, SubjectTotal } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { info, error as logError } from '@tauri-apps/plugin-log';

  import SettingsTitlebar from '$lib/components/settings/SettingsTitlebar.svelte';
  import OverviewView from '$lib/components/stats/OverviewView.svelte';
  import YearlyView from '$lib/components/stats/YearlyView.svelte';
  import WeekCalendar from '$lib/components/stats/WeekCalendar.svelte';
  import ToastHost from '$lib/components/settings/ToastHost.svelte';

  // A short sound on the red primary buttons (Settings → Notifications).
  onMount(installClickSound);

  type Tab = 'overview' | 'calendar' | 'charts';

  let activeTab = $state<Tab>('overview');
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });

  // Whole-window zoom (native webview zoom, so tooltips and layout stay correct).
  const ZOOM_MIN = 50;
  const ZOOM_MAX = 200;
  let zoom = $state(100);

  async function setZoom(value: number, persist = true) {
    zoom = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, Math.round(value / 10) * 10));
    try {
      await getCurrentWebview().setZoom(zoom / 100);
      if (persist) settings.set(await setSetting('stats_zoom', String(zoom)));
    } catch (e) {
      await logError(`[stats] zoom failed: ${e}`);
    }
  }

  function onZoomKey(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    if (e.key === '=' || e.key === '+') void setZoom(zoom + 10);
    else if (e.key === '-') void setZoom(zoom - 10);
    else if (e.key === '0') void setZoom(100);
    else return;
    e.preventDefault();
  }

  function onZoomWheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    void setZoom(zoom + (e.deltaY < 0 ? 10 : -10));
  }
  let detailed = $state<DetailedStats | null>(null);
  let heatmap = $state<HeatmapStats | null>(null);
  let heatmapLoaded = $state(false);
  // Breakdown is always "all subjects, all time" — independent of subjectFilter,
  // which narrows the tabs above it. Loaded once, alongside the heatmap.
  let breakdown = $state<SubjectTotal[] | null>(null);

  // Filters Today/This Week/heatmap to one subject, Uncategorized, or everything.
  let subjectFilter = $state<SubjectFilter>('all');

  function filterEquals(a: SubjectFilter, b: SubjectFilter): boolean {
    if (typeof a === 'string' || typeof b === 'string') return a === b;
    return a.subject === b.subject;
  }

  async function switchTab(tab: Tab) {
    activeTab = tab;
    if (tab === 'charts' && !heatmapLoaded) {
      try {
        heatmap = await statsGetHeatmap(subjectFilter);
        breakdown = await statsGetSubjectBreakdown();
        heatmapLoaded = true;
      } catch (e) {
        await logError(`[stats] failed to load heatmap: ${e}`);
      }
    }
  }

  async function selectSubjectFilter(filter: SubjectFilter) {
    subjectFilter = filter;
    try {
      detailed = await statsGetDetailed(subjectFilter);
      if (heatmapLoaded) heatmap = await statsGetHeatmap(subjectFilter);
    } catch (e) {
      await logError(`[stats] failed to refresh stats for subject filter: ${e}`);
    }
  }

  const TABS: { id: Tab; label: () => string }[] = [
    { id: 'overview', label: () => (zh ? '概览' : 'Overview') },
    { id: 'calendar', label: () => (zh ? '周历' : 'Calendar') },
    { id: 'charts', label: () => (zh ? '图表' : 'Charts') },
  ];

  onMount(() => {
    const cleanups: UnlistenFn[] = [];
    window.addEventListener('keydown', onZoomKey);
    window.addEventListener('wheel', onZoomWheel, { passive: false });
    cleanups.push(() => {
      window.removeEventListener('keydown', onZoomKey);
      window.removeEventListener('wheel', onZoomWheel);
    });

    (async () => {
      try {
        const s = await getSettings();
        settings.set(s);
        setLocale(s.language);
        if (s.stats_zoom !== 100) await setZoom(s.stats_zoom, false);
        await info(`[stats] settings loaded, locale=${s.language}`);

        const themes = await getThemes();
        const osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const activeTheme = themes.find((t) => t.name === resolveThemeName(s, osDark)) ?? themes[0];
        if (activeTheme) applyTheme(activeTheme);

        // Show window immediately after theme is applied
        await getCurrentWebviewWindow().show();

        detailed = await statsGetDetailed(subjectFilter);
        cleanups.push(await watchSubjects());
        await info(`[stats] initialized, theme=${activeTheme?.name ?? 'none'}`);
      } catch (e) {
        await logError(`[stats] initialization failed: ${e}`);
        throw e;
      }

      cleanups.push(
        await onRoundChange(async () => {
          try {
            detailed = await statsGetDetailed(subjectFilter);
            if (heatmapLoaded) {
              heatmap = await statsGetHeatmap(subjectFilter);
              breakdown = await statsGetSubjectBreakdown();
            }
          } catch (e) {
            await logError(`[stats] failed to refresh stats after round change: ${e}`);
          }
        }),
        await onSessionsCleared(async () => {
          try {
            detailed = await statsGetDetailed(subjectFilter);
            if (heatmapLoaded) {
              heatmap = await statsGetHeatmap(subjectFilter);
              breakdown = await statsGetSubjectBreakdown();
            }
          } catch (e) {
            await logError(`[stats] failed to refresh stats after session clear: ${e}`);
          }
        }),
        await onSessionsChanged(async () => {
          try {
            detailed = await statsGetDetailed(subjectFilter);
            if (heatmapLoaded) {
              heatmap = await statsGetHeatmap(subjectFilter);
              breakdown = await statsGetSubjectBreakdown();
            }
          } catch (e) {
            await logError(`[stats] failed to refresh stats after a calendar edit: ${e}`);
          }
        }),
        await onSettingsChanged(async (updated) => {
          const prev = {
            mode: $settings.theme_mode,
            light: $settings.theme_light,
            dark: $settings.theme_dark,
            language: $settings.language,
          };
          settings.set(updated);
          if (updated.language !== prev.language) {
            setLocale(updated.language);
          }
          if (
            updated.theme_mode !== prev.mode ||
            updated.theme_light !== prev.light ||
            updated.theme_dark !== prev.dark
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
  <SettingsTitlebar title={m.stats_title()} maximizable>
    {#snippet left()}
      <div class="zoom" role="group" aria-label={zh ? '缩放' : 'Zoom'}>
        <button onclick={() => setZoom(zoom - 10)} disabled={zoom <= ZOOM_MIN} aria-label={zh ? '缩小' : 'Zoom out'} title="Ctrl −">−</button>
        <button class="zoom-value" onclick={() => setZoom(100)} title={zh ? '恢复 100%（Ctrl 0）' : 'Reset to 100% (Ctrl 0)'}>{zoom}%</button>
        <button onclick={() => setZoom(zoom + 10)} disabled={zoom >= ZOOM_MAX} aria-label={zh ? '放大' : 'Zoom in'} title="Ctrl +">+</button>
      </div>
    {/snippet}
  </SettingsTitlebar>

  <div class="toolbar">
    <div class="s-segment tabs" role="tablist">
      {#each TABS as tab (tab.id)}
        <button role="tab" aria-selected={activeTab === tab.id} class:on={activeTab === tab.id} onclick={() => switchTab(tab.id)}>
          {tab.label()}
        </button>
      {/each}
    </div>

    <!-- Subject filter: narrows the overview and charts. The calendar always
         shows every subject, told apart by color. -->
    {#if $subjects.length > 0 && activeTab !== 'calendar'}
      <div class="filters" role="radiogroup" aria-label={zh ? '按科目筛选' : 'Filter by subject'}>
        <button class="chip" role="radio" aria-checked={filterEquals(subjectFilter, 'all')}
          class:on={filterEquals(subjectFilter, 'all')} onclick={() => selectSubjectFilter('all')}>
          {m.subject_filter_all()}
        </button>
        {#each $subjects as subject (subject.id)}
          <button class="chip" role="radio" aria-checked={filterEquals(subjectFilter, { subject: subject.id })}
            class:on={filterEquals(subjectFilter, { subject: subject.id })}
            onclick={() => selectSubjectFilter({ subject: subject.id })}>
            <span class="dot" style:background={subject.color}></span>{subject.name}
          </button>
        {/each}
        <button class="chip" role="radio" aria-checked={filterEquals(subjectFilter, 'uncategorized')}
          class:on={filterEquals(subjectFilter, 'uncategorized')} onclick={() => selectSubjectFilter('uncategorized')}>
          <span class="dot none"></span>{m.subject_filter_uncategorized()}
        </button>
      </div>
    {/if}
  </div>

  <div class="content" class:calendar-tab={activeTab === 'calendar'}>
    {#if activeTab === 'overview'}
      <div class="page">
        <OverviewView today={detailed?.today ?? null} week={detailed?.week ?? null} streak={detailed?.streak ?? null} />
      </div>
    {:else if activeTab === 'calendar'}
      <WeekCalendar subjectFilter="all" />
    {:else}
      <div class="page">
        <YearlyView {heatmap} {breakdown} />
        <StudyTrendChart {subjectFilter} />
        <SubjectPieChart />
      </div>
    {/if}
  </div>
  <ToastHost />
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    animation: app-fade-in 0.18s ease;
    overflow: hidden;
    cursor: default;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .zoom button {
    height: 28px;
    min-width: 28px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--ui-on-brand);
    font: inherit;
    font-size: 15px;
    font-weight: 700;
    line-height: 1;
    cursor: pointer;
    transition: background 150ms ease;
  }

  .zoom .zoom-value {
    min-width: 46px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .zoom button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--ui-on-brand) 18%, transparent);
  }

  .zoom button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
    padding: 12px 24px;
    border-bottom: 1px solid var(--ui-border);
    background: var(--ui-sidebar);
    flex-shrink: 0;
  }

  .tabs button {
    min-width: 64px;
  }

  .filters {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: none;
    min-width: 0;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 11px;
    flex-shrink: 0;
    border-radius: 999px;
    border: 1px solid var(--ui-border-strong);
    background: var(--ui-surface);
    color: var(--ui-text-muted);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background 150ms ease,
      color 150ms ease,
      border-color 150ms ease;
  }

  .chip:hover {
    color: var(--ui-text);
    border-color: color-mix(in srgb, var(--ui-brand) 45%, var(--ui-border-strong));
  }

  .chip.on {
    background: var(--ui-selected);
    border-color: var(--ui-brand);
    color: var(--ui-brand-strong);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .dot.none {
    border: 1.5px dashed var(--ui-text-muted);
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    background: var(--ui-page);
    scrollbar-width: thin;
    scrollbar-color: var(--ui-border-strong) transparent;
  }

  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
    margin: 0 auto;
    padding: 20px 24px 28px;
  }

  /* The calendar gets the whole tab: its grid stretches to the window height
     and scrolls inside; very small windows scroll the tab instead. */
  .calendar-tab {
    display: flex;
    flex-direction: column;
  }

  .calendar-tab > :global(.week-calendar) {
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
  }

  .calendar-tab :global(.calendar-surface) {
    flex: 1 1 0;
    min-height: 240px;
    display: flex;
    flex-direction: column;
  }

  .calendar-tab :global(.calendar-scroll) {
    flex: 1 1 0;
    max-height: none;
  }
</style>
