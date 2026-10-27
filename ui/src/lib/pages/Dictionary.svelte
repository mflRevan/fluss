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

<PageHeader index="02" title="Dictionary" subtitle="Words Fluss should know, and the corrections it applies — what it hears, mapped to what you meant.">
  {#snippet actions()}
    <button class="btn primary" onclick={() => (sheet = { rule: null })}>+ New rule</button>
  {/snippet}
</PageHeader>

<div class="bar">
  <div class="search">
    <Icon name="search" size={15} />
    <input bind:this={searchEl} bind:value={query} placeholder="Search {total} rules" />
    {#if query}
      <button class="clear label" onclick={() => (query = "")}>clear</button>
    {:else}
      <Keys combo="Ctrl+F" size="sm" />
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
  <div class="empty" in:fade={{ duration: 120 }}>
    <div class="demo mono">
      <div><span class="faint">you said</span>&nbsp;&nbsp;omarchy</div>
      <div><span class="faint">it typed</span>&nbsp;&nbsp;<s>on March 9</s></div>
      <div class="fix"><Keys combo={app.status?.hotkeys.add_rule ?? "Ctrl+Win+R"} size="sm" /> <span class="faint">→</span> <span class="gold">Omarchy</span></div>
    </div>
    <div class="etxt">
      <h3 class="display">Teach Fluss in one second.</h3>
      <p class="muted">
        When a word comes out wrong, press <b>{app.status?.hotkeys.add_rule ?? "Ctrl+Win+R"}</b> right away. Fluss already knows what you just said — type the right spelling, press Enter.
        It even fixes the text you just dictated.
      </p>
      <button class="btn" onclick={() => (sheet = { rule: null })}>Add your first word</button>
    </div>
  </div>
{:else if loaded && shown.length === 0}
  <div class="none" in:fade={{ duration: 120 }}>
    <p class="muted">Nothing matches “{query}”.</p>
    <button class="btn" onclick={() => (sheet = { rule: null })}>Create “{query}”</button>
  </div>
{:else}
  <div class="thead label">
    <span>Writes</span>
    <span>When it hears</span>
    <span class="r">Used</span>
    <span></span>
  </div>
  <div class="list">
    {#each shown as h (h.rule.target)}
      <div class="rule" animate:flip={{ duration: 140 }} in:fly={{ y: 4, duration: 140 }} out:fade={{ duration: 80 }}>
        <div class="target">
          {#each mark(h.rule.target, query) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}
        </div>
        <div class="vars">
          {#if h.rule.variants.length === 0}
            <span class="vocab mono">spelling only</span>
          {/if}
          {#each h.rule.variants as v}
            <span class="var mono" class:hit={h.matched_variant === v}>
              {#each mark(v, query) as p}{#if p.m}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}
              <button aria-label="Remove variant" onclick={() => removeVariant(h.rule, v)}>×</button>
            </span>
          {/each}
        </div>
        <div class="uses mono">{h.rule.uses}</div>
        <div class="acts">
          <button class="btn ghost sm" onclick={() => (sheet = { rule: h.rule })}>Edit</button>
          <button class="btn ghost sm danger" onclick={() => remove(h.rule)}>Delete</button>
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
    gap: 12px;
    align-items: center;
    margin-bottom: 14px;
    flex: none;
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 38px;
    padding: 0 10px 0 12px;
    border: 1px solid var(--rule-strong);
    background: var(--plate);
    color: var(--gold-dim);
    transition: border-color var(--t1);
  }
  .search:focus-within {
    border-color: var(--gold);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font-size: 13.5px;
  }
  .search input::placeholder {
    color: var(--text-3);
  }
  .clear {
    border: 0;
    background: transparent;
  }
  .clear:hover {
    color: var(--gold);
  }
  .sorts {
    display: flex;
    border: 1px solid var(--rule-strong);
  }
  .sorts button {
    border: 0;
    height: 36px;
    padding: 0 12px;
    background: transparent;
    font-family: var(--mono);
    font-size: 10.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .sorts button + button {
    border-left: 1px solid var(--rule);
  }
  .sorts button.on {
    background: var(--gold);
    color: var(--gold-ink);
  }
  .thead,
  .rule {
    display: grid;
    grid-template-columns: minmax(150px, 220px) 1fr 50px 140px;
    gap: 16px;
    align-items: center;
  }
  .thead {
    padding: 0 10px 8px 0;
    margin-right: -10px;
    padding-right: 10px;
    border-bottom: 1px solid var(--rule);
    flex: none;
  }
  .r {
    text-align: right;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    margin-right: -10px;
    padding-right: 10px;
  }
  .rule {
    min-height: 46px;
    padding: 6px 0;
    border-bottom: 1px solid var(--rule);
    transition: background var(--t1);
  }
  .rule:hover {
    background: var(--plate);
  }
  .target {
    font-size: 14.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rule:hover .target {
    color: var(--gold);
  }
  .vars {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    min-width: 0;
  }
  .var {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 22px;
    padding: 0 4px 0 7px;
    border: 1px solid var(--rule-strong);
    color: var(--text-2);
    font-size: 11.5px;
  }
  .var.hit {
    border-color: var(--gold);
  }
  .var button {
    border: 0;
    background: transparent;
    color: var(--text-3);
    width: 0;
    overflow: hidden;
    opacity: 0;
    padding: 0;
    font-size: 13px;
    line-height: 1;
    transition: opacity var(--t1), width var(--t1);
  }
  .rule:hover .var button {
    opacity: 1;
    width: 14px;
  }
  .var button:hover {
    color: var(--danger);
  }
  .vocab {
    font-size: 11px;
    color: var(--text-3);
  }
  mark {
    background: var(--gold);
    color: var(--gold-ink);
  }
  .uses {
    font-size: 12px;
    color: var(--text-3);
    text-align: right;
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    opacity: 0;
    transition: opacity var(--t1);
  }
  .rule:hover .acts {
    opacity: 1;
  }
  .empty {
    flex: 1;
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 48px;
    padding: 8px 0 40px;
    border-top: 1px solid var(--rule);
  }
  .demo {
    display: grid;
    gap: 10px;
    padding: 22px 26px;
    border-left: 2px solid var(--gold);
    background: var(--plate);
    font-size: 13px;
  }
  .demo s {
    color: var(--danger);
  }
  .fix {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
  }
  .etxt {
    display: grid;
    gap: 14px;
    justify-items: start;
    max-width: 440px;
  }
  .etxt h3 {
    font-size: 32px;
  }
  .etxt b {
    color: var(--gold);
    font-weight: 500;
  }
  .none {
    padding: 40px 0;
    display: grid;
    justify-items: start;
    gap: 12px;
  }
</style>