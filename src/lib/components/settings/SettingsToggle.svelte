<script lang="ts">
  // A labelled switch row. The whole row is the switch, so a click anywhere toggles it.
  import TooltipInfo from '$lib/components/TooltipInfo.svelte';

  interface Props {
    label: string;
    checked: boolean;
    description?: string;
    tooltip?: string;
    disabled?: boolean;
    onclick: () => void;
  }

  let { label, checked, description, tooltip, disabled = false, onclick }: Props = $props();
</script>

<button class="s-row" role="switch" aria-checked={checked} {disabled} {onclick}>
  <span class="s-text">
    <span class="s-label">{label}{#if tooltip}<TooltipInfo text={tooltip} />{/if}</span>
    {#if description}
      <span class="s-desc">{description}</span>
    {/if}
  </span>
  <span class="s-switch" class:on={checked} aria-hidden="true"></span>
</button>

<style>
  button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
</style>
