<script lang="ts">
  import type { Snippet } from "svelte";
  let { index = "", title, subtitle = "", actions }: { index?: string; title: string; subtitle?: string; actions?: Snippet } = $props();
</script>

<header>
  <div class="txt">
    {#if index}<div class="label"><b>{index}</b> — {title}</div>{/if}
    <h1 class="display">{title}</h1>
    {#if subtitle}<p>{subtitle}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 22px;
    margin-bottom: 4px;
    flex: none;
  }
  h1 {
    font-size: 44px;
    margin-top: 10px;
  }
  p {
    margin-top: 10px;
    color: var(--text-2);
    max-width: 560px;
    font-size: 13.5px;
  }
  .actions {
    display: flex;
    gap: 8px;
    padding-bottom: 4px;
  }
</style>
