<script lang="ts">
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { app } from "../state.svelte";
</script>

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" animate:flip={{ duration: 140 }} in:fly={{ x: 16, duration: 160 }} out:fly={{ x: 16, duration: 110 }}>
      <span class="tag">{t.kind === "error" ? "ERR" : t.kind === "success" ? "OK" : "INFO"}</span>
      <span class="msg">{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: 20px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-end;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: stretch;
    max-width: 400px;
    border: 1px solid var(--rule-strong);
    border-left: 2px solid var(--gold);
    background: var(--plate);
    font-size: 12.5px;
    pointer-events: auto;
  }
  .toast.error {
    border-left-color: var(--danger);
  }
  .tag {
    display: grid;
    place-items: center;
    padding: 0 10px;
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.12em;
    color: var(--gold);
    border-right: 1px solid var(--rule);
  }
  .error .tag {
    color: var(--danger);
  }
  .msg {
    padding: 9px 14px 9px 12px;
  }
</style>
