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
    <span class="chip" animate:flip={{ duration: 120 }} in:scale={{ start: 0.85, duration: 120 }}>
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
    gap: 5px;
    min-height: 38px;
    padding: 5px 8px;
    border: 1px solid var(--rule);
    background: var(--plate);
    cursor: text;
    transition: border-color var(--t1) var(--q);
  }
  .chips:hover {
    border-color: var(--rule-strong);
  }
  .chips:focus-within {
    border-color: var(--gold);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 3px 0 8px;
    border: 1px solid var(--rule-strong);
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--text);
  }
  .chip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 0;
    background: transparent;
    color: var(--text-3);
  }
  .chip button:hover {
    color: var(--danger);
  }
  input {
    flex: 1;
    min-width: 120px;
    height: 26px;
    border: 0;
    outline: none;
    background: transparent;
    font-family: var(--mono);
    font-size: 12px;
  }
  input::placeholder {
    color: var(--text-3);
    font-family: var(--font);
    font-size: 13px;
  }
</style>