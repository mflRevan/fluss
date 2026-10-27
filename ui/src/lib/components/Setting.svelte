<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    hint = "",
    children,
    below,
    disabled = false,
    stack = false,
  }: { title: string; hint?: string; icon?: string; children?: Snippet; below?: Snippet; disabled?: boolean; stack?: boolean } = $props();
</script>

<div class="row" class:disabled class:stack>
  <div class="txt">
    <div class="title">{title}</div>
    {#if hint}<div class="hint">{hint}</div>{/if}
  </div>
  {#if children}<div class="ctl">{@render children()}</div>{/if}
  {#if below}<div class="below">{@render below()}</div>{/if}
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    column-gap: 20px;
    padding: 11px 0;
    border-bottom: 1px solid var(--rule);
    transition: opacity var(--t2);
  }
  .row.stack {
    grid-template-columns: 1fr;
    row-gap: 9px;
  }
  .row.disabled {
    opacity: 0.35;
    pointer-events: none;
  }
  .txt {
    min-width: 0;
  }
  .title {
    font-size: 13.5px;
    color: var(--text);
  }
  .hint {
    color: var(--text-3);
    font-size: 12px;
    margin-top: 2px;
    line-height: 1.4;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-self: end;
  }
  .stack .ctl {
    justify-self: stretch;
  }
  .below {
    grid-column: 1 / -1;
    margin-top: 10px;
  }
</style>
