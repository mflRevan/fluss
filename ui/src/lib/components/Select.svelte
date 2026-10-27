<script lang="ts">
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";

  let {
    value,
    options,
    onchange,
    width = 260,
    placeholder = "Select…",
  }: {
    value: string;
    options: { value: string; label: string; hint?: string }[];
    onchange?: (v: string) => void;
    width?: number;
    placeholder?: string;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement;
  let hi = $state(0);
  const current = $derived(options.find((o) => o.value === value));

  function pick(v: string) {
    open = false;
    onchange?.(v);
  }

  function key(e: KeyboardEvent) {
    if (!open && (e.key === "Enter" || e.key === " " || e.key === "ArrowDown")) {
      open = true;
      hi = Math.max(0, options.findIndex((o) => o.value === value));
      e.preventDefault();
      return;
    }
    if (!open) return;
    if (e.key === "Escape") open = false;
    else if (e.key === "ArrowDown") hi = Math.min(options.length - 1, hi + 1);
    else if (e.key === "ArrowUp") hi = Math.max(0, hi - 1);
    else if (e.key === "Enter") pick(options[hi].value);
    else return;
    e.preventDefault();
  }
</script>

<svelte:window onpointerdown={(e) => open && !root.contains(e.target as Node) && (open = false)} />

<div class="select" bind:this={root} style="width:{width}px">
  <button class="trigger" class:open onclick={() => (open = !open)} onkeydown={key}>
    <span class="label" class:placeholder={!current}>{current?.label ?? placeholder}</span>
    <span class="chev"><Icon name="chevronDown" size={16} /></span>
  </button>
  {#if open}
    <div class="menu" transition:fly={{ y: -6, duration: 160 }}>
      {#each options as o, i}
        <button class="item" class:sel={o.value === value} class:hi={i === hi} onpointerenter={() => (hi = i)} onclick={() => pick(o.value)}>
          <span class="txt">
            <span>{o.label}</span>
            {#if o.hint}<span class="hint">{o.hint}</span>{/if}
          </span>
          {#if o.value === value}<Icon name="check" size={15} />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select {
    position: relative;
  }
  .trigger {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 10px 0 12px;
    border-radius: 10px;
    border: 1px solid var(--border-2);
    background: var(--surface);
    text-align: left;
    transition:
      border-color var(--t-fast),
      background var(--t-fast);
  }
  .trigger:hover,
  .trigger.open {
    background: var(--surface-2);
  }
  .trigger.open {
    border-color: var(--accent);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .placeholder {
    color: var(--text-3);
  }
  .chev {
    display: flex;
    color: var(--text-3);
    transition: transform var(--t-med) var(--ease);
  }
  .open .chev {
    transform: rotate(180deg);
  }
  .menu {
    position: absolute;
    z-index: 50;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    padding: 5px;
    border-radius: 12px;
    background: var(--surface-solid);
    box-shadow: var(--shadow-pop);
    max-height: 280px;
    overflow: auto;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    color: var(--text);
  }
  .item.hi {
    background: var(--surface-2);
  }
  .item.sel {
    color: var(--accent-text);
  }
  .txt {
    display: flex;
    flex-direction: column;
  }
  .hint {
    font-size: 11.5px;
    color: var(--text-3);
  }
</style>
