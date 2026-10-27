<script lang="ts">
  // Variants editor: Enter/Tab/comma commit a chip, Backspace on empty removes the last.
  import { flip } from "svelte/animate";
  import { scale } from "svelte/transition";
  import Icon from "./Icon.svelte";

  let {
    chips = $bindable([]),
    placeholder = "Add a variant…",
    draft = $bindable(""),
    input = $bindable(),
    onkeydown,
  }: {
    chips?: string[];
    placeholder?: string;
    draft?: string;
    input?: HTMLInputElement;
    onkeydown?: (e: KeyboardEvent) => void;
  } = $props();

  function commit() {
    const v = draft.trim().replace(/,$/, "").trim();
    if (v && !chips.some((c) => c.toLowerCase() === v.toLowerCase())) chips = [...chips, v];
    draft = "";
  }

  function key(e: KeyboardEvent) {
    onkeydown?.(e);
    if (e.defaultPrevented) return;
    if ((e.key === "Enter" || e.key === "Tab" || e.key === ",") && draft.trim()) {
      e.preventDefault();
      commit();
    } else if (e.key === "Backspace" && !draft && chips.length) {
      chips = chips.slice(0, -1);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="chips" onclick={() => input?.focus()}>
  {#each chips as c, i (c)}
    <span class="chip" animate:flip={{ duration: 180 }} in:scale={{ start: 0.7, duration: 180 }}>
      {c}
      <button aria-label="Remove {c}" onclick={(e) => (e.stopPropagation(), (chips = chips.filter((_, j) => j !== i)))}>
        <Icon name="x" size={12} stroke={2.2} />
      </button>
    </span>
  {/each}
  <input bind:this={input} bind:value={draft} onkeydown={key} onblur={commit} placeholder={chips.length ? "" : placeholder} />
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 5px 8px;
    border-radius: 10px;
    border: 1px solid var(--border-2);
    background: var(--surface);
    cursor: text;
    transition: border-color var(--t-fast), box-shadow var(--t-fast);
  }
  .chips:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(45, 212, 191, 0.18);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 4px 0 10px;
    border-radius: 8px;
    background: var(--surface-3);
    border: 1px solid var(--border);
    font-size: 12.5px;
    font-weight: 520;
  }
  .chip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-3);
  }
  .chip button:hover {
    background: var(--surface-2);
    color: var(--rose);
  }
  input {
    flex: 1;
    min-width: 120px;
    height: 28px;
    border: 0;
    outline: none;
    background: transparent;
  }
  input::placeholder {
    color: var(--text-3);
  }
</style>
