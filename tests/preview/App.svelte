<script lang="ts">
  // Visual review harness — mock IPC, never touches real data.
  //   ?view=timers|settings|charts|calendar|subjects   show one section (default: all)
  //   ?view=stats[&tab=charts|calendar]|tasks          the real stats / tasks window
  //   ?view=sync&gcal=none|ready|connected|error      Settings → Calendar sync
  //   ?view=jots[&pad=closed]                          碎碎念: focus (stack) and break (list) side by side
  //   ?theme=<id>  ?state=idle|running|paused  ?appearance=mechanical|ring
  //   ?bg=<url>                               background picture (default: generated pattern)
  //   ?fit=tile|cover  ?opacity=35  ?scale=100  how it is drawn
  import '../../src/app.css';
  import '../../src/lib/styles/ui.css';
  import '../../src/lib/styles/classic-tomato.css';
  import { onMount } from 'svelte';
  import { settings } from '../../src/lib/stores/settings';
  import { applyTheme, activeTheme } from '../../src/lib/stores/theme';
  import { collection } from '../../src/lib/themes/collection';
  import { setLocale as setParaglideLocale } from '../../src/paraglide/runtime';
  // The app replaces paraglide's getLocale with its own reactive locale; set both.
  import { setLocale as setAppLocale } from '../../src/lib/locale.svelte';
  const setLocale = (l: 'zh' | 'en', options: { reload: false }) => { setParaglideLocale(l, options); setAppLocale(l); };
  import PlayfulTimer from '../../src/lib/components/PlayfulTimer.svelte';
  import ClassicTimer from '../../src/lib/components/classic/ClassicTimer.svelte';
  import TimerAppearance from '../../src/lib/components/settings/TimerAppearance.svelte';
  import PageBackdrop from '../../src/lib/components/settings/PageBackdrop.svelte';
  import SubjectsSection from '../../src/lib/components/settings/sections/SubjectsSection.svelte';
  import CalendarSyncSection from '../../src/lib/components/settings/sections/CalendarSyncSection.svelte';
  import ToastHost from '../../src/lib/components/settings/ToastHost.svelte';
  import ThemeBackground from '../../src/lib/components/ThemeBackground.svelte';
  import ThemeCollection from '../../src/lib/components/settings/ThemeCollection.svelte';
  import WeekCalendar from '../../src/lib/components/stats/WeekCalendar.svelte';
  import StudyTrendChart from '../../src/lib/components/stats/StudyTrendChart.svelte';
  import SubjectPieChart from '../../src/lib/components/stats/SubjectPieChart.svelte';
  import StatsPage from '../../src/routes/stats/+page.svelte';
  import TasksPage from '../../src/routes/tasks/+page.svelte';
  import { timerLayout } from '../../src/lib/utils/timerLayout';
  import { snapshot, advance, subjectPalette, setJotInFocus } from './mock-ipc';
  import Titlebar from '../../src/lib/components/Titlebar.svelte';
  import JotPad from '../../src/lib/components/jots/JotPad.svelte';
  import { openJotPad } from '../../src/lib/stores/jots';
  import type { Theme } from '../../src/lib/types';

  const params = new URLSearchParams(location.search);
  const view = params.get('view') ?? 'all';
  // ?chrome=none: just the component (for product images); ?lang=en|zh
  const bare = params.get('chrome') === 'none';
  const lang = params.get('lang') === 'en' ? 'en' : 'zh';
  const show = (name: string) => view === 'all' || view === name;
  const PATTERN = `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240"><rect width="240" height="240" fill="#f5b8c8"/>` +
    `<g fill="#b80e46"><circle cx="52" cy="70" r="16"/><circle cx="84" cy="74" r="16"/><circle cx="172" cy="190" r="16"/><circle cx="204" cy="194" r="16"/></g>` +
    `<g stroke="#45684a" stroke-width="4" fill="none"><path d="M52 54 Q64 20 70 18 M84 58 Q76 28 70 18"/><path d="M172 174 Q184 140 190 138 M204 178 Q196 148 190 138"/></g></svg>`)}`;
  const background = { path: params.get('bg') ?? PATTERN, opacity: Number(params.get('opacity') ?? 35),
    fit: params.get('fit') === 'cover' ? 'cover' : 'tile', scale: Number(params.get('scale') ?? 100) };
  // ?win=1920x1000 previews one window size instead of the default set.
  const windows: [number, number][] = params.get('win')
    ? [params.get('win')!.split('x').map(Number) as [number, number]]
    : [[240, 240], [300, 330], [360, 478], [1120, 620]];

  let themes = $state<Theme[]>([]);
  const initial = params.get('state');
  let snapshotState = $state({ ...snapshot, elapsed_secs: initial === 'idle' ? 0 : initial === 'paused' ? 750 : 330,
    is_running: !initial || initial === 'running', is_paused: initial === 'paused' });
  let classic = $derived($activeTheme?.name === 'Classic Tomato');
  function setRun(kind: 'idle' | 'running' | 'paused') {
    snapshotState = { ...snapshotState, elapsed_secs: kind === 'idle' ? 0 : Math.min(750, snapshotState.total_secs - 30), is_running: kind === 'running', is_paused: kind === 'paused' };
  }
  // A natural completion: the last running second, then the engine's next (idle) round.
  function complete() {
    snapshotState = { ...snapshotState, is_running: true, is_paused: false, elapsed_secs: snapshotState.total_secs - 1 };
    setTimeout(() => { snapshotState = advance(); }, 120);
  }
  function toggleAppearance() {
    settings.update(s => ({ ...s, timer_appearance: s.timer_appearance === 'ring' ? 'mechanical' : 'ring' }));
  }
  let zh = $state(lang === 'zh');
  // Before any component renders, so first paints use the right language.
  setLocale(lang, { reload: false });
  // 碎碎念: the second window is on a short break, so its pad shows the full list.
  let breakState = $state({ ...snapshot, round_type: 'short-break' as const, previous_round_type: 'work', total_secs: 300, elapsed_secs: 72, is_running: true, is_paused: false });
  $effect(() => setJotInFocus(snapshotState.round_type === 'work' && (snapshotState.is_running || snapshotState.is_paused)));
  if (view === 'jots' && params.get('pad') !== 'closed') openJotPad();

  onMount(() => {
    setLocale(lang, { reload: false });
    const tab = ['overview', 'calendar', 'charts'].indexOf(params.get('tab') ?? '');
    if (view === 'stats' && tab > 0) setTimeout(() => document.querySelectorAll<HTMLElement>('[role=tab]')[tab]?.click(), 150);
    void Promise.all(collection.map(async t => ({ ...await (await fetch(`/themes/${t.id}.json`)).json(), is_custom: false }))).then(values => {
      themes = values;
      const all = Object.fromEntries(values.map(t => [t.name, { timer: background, calendar: { ...background, opacity: 45 } }]));
      const appearance = params.get('appearance') === 'ring' ? 'ring' : 'mechanical';
      settings.update(s => ({ ...s, theme_mode: 'light', theme_light: values[0].name, language: lang, active_subject_id: 1, timer_appearance: appearance, theme_backgrounds: params.get('bg') === 'none' ? '{}' : JSON.stringify(all) }));
      use(values.find(t => collection.find(c => c.name === t.name)?.id === params.get('theme')) ?? values[0]);
    });
  });
  function use(theme: Theme) {
    applyTheme(theme);
    settings.update(s => ({ ...s, theme_light: theme.name }));
    const item = collection.find(c => c.name === theme.name);
    // Classic Tomato: each subject's own variety. Other themes: six colors spread over their palette.
    const sample = item && item.id !== 'classic-tomato' ? [0, 1, 2, 3, 4, 5].map(i => item.subjectColors[(i * 3) % item.subjectColors.length]) : [];
    subjectPalette.splice(0, subjectPalette.length, ...sample);
  }
  function changeLocale() { zh = !zh; setLocale(zh ? 'zh' : 'en', { reload: false }); }
</script>

{#if view === 'stats'}
  <StatsPage />
{:else if view === 'tasks'}
  <TasksPage />
{:else}
<div class="preview" class:bare>
  {#if !bare}
  <div class="preview-nav">
    <span>PomoPipen / visual review · 示例数据，不写入学习记录</span>
    <span class="themes">{#each themes as theme}<button class:on={$activeTheme?.name === theme.name} onclick={() => use(theme)}>{theme.name}</button>{/each}</span>
    <button onclick={() => { snapshotState = advance(); }}>下一轮 →</button>
    <button onclick={() => setRun('idle')}>待开始</button><button onclick={() => setRun('running')}>运行</button><button onclick={() => setRun('paused')}>暂停</button><button onclick={complete}>完成 →</button>
    {#if classic}<button onclick={toggleAppearance}>A / B 外观</button>{/if}
    <button onclick={changeLocale}>中 / EN</button>
  </div>
  {/if}

  {#if show('timers')}
    <section class="windows">
      {#each windows as [w, h]}
        {@const layout = timerLayout(w, h)}
        <figure>
          <div class="timer-window app" style="width:{w}px;height:{h}px">
            <ThemeBackground />
            <div class="titlebar">
              <span class="ib btn-icon"><svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"><rect x="2" y="2.5" width="3" height="3" rx=".5"/><path d="M7 4h7M2 9h3M7 9h7M2 14h3M7 14h7"/></svg></span>
              <span class="ib btn-icon"><svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"><path d="M2 4h12M2 8h12M2 12h12"/><circle cx="5" cy="4" r="1.8" fill="var(--color-background)"/><circle cx="11" cy="8" r="1.8" fill="var(--color-background)"/><circle cx="7" cy="12" r="1.8" fill="var(--color-background)"/></svg></span>
              <span class="ib btn-icon"><svg viewBox="0 0 16 16"><rect x="2" y="9" width="3" height="5" rx=".5" fill="currentColor" opacity=".6"/><rect x="6.5" y="5" width="3" height="9" rx=".5" fill="currentColor" opacity=".8"/><rect x="11" y="2" width="3" height="12" rx=".5" fill="currentColor"/></svg></span>
              <span class="sp"></span>
              <span class="ib btn-icon"><svg viewBox="0 0 12 12"><path d="M1.5 6h9" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg></span>
              <span class="ib btn-icon"><svg viewBox="0 0 12 12"><rect x="1.5" y="1.5" width="9" height="9" rx="1" fill="none" stroke="currentColor" stroke-width="1.4"/></svg></span>
              <span class="ib btn-icon"><svg viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg></span>
            </div>
            <div class="timer-content">{#if classic}<ClassicTimer state={snapshotState} uiScale={layout.uiScale} compact={layout.compact} viewWidth={w} viewHeight={h} />{:else}<PlayfulTimer state={snapshotState} uiScale={layout.uiScale} compact={layout.compact} />{/if}</div>
          </div>
          <figcaption class:hidden={bare}>{w} × {h} · {layout.compact ? 'compact' : 'full'} · ×{layout.uiScale.toFixed(2)}</figcaption>
        </figure>
      {/each}
    </section>
  {/if}

  {#if view === 'jots'}
    <section class="windows">
      {#each [snapshotState, breakState] as snap, i (i)}
        {@const [w, h] = windows.length === 1 ? windows[0] : [360, 478]}
        {@const layout = timerLayout(w, h)}
        <figure>
          <div class="timer-window app" style="width:{w}px;height:{h}px">
            <ThemeBackground />
            <Titlebar />
            <div class="timer-content">{#if classic}<ClassicTimer state={snap} uiScale={layout.uiScale} compact={layout.compact} viewWidth={w} viewHeight={h} />{:else}<PlayfulTimer state={snap} uiScale={layout.uiScale} compact={layout.compact} />{/if}</div>
            <JotPad {snap} />
          </div>
          <figcaption>{i === 0 ? (zh ? '专注中：列表收成一叠' : 'Focus: the list folds into a stack') : (zh ? '休息：逐条处理' : 'Break: handle them one by one')}</figcaption>
        </figure>
      {/each}
    </section>
  {/if}

  {#if show('settings')}
    <section class="appearance-panel window pomo-ui"><ThemeCollection {themes} osDark={false} />{#if classic}<div style="margin-top:18px"><TimerAppearance /></div><div style="margin-top:18px"><PageBackdrop /></div>{/if}</section>
  {/if}

  {#if show('charts')}
    <section class="charts"><StudyTrendChart /><SubjectPieChart /></section>
  {/if}

  {#if show('calendar')}
    <section class="calendar-panel window pomo-ui"><WeekCalendar /></section>
  {/if}

  {#if show('subjects')}
    <section class="subjects-panel window pomo-ui"><div class="content"><div class="s-page"><SubjectsSection /></div></div></section>
  {/if}

  {#if show('sync')}
    <section class="subjects-panel window pomo-ui"><div class="content"><div class="s-page"><CalendarSyncSection /></div></div></section>
  {/if}
  <ToastHost />
</div>
{/if}

<style>
  :global(html), :global(body) { height: auto; overflow: auto; }
  .preview { padding: 16px 20px 45px; }
  .preview.bare { padding: 0; }
  .bare .windows { gap: 0; }
  .bare .timer-window { border-radius: 0; border: 0; }
  figcaption.hidden { display: none; }
  .titlebar { position: relative; z-index: 1; height: 40px; flex-shrink: 0; display: flex; align-items: center; gap: 4px; padding: 0 8px; }
  .titlebar .sp { flex: 1; }
  .titlebar .ib { width: 28px; height: 28px; display: grid; place-items: center; color: var(--color-foreground-darker); }
  .titlebar .ib svg { width: 15px; height: 15px; }
  .titlebar .ib:nth-last-child(-n+3) svg { width: 12px; height: 12px; }
  .bare .calendar-panel { margin: 0; max-width: none; height: 100vh; display: flex; flex-direction: column; border: 0; border-radius: 0; }
  .bare .calendar-panel :global(.week-calendar) { flex: 1; min-height: 0; }
  .bare .calendar-panel :global(.calendar-surface) { flex: 1 1 0; min-height: 0; display: flex; flex-direction: column; }
  .bare .calendar-panel :global(.calendar-scroll) { flex: 1 1 0; max-height: none; }
  .bare .appearance-panel { margin: 0; max-width: none; border: 0; border-radius: 0; }
  .preview-nav { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; padding-bottom: 18px; font-size: 11px; color: var(--color-foreground-darker); }
  .themes { display: flex; gap: 6px; }
  button { background: none; border: 1px solid var(--color-separator); border-radius: 5px; padding: 5px 9px; color: var(--color-foreground); cursor: pointer; font-size: 11px; }
  button.on { background: var(--pomo-main); color: var(--pomo-on); }
  .windows { display: flex; flex-wrap: wrap; gap: 22px; align-items: flex-start; }
  figure { margin: 0; }
  figcaption { font-size: 10px; margin-top: 6px; color: var(--color-foreground-darker); }
  .timer-window { position: relative; isolation: isolate; overflow: hidden; display: flex; flex-direction: column; background: var(--color-background); border: 1px solid var(--color-separator); border-radius: 8px; }
  .timer-content { position: relative; z-index: 1; flex: 1; display: flex; align-items: center; justify-content: center; overflow: hidden; }
  .appearance-panel, .charts, .calendar-panel { margin-top: 28px; max-width: 900px; border: 1px solid var(--color-separator); border-radius: 12px; background: var(--color-background); }
  .appearance-panel { max-width: 520px; padding: 16px; }
  .subjects-panel { margin-top: 28px; max-width: 620px; height: 760px; display: flex; border: 1px solid var(--color-separator); border-radius: 8px; overflow: hidden; }
  .subjects-panel .content { flex: 1; overflow-y: auto; }
  :global(html[data-pomo-theme='classic-tomato']) .timer-window { background: var(--color-background) var(--classic-grain); }
  .charts { display: grid; gap: 16px; padding: 18px; }
</style>
