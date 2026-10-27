<script lang="ts">
  // The model picker, shared by the Model page and onboarding.
  import { call } from "../backend";
  import { app, toast } from "../state.svelte";
  import type { ModelInfo } from "../types";
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


</script>

<div class="list" class:compact>
  {#each catalog as m (m.id)}
    {@const isActive = m.id === active}
    {@const busy = isActive && (st?.phase === "installing" || st?.phase === "loading")}
    <button class="model" class:active={isActive} onclick={() => choose(m)}>
      <span class="mark">{isActive ? "◆" : "◇"}</span>
      <div class="body">
        <div class="top">
          <span class="name">{m.name}</span>
          <span class="vendor mono">{m.vendor}</span>
          {#if m.recommended}<span class="rec">Recommended</span>{/if}
          <span class="grow"></span>
          {#if busy}
            <span class="busy mono">{st?.phase === "installing" && st?.progress != null ? `${Math.round(st.progress * 100)}%` : "…"}</span>
            <ProgressRing value={st?.phase === "installing" ? (st?.progress ?? null) : null} size={16} stroke={2} />
          {/if}
        </div>
        {#if !compact}<p class="tag">{m.tagline}</p>{/if}
        <div class="spec mono">
          <span>{m.multilingual ? (m.engine === "custom" ? "any language" : "25 languages") : "English"}</span>
          {#if m.engine !== "custom"}
            <span class:gold={m.installed}>{m.installed ? "installed" : `${size(m.download_mb)} download`}</span>
            <span>~{size(m.memory_mb)} {m.gpu ? "VRAM" : "RAM"}</span>
          {/if}
        </div>
      </div>
    </button>
  {/each}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
  }
  .model {
    position: relative;
    display: flex;
    gap: 14px;
    padding: 11px 14px 11px 12px;
    text-align: left;
    border: 0;
    border-bottom: 1px solid var(--rule);
    background: transparent;
    transition: background var(--t1) var(--q);
  }
  .model:hover {
    background: var(--plate-2);
  }
  .model.active {
    background: var(--gold-wash);
  }
  .model.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: -1px;
    bottom: -1px;
    width: 2px;
    background: var(--gold);
  }
  .mark {
    color: var(--gold-dim);
    font-size: 11px;
    line-height: 20px;
  }
  .active .mark {
    color: var(--gold);
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name {
    font-size: 14.5px;
    color: var(--text);
    white-space: nowrap;
  }
  .active .name {
    color: var(--gold);
  }
  .rec {
    flex: none;
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 1px 6px;
    border: 1px solid var(--gold-dim);
    color: var(--gold);
  }
  .vendor {
    font-size: 10.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .busy {
    font-size: 11px;
    color: var(--gold);
  }
  .tag {
    margin-top: 2px;
    color: var(--text-2);
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .spec {
    display: flex;
    gap: 16px;
    margin-top: 4px;
    white-space: nowrap;
    overflow: hidden;
    font-size: 10.5px;
    color: var(--text-3);
    letter-spacing: 0.02em;
  }
  .compact .model {
    padding: 11px 14px 11px 12px;
  }
</style>