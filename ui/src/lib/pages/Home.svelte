<script lang="ts">
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import { call } from "../backend";
  import FlowField from "../components/FlowField.svelte";
  import Icon from "../components/Icon.svelte";
  import Keys from "../components/Keys.svelte";
  import Orb from "../components/Orb.svelte";
  import RuleSheet from "../components/RuleSheet.svelte";
  import { app, toast } from "../state.svelte";
  import type { HistoryItem } from "../types";

  const s = $derived(app.status);
  const phase = $derived(s?.phase ?? "idle");
  const model = $derived(s?.model.phase ?? "unloaded");
  const listening = $derived(phase === "listening" || phase === "hands-free");

  const headline = $derived.by(() => {
    if (!app.connected) return "Connecting to Fluss";
    if (listening) return phase === "hands-free" ? "Listening, hands-free" : "Listening";
    if (phase === "finishing") return "Writing it down";
    switch (model) {
      case "ready":
        return "Ready when you are";
      case "loading":
        return "Warming up";
      case "installing":
        return "Setting things up";
      case "unloaded":
        return "Resting";
      case "not-installed":
        return "Pick a voice model";
      case "error":
        return "Something needs attention";
    }
  });

  const sub = $derived.by(() => {
    if (listening) return app.partial || "Speak naturally — Fluss types as you pause.";
    if (phase === "finishing") return app.partial || "Almost there…";
    switch (model) {
      case "loading":
        return `Loading ${s?.model.name ?? "the model"} into memory. You can already start talking.`;
      case "installing":
        return `${s?.model.detail || "Downloading"} · ${Math.round((s?.model.progress ?? 0) * 100)}%`;
      case "unloaded":
        return "The model sleeps to save memory and wakes up on your next dictation.";
      case "not-installed":
        return "Fluss needs a speech model before it can type for you.";
      case "error":
        return s?.model.error ?? "The model failed to load.";
      default:
        return "";
    }
  });

  let history = $state<HistoryItem[]>([]);
  $effect(() => {
    app.historyVersion;
    call<HistoryItem[]>("history.list").then((h) => (history = h.slice(0, 12)));
  });

  const timeSaved = $derived.by(() => {
    if (!s) return "—";
    const typing = s.stats.words / 40; // minutes at 40 wpm
    const speaking = s.stats.audio_seconds / 60;
    const m = Math.max(0, typing - speaking);
    return m >= 90 ? `${(m / 60).toFixed(1)} h` : `${Math.round(m)} min`;
  });

  const fmt = (n: number | undefined) => (n ?? 0).toLocaleString();

  function ago(ms: number) {
    const d = (Date.now() - ms) / 1000;
    if (d < 60) return "just now";
    if (d < 3600) return `${Math.floor(d / 60)} min ago`;
    if (d < 86400) return `${Math.floor(d / 3600)} h ago`;
    return new Date(ms).toLocaleDateString();
  }

  // Select words in a history entry to turn them into a rule.
  let sel = $state<{ id: number; a: number; b: number } | null>(null);
  let dragging = false;
  let sheet = $state<{ heard: string } | null>(null);

  function words(t: string) {
    return t.split(/\s+/);
  }
  function selText(item: HistoryItem) {
    if (!sel || sel.id !== item.id) return "";
    const [a, b] = [Math.min(sel.a, sel.b), Math.max(sel.a, sel.b)];
    return words(item.text)
      .slice(a, b + 1)
      .join(" ")
      .replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "");
  }
  function inSel(id: number, i: number) {
    if (!sel || sel.id !== id) return false;
    return i >= Math.min(sel.a, sel.b) && i <= Math.max(sel.a, sel.b);
  }

  async function copy(t: string) {
    await navigator.clipboard.writeText(t);
    toast("Copied", "success", 1400);
  }

  async function act(method: string) {
    try {
      await call(method, method === "model.install" ? { id: s?.model.id } : null);
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    }
  }
</script>

<svelte:window onpointerup={() => (dragging = false)} />

<section class="hero card" class:live={listening}>
  <FlowField energy={listening ? 1 : phase === "finishing" ? 0.7 : model === "ready" ? 0.25 : 0.12} />
  <div class="hero-in">
    <Orb {model} {phase} progress={s?.model.progress ?? null} />
    <div class="hero-txt">
      <div class="eyebrow">
        {#if s}
          <span class="badge accent"><Icon name="cpu" size={12} />{s.model.name}</span>
          {#if s.model.device}<span class="badge">{s.model.device}</span>{/if}
          {#if s.model.memory_mb}<span class="badge">{(s.model.memory_mb / 1024).toFixed(1)} GB in memory</span>{/if}
        {/if}
      </div>
      {#key headline}
        <h1 in:fly={{ y: 8, duration: 260 }}>{headline}</h1>
      {/key}
      <p class="sub" class:partial={listening && app.partial}>{sub}</p>
      {#if !listening && phase === "idle"}
        <div class="cta" in:fade>
          {#if model === "ready"}
            <span class="hold">Hold</span>
            <Keys combo={s?.hotkeys.push_to_talk ?? "Ctrl+Win"} />
            <span class="hold">anywhere and speak</span>
          {:else if model === "not-installed"}
            <button class="btn primary" onclick={() => (app.page = "model")}><Icon name="download" size={15} />Choose a model</button>
          {:else if model === "unloaded"}
            <button class="btn" onclick={() => act("model.load")}><Icon name="zap" size={15} />Wake it now</button>
          {:else if model === "error"}
            <button class="btn" onclick={() => act("model.load")}><Icon name="refresh" size={15} />Try again</button>
            <button class="btn ghost" onclick={() => (app.page = "model")}>Model settings</button>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</section>

<div class="stats">
  <div class="stat card">
    <span class="k">Dictations</span>
    <span class="v">{fmt(s?.stats.dictations)}</span>
  </div>
  <div class="stat card">
    <span class="k">Words written</span>
    <span class="v">{fmt(s?.stats.words)}</span>
  </div>
  <div class="stat card">
    <span class="k">Typical delay</span>
    <span class="v">{s?.stats.median_latency_ms ?? "—"}<small>{s?.stats.median_latency_ms ? " ms" : ""}</small></span>
  </div>
  <div class="stat card">
    <span class="k">Time saved</span>
    <span class="v gradient-text">{timeSaved}</span>
  </div>
</div>

<div class="cols">
  <section class="card recent">
    <div class="rhead">
      <h3>Recent</h3>
      <span class="faint small">Drag over words to teach Fluss a correction</span>
    </div>
    {#if history.length === 0}
      <div class="empty">
        <Icon name="wave" size={28} />
        <p>Your dictations show up here for a while. They're kept in memory only.</p>
      </div>
    {:else}
      <ul>
        {#each history as h (h.id)}
          <li animate:flip={{ duration: 250 }} in:fly={{ y: -8, duration: 250 }}>
            <div class="meta">
              <span class="app">{h.app || "Unknown app"}</span>
              <span class="faint">· {ago(h.at)} · {h.latency_ms} ms</span>
              <span class="grow"></span>
              {#if selText(h)}
                <button class="btn primary sm" in:fade={{ duration: 100 }} onclick={() => ((sheet = { heard: selText(h) }), (sel = null))}>
                  <Icon name="sparkles" size={13} />Fix “{selText(h)}”
                </button>
              {:else}
                <button class="btn ghost sm" onclick={() => copy(h.text)}>Copy</button>
              {/if}
            </div>
            <p class="txt">
              {#each words(h.text) as w, i}
                <span
                  class="w"
                  class:on={inSel(h.id, i)}
                  role="presentation"
                  onpointerdown={(e) => {
                    e.preventDefault();
                    dragging = true;
                    sel = { id: h.id, a: i, b: i };
                  }}
                  onpointerenter={() => dragging && sel?.id === h.id && (sel = { ...sel, b: i })}>{w}</span
                >{" "}
              {/each}
            </p>
            {#if h.raw && h.raw !== h.text}
              <p class="final faint">heard as “{h.raw}”</p>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <aside class="card keys">
    <h3>Shortcuts</h3>
    <div class="kitem">
      <div>
        <div class="kt">Dictate</div>
        <div class="faint small">Hold, speak, release</div>
      </div>
      <Keys combo={s?.hotkeys.push_to_talk ?? ""} size="sm" />
    </div>
    <div class="kitem">
      <div>
        <div class="kt">Hands-free</div>
        <div class="faint small">Until you press again</div>
      </div>
      <Keys combo={s?.hotkeys.hands_free ?? ""} size="sm" />
    </div>
    <div class="kitem">
      <div>
        <div class="kt">Fix a word</div>
        <div class="faint small">Right after Fluss gets it wrong</div>
      </div>
      <Keys combo={s?.hotkeys.add_rule ?? ""} size="sm" />
    </div>
    <button class="btn ghost full" onclick={() => (app.page = "shortcuts")}>Change shortcuts <Icon name="chevron" size={14} /></button>
  </aside>
</div>

{#if sheet}
  <RuleSheet heard={sheet.heard} onclose={() => (sheet = null)} />
{/if}

<style>
  .hero {
    position: relative;
    overflow: hidden;
    border-radius: 22px;
    min-height: 236px;
    display: flex;
    align-items: center;
    background:
      radial-gradient(600px 260px at 15% 50%, rgba(34, 211, 238, 0.1), transparent 70%),
      var(--surface);
    transition: box-shadow 600ms var(--ease), border-color 600ms;
  }
  .hero.live {
    border-color: rgba(45, 212, 191, 0.35);
    box-shadow:
      var(--shadow),
      0 0 60px -20px rgba(34, 211, 238, 0.5);
  }
  .hero-in {
    position: relative;
    display: flex;
    align-items: center;
    gap: 34px;
    padding: 32px 40px;
    width: 100%;
  }
  .hero-txt {
    min-width: 0;
    flex: 1;
  }
  .eyebrow {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  h1 {
    font-size: 32px;
    font-weight: 720;
    letter-spacing: -0.035em;
  }
  .sub {
    margin: 8px 0 0;
    color: var(--text-2);
    font-size: 14.5px;
    max-width: 520px;
    min-height: 21px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sub.partial {
    color: var(--text);
    font-size: 16px;
  }
  .cta {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 18px;
  }
  .hold {
    color: var(--text-2);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    margin-top: 14px;
  }
  .stat {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .k {
    font-size: 12px;
    color: var(--text-3);
    font-weight: 550;
  }
  .v {
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.03em;
    font-variant-numeric: tabular-nums;
  }
  .v small {
    font-size: 13px;
    color: var(--text-3);
    font-weight: 600;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 290px;
    gap: 14px;
    margin-top: 14px;
    align-items: start;
  }
  .recent {
    padding: 18px 4px 6px 18px;
  }
  .rhead {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding-right: 18px;
    margin-bottom: 8px;
  }
  h3 {
    font-size: 15px;
  }
  .small {
    font-size: 12px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 14px 0 0;
    max-height: 340px;
    overflow: auto;
  }
  li {
    padding: 12px 0;
    border-top: 1px solid var(--border);
  }
  li:first-child {
    border-top: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    min-height: 26px;
  }
  .app {
    font-weight: 600;
    color: var(--text-2);
  }
  .grow {
    flex: 1;
  }
  .btn.sm {
    height: 26px;
    padding: 0 10px;
    font-size: 12px;
    border-radius: 8px;
  }
  .txt {
    margin: 4px 0 0;
    font-size: 14px;
    line-height: 1.55;
    cursor: text;
  }
  .w {
    border-radius: 4px;
    padding: 1px 0;
    transition: background 100ms;
  }
  .w:hover {
    background: var(--surface-2);
  }
  .w.on {
    background: rgba(45, 212, 191, 0.25);
    box-shadow: 0 0 0 2px rgba(45, 212, 191, 0.25);
  }
  .final {
    margin: 4px 0 0;
    font-size: 12.5px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .empty {
    padding: 30px 18px 34px 0;
    display: grid;
    justify-items: center;
    gap: 8px;
    color: var(--text-3);
    text-align: center;
  }
  .keys {
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .keys h3 {
    margin-bottom: 6px;
  }
  .kitem {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .kt {
    font-weight: 600;
    font-size: 13px;
  }
  .full {
    justify-content: center;
    margin-top: 8px;
  }
  @media (max-width: 1000px) {
    .cols {
      grid-template-columns: 1fr;
    }
    .stats {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
