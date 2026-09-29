<script lang="ts">
  // Classic Tomato only: the print tiled behind the settings, tasks and stats
  // pages — tomatoes on the vine (default), a variety chart, or plain paper.
  import { settings } from '$lib/stores/settings';
  import { setSetting } from '$lib/ipc';
  import { notify } from '$lib/stores/toast';
  import { getLocale } from '$paraglide/runtime.js';

  type Pattern = 'vine' | 'varieties' | 'none';
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let current = $derived<Pattern>($settings.classic_pattern ?? 'vine');
  let busy = $state(false);

  let options = $derived<{ id: Pattern; name: string; desc: string; art: string | null }[]>([
    { id: 'vine', name: zh ? '番茄藤' : 'On the vine', desc: zh ? '带果柄的红番茄串（默认）' : 'Red tomatoes on the vine (default)', art: '/classic-tomato/art/vine.svg' },
    { id: 'varieties', name: zh ? '品种图谱' : 'Variety chart', desc: zh ? '多色番茄品种散落' : 'Assorted colorful varieties', art: '/classic-tomato/art/varieties.svg' },
    { id: 'none', name: zh ? '素纸' : 'Plain paper', desc: zh ? '不加图案' : 'No pattern', art: null },
  ]);

  async function choose(id: Pattern) {
    if (busy || id === current) return;
    busy = true;
    try {
      settings.set(await setSetting('classic_pattern', id));
    } catch (e) {
      notify(zh ? `没有切换：${e}` : `Could not switch: ${e}`, { tone: 'error' });
    } finally {
      busy = false;
    }
  }
</script>

<section class="s-group" aria-label={zh ? '页面背景' : 'Page background'}>
  <h3 class="s-group-title">{zh ? '页面背景' : 'Page background'}</h3>
  <div class="cards" role="radiogroup" aria-label={zh ? '页面背景' : 'Page background'}>
    {#each options as option (option.id)}
      <button class="card" class:on={current === option.id} role="radio" aria-checked={current === option.id} disabled={busy}
        onclick={() => choose(option.id)}>
        <span class="swatch" style:background-image={option.art ? `url('${option.art}')` : 'none'} aria-hidden="true"></span>
        <span class="title"><b>{option.name}</b>{#if current === option.id}<em>✓ {zh ? '使用中' : 'In use'}</em>{/if}</span>
        <span class="desc">{option.desc}</span>
      </button>
    {/each}
  </div>
  <p class="s-note">{zh ? '图案铺在设置、任务和统计页的空白处，淡淡衬在卡片后面。' : 'The print fills the empty space behind the settings, tasks and stats pages, faded behind the cards.'}</p>
</section>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 12px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px 10px;
    border: 1.5px solid color-mix(in srgb, var(--ui-text) 30%, transparent);
    border-radius: 6px;
    background: var(--ui-surface);
    color: var(--ui-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .card:hover:not(:disabled) {
    border-color: var(--ui-text);
  }

  .card.on {
    border: 2px solid var(--color-long-round);
    padding: 7.5px 9.5px 9.5px;
  }

  .swatch {
    height: 84px;
    margin-bottom: 6px;
    border-radius: 4px;
    background-color: var(--ui-page);
    background-size: 300px auto;
    background-position: -20px -14px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--ui-text) 12%, transparent);
  }

  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13.5px;
  }

  .title em {
    font-style: normal;
    font-size: 12px;
    font-weight: 700;
    color: var(--color-long-round);
  }

  .desc {
    font-size: 12px;
    color: var(--ui-text-muted);
  }
</style>
