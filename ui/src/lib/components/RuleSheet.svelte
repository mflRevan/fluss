<script lang="ts">
  // Modal editor for one rule (add or edit). Used by Dictionary and Home.
  import { onMount } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { call } from "../backend";
  import { app, toast } from "../state.svelte";
  import type { Rule, UpsertOutcome } from "../types";
  import ChipInput from "./ChipInput.svelte";
  import Icon from "./Icon.svelte";

  let {
    rule = null,
    heard = "",
    target: initialTarget = "",
    onclose,
  }: { rule?: Rule | null; heard?: string; target?: string; onclose: () => void } = $props();

  let target = $state("");
  let chips = $state<string[]>([]);
  let draft = $state("");
  let targetInput: HTMLInputElement;
  let saving = $state(false);
  let existing = $state<Rule | null>(null);

  onMount(() => {
    target = rule?.target ?? initialTarget;
    chips = rule ? [...rule.variants] : heard ? [heard] : [];
    setTimeout(() => targetInput?.focus(), 30);
  });

  // When adding, show if the target already exists so the user knows it merges.
  $effect(() => {
    const t = target.trim();
    if (rule || !t) {
      existing = null;
      return;
    }
    const ctl = { stop: false };
    call("rules.list", { query: t, limit: 5 }).then((hits: any[]) => {
      if (ctl.stop) return;
      existing = hits.find((h) => h.rule.target.toLowerCase() === t.toLowerCase())?.rule ?? null;
    });
    return () => (ctl.stop = true);
  });

  async function save() {
    const t = target.trim();
    const vars = [...chips, ...(draft.trim() ? [draft.trim()] : [])];
    if (!t) {
      toast("Type what it should be", "error");
      targetInput.focus();
      return;
    }
    saving = true;
    try {
      if (rule) {
        await call("rules.replace", { old_target: rule.target, rule: { ...rule, target: t, variants: vars } });
        toast(`Updated ${t}`, "success");
      } else {
        const out = await call<UpsertOutcome>("rules.upsert", { target: t, variants: vars, apply_last: false });
        const moved = out.moved.length ? ` (moved ${out.moved.map((m) => `“${m[0]}”`).join(", ")})` : "";
        toast(`${out.created ? "Added" : "Updated"} ${t}${moved}`, "success");
      }
      app.rulesVersion++;
      onclose();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    } finally {
      saving = false;
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) save();
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim" transition:fade={{ duration: 160 }} onclick={onclose} role="presentation"></div>
<div class="sheet" role="dialog" aria-modal="true" transition:scale={{ start: 0.95, duration: 220 }}>
  <div class="head">
    <h2>{rule ? "Edit rule" : "New rule"}</h2>
    <button class="btn ghost icon" aria-label="Close" onclick={onclose}><Icon name="x" size={16} /></button>
  </div>

  <label class="lbl" for="rs-target">Should be</label>
  <input
    id="rs-target"
    class="input big"
    bind:this={targetInput}
    bind:value={target}
    placeholder="e.g. Kubernetes"
    onkeydown={(e) => e.key === "Enter" && !e.ctrlKey && (e.preventDefault(), save())}
  />
  {#if existing}
    <div class="merge" transition:fade={{ duration: 120 }}>
      <Icon name="sparkles" size={14} /> Adds to your existing rule — already heard as
      {existing.variants.length ? existing.variants.map((v) => `“${v}”`).join(", ") : "nothing yet"}
    </div>
  {/if}

  <label class="lbl" for="rs-heard">When Fluss hears <span class="faint">(optional — leave empty to just teach the spelling)</span></label>
  <ChipInput bind:chips bind:draft placeholder="What it wrongly types, e.g. cooper netties" />

  <div class="foot">
    <span class="faint tip"><kbd>Enter</kbd> adds a variant · <kbd>Ctrl</kbd>+<kbd>Enter</kbd> saves</span>
    <button class="btn" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={saving} onclick={save}>
      <Icon name="check" size={15} stroke={2.2} />{rule ? "Save" : "Add rule"}
    </button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    backdrop-filter: blur(6px);
    z-index: 70;
  }
  .sheet {
    position: fixed;
    z-index: 71;
    top: 50%;
    left: 50%;
    width: min(520px, calc(100vw - 48px));
    translate: -50% -50%;
    padding: 22px;
    border-radius: 20px;
    background: var(--surface-solid);
    box-shadow: var(--shadow-pop);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  h2 {
    font-size: 18px;
  }
  .icon {
    width: 32px;
    padding: 0;
    justify-content: center;
  }
  .lbl {
    display: block;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
    margin: 16px 0 7px;
  }
  .big {
    height: 44px;
    font-size: 16px;
    font-weight: 560;
  }
  .merge {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12.5px;
    color: var(--accent-text);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 22px;
  }
  .tip {
    flex: 1;
    font-size: 12px;
  }
  kbd {
    font-family: var(--font);
    font-size: 11px;
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid var(--border-2);
    background: var(--surface-2);
  }
</style>
