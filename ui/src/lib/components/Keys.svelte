<script lang="ts">
  // A shortcut as mono keycaps: [CTRL] + [WIN].
  let { combo, size = "md", pressed = false }: { combo: string; size?: "sm" | "md" | "lg"; pressed?: boolean } = $props();

  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
  const keys = $derived(
    combo
      .split("+")
      .map((k) => k.trim())
      .filter(Boolean)
      .map((k) => (k === "Win" && isMac ? "Cmd" : k === "Escape" ? "Esc" : k)),
  );
</script>

<span class="keys {size}" class:pressed>
  {#each keys as k, i}
    {#if i > 0}<span class="plus">+</span>{/if}
    <kbd>{k}</kbd>
  {/each}
</span>

<style>
  .keys {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }
  .plus {
    color: var(--text-3);
    font-family: var(--mono);
    font-size: 10px;
  }
  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 26px;
    height: 22px;
    padding: 0 7px;
    font-family: var(--mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text);
    border: 1px solid var(--rule-strong);
    border-bottom-color: var(--gold-dim);
    background: var(--plate);
    transition: color var(--t1) var(--q), border-color var(--t1) var(--q), background var(--t1);
  }
  .sm kbd {
    min-width: 22px;
    height: 19px;
    padding: 0 5px;
    font-size: 9.5px;
  }
  .lg kbd {
    min-width: 40px;
    height: 34px;
    padding: 0 12px;
    font-size: 13px;
  }
  .pressed kbd {
    color: var(--gold-ink);
    background: var(--gold);
    border-color: var(--gold);
  }
</style>
