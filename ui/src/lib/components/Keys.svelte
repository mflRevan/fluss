<script lang="ts">
  // A shortcut rendered as keycaps: "Ctrl+Win+Space".
  let { combo, size = "md", pressed = false }: { combo: string; size?: "sm" | "md" | "lg"; pressed?: boolean } = $props();

  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
  const keys = $derived(
    combo
      .split("+")
      .map((k) => k.trim())
      .filter(Boolean)
      .map((k) => (k === "Win" && isMac ? "⌘" : k === "Escape" ? "Esc" : k)),
  );
</script>

<span class="keys {size}" class:pressed>
  {#each keys as k, i}
    {#if i > 0}<span class="plus">+</span>{/if}
    <kbd class:wide={k.length > 3}>
      {#if k === "Win"}
        <svg viewBox="0 0 16 16" width="11" height="11" aria-label="Windows"><path fill="currentColor" d="M1 2.3 6.6 1.5v5.4H1Zm6.4-.9L15 0v6.9H7.4ZM1 7.7h5.6v5.4L1 12.3Zm6.4 0H15V15l-7.6-1.1Z" /></svg>
      {:else}
        {k}
      {/if}
    </kbd>
  {/each}
</span>

<style>
  .keys {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .plus {
    color: var(--text-3);
    font-size: 11px;
  }
  kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
    font-family: var(--font);
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    background: linear-gradient(180deg, var(--key-top), color-mix(in srgb, var(--key-top) 82%, var(--key-side)));
    border: 1px solid var(--border-2);
    box-shadow:
      0 2px 0 var(--key-side),
      0 3px 8px -2px rgba(0, 0, 0, 0.35);
    transform: translateY(-1px);
    transition:
      transform 120ms var(--ease),
      box-shadow 120ms var(--ease);
  }
  .sm kbd {
    min-width: 22px;
    height: 21px;
    padding: 0 6px;
    font-size: 11px;
    border-radius: 6px;
  }
  .lg kbd {
    min-width: 40px;
    height: 38px;
    padding: 0 12px;
    font-size: 14px;
    border-radius: 10px;
  }
  .lg kbd svg {
    width: 14px;
    height: 14px;
  }
  .pressed kbd {
    transform: translateY(1px);
    box-shadow:
      0 0 0 var(--key-side),
      var(--glow);
    color: var(--accent-text);
  }
</style>
