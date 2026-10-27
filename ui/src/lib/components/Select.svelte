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
    width?: number | string;
    placeholder?: string;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement;
  let hi = $state(0);
  const current = $derived(options.find((o) => o.value === value));
  const w = $derived(typeof width === "number" ? `${width}px` : width);

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

<div class="select" bind:this={root} style="width:{w}">
  <button class="trigger" class:open onclick={() => (open = !open)} onkeydown={key}>
    <span class="val" class:placeholder={!current}>{current?.label ?? placeholder}</span>
    <span class="chev"><Icon name="chevronDown" size={14} /></span>
  </button>
  {#if open}
    <div class="menu" transition:fly={{ y: -4, duration: 120 }}>
      {#each options as o, i}
        <button class="item" class:sel={o.value === value} class:hi={i === hi} onpointerenter={() => (hi = i)} onclick={() => pick(o.value)}>
          <span class="txt">
            <span class="name">{o.label}</span>
            {#if o.hint}<span class="hint">{o.hint}</span>{/if}
          </span>
          {#if o.value === value}<span class="mark">◆</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select {
    position: relative;
    max-width: 100%;
  }
  .trigger {
    width: 100%;
    height: 34px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 0 10px 0 12px;
    border: 1px solid var(--rule-strong);
    background: var(--void);
    text-align: left;
    transition: border-color var(--t1) var(--q);
  }
  .trigger:hover {
    border-color: var(--gold-dim);
  }
  .trigger.open {
    border-color: var(--gold);
  }
  .val {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }
  .placeholder {
    color: var(--text-3);
  }
  .chev {
    display: flex;
    color: var(--gold-dim);
    transition: transform var(--t2) var(--q);
  }
  .open .chev {
    transform: rotate(180deg);
  }
  .menu {
    position: absolute;
    z-index: 50;
    top: calc(100% - 1px);
    left: 0;
    right: 0;
    border: 1px solid var(--gold-dim);
    background: var(--plate);
    max-height: 264px;
    overflow: auto;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px;
    border: 0;
    background: transparent;
    text-align: left;
    color: var(--text-2);
  }
  .item + .item {
    border-top: 1px solid var(--rule);
  }
  .item.hi {
    background: var(--gold-wash);
    color: var(--text);
  }
  .item.sel .name {
    color: var(--gold);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .name {
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mark {
    color: var(--gold);
    font-size: 9px;
  }
</style>
