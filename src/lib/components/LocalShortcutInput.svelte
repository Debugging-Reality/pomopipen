<script lang="ts">
  // Records a single key (no modifiers) as a KeyboardEvent.key string, shown as
  // a keycap. Click (or Enter/Space) to record, Esc or clicking away cancels.
  // Used for local (focus-scoped) shortcut bindings.

  import { formatLocalKey } from '$lib/utils/localShortcuts';
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

  const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta', 'CapsLock']);

  function onKeydown(e: KeyboardEvent) {
    if (!listening) {
      // Handle activation here so the window's own Space shortcut doesn't fire.
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
    if (MODIFIER_KEYS.has(e.key)) return;
    listening = false;
    onchange?.(e.key);
  }
</script>

<button
  class="key-field"
  class:listening
  onclick={() => (listening = !listening)}
  onblur={() => (listening = false)}
  onkeydown={onKeydown}
  aria-label={`${label} ${formatLocalKey(value)}`}
  title={zh ? '点击后按下新的按键' : 'Click, then press a new key'}
>
  {#if listening}
    <span class="prompt">{zh ? '请按下按键 · Esc 取消' : 'Press a key · Esc cancels'}</span>
  {:else if value}
    <kbd class="s-kbd">{formatLocalKey(value)}</kbd>
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
