<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    hint = "",
    icon = "",
    children,
    below,
    disabled = false,
  }: { title: string; hint?: string; icon?: string; children?: Snippet; below?: Snippet; disabled?: boolean } = $props();
</script>

<div class="row" class:disabled>
  <div class="main">
    {#if icon}
      <span class="ico"><Icon name={icon} size={17} /></span>
    {/if}
    <div class="txt">
      <div class="title">{title}</div>
      {#if hint}<div class="hint">{hint}</div>{/if}
    </div>
    <div class="ctl">{@render children?.()}</div>
  </div>
  {#if below}<div class="below">{@render below()}</div>{/if}
</div>

<style>
  .row {
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
    transition: opacity var(--t-med);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.disabled {
    opacity: 0.45;
    pointer-events: none;
  }
  .main {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .ico {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: var(--accent-text);
    background: var(--flow-soft);
    border: 1px solid var(--border);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 560;
    font-size: 13.5px;
  }
  .hint {
    color: var(--text-3);
    font-size: 12.5px;
    margin-top: 2px;
    line-height: 1.4;
  }
  .ctl {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .below {
    margin-top: 12px;
  }
</style>
