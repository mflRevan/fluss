<script lang="ts">
  // Modal editor for one rule (add or edit). Used by Dictionary and Home.
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { call } from "../backend";
  import { app, toast } from "../state.svelte";
  import type { Rule, UpsertOutcome } from "../types";
  import ChipInput from "./ChipInput.svelte";
  import Icon from "./Icon.svelte";
  import Keys from "./Keys.svelte";

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

<div class="scrim" transition:fade={{ duration: 110 }} onclick={onclose} role="presentation"></div>
<div class="sheet" role="dialog" aria-modal="true" transition:fly={{ y: 8, duration: 140 }}>
  <div class="head">
    <span class="label"><b>◆</b> {rule ? "Edit rule" : "New rule"}</span>
    <button class="btn ghost icon" aria-label="Close" onclick={onclose}><Icon name="x" size={14} /></button>
  </div>

  <label class="label lbl" for="rs-target">Should be</label>
  <input
    id="rs-target"
    class="big display"
    bind:this={targetInput}
    bind:value={target}
    placeholder="Kubernetes"
    onkeydown={(e) => e.key === "Enter" && !e.ctrlKey && (e.preventDefault(), save())}
  />
  {#if existing}
    <div class="merge mono" transition:fade={{ duration: 90 }}>
      ◆ merges into existing rule — heard as {existing.variants.length ? existing.variants.map((v) => `“${v}”`).join(", ") : "nothing yet"}
    </div>
  {/if}

  <label class="label lbl" for="rs-heard">When Fluss hears <span class="opt">optional — empty just teaches the spelling</span></label>
  <ChipInput bind:chips bind:draft placeholder="what it wrongly types, e.g. cooper netties" />

  <div class="foot">
    <span class="tip mono"><Keys combo="Enter" size="sm" /> adds a variant · <Keys combo="Ctrl+Enter" size="sm" /> saves</span>
    <button class="btn ghost" onclick={onclose}>Cancel</button>
    <button class="btn primary" disabled={saving} onclick={save}>{rule ? "Save" : "Add rule"}</button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    z-index: 70;
  }
  .sheet {
    --c: 14px;
    position: fixed;
    z-index: 71;
    top: 50%;
    left: 50%;
    width: min(560px, calc(100vw - 48px));
    translate: -50% -50%;
    padding: 20px 24px 22px;
    background: var(--plate);
    border-top: 1px solid var(--gold);
    clip-path: polygon(0 0, 100% 0, 100% calc(100% - var(--c)), calc(100% - var(--c)) 100%, 0 100%);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .lbl {
    display: block;
    margin: 16px 0 8px;
  }
  .opt {
    color: var(--text-3);
    letter-spacing: 0.04em;
    text-transform: none;
    margin-left: 8px;
  }
  .big {
    width: 100%;
    height: 54px;
    padding: 0 2px;
    border: 0;
    border-bottom: 1px solid var(--rule-strong);
    background: transparent;
    outline: none;
    font-size: 34px;
    color: var(--gold);
    transition: border-color var(--t1);
  }
  .big:focus {
    border-bottom-color: var(--gold);
  }
  .big::placeholder {
    color: var(--text-3);
  }
  .merge {
    margin-top: 8px;
    font-size: 11px;
    color: var(--gold);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 24px;
  }
  .tip {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    color: var(--text-3);
  }
</style>