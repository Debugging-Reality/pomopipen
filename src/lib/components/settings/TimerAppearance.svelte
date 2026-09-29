<script lang="ts">
  // Classic Tomato only: pick the mechanical kitchen timer (A) or the real
  // tomato in a countdown ring (B). Switching never pauses or resets a round.
  import { settings } from '$lib/stores/settings';
  import { setSetting } from '$lib/ipc';
  import { notify } from '$lib/stores/toast';
  import { getLocale } from '$paraglide/runtime.js';
  import MechanicalDrum from '../classic/MechanicalDrum.svelte';
  import TomatoRing from '../classic/TomatoRing.svelte';

  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let current = $derived($settings.timer_appearance === 'ring' ? 'ring' : 'mechanical');
  let busy = $state(false);

  const options = $derived([
    {
      id: 'mechanical' as const,
      name: zh ? '机械番茄钟' : 'Mechanical tomato',
      desc: zh ? '刻度鼓随时间转动，开始前可拖拧调时。' : 'The scale drum turns with time; twist it to set the length.',
    },
    {
      id: 'ring' as const,
      name: zh ? '鲜番茄环' : 'Tomato ring',
      desc: zh ? '真实番茄静止，外圈按时间消耗。' : 'A still tomato; the ring around it runs down.',
    },
  ]);

  async function choose(id: 'mechanical' | 'ring') {
    if (busy || id === current) return;
    busy = true;
    try {
      settings.set(await setSetting('timer_appearance', id));
    } catch (e) {
      notify(zh ? `没有切换：${e}` : `Could not switch: ${e}`, { tone: 'error' });
    } finally {
      busy = false;
    }
  }
</script>

<section class="s-group" aria-label={zh ? '计时器外观' : 'Timer appearance'}>
  <h3 class="s-group-title">{zh ? '计时器外观' : 'Timer appearance'}</h3>
  <div class="cards" role="radiogroup" aria-label={zh ? '计时器外观' : 'Timer appearance'}>
    {#each options as option (option.id)}
      <button class="card" class:on={current === option.id} role="radio" aria-checked={current === option.id} disabled={busy}
        onclick={() => choose(option.id)}>
        <span class="preview" aria-hidden="true">
          {#if option.id === 'mechanical'}
            <MechanicalDrum totalSecs={1500} remainingSecs={1500} width={124} label="" />
          {:else}
            <TomatoRing elapsed={0} color="#C83224" size={100} fruit={72} gap={10} stroke={3.5} label="" />
          {/if}
        </span>
        <span class="title"><b>{option.name}</b>{#if current === option.id}<em>✓ {zh ? '使用中' : 'In use'}</em>{/if}</span>
        <span class="desc">{option.desc}</span>
      </button>
    {/each}
  </div>
  <p class="s-note">{zh ? '切换外观不会暂停或重置正在进行的计时。' : 'Switching never pauses or resets the running round.'}</p>
</section>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 14px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px 12px;
    border: 1.5px solid color-mix(in srgb, var(--ui-text) 30%, transparent);
    border-radius: 6px;
    background: var(--ui-surface);
    color: var(--ui-text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition: border-color 150ms ease;
  }

  .card:hover:not(:disabled) {
    border-color: var(--ui-text);
  }

  /* Selected: a navy frame and the words "in use" — never color alone. */
  .card.on {
    border: 2px solid var(--color-long-round);
    padding: 9.5px 11.5px 11.5px;
  }

  .preview {
    display: grid;
    place-items: center;
    height: 112px;
    margin-bottom: 8px;
    border-radius: 4px;
    background: var(--ui-page);
  }

  .title {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 14px;
  }

  .title em {
    font-style: normal;
    font-size: 12px;
    font-weight: 700;
    color: var(--color-long-round);
  }

  .desc {
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--ui-text-muted);
  }
</style>
