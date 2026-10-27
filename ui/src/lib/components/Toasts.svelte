<script lang="ts">
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { app } from "../state.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.kind}" animate:flip={{ duration: 220 }} in:fly={{ y: 16, duration: 260 }} out:fly={{ x: 40, duration: 200 }}>
      <span class="dot"><Icon name={t.kind === "error" ? "x" : t.kind === "success" ? "check" : "info"} size={14} stroke={2.2} /></span>
      <span>{t.text}</span>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 18px;
    bottom: 18px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 380px;
    padding: 10px 14px 10px 10px;
    border-radius: 12px;
    background: var(--surface-solid);
    box-shadow: var(--shadow-pop);
    font-size: 13px;
    pointer-events: auto;
  }
  .dot {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgba(56, 189, 248, 0.18);
    color: var(--sky);
  }
  .success .dot {
    background: rgba(52, 211, 153, 0.18);
    color: var(--green);
  }
  .error .dot {
    background: rgba(251, 113, 133, 0.18);
    color: var(--rose);
  }
</style>
