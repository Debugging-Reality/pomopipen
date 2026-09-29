<script lang="ts">
  // Invisible edge/corner strips so decoration-free windows can be resized.
  // Not rendered on macOS, where native decorations provide resizing.
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { isMac } from '$lib/utils/platform';

  const handles = [
    ['n', 'North'], ['s', 'South'], ['e', 'East'], ['w', 'West'],
    ['ne', 'NorthEast'], ['nw', 'NorthWest'], ['se', 'SouthEast'], ['sw', 'SouthWest'],
  ] as const;

  function startResize(direction: (typeof handles)[number][1]) {
    void getCurrentWebviewWindow().startResizeDragging(direction);
  }
</script>

{#if !isMac}
  {#each handles as [edge, direction]}
    <div class="rh rh-{edge}" onmousedown={() => startResize(direction)} role="none"></div>
  {/each}
{/if}

<style>
  .rh { position: fixed; z-index: 9999; }
  .rh-n { top: 0; left: 6px; right: 6px; height: 5px; cursor: n-resize; }
  .rh-s { bottom: 0; left: 6px; right: 6px; height: 5px; cursor: s-resize; }
  .rh-e { right: 0; top: 6px; bottom: 6px; width: 5px; cursor: e-resize; }
  .rh-w { left: 0; top: 6px; bottom: 6px; width: 5px; cursor: w-resize; }
  .rh-ne { top: 0; right: 0; width: 10px; height: 10px; cursor: ne-resize; }
  .rh-nw { top: 0; left: 0; width: 10px; height: 10px; cursor: nw-resize; }
  .rh-se { bottom: 0; right: 0; width: 10px; height: 10px; cursor: se-resize; }
  .rh-sw { bottom: 0; left: 0; width: 10px; height: 10px; cursor: sw-resize; }
</style>
