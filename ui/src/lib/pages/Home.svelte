<script lang="ts">
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import { call } from "../backend";
  import Iris, { type IrisMode } from "../components/Iris.svelte";
  import Keys from "../components/Keys.svelte";
  import RuleSheet from "../components/RuleSheet.svelte";
  import { app, toast } from "../state.svelte";
  import type { HistoryItem } from "../types";

  const s = $derived(app.status);
  const phase = $derived(s?.phase ?? "idle");
  const model = $derived(s?.model.phase ?? "unloaded");
  const listening = $derived(phase === "listening" || phase === "hands-free");
  const irisMode = $derived<IrisMode>(
    phase === "listening" ? "listening" : phase === "hands-free" ? "hands-free" : phase === "finishing" ? "thinking" : model === "loading" || model === "installing" ? "loading" : "idle",
  );

  const headline = $derived.by(() => {
    if (!app.connected) return "Connecting to Fluss";
    if (listening) return phase === "hands-free" ? "Listening, hands-free." : "Listening.";
    if (phase === "finishing") return "Writing it down.";
    switch (model) {
      case "ready":
        return "Ready when you are.";
      case "loading":
        return "Warming up.";
      case "installing":
        return "Setting things up.";
      case "unloaded":
        return "Resting.";
      case "not-installed":
        return "Pick a voice model.";
      case "error":
        return "Needs attention.";
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
    return m >= 90 ? `${(m / 60).toFixed(1)}h` : `${Math.round(m)}m`;
  });

  const fmt = (n: number | undefined) => (n ?? 0).toLocaleString();

  function ago(ms: number) {
    const d = (Date.now() - ms) / 1000;
    if (d < 60) return "now";
    if (d < 3600) return `${Math.floor(d / 60)}m ago`;
    if (d < 86400) return `${Math.floor(d / 3600)}h ago`;
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

<section class="hero">
  <div class="iris"><Iris size={212} mode={irisMode} progress={model === "installing" ? (s?.model.progress ?? null) : null} /></div>
  <div class="hero-txt">
    <div class="label readout">
      {#if s}
        <span class="gold">◆ {s.model.name}</span>
        {#if s.model.device}<span>{s.model.device}</span>{/if}
        {#if s.model.memory_mb}<span>{(s.model.memory_mb / 1024).toFixed(1)} GB resident</span>{/if}
      {/if}
    </div>
    {#key headline}
      <h1 class="display" in:fly={{ y: 4, duration: 140 }}>{headline}</h1>
    {/key}
    <p class="sub" class:partial={(listening || phase === "finishing") && app.partial}>{sub}</p>
    {#if !listening && phase === "idle"}
      <div class="cta" in:fade={{ duration: 120 }}>
        {#if model === "ready"}
          <span class="hold">Hold</span>
          <Keys combo={s?.hotkeys.push_to_talk ?? "Ctrl+Win"} />
          <span class="hold">anywhere and speak.</span>
        {:else if model === "not-installed"}
          <button class="btn primary" onclick={() => (app.page = "model")}>Choose a model</button>
        {:else if model === "unloaded"}
          <button class="btn" onclick={() => act("model.load")}>Wake it now</button>
        {:else if model === "error"}
          <button class="btn" onclick={() => act("model.load")}>Try again</button>
          <button class="btn ghost" onclick={() => (app.page = "model")}>Model settings</button>
        {/if}
      </div>
    {/if}
  </div>
</section>

<div class="stats">
  <div class="stat"><span class="label">Dictations</span><span class="v mono">{fmt(s?.stats.dictations)}</span></div>
  <div class="stat"><span class="label">Words written</span><span class="v mono">{fmt(s?.stats.words)}</span></div>
  <div class="stat">
    <span class="label">Typical delay</span>
    <span class="v mono">{s?.stats.median_latency_ms ?? "—"}<small>{s?.stats.median_latency_ms ? "ms" : ""}</small></span>
  </div>
  <div class="stat"><span class="label">Time saved</span><span class="v mono gold">{timeSaved}</span></div>
</div>

<div class="lower">
  <section class="recent">
    <div class="rhead">
      <span class="label"><b>◆</b> Recent</span>
      <span class="label">Drag across words to teach a correction</span>
    </div>
    {#if history.length === 0}
      <div class="empty">
        <p class="display">Nothing yet.</p>
        <p class="faint">Your dictations appear here for a while — kept in memory only, never written to disk.</p>
      </div>
    {:else}
      <ul>
        {#each history as h (h.id)}
          <li animate:flip={{ duration: 140 }} in:fly={{ y: -4, duration: 140 }}>
            <div class="meta">
              <span class="app">{h.app || "unknown"}</span>
              <span>{ago(h.at)}</span>
              <span>{h.latency_ms} ms</span>
              <span class="grow"></span>
              {#if selText(h)}
                <button class="btn primary sm" in:fade={{ duration: 90 }} onclick={() => ((sheet = { heard: selText(h) }), (sel = null))}>Fix “{selText(h)}”</button>
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
              <p class="final">heard as “{h.raw}”</p>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <aside class="keys">
    <div class="rhead"><span class="label"><b>◆</b> Shortcuts</span></div>
    <div class="kitem">
      <div>
        <div class="kt">Dictate</div>
        <div class="ks">Hold, speak, release</div>
      </div>
      <Keys combo={s?.hotkeys.push_to_talk ?? ""} size="sm" />
    </div>
    <div class="kitem">
      <div>
        <div class="kt">Hands-free</div>
        <div class="ks">Until you press again</div>
      </div>
      <Keys combo={s?.hotkeys.hands_free ?? ""} size="sm" />
    </div>
    <div class="kitem">
      <div>
        <div class="kt">Fix a word</div>
        <div class="ks">Right after it goes wrong</div>
      </div>
      <Keys combo={s?.hotkeys.add_rule ?? ""} size="sm" />
    </div>
    <button class="btn ghost sm more" onclick={() => (app.page = "shortcuts")}>Change shortcuts →</button>
  </aside>
</div>

{#if sheet}
  <RuleSheet heard={sheet.heard} onclose={() => (sheet = null)} />
{/if}

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: 40px;
    padding: 4px 0 26px;
    border-bottom: 1px solid var(--rule);
    flex: none;
  }
  .iris {
    flex: none;
    margin-left: -6px;
  }
  .hero-txt {
    min-width: 0;
    flex: 1;
  }
  .readout {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
  }
  h1 {
    font-size: 58px;
    margin-top: 16px;
  }
  .sub {
    margin-top: 14px;
    color: var(--text-2);
    font-size: 14px;
    max-width: 480px;
    min-height: 20px;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sub.partial {
    color: var(--text);
    font-size: 15px;
  }
  .cta {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 20px;
  }
  .hold {
    color: var(--text-2);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    border-bottom: 1px solid var(--rule);
    flex: none;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 0 16px 20px;
  }
  .stat:first-child {
    padding-left: 0;
  }
  .stat + .stat {
    border-left: 1px solid var(--rule);
  }
  .v {
    font-size: 26px;
    line-height: 1;
    font-weight: 400;
    letter-spacing: -0.02em;
  }
  .v small {
    font-size: 12px;
    color: var(--text-3);
    margin-left: 3px;
    letter-spacing: 0;
  }
  .lower {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 268px;
  }
  .recent {
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-right: 24px;
    border-right: 1px solid var(--rule);
  }
  .rhead {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 16px 0 6px;
    flex: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 0 0;
    overflow: auto;
    min-height: 0;
  }
  li {
    padding: 11px 0;
    border-top: 1px solid var(--rule);
  }
  li:first-child {
    border-top: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 14px;
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    color: var(--text-3);
    min-height: 26px;
  }
  .app {
    color: var(--gold-dim);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }
  .grow {
    flex: 1;
  }
  .txt {
    margin-top: 3px;
    font-size: 14px;
    line-height: 1.55;
    cursor: text;
  }
  .w {
    padding: 1px 0;
    transition: background var(--t1), color var(--t1);
  }
  .w:hover {
    color: var(--gold);
  }
  .w.on {
    background: var(--gold);
    color: var(--gold-ink);
  }
  .final {
    margin-top: 3px;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-3);
  }
  .empty {
    padding: 28px 0;
    display: grid;
    gap: 8px;
    max-width: 380px;
  }
  .empty .display {
    font-size: 26px;
    color: var(--text-2);
  }
  .keys {
    display: flex;
    flex-direction: column;
    padding-left: 24px;
  }
  .kitem {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 12px 0;
    border-bottom: 1px solid var(--rule);
  }
  .kt {
    font-size: 13.5px;
  }
  .ks {
    font-size: 11.5px;
    color: var(--text-3);
    margin-top: 1px;
  }
  .more {
    align-self: flex-start;
    margin-top: 12px;
    margin-left: -11px;
  }
</style>