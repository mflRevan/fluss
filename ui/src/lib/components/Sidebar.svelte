<script lang="ts">
  import { app, type Page } from "../state.svelte";
  import Icon from "./Icon.svelte";
  import Logo from "./Logo.svelte";

  const groups: { label: string; items: { id: Page; label: string; icon: string }[] }[] = [
    {
      label: "",
      items: [
        { id: "home", label: "Home", icon: "home" },
        { id: "dictionary", label: "Dictionary", icon: "book" },
      ],
    },
    {
      label: "Settings",
      items: [
        { id: "shortcuts", label: "Shortcuts", icon: "keyboard" },
        { id: "model", label: "Model", icon: "cpu" },
        { id: "audio", label: "Microphone", icon: "mic" },
        { id: "text", label: "Text", icon: "type" },
        { id: "output", label: "Typing", icon: "output" },
        { id: "advanced", label: "Advanced", icon: "sliders" },
      ],
    },
  ];

  const s = $derived(app.status);
  const modelLabel = $derived.by(() => {
    if (!app.connected) return "Connecting…";
    switch (s?.model.phase) {
      case "ready":
        return s.phase === "idle" ? "Ready" : s.phase === "finishing" ? "Transcribing" : "Listening";
      case "loading":
        return "Loading model";
      case "installing":
        return "Installing";
      case "unloaded":
        return "Sleeping";
      case "not-installed":
        return "Model not installed";
      case "error":
        return "Needs attention";
      default:
        return "…";
    }
  });
  const dot = $derived(
    !app.connected
      ? "off"
      : s?.phase === "listening" || s?.phase === "hands-free"
        ? "live"
        : s?.model.phase === "ready"
          ? "ok"
          : s?.model.phase === "error" || s?.model.phase === "not-installed"
            ? "bad"
            : "busy",
  );
</script>

<nav class="sidebar">
  <div class="brand" data-tauri-drag-region>
    <Logo size={28} />
    <span class="name">Fluss</span>
  </div>

  {#each groups as g}
    {#if g.label}<div class="glabel">{g.label}</div>{/if}
    {#each g.items as it}
      <button class="item" class:active={app.page === it.id} onclick={() => (app.page = it.id)}>
        <Icon name={it.icon} size={17} />
        <span>{it.label}</span>
      </button>
    {/each}
  {/each}

  <div class="grow"></div>

  <button class="item" class:active={app.page === "about"} onclick={() => (app.page = "about")}>
    <Icon name="info" size={17} />
    <span>About</span>
  </button>

  <div class="status">
    <span class="dot {dot}"></span>
    <div class="st">
      <div class="st1">{modelLabel}</div>
      <div class="st2">{s?.model.name ?? "Fluss"}</div>
    </div>
  </div>
</nav>

<style>
  .sidebar {
    width: 220px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 12px 12px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
    backdrop-filter: blur(30px) saturate(1.4);
    position: relative;
    z-index: 41;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 18px;
  }
  .name {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.03em;
  }
  .glabel {
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 16px 10px 6px;
  }
  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    font-size: 13.5px;
    font-weight: 520;
    text-align: left;
    transition:
      background var(--t-fast),
      color var(--t-fast);
  }
  .item:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .item.active {
    background: var(--surface-3);
    color: var(--text);
  }
  .item.active::before {
    content: "";
    position: absolute;
    left: -12px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--flow);
  }
  .item.active :global(svg) {
    color: var(--accent-text);
  }
  .grow {
    flex: 1;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 10px;
    padding: 10px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex: none;
    background: var(--text-3);
  }
  .dot.ok {
    background: var(--green);
    box-shadow: 0 0 10px var(--green);
  }
  .dot.live {
    background: var(--rose);
    box-shadow: 0 0 10px var(--rose);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dot.busy {
    background: var(--amber);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dot.bad {
    background: var(--rose);
  }
  .st {
    min-width: 0;
  }
  .st1 {
    font-size: 12.5px;
    font-weight: 600;
  }
  .st2 {
    font-size: 11.5px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  @keyframes pulse {
    50% {
      opacity: 0.45;
    }
  }
</style>
