<script lang="ts">
  import type { Snippet } from "svelte";
  let { title, subtitle = "", actions }: { title: string; subtitle?: string; actions?: Snippet } = $props();
</script>

<header>
  <div>
    <h1>{title}</h1>
    {#if subtitle}<p>{subtitle}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 22px;
  }
  h1 {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  p {
    margin: 6px 0 0;
    color: var(--text-2);
    max-width: 560px;
  }
  .actions {
    display: flex;
    gap: 8px;
  }
</style>
