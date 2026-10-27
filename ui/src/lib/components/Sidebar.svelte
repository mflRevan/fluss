<script lang="ts">
  import { app, type Page } from "../state.svelte";
  import Iris, { type IrisMode } from "./Iris.svelte";
  import Logo from "./Logo.svelte";

  const groups: { label: string; items: { id: Page; label: string }[] }[] = [
    {
      label: "Operate",
      items: [
        { id: "home", label: "Home" },
        { id: "dictionary", label: "Dictionary" },
      ],
    },
    {
      label: "Configure",
      items: [
        { id: "shortcuts", label: "Shortcuts" },
        { id: "model", label: "Model" },
        { id: "audio", label: "Microphone" },
        { id: "text", label: "Text" },
        { id: "output", label: "Typing" },
        { id: "advanced", label: "Advanced" },
      ],
    },
  ];
  let n = 0;
  const index = new Map<Page, string>();
  for (const g of groups) for (const it of g.items) index.set(it.id, String(++n).padStart(2, "0"));
  index.set("about", String(++n).padStart(2, "0"));

  const s = $derived(app.status);
  const stateLabel = $derived.by(() => {
    if (!app.connected) return "Offline";
    switch (s?.model.phase) {
      case "ready":
        return s.phase === "idle" ? "Ready" : s.phase === "finishing" ? "Writing" : s.phase === "hands-free" ? "Hands-free" : "Listening";
      case "loading":
        return "Loading";
      case "installing":
        return "Installing";
      case "unloaded":
        return "Asleep";
      case "not-installed":
        return "No model";
      case "error":
        return "Fault";
      default:
        return "—";
    }
  });
  const mode = $derived<IrisMode>(
    s?.phase === "listening" ? "listening" : s?.phase === "hands-free" ? "hands-free" : s?.phase === "finishing" ? "thinking" : s?.model.phase === "loading" || s?.model.phase === "installing" ? "loading" : "idle",
  );
  const bad = $derived(!app.connected || s?.model.phase === "error" || s?.model.phase === "not-installed");
  const mem = $derived(s?.model.memory_mb ? `${(s.model.memory_mb / 1024).toFixed(1)} GB` : "");
</script>

<nav class="sidebar">
  <div class="brand" data-tauri-drag-region>
    <Logo size={20} />
    <span class="name display">Fluss</span>
  </div>

  {#each groups as g}
    <div class="label glabel">{g.label}</div>
    {#each g.items as it}
      <button class="item" class:active={app.page === it.id} onclick={() => (app.page = it.id)}>
        <span class="idx">{index.get(it.id)}</span>
        <span>{it.label}</span>
      </button>
    {/each}
  {/each}

  <div class="grow"></div>

  <button class="item" class:active={app.page === "about"} onclick={() => (app.page = "about")}>
    <span class="idx">{index.get("about")}</span>
    <span>About</span>
  </button>

  <div class="status">
    <Iris size={44} {mode} />
    <div class="st">
      <div class="st1" class:bad>{stateLabel}</div>
      <div class="st2">{s?.model.name ?? "Fluss"}</div>
      <div class="st2">{[s?.model.device, mem].filter(Boolean).join(" · ") || "—"}</div>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    width: 232px;
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 18px 0 16px;
    background: var(--void);
    border-right: 1px solid var(--rule);
    position: relative;
    z-index: 41;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 2px 24px 26px;
  }
  .name {
    font-size: 27px;
    font-weight: 400;
    line-height: 1;
    padding-bottom: 3px;
  }
  .glabel {
    padding: 14px 24px 8px;
  }
  .item {
    position: relative;
    display: flex;
    align-items: baseline;
    gap: 14px;
    height: 34px;
    padding: 0 24px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    text-align: left;
    align-items: center;
    transition: color var(--t1) var(--q), background var(--t1) var(--q);
  }
  .idx {
    font-family: var(--mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    color: var(--text-3);
    width: 16px;
    transition: color var(--t1);
  }
  .item:hover {
    color: var(--text);
  }
  .item:hover .idx {
    color: var(--gold-dim);
  }
  .item.active {
    color: var(--gold);
    background: var(--gold-wash);
  }
  .item.active .idx {
    color: var(--gold);
  }
  .item.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--gold);
  }
  .grow {
    flex: 1;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 12px 16px 0;
    padding: 14px 8px 0;
    border-top: 1px solid var(--rule);
  }
  .st {
    min-width: 0;
    font-family: var(--mono);
  }
  .st1 {
    font-size: 11px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--gold);
  }
  .st1.bad {
    color: var(--danger);
  }
  .st2 {
    font-size: 10.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 2px;
  }
</style>
