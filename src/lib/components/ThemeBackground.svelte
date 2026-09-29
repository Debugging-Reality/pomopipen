<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { settings } from '$lib/stores/settings';
  import { activeTheme } from '$lib/stores/theme';
  import { backgroundFor, parseBackgrounds, type BackgroundTarget } from '$lib/themes/backgrounds';
  import { patternTile, type PatternTile } from '$lib/utils/patternTile';
  import { coverTileWidth } from '$lib/utils/timerLayout';

  // Tiled patterns keep a fixed size like wallpaper: a bigger window shows more
  // repeats instead of a blown-up picture. At 100 % one copy of the picture
  // covers a default-size timer window, matching the look at that size.
  let { target = 'timer' }: { target?: BackgroundTarget } = $props();
  let choice = $derived(backgroundFor(parseBackgrounds($settings.theme_backgrounds), $activeTheme?.name, target));
  let failed = $state('');
  let tile = $state<PatternTile | null>(null);
  let source = $derived(choice ? convertFileSrc(choice.path) : '');
  let visible = $derived(!!source && failed !== source);
  let size = $derived.by(() => {
    if (!tile || !choice) return null;
    const scale = (coverTileWidth(tile.naturalWidth, tile.naturalHeight) / tile.naturalWidth) * (choice.scale / 100);
    return { width: tile.width * scale, height: tile.height * scale };
  });

  $effect(() => {
    const src = source;
    const tiled = choice?.fit === 'tile';
    tile = null;
    if (!src || !tiled) return;
    let cancelled = false;
    patternTile(src)
      .then(result => { if (!cancelled) tile = result; })
      .catch(() => { if (!cancelled) failed = src; });
    return () => { cancelled = true; };
  });
</script>

{#if visible && choice}
  <div class="backdrop" aria-hidden="true" style:opacity={choice.opacity / 100}>
    {#if choice.fit === 'cover'}
      <img src={source} alt="" draggable="false" onerror={() => { failed = source; }} />
    {:else if tile && size}
      <div class="tiles" style:background-image={`url("${tile.url}")`}
        style:background-size={`${size.width.toFixed(1)}px ${size.height.toFixed(1)}px`}></div>
    {/if}
  </div>
{/if}

<style>
  .backdrop { position: absolute; inset: 0; overflow: hidden; pointer-events: none; z-index: 0; }
  img { display: block; width: 100%; height: 100%; object-fit: cover; object-position: center center; }
  .tiles { position: absolute; inset: 0; background-repeat: repeat; background-position: center center; }
</style>
