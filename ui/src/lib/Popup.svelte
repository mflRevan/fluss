<script lang="ts">
  // The "fix a word" popup. Opens pre-filled with what you just said; the
  // cursor is already in "Should be"; Enter saves. Built for one-second edits.
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { call, inTauri, invoke } from "./backend";
  import Keys from "./components/Keys.svelte";
  import Logo from "./components/Logo.svelte";
  import { applyTheme, app } from "./state.svelte";
  import type { Config, PopupContext, Rule, SearchHit, UpsertOutcome } from "./types";

  let ctx = $state<PopupContext | null>(null);
  let start = $state(0);
  let end = $state(0);
  let extra = $state<string[]>([]);
  let variantDraft = $state("");
  let target = $state("");
  let hits = $state<SearchHit[]>([]);
  let hi = $state(-1);
  let saved = $state<null | { text: string }>(null);
  let error = $state("");
  let shown = $state(false);
  let targetEl = $state<HTMLInputElement>();
  let variantEl = $state<HTMLInputElement>();

  const words = $derived(ctx?.chunk.context ?? []);
  const heard = $derived(words.slice(start, end).join(" "));
  const exact = $derived(hits.find((h) => h.rule.target.toLowerCase() === target.trim().toLowerCase())?.rule ?? null);
  const variants = $derived.by(() => {
    const all = [...(heard ? [heard] : []), ...extra, ...(variantDraft.trim() ? [variantDraft.trim()] : [])];
    const seen = new Set<string>();
    return all.filter((v) => {
      const k = v.toLowerCase();
      if (seen.has(k)) return false;
      seen.add(k);
      return true;
    });
  });

  async function load() {
    saved = null;
    error = "";
    extra = [];
    variantDraft = "";
    target = "";
    hits = [];
    hi = -1;
    // Show the card and focus immediately; the "heard" words fill in a moment later.
    shown = true;
    tick().then(() => targetEl?.focus());
    try {
      const c = await call<PopupContext>("popup.context");
      ctx = c;
      start = c.chunk.start;
      end = c.chunk.end;
    } catch {
      ctx = { chunk: { context: [], start: 0, end: 0, text: "" }, return_hwnd: 0, from_dictation: false, can_apply_last: false };
      start = end = 0;
    }
  }

  // Live search as you type the target.
  $effect(() => {
    const q = target.trim();
    const ctl = { stop: false };
    if (!q) {
      hits = [];
      return;
    }
    call<SearchHit[]>("rules.list", { query: q, limit: 5 }).then((h) => {
      if (ctl.stop) return;
      hits = h;
      if (hi >= h.length) hi = -1;
    });
    return () => (ctl.stop = true);
  });

  // Hide, hand focus back to the app you were typing in, then tell the daemon
  // (it applies "fix my last dictation" only once focus is back).
  async function close() {
    shown = false;
    await invoke("popup_finish", { hwnd: ctx?.return_hwnd ?? 0 });
    await Promise.race([call("popup.closed").catch(() => {}), new Promise((r) => setTimeout(r, 1500))]);
    await invoke("maybe_exit");
  }

  async function save() {
    const t = target.trim();
    if (!t) {
      error = "Type what it should be";
      targetEl?.focus();
      return;
    }
    try {
      const out = await call<UpsertOutcome>("rules.upsert", { target: t, variants, apply_last: !!ctx?.can_apply_last });
      saved = { text: out.created ? "Added" : "Updated" };
      setTimeout(close, 320);
    } catch (e) {
      error = String(e instanceof Error ? e.message : e);
    }
  }

  function pickHit(h: Rule) {
    target = h.target;
    hi = -1;
  }

  function onKey(e: KeyboardEvent) {
    if (saved) return;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    // Chunk adjustments work from anywhere.
    if ((e.ctrlKey || e.metaKey) && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      e.preventDefault();
      const n = words.length;
      if (!n) return;
      if (e.shiftKey) end = e.key === "ArrowLeft" ? Math.max(start + 1, end - 1) : Math.min(n, end + 1);
      else start = e.key === "ArrowLeft" ? Math.max(0, start - 1) : Math.min(end - 1, start + 1);
      return;
    }
  }

  function onTargetKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && hits.length) {
      e.preventDefault();
      hi = (hi + 1) % hits.length;
      target = hits[hi].rule.target;
    } else if (e.key === "ArrowUp" && hits.length) {
      e.preventDefault();
      hi = hi <= 0 ? hits.length - 1 : hi - 1;
      target = hits[hi].rule.target;
    } else if (e.key === "Enter") {
      e.preventDefault();
      save();
    } else if (e.key === "Tab" && !e.shiftKey) {
      // Keep the current chunk and type another way it gets misheard.
      e.preventDefault();
      if (heard && !extra.includes(heard)) extra = [...extra, heard];
      variantEl?.focus();
    } else {
      hi = -1;
      error = "";
    }
  }

  function onVariantKey(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === "Tab") {
      if (variantDraft.trim()) {
        e.preventDefault();
        extra = [...extra, variantDraft.trim()];
        variantDraft = "";
        if (e.key === "Enter" && e.ctrlKey) save();
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        save();
      }
    } else if (e.key === "Backspace" && !variantDraft && extra.length) {
      extra = extra.slice(0, -1);
    }
  }

  let anchor = -1;
  function clickWord(i: number, ev: MouseEvent) {
    if (ev.shiftKey && anchor >= 0) {
      start = Math.min(anchor, i);
      end = Math.max(anchor, i) + 1;
    } else {
      anchor = i;
      start = i;
      end = i + 1;
    }
    targetEl?.focus();
  }

  // Size the native window to the card so nothing is clipped and no dead glass remains.
  function fitWindow(node: HTMLElement) {
    if (!inTauri) return;
    const ro = new ResizeObserver(async () => {
      const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
      const r = node.getBoundingClientRect();
      await getCurrentWindow().setSize(new LogicalSize(600, Math.ceil(r.height + 36)));
    });
    ro.observe(node);
    return { destroy: () => ro.disconnect() };
  }

  onMount(() => {
    call<Config>("config.get")
      .then((c) => {
        app.config = c;
        applyTheme();
      })
      .catch(() => applyTheme());
    load();
    let unShow: (() => void) | undefined;
    if (inTauri) {
      import("@tauri-apps/api/event").then(({ listen }) => listen("popup-show", () => load()).then((u) => (unShow = u)));
    }
    return () => unShow?.();
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="stage" class:dev={!inTauri}>
  {#if shown}
    <div class="pop" use:fitWindow in:fly={{ y: 6, duration: 140, opacity: 0 }} out:fade={{ duration: 80 }}>
      <div class="head">
        <Logo size={16} />
        <span class="label title">Fix a word</span>
        <span class="grow"></span>
        <span class="hint"><Keys combo="Enter" size="sm" /> save <Keys combo="Esc" size="sm" /> close</span>
      </div>

      <div class="lbl label">
        <span>Heard</span>
        {#if words.length}<span class="adj"><Keys combo="Ctrl" size="sm" /><kbd>←</kbd><kbd>→</kbd> adjust</span>{/if}
      </div>
      <div class="heard">
        {#if words.length === 0}
          <span class="faint">Nothing recent — type how it gets misheard below.</span>
        {/if}
        {#each words as w, k}
          <button class="w" class:sel={k >= start && k < end} class:first={k === start} class:last={k === end - 1} tabindex="-1" onclick={(e) => clickWord(k, e)}>{w}</button>
        {/each}
      </div>
      {#if extra.length}
        <div class="extra" transition:fly={{ y: -4, duration: 110 }}>
          {#each extra as v, k}
            <span class="chip mono">{v}<button tabindex="-1" aria-label="Remove" onclick={() => (extra = extra.filter((_, j) => j !== k))}>×</button></span>
          {/each}
        </div>
      {/if}

      <div class="lbl label"><span>Should be</span></div>
      <div class="field" class:ok={!!saved} class:err={!!error}>
        <input
          bind:this={targetEl}
          bind:value={target}
          onkeydown={onTargetKey}
          class="display"
          placeholder={heard ? `instead of “${heard}”` : "the right spelling"}
          spellcheck="false"
          autocomplete="off"
        />
        {#if saved}
          <span class="savedmark" in:fade={{ duration: 90 }}>◆</span>
        {/if}
      </div>
      <input bind:this={variantEl} bind:value={variantDraft} onkeydown={onVariantKey} class="variant mono" placeholder="tab — add another way it gets misheard" spellcheck="false" autocomplete="off" />

      <div class="foot mono">
        {#if error}
          <span class="msg err" in:fade={{ duration: 90 }}>{error}</span>
        {:else if saved}
          <span class="msg ok" in:fade={{ duration: 90 }}>◆ {saved.text.toLowerCase()} {target.trim()}{ctx?.can_apply_last ? " · fixed your last dictation" : ""}</span>
        {:else if exact}
          <span class="msg ok" in:fade={{ duration: 90 }}>◆ adds to {exact.target}{exact.variants.length ? ` — already: ${exact.variants.slice(0, 3).join(", ")}` : ""}</span>
        {:else if target.trim()}
          <span class="msg" in:fade={{ duration: 90 }}>new rule · {variants.map((v) => `“${v}”`).join(", ") || "spelling only"} → {target.trim()}</span>
        {:else}
          <span class="msg">existing words appear as you type</span>
        {/if}
      </div>

      {#if hits.length && !saved}
        <div class="hits">
          {#each hits as h, k (h.rule.target)}
            <button class="hit" class:hi={k === hi} tabindex="-1" onclick={() => pickHit(h.rule)}>
              <span class="ht">{h.rule.target}</span>
              <span class="hv mono">{h.rule.variants.length ? h.rule.variants.join(", ") : "spelling only"}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .stage {
    height: 100%;
    display: grid;
    place-items: start center;
    padding: 18px;
  }
  .stage.dev {
    background: #15130f;
  }
  .pop {
    --c: 14px;
    width: 564px;
    padding: 16px 20px 16px;
    background: var(--void);
    border-top: 1px solid var(--gold);
    box-shadow: inset 0 0 0 1px var(--rule-strong);
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - var(--c)), calc(100% - var(--c)) 100%, 0 100%);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 4px;
  }
  .title {
    color: var(--gold);
  }
  .grow {
    flex: 1;
  }
  .adj kbd {
    display: inline-grid;
    place-items: center;
    width: 19px;
    height: 19px;
    border: 1px solid var(--rule-strong);
    border-bottom-color: var(--gold-dim);
    color: var(--text);
    font-size: 10px;
    margin-left: -2px;
  }
  .hint,
  .adj {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--mono);
    font-size: 10px;
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0.04em;
  }
  .lbl {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin: 14px 0 8px;
  }
  .heard {
    display: flex;
    flex-wrap: wrap;
    min-height: 32px;
    align-items: center;
    font-size: 15px;
  }
  .w {
    position: relative;
    border: 0;
    background: transparent;
    padding: 4px 4px;
    margin: 0;
    color: var(--text-2);
    transition: color var(--t1), background var(--t1);
  }
  .w:hover {
    color: var(--text);
  }
  .w.sel {
    color: var(--gold);
    background: var(--gold-wash);
  }
  .w.sel.first::before,
  .w.sel.last::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 5px;
    border: 1px solid var(--gold);
  }
  .w.sel.first::before {
    left: 0;
    border-right: 0;
  }
  .w.sel.last::after {
    right: 0;
    border-left: 0;
  }
  .extra {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 22px;
    padding: 0 3px 0 8px;
    border: 1px solid var(--rule-strong);
    font-size: 11px;
  }
  .chip button {
    border: 0;
    background: transparent;
    color: var(--text-3);
  }
  .chip button:hover {
    color: var(--danger);
  }
  .field {
    position: relative;
    border-bottom: 1px solid var(--gold);
  }
  .field.err {
    border-bottom-color: var(--danger);
  }
  .field input {
    width: 100%;
    height: 52px;
    padding: 0 36px 0 2px;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 34px;
    color: var(--gold);
  }
  .field input::placeholder {
    color: var(--text-3);
    font-size: 26px;
  }
  .savedmark {
    position: absolute;
    right: 6px;
    top: 14px;
    color: var(--gold);
    font-size: 16px;
  }
  .variant {
    width: 100%;
    margin-top: 10px;
    height: 30px;
    padding: 0 10px;
    border: 1px dashed var(--rule-strong);
    background: transparent;
    outline: none;
    font-size: 11.5px;
  }
  .variant:focus {
    border-style: solid;
    border-color: var(--gold-dim);
  }
  .variant::placeholder {
    color: var(--text-3);
  }
  .foot {
    min-height: 18px;
    margin-top: 10px;
  }
  .msg {
    font-size: 11px;
    color: var(--text-3);
    letter-spacing: 0.02em;
  }
  .msg.ok {
    color: var(--gold);
  }
  .msg.err {
    color: var(--danger);
  }
  .hits {
    display: flex;
    flex-direction: column;
    margin-top: 10px;
    border-top: 1px solid var(--rule);
  }
  .hit {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 7px 8px;
    border: 0;
    border-bottom: 1px solid var(--rule);
    background: transparent;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
  }
  .ht {
    font-size: 13.5px;
  }
  .hv {
    font-size: 10.5px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .hit:hover,
  .hit.hi {
    background: var(--gold-wash);
  }
  .hit.hi .ht {
    color: var(--gold);
  }
</style>