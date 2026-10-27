<script lang="ts">
  // The model picker, shared by the Model page and onboarding.
  import { call } from "../backend";
  import { app, toast } from "../state.svelte";
  import type { ModelInfo } from "../types";
  import Icon from "./Icon.svelte";
  import ProgressRing from "./ProgressRing.svelte";

  let { compact = false, onpick }: { compact?: boolean; onpick?: (m: ModelInfo) => void } = $props();

  let catalog = $state<ModelInfo[]>([]);
  $effect(() => {
    app.status?.model.phase; // refresh "installed" flags as installs finish
    call<ModelInfo[]>("model.catalog").then((c) => (catalog = c));
  });

  const active = $derived(app.config?.model.id);
  const st = $derived(app.status?.model);

  const size = (mb: number) => (mb >= 1000 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`);

  async function choose(m: ModelInfo) {
    onpick?.(m);
    if (m.id === active && m.installed) return;
    try {
      await call("model.use", { id: m.id });
      if (!m.installed && m.engine !== "custom") await call("model.install", { id: m.id });
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    }
  }

  const icon = (m: ModelInfo) => (m.engine === "custom" ? "globe" : m.gpu ? "gpu" : "zap");
</script>

<div class="grid" class:compact>
  {#each catalog as m (m.id)}
    {@const isActive = m.id === active}
    {@const busy = isActive && (st?.phase === "installing" || st?.phase === "loading")}
    <button class="model" class:active={isActive} onclick={() => choose(m)}>
      <div class="top">
        <span class="ic"><Icon name={icon(m)} size={18} /></span>
        <div class="names">
          <div class="name">{m.name}</div>
          <div class="vendor">{m.vendor}</div>
        </div>
        {#if busy}
          <ProgressRing value={st?.phase === "installing" ? (st?.progress ?? null) : null} size={28} stroke={3} />
        {:else if isActive}
          <span class="check"><Icon name="check" size={14} stroke={2.6} /></span>
        {/if}
      </div>
      <p class="tag">{m.tagline}</p>
      <div class="meta">
        {#if m.recommended}<span class="badge accent">Recommended</span>{/if}
        <span class="badge">{m.multilingual ? (m.engine === "custom" ? "Any language" : "25 languages") : "English"}</span>
        {#if m.engine !== "custom"}
          <span class="badge">{m.installed ? "Installed" : `${size(m.download_mb)} download`}</span>
          {#if !compact}<span class="badge">~{size(m.memory_mb)} {m.gpu ? "VRAM" : "RAM"}</span>{/if}
        {/if}
      </div>
      {#if busy}
        <div class="state">{st?.detail || (st?.phase === "loading" ? "Loading…" : "Installing…")}</div>
      {/if}
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
  }
  .model {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    text-align: left;
    border-radius: 16px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow);
    transition:
      border-color var(--t-med),
      transform var(--t-med) var(--ease),
      background var(--t-med),
      box-shadow var(--t-med);
  }
  .model:hover {
    border-color: var(--border-2);
    transform: translateY(-2px);
    background: var(--surface-2);
  }
  .model.active {
    border-color: transparent;
    background:
      linear-gradient(var(--surface-solid), var(--surface-solid)) padding-box,
      var(--flow) border-box;
    border: 1.5px solid transparent;
    box-shadow:
      var(--shadow),
      0 10px 40px -16px rgba(34, 211, 238, 0.55);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ic {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    background: var(--flow-soft);
    color: var(--accent-text);
    border: 1px solid var(--border);
    flex: none;
  }
  .names {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-weight: 650;
    font-size: 14.5px;
  }
  .vendor {
    font-size: 12px;
    color: var(--text-3);
  }
  .check {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--flow);
    color: #031615;
  }
  .tag {
    margin: 0;
    color: var(--text-2);
    font-size: 13px;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .state {
    font-size: 12px;
    color: var(--accent-text);
  }
  .compact .model {
    padding: 14px;
  }
</style>
