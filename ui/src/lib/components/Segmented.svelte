<script lang="ts">
  let {
    value,
    options,
    onchange,
  }: { value: string; options: { value: string; label: string }[]; onchange?: (v: string) => void } = $props();

  const idx = $derived(Math.max(0, options.findIndex((o) => o.value === value)));
</script>

<div class="seg" style="--n:{options.length};--i:{idx}">
  <span class="pill"></span>
  {#each options as o}
    <button class:active={o.value === value} onclick={() => onchange?.(o.value)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    position: relative;
    display: grid;
    grid-template-columns: repeat(var(--n), 1fr);
    padding: 3px;
    border-radius: 11px;
    background: var(--surface);
    border: 1px solid var(--border-2);
    min-width: calc(var(--n) * 84px);
  }
  .pill {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc((100% - 6px) / var(--n));
    transform: translateX(calc(var(--i) * 100%));
    border-radius: 8px;
    background: var(--surface-3);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.06) inset,
      0 4px 12px -4px rgba(0, 0, 0, 0.4);
    transition: transform 380ms var(--spring);
  }
  button {
    position: relative;
    z-index: 1;
    height: 28px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    font-size: 12.5px;
    font-weight: 550;
    color: var(--text-2);
    border-radius: 8px;
    transition: color var(--t-fast);
  }
  button.active {
    color: var(--text);
  }
</style>
