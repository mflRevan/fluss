<script lang="ts">
  // The "fix a word" popup. Opens pre-filled with what you just said; the
  // cursor is already in "Should be"; Enter saves. Built for one-second edits.
  import { onMount, tick } from "svelte";
  import { fade, fly, scale } from "svelte/transition";
  import { call, inTauri, invoke } from "./backend";
  import Icon from "./components/Icon.svelte";
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
    <div class="pop" use:fitWindow in:scale={{ start: 0.94, duration: 180, opacity: 0 }} out:fade={{ duration: 100 }}>
      <div class="head">
        <span class="spark"><Icon name="sparkles" size={15} /></span>
        <span class="title">Fix a word</span>
        <span class="grow"></span>
        <span class="hint"><kbd>Enter</kbd> save · <kbd>Esc</kbd> close</span>
      </div>

      <div class="lbl">
        Heard
        {#if words.length}<span class="faint"><kbd>Ctrl</kbd>+<kbd>←</kbd><kbd>→</kbd> adjust</span>{/if}
      </div>
      <div class="heard">
        {#if words.length === 0}
          <span class="faint">Nothing recent — type how it gets misheard below.</span>
        {/if}
        {#each words as w, k}
          <button class="w" class:sel={k >= start && k < end} class:first={k === start} class:last={k === end - 1} tabindex="-1" onclick={(e) => clickWord(k, e)}>{w}</button>
        {/each}
      </div>
      {#if extra.length || variantDraft || document.activeElement === variantEl}
        <div class="extra" transition:fly={{ y: -4, duration: 140 }}>
          {#each extra as v, k}
            <span class="chip">{v}<button tabindex="-1" aria-label="Remove" onclick={() => (extra = extra.filter((_, j) => j !== k))}><Icon name="x" size={11} stroke={2.4} /></button></span>
          {/each}
        </div>
      {/if}

      <div class="lbl">Should be</div>
      <div class="field" class:ok={!!saved} class:err={!!error}>
        <input
          bind:this={targetEl}
          bind:value={target}
          onkeydown={onTargetKey}
          placeholder={heard ? `What did you mean instead of “${heard}”?` : "The right spelling"}
          spellcheck="false"
          autocomplete="off"
        />
        {#if saved}
          <span class="savedmark" in:scale={{ start: 0.4, duration: 260 }}><Icon name="check" size={16} stroke={2.6} /></span>
        {/if}
      </div>
      <input
        bind:this={variantEl}
        bind:value={variantDraft}
        onkeydown={onVariantKey}
        class="variant"
        placeholder="Tab: add another way it gets misheard"
        spellcheck="false"
        autocomplete="off"
      />

      <div class="foot">
        {#if error}
          <span class="msg err" in:fade>{error}</span>
        {:else if saved}
          <span class="msg ok" in:fade>{saved.text} {target.trim()}{ctx?.can_apply_last ? " · fixed your last dictation" : ""}</span>
        {:else if exact}
          <span class="msg" in:fade>
            <Icon name="sparkles" size={13} /> Adding to <b>{exact.target}</b>{exact.variants.length ? ` — already: ${exact.variants.slice(0, 3).join(", ")}` : ""}
          </span>
        {:else if target.trim()}
          <span class="msg faint" in:fade>New rule: {variants.map((v) => `“${v}”`).join(", ") || "spelling only"} → <b>{target.trim()}</b></span>
        {:else}
          <span class="msg faint">Type the right spelling — existing words show up as you type</span>
        {/if}
      </div>

      {#if hits.length && !saved}
        <div class="hits">
          {#each hits as h, k (h.rule.target)}
            <button class="hit" class:hi={k === hi} tabindex="-1" onclick={() => pickHit(h.rule)}>
              <b>{h.rule.target}</b>
              <span class="faint">{h.rule.variants.length ? h.rule.variants.join(", ") : "spelling only"}</span>
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
    background:
      radial-gradient(900px 500px at 30% 20%, #1d3b4a, transparent 60%),
      radial-gradient(700px 500px at 80% 90%, #3a2a5a, transparent 60%),
      #0c1218;
  }
  .pop {
    width: 564px;
    padding: 16px 18px 14px;
    border-radius: 20px;
    /* Opaque: a transparent native window can't blur the desktop behind it. */
    background:
      radial-gradient(420px 160px at 0% 0%, rgba(34, 211, 238, 0.08), transparent 70%),
      var(--surface-solid);
    box-shadow: 0 0 0 1px var(--border-2), 0 12px 32px -12px rgba(0, 0, 0, 0.6);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
  }
  .spark {
    width: 26px;
    height: 26px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--flow);
    color: #031615;
  }
  .title {
    font-weight: 650;
    font-size: 14px;
  }
  .grow {
    flex: 1;
  }
  .hint,
  .lbl .faint {
    font-size: 11.5px;
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 500;
  }
  kbd {
    font-family: var(--font);
    font-size: 10.5px;
    padding: 0 5px;
    border-radius: 4px;
    border: 1px solid var(--border-2);
    background: var(--surface-2);
  }
  .lbl {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    margin: 10px 0 6px;
  }
  .heard {
    display: flex;
    flex-wrap: wrap;
    gap: 0;
    min-height: 34px;
    align-items: center;
    font-size: 15px;
  }
  .w {
    border: 0;
    background: transparent;
    padding: 4px 5px;
    margin: 0;
    color: var(--text-2);
    border-radius: 0;
    transition:
      background 120ms,
      color 120ms;
  }
  .w:hover {
    color: var(--text);
  }
  .w.sel {
    background: rgba(45, 212, 191, 0.2);
    color: var(--text);
    font-weight: 600;
  }
  .w.sel.first {
    border-radius: 8px 0 0 8px;
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .w.sel.last {
    border-radius: 0 8px 8px 0;
  }
  .w.sel.first.last {
    border-radius: 8px;
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .extra {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 3px 0 9px;
    border-radius: 7px;
    background: var(--surface-3);
    font-size: 12.5px;
  }
  .chip button {
    border: 0;
    background: transparent;
    color: var(--text-3);
    display: grid;
    width: 18px;
    height: 18px;
    place-items: center;
  }
  .field {
    position: relative;
    border-radius: 12px;
    background:
      linear-gradient(var(--surface-solid), var(--surface-solid)) padding-box,
      var(--flow) border-box;
    border: 1.5px solid transparent;
    box-shadow: 0 0 0 4px rgba(45, 212, 191, 0.12);
    transition: box-shadow 200ms;
  }
  .field.ok {
    box-shadow: 0 0 0 6px rgba(52, 211, 153, 0.25);
  }
  .field.err {
    background:
      linear-gradient(var(--surface-solid), var(--surface-solid)) padding-box,
      var(--rose) border-box;
  }
  .field input {
    width: 100%;
    height: 46px;
    padding: 0 44px 0 14px;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 18px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .field input::placeholder {
    font-weight: 450;
    font-size: 14px;
    color: var(--text-3);
  }
  .savedmark {
    position: absolute;
    right: 10px;
    top: 10px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--green);
    color: #04210f;
  }
  .variant {
    width: 100%;
    margin-top: 8px;
    height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px dashed var(--border-2);
    background: transparent;
    outline: none;
    font-size: 12.5px;
  }
  .variant:focus {
    border-style: solid;
    border-color: var(--accent);
  }
  .variant::placeholder {
    color: var(--text-3);
  }
  .foot {
    min-height: 22px;
    margin-top: 8px;
  }
  .msg {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--accent-text);
  }
  .msg.faint {
    color: var(--text-3);
  }
  .msg.err {
    color: var(--rose);
  }
  .msg.ok {
    color: var(--green);
  }
  .hits {
    display: flex;
    flex-direction: column;
    margin-top: 6px;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .hit {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 6px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
  }
  .hit span {
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
  }
  .hit:hover,
  .hit.hi {
    background: var(--surface-2);
  }
  .hit.hi b {
    color: var(--accent-text);
  }
</style>
