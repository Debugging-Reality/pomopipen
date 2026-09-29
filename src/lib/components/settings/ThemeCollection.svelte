<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { settings } from '$lib/stores/settings';
  import { applyTheme } from '$lib/stores/theme';
  import { importBackground, setSetting } from '$lib/ipc';
  import { collection, collectionFor } from '$lib/themes/collection';
  import { backgroundFor, parseBackgrounds, withBackground, SCALE_RANGE, type BackgroundChoice, type BackgroundTarget } from '$lib/themes/backgrounds';
  import { resolveThemeName } from '$lib/utils/theme';
  import { getLocale } from '$paraglide/runtime.js';
  import { notify } from '$lib/stores/toast';
  import type { Theme } from '$lib/types';

  let { themes, osDark }: { themes: Theme[]; osDark: boolean } = $props();
  let busy = $state(false);
  let error = $state('');
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let current = $derived(resolveThemeName($settings, osDark));
  let currentLabel = $derived.by(() => { const c = collectionFor(current); return c ? (zh ? c.nameZh : c.name) : current; });
  let backgrounds = $derived(parseBackgrounds($settings.theme_backgrounds));
  // Classic Tomato's calendar has a built-in basket: opacity only, no picture.
  let classicCurrent = $derived(collectionFor(current)?.id === 'classic-tomato');
  let basketDrafts = $state<Record<string, number>>({});

  async function saveBasket(key: 'basket_frame_opacity' | 'basket_floor_opacity', value: number) {
    error = '';
    try { await save(key, String(value)); }
    catch (e) { error = String(e); }
    finally { delete basketDrafts[key]; }
  }
  const targets: BackgroundTarget[] = ['timer', 'calendar'];

  async function save(key: string, value: string) {
    settings.set(await setSetting(key, value));
  }

  async function select(name: string) {
    const theme = themes.find(t => t.name === name);
    if (!theme || busy) return;
    busy = true; error = '';
    try {
      const slot = $settings.theme_mode === 'dark' || ($settings.theme_mode === 'auto' && osDark) ? 'theme_dark' : 'theme_light';
      await save(slot, name);
      await save('theme_mode', slot === 'theme_dark' ? 'dark' : 'light');
      applyTheme(theme);
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  /** Only the current theme's picture changes; other themes keep theirs. */
  async function update(target: BackgroundTarget, patch: Partial<BackgroundChoice> | null) {
    error = '';
    try { await save('theme_backgrounds', JSON.stringify(withBackground(backgrounds, current, target, patch))); }
    catch (e) { error = String(e); }
  }

  async function choose(target: BackgroundTarget) {
    if (busy) return;
    error = '';
    try {
      const selected = await open({ multiple: false, filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'webp'] }] });
      if (typeof selected !== 'string') return;
      busy = true;
      await update(target, { path: await importBackground(selected, target) });
      if (!error) notify(zh ? `已为「${currentLabel}」换上新背景` : `New background for ${currentLabel}`);
    } catch (e) { error = String(e); }
    finally { busy = false; }
  }

  // Slider positions while dragging; saved once released.
  let drafts = $state<Record<string, number>>({});

  async function commitDraft(target: BackgroundTarget, key: 'scale' | 'opacity', value: number) {
    await update(target, { [key]: value });
    delete drafts[`${target}-${key}`];
  }

  function filename(path: string) { return path.split(/[\\/]/).pop() ?? ''; }
</script>

<section class="s-group" aria-label={zh ? '主题' : 'Themes'}>
  <h3 class="s-group-title">{zh ? '主题配色' : 'Color theme'}</h3>
  <p class="s-note">{zh ? '四套主题，每套主题的计时器和周历背景各自保存。' : 'Four themes; each keeps its own timer and calendar pictures.'}</p>
  <div class="grid">
    {#each collection as item}
      {@const theme = themes.find(t => t.name === item.name)}
      <button class="theme" data-id={item.id} class:chosen={current === item.name} disabled={busy || !theme}
        aria-pressed={current === item.name} onclick={() => select(item.name)}
        style="--card-bg:{theme?.colors['--pomo-base'] ?? item.colors[1]}; --card-ink:{theme?.colors['--pomo-ink'] ?? item.colors[0]}; --card-accent:{item.colors[0]}; --card-pop:{item.colors[2]}">
        {#if item.id === 'classic-tomato'}
          <span class="card-art tomato"><img src="/classic-tomato/b-fruit.png" alt="" /></span>
        {:else}
          <span class="card-art"><span class="shape"></span><span class="spark">✦</span></span>
        {/if}
        <span class="card-content"><strong>{zh ? item.nameZh : item.name}</strong><small>{zh ? item.descriptionZh : item.description}</small>
          <span class="swatches">{#each item.colors as color}<i style:background={color}></i>{/each}
            {#if current === item.name}<b>{zh ? '使用中' : 'Active'} ✓</b>{/if}</span></span>
      </button>
    {/each}
  </div>
</section>

<section class="s-group" aria-label={zh ? '背景图' : 'Background pictures'}>
  <h3 class="s-group-title">{zh ? `背景图 · ${currentLabel}` : `Background pictures · ${currentLabel}`}</h3>
  <div class="s-card">
    {#each targets as target}
      {@const choice = backgroundFor(backgrounds, current, target)}
      {#if target === 'calendar' && classicCurrent}
        <div class="s-row stacked">
          <div class="s-text">
            <span class="s-label">{zh ? '周历边框 · 番茄篮' : 'Calendar frame · tomato basket'}</span>
            <span class="s-desc">{zh ? '经典番茄的周历装在一只红色菜篮里：篮沿和篮壁是边框，篮底的方格衬在日历里。不能换图，只能调浓淡。' : "Classic Tomato's week sits inside a red produce basket: rim and walls form the frame, and the basket floor shows faintly under the grid. It can't be replaced — only faded."}</span>
          </div>
          <div class="options">
            {#each [['basket_frame_opacity', zh ? '篮筐边框' : 'Frame'], ['basket_floor_opacity', zh ? '篮底方格' : 'Floor grid']] as [key, label] (key)}
              {@const value = basketDrafts[key] ?? $settings[key as 'basket_frame_opacity' | 'basket_floor_opacity']}
              <span class="option-label">{label}</span>
              <div class="slider">
                <input class="s-range" type="range" min="0" max="100" step="5" {value}
                  aria-label={label}
                  style:--frac={value / 100}
                  oninput={e => (basketDrafts[key] = Number(e.currentTarget.value))}
                  onchange={e => saveBasket(key as 'basket_frame_opacity' | 'basket_floor_opacity', Number(e.currentTarget.value))} />
                <span class="s-value">{value}%</span>
              </div>
            {/each}
          </div>
        </div>
      {:else}
      <div class="s-row stacked">
        <div class="head">
          <div class="s-text">
            <span class="s-label">{target === 'timer' ? (zh ? '计时器背景' : 'Timer background') : (zh ? '周历背景' : 'Calendar background')}</span>
            <span class="s-desc file">{choice ? filename(choice.path) : (zh ? '未设置，使用主题纯色' : 'None — theme color')}</span>
          </div>
          <div class="s-control">
            {#if choice}<button class="s-btn s-btn--ghost" disabled={busy} onclick={() => update(target, null)}>{zh ? '恢复纯色' : 'Clear'}</button>{/if}
            <button class="s-btn" disabled={busy} onclick={() => choose(target)}>{choice ? (zh ? '更换图片' : 'Replace') : (zh ? '选择图片' : 'Choose image')}</button>
          </div>
        </div>
        {#if choice}
          {@const opacity = drafts[`${target}-opacity`] ?? choice.opacity}
          <div class="options">
            <span class="option-label">{zh ? '显示方式' : 'Display'}</span>
            <div class="s-segment" role="radiogroup" aria-label={zh ? '显示方式' : 'Display'}>
              <button role="radio" aria-checked={choice.fit === 'tile'} class:on={choice.fit === 'tile'} onclick={() => update(target, { fit: 'tile' })}>{zh ? '平铺图案' : 'Tile pattern'}</button>
              <button role="radio" aria-checked={choice.fit === 'cover'} class:on={choice.fit === 'cover'} onclick={() => update(target, { fit: 'cover' })}>{zh ? '铺满照片' : 'Fill photo'}</button>
            </div>
            {#if choice.fit === 'tile'}
              {@const size = drafts[`${target}-scale`] ?? choice.scale}
              <span class="option-label">{zh ? '图案大小' : 'Pattern size'}</span>
              <div class="slider">
                <input class="s-range" type="range" min={SCALE_RANGE[0]} max={SCALE_RANGE[1]} step="5" value={size}
                  aria-label={zh ? '图案大小' : 'Pattern size'}
                  style:--frac={(size - SCALE_RANGE[0]) / (SCALE_RANGE[1] - SCALE_RANGE[0])}
                  oninput={e => (drafts[`${target}-scale`] = Number(e.currentTarget.value))}
                  onchange={e => commitDraft(target, 'scale', Number(e.currentTarget.value))} />
                <span class="s-value">{size}%</span>
              </div>
            {/if}
            <span class="option-label">{zh ? '透明度' : 'Opacity'}</span>
            <div class="slider">
              <input class="s-range" type="range" min="0" max="100" step="5" value={opacity}
                aria-label={zh ? '图片透明度' : 'Image opacity'}
                style:--frac={opacity / 100}
                oninput={e => (drafts[`${target}-opacity`] = Number(e.currentTarget.value))}
                onchange={e => commitDraft(target, 'opacity', Number(e.currentTarget.value))} />
              <span class="s-value">{opacity}%</span>
            </div>
          </div>
        {/if}
      </div>
      {/if}
    {/each}
  </div>
  <p class="s-hint">{zh ? '支持 JPG、PNG、WebP。平铺图案：自动找出图案的循环单元无缝拼接，大小固定，放大窗口只是多铺几块；铺满照片：适合风景照，居中裁切。' : 'JPG, PNG, WebP. Tile finds the pattern\'s repeat and joins it seamlessly at a fixed size, so bigger windows just show more of it; Fill crops a photo around its center.'}</p>
  {#if error}<p class="s-hint error" role="alert">{zh ? '操作失败：' : 'Could not save: '}{error}</p>{/if}
</section>

<style>
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(210px, 1fr)); gap: 12px; }
  .theme { position: relative; min-height: 124px; border: 3px solid var(--card-accent); border-radius: 18px; background: var(--card-bg); color: var(--card-ink); cursor: pointer; overflow: hidden; text-align: left; padding: 12px 14px; font: inherit; transition: transform 150ms ease, box-shadow 150ms ease; }
  .theme:hover { transform: translateY(-2px); box-shadow: 0 6px 16px color-mix(in srgb, var(--card-accent) 22%, transparent); }
  .theme:active { transform: translateY(0); }
  .theme.chosen { box-shadow: 0 0 0 3px var(--card-pop) inset; }
  .card-art { position: absolute; right: -9px; top: -8px; width: 90px; height: 90px; }
  .shape { display: block; width: 75px; height: 75px; background: var(--card-pop); border: 7px solid var(--card-accent); border-radius: 50%; }
  .theme[data-id='citrus-club'] .shape { border-radius: 50% 50% 12px 12px; background: var(--card-accent); border-color: var(--card-pop); }
  .theme[data-id='berry-planet'] .shape { border-radius: 22px; transform: rotate(-12deg); }
  /* Classic Tomato: a printed label with one real tomato, not a candy shape. */
  .theme[data-id='classic-tomato'] { border: 2px solid #202620; border-radius: 6px; }
  .theme[data-id='classic-tomato'].chosen { box-shadow: 0 0 0 3px #203D65 inset; }
  .card-art.tomato { right: 10px; top: 10px; width: 58px; height: 58px; }
  .card-art.tomato img { width: 58px; height: 58px; filter: drop-shadow(1px 2px 2px rgb(70 35 15 / 25%)); }
  .spark { position: absolute; left: 13px; top: 14px; font-size: 28px; color: var(--card-accent); }
  .card-content { position: relative; display: flex; flex-direction: column; min-height: 96px; justify-content: flex-end; }
  .card-content strong { font-size: 15px; font-weight: 800; }
  small { font-size: 12px; line-height: 1.4; margin-top: 3px; }
  .swatches { display: flex; gap: 5px; align-items: center; margin-top: 9px; }
  i { width: 15px; height: 15px; border-radius: 50%; border: 1px solid var(--card-accent); }
  b { font-size: 12px; margin-left: 4px; }
  button:disabled { opacity: .5; cursor: default; }
  .head { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .options { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 10px 14px; padding: 12px; border-radius: 11px; background: var(--ui-page); }
  .option-label { font-size: 13px; color: var(--ui-text-muted); }
  .s-segment { justify-self: start; }
  .slider { display: flex; align-items: center; gap: 12px; }
</style>