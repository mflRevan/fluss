<script lang="ts">
  import { flip } from "svelte/animate";
  import { fade, fly } from "svelte/transition";
  import { call } from "../backend";
  import Icon from "../components/Icon.svelte";
  import Keys from "../components/Keys.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import RuleSheet from "../components/RuleSheet.svelte";
  import { app, toast } from "../state.svelte";
  import type { Rule, SearchHit } from "../types";

  let query = $state("");
  let hits = $state<SearchHit[]>([]);
  let total = $state(0);
  let loaded = $state(false);
  let sheet = $state<{ rule: Rule | null } | null>(null);
  let sort = $state<"recent" | "used" | "az">("recent");
  let searchEl: HTMLInputElement;

  $effect(() => {
    app.rulesVersion;
    const q = query;
    const ctl = { stop: false };
    call<SearchHit[]>("rules.list", { query: q, limit: 500 }).then((h) => {
      if (ctl.stop) return;
      hits = h;
      loaded = true;
      if (!q) total = h.length;
    });
    return () => (ctl.stop = true);
  });

  const shown = $derived.by(() => {
    if (query.trim()) return hits;
    const h = [...hits];
    if (sort === "used") h.sort((a, b) => b.rule.uses - a.rule.uses);
    if (sort === "az") h.sort((a, b) => a.rule.target.localeCompare(b.rule.target));
    return h;
  });

  async function remove(r: Rule) {
    try {
      await call("rules.delete", { target: r.target });
      app.rulesVersion++;
      toast(`Removed ${r.target}`, "info");
    } catch (e) {
      toast(String(e), "error");
    }
  }

  async function removeVariant(r: Rule, v: string) {
    await call("rules.delete_variant", { target: r.target, variant: v });
    app.rulesVersion++;
  }

  function mark(text: string, q: string) {
    const i = q ? text.toLowerCase().indexOf(q.toLowerCase()) : -1;
    if (i < 0) return [{ t: text, m: false }];
    return [
      { t: text.slice(0, i), m: false },
      { t: text.slice(i, i + q.length), m: true },
      { t: text.slice(i + q.length), m: false },
    ];
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      e.preventDefault();
      searchEl?.focus();
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "n" && !sheet) {
      e.preventDefault();
      sheet = { rule: null };
    }
  }}
/>

<PageHeader title="Dictionary" subtitle="Words Fluss should know and the corrections it applies. Every rule maps what it hears to what you meant.">
  {#snippet actions()}
    <button class="btn primary" onclick={() => (sheet = { rule: null })}><Icon name="plus" size={15} stroke={2.2} />New rule</button>
  {/snippet}
</PageHeader>

<div class="bar">
  <div class="search">
    <Icon name="search" size={16} />
    <input bind:this={searchEl} bind:value={query} placeholder="Search {total} rules…" />
    {#if query}
      <button class="clear" onclick={() => (query = "")} aria-label="Clear"><Icon name="x" size={14} /></button>
    {:else}
      <span class="hint"><kbd>Ctrl</kbd><kbd>F</kbd></span>
    {/if}
  </div>
  {#if !query}
    <div class="sorts">
      {#each [["recent", "Newest"], ["used", "Most used"], ["az", "A–Z"]] as [k, l]}
        <button class:on={sort === k} onclick={() => (sort = k as any)}>{l}</button>
      {/each}
    </div>
  {/if}
</div>

{#if loaded && total === 0 && !query}
  <div class="empty card" in:fade>
    <div class="demo">
      <div class="said"><span class="faint">You said</span> “Omarchy”</div>
      <div class="typed"><span class="faint">Fluss typed</span> <s>on March 9</s></div>
      <div class="fix"><Keys combo={app.status?.hotkeys.add_rule ?? "Ctrl+Win+R"} /> <Icon name="arrowRight" size={16} /> <b>Omarchy</b> <Icon name="check" size={16} /></div>
    </div>
    <h3>Teach Fluss in one second</h3>
    <p class="muted">
      When a word comes out wrong, press <b>{app.status?.hotkeys.add_rule ?? "Ctrl+Win+R"}</b> right away. Fluss already knows what you just said — type the right spelling,
      press Enter, done. It even fixes the text you just dictated.
    </p>
    <button class="btn" onclick={() => (sheet = { rule: null })}><Icon name="plus" size={15} />Add your first word</button>
  </div>
{:else if loaded && shown.length === 0}
  <div class="none" in:fade>
    <p class="muted">Nothing matches “{query}”.</p>
    <button class="btn" onclick={() => (sheet = { rule: null })}><Icon name="plus" size={15} />Create “{query}”</button>
  </div>
{:else}
  <div class="list card">
    {#each shown as h (h.rule.target)}
      <div class="rule" animate:flip={{ duration: 260 }} in:fly={{ y: 6, duration: 220 }} out:fade={{ duration: 120 }}>
        <div class="target">
          {#each mark(h.rule.target, query) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}
        </div>
        <div class="vars">
          {#if h.rule.variants.length === 0}
            <span class="faint vocab"><Icon name="sparkles" size={13} />Spelling only</span>
          {/if}
          {#each h.rule.variants as v}
            <span class="var" class:hit={h.matched_variant === v}>
              {#each mark(v, query) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}
              <button aria-label="Remove variant" onclick={() => removeVariant(h.rule, v)}><Icon name="x" size={11} stroke={2.3} /></button>
            </span>
          {/each}
        </div>
        <div class="uses" title="Times used">{h.rule.uses}×</div>
        <div class="acts">
          <button class="btn ghost ic" aria-label="Edit" onclick={() => (sheet = { rule: h.rule })}><Icon name="edit" size={15} /></button>
          <button class="btn ghost ic danger" aria-label="Delete" onclick={() => remove(h.rule)}><Icon name="trash" size={15} /></button>
        </div>
      </div>
    {/each}
  </div>
{/if}

{#if sheet}
  <RuleSheet rule={sheet.rule} target={sheet.rule ? "" : query} onclose={() => (sheet = null)} />
{/if}

<style>
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-bottom: 14px;
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 42px;
    padding: 0 12px 0 14px;
    border-radius: 12px;
    border: 1px solid var(--border-2);
    background: var(--surface);
    color: var(--text-3);
    transition: border-color var(--t-fast), box-shadow var(--t-fast);
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(45, 212, 191, 0.16);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font-size: 14px;
  }
  .clear {
    border: 0;
    background: transparent;
    color: var(--text-3);
    display: grid;
  }
  .hint {
    display: flex;
    gap: 3px;
  }
  kbd {
    font-family: var(--font);
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--border-2);
    background: var(--surface-2);
    color: var(--text-3);
  }
  .sorts {
    display: flex;
    padding: 3px;
    border-radius: 11px;
    border: 1px solid var(--border-2);
    background: var(--surface);
  }
  .sorts button {
    border: 0;
    height: 34px;
    padding: 0 12px;
    border-radius: 8px;
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
    font-weight: 550;
  }
  .sorts button.on {
    background: var(--surface-3);
    color: var(--text);
  }
  .list {
    display: flex;
    flex-direction: column;
    padding: 4px;
  }
  .rule {
    display: grid;
    grid-template-columns: minmax(140px, 200px) 1fr auto auto;
    align-items: center;
    gap: 14px;
    min-height: 50px;
    padding: 8px 8px 8px 14px;
    border-radius: 11px;
    transition: background var(--t-fast);
  }
  .rule + .rule {
    box-shadow: 0 -1px 0 var(--border);
  }
  .rule:hover {
    background: var(--surface-2);
    box-shadow: none;
  }
  .rule:hover + .rule {
    box-shadow: none;
  }
  .target {
    font-weight: 650;
    font-size: 14.5px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .vars {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
  }
  .var {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 6px 0 9px;
    border-radius: 7px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    color: var(--text-2);
    font-size: 12.5px;
  }

  .var.hit {
    border-color: rgba(45, 212, 191, 0.4);
  }
  .var button {
    border: 0;
    background: transparent;
    color: var(--text-3);
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border-radius: 5px;
    width: 0;
    overflow: hidden;
    opacity: 0;
    transition: opacity var(--t-fast), width var(--t-fast);
  }
  .rule:hover .var button {
    opacity: 1;
    width: 18px;
  }
  .var button:hover {
    color: var(--rose);
    background: var(--surface-3);
  }
  .vocab {
    display: inline-flex;
    gap: 5px;
    align-items: center;
    font-size: 12.5px;
  }
  mark {
    background: rgba(45, 212, 191, 0.28);
    color: inherit;
    border-radius: 3px;
  }
  .uses {
    font-size: 12px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    min-width: 34px;
    text-align: right;
  }
  .acts {
    display: flex;
    opacity: 0.35;
    transition: opacity var(--t-fast);
  }
  .rule:hover .acts {
    opacity: 1;
  }
  .ic {
    width: 32px;
    padding: 0;
    justify-content: center;
  }
  .empty {
    padding: 34px;
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 10px;
  }
  .empty p {
    max-width: 460px;
    margin: 0 0 8px;
  }
  .demo {
    display: grid;
    gap: 8px;
    justify-items: center;
    padding: 18px 26px;
    margin-bottom: 10px;
    border-radius: 16px;
    background: var(--flow-soft);
    border: 1px solid var(--border);
  }
  .demo s {
    color: var(--rose);
  }
  .fix {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
  }
  .fix :global(svg:last-child) {
    color: var(--green);
  }
  .none {
    padding: 40px;
    display: grid;
    justify-items: center;
    gap: 8px;
  }
</style>
