<script lang="ts">
  let {
    value,
    options,
    onchange,
  }: { value: string; options: { value: string; label: string }[]; onchange?: (v: string) => void } = $props();
</script>

<div class="seg" role="radiogroup">
  {#each options as o}
    <button class:active={o.value === value} role="radio" aria-checked={o.value === value} onclick={() => onchange?.(o.value)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    border: 1px solid var(--rule-strong);
    background: var(--void);
  }
  button {
    position: relative;
    height: 28px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    white-space: nowrap;
    transition: color var(--t1) var(--q), background var(--t1) var(--q);
  }
  button + button {
    border-left: 1px solid var(--rule);
  }
  button:hover {
    color: var(--text);
  }
  button.active {
    background: var(--gold);
    color: var(--gold-ink);
  }
</style>
