<script lang="ts">
  // Captures a keyboard combination and formats it as a shortcut string
  // matching Rust's parse_shortcut format (e.g. "Control+F1", "Shift+Alt+A").
  // Shown as keycaps; click (or Enter/Space) to record, Esc or clicking away cancels.

  import { settings } from '$lib/stores/settings';
  import { getLocale } from '$paraglide/runtime.js';

  interface Props {
    value?: string;
    label?: string;
    onchange?: (value: string) => void;
  }

  let { value = '', label = '', onchange }: Props = $props();

  let listening = $state(false);
  let zh = $derived.by(() => { void $settings.language; return getLocale().startsWith('zh'); });
  let parts = $derived(value ? value.split('+').map(display) : []);

  function display(part: string): string {
    if (part === 'Control') return 'Ctrl';
    if (part === 'Super') return 'Win';
    if (part === 'Left') return '←';
    if (part === 'Right') return '→';
    if (part === 'Up') return '↑';
    if (part === 'Down') return '↓';
    return part;
  }

  function codeToKey(code: string): string | null {
    if (code.startsWith('Key')) return code.slice(3); // "KeyA" → "A"
    if (code.startsWith('Digit')) return code.slice(5); // "Digit1" → "1"
    if (/^F([1-9]|1[0-2])$/.test(code)) return code; // "F1"–"F12"
    if (code === 'Space') return 'Space';
    if (code === 'Enter') return 'Enter';
    if (code === 'ArrowLeft') return 'Left';
    if (code === 'ArrowRight') return 'Right';
    if (code === 'ArrowUp') return 'Up';
    if (code === 'ArrowDown') return 'Down';
    return null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!listening) {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        e.stopPropagation();
        listening = true;
      }
      return;
    }
    e.preventDefault();
    e.stopPropagation();

    if (e.key === 'Escape') {
      listening = false;
      return;
    }
    // Ignore bare modifier keys
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

    const key = codeToKey(e.code);
    if (!key) return;

    const combo: string[] = [];
    if (e.ctrlKey) combo.push('Control');
    if (e.shiftKey) combo.push('Shift');
    if (e.altKey) combo.push('Alt');
    if (e.metaKey) combo.push('Super');
    combo.push(key);

    listening = false;
    onchange?.(combo.join('+'));
  }
</script>

<button
  class="key-field"
  class:listening
  onclick={() => (listening = !listening)}
  onblur={() => (listening = false)}
  onkeydown={onKeydown}
  aria-label={`${label} ${parts.join(' + ')}`}
  title={zh ? '点击后按下新的组合键' : 'Click, then press a new key combination'}
>
  {#if listening}
    <span class="prompt">{zh ? '请按下组合键 · Esc 取消' : 'Press keys · Esc cancels'}</span>
  {:else if parts.length}
    {#each parts as part, i}
      {#if i > 0}<span class="plus">+</span>{/if}
      <kbd class="s-kbd">{part}</kbd>
    {/each}
  {:else}
    <span class="prompt">{zh ? '未设置' : 'Not set'}</span>
  {/if}
</button>

<style>
  .key-field {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    min-width: 168px;
    height: 36px;
    padding: 0 6px;
    border-radius: 10px;
    border: 1px dashed transparent;
    background: none;
    font: inherit;
    cursor: pointer;
    transition:
      background 150ms ease,
      border-color 150ms ease;
  }

  .key-field:hover {
    background: var(--ui-hover);
    border-color: var(--ui-border-strong);
  }

  .key-field.listening {
    justify-content: center;
    border-style: solid;
    border-color: var(--ui-brand);
    background: var(--ui-surface);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ui-brand) 20%, transparent);
  }

  .plus {
    font-size: 12px;
    color: var(--ui-text-muted);
  }

  .prompt {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--ui-text-muted);
  }

  .listening .prompt {
    color: var(--ui-brand-strong);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.55;
    }
  }
</style>
