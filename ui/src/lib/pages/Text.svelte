<script lang="ts">
  import { fade } from "svelte/transition";
  import { call } from "../backend";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Setting from "../components/Setting.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg } from "../state.svelte";

  const c = $derived(app.config!);

  let sample = $state("um so I think we should ship on thursday comma what do you think question mark new line thanks");
  let out = $state("");
  $effect(() => {
    // Re-run when settings or the dictionary change.
    JSON.stringify(c.text);
    app.rulesVersion;
    const t = sample;
    const ctl = { stop: false };
    const id = setTimeout(
      () =>
        call<{ text: string }>("rules.preview", { text: t }).then((r) => {
          if (!ctl.stop) out = r.text;
        }),
      120,
    );
    return () => {
      ctl.stop = true;
      clearTimeout(id);
    };
  });
</script>

<PageHeader title="Text" subtitle="How Fluss cleans up what you say before it types it." />

<div class="try card">
  <div class="col">
    <div class="lbl"><Icon name="mic" size={14} />What you say</div>
    <textarea bind:value={sample} rows="3" spellcheck="false"></textarea>
  </div>
  <div class="arrow"><Icon name="arrowRight" size={18} /></div>
  <div class="col">
    <div class="lbl"><Icon name="type" size={14} />What Fluss types</div>
    {#key out}
      <div class="out" in:fade={{ duration: 150 }}>{out}</div>
    {/key}
  </div>
</div>

<div class="section-title">Cleanup</div>
<div class="card">
  <Setting icon="quote" title="Spoken punctuation" hint="Say “comma”, “period”, “question mark”, “new line”, “new paragraph”, “open paren”… and get the symbol.">
    <Toggle checked={c.text.spoken_punctuation} onchange={(v) => setCfg("text.spoken_punctuation", v)} />
  </Setting>
  <Setting icon="wave" title="Remove filler words" hint="Drops “um”, “uh” and “erm”.">
    <Toggle checked={c.text.remove_fillers} onchange={(v) => setCfg("text.remove_fillers", v)} />
  </Setting>
  <Setting icon="book" title="Apply my dictionary" hint="Your corrections from the Dictionary page.">
    <Toggle checked={c.text.apply_rules} onchange={(v) => setCfg("text.apply_rules", v)} />
  </Setting>
</div>

<div class="section-title">Style</div>
<div class="card">
  <Setting icon="type" title="Capitalization" hint="“all lowercase” keeps acronyms like API and words from your dictionary as they are.">
    <Segmented
      value={c.text.casing}
      options={[
        { value: "natural", label: "Normal" },
        { value: "lowercase", label: "all lowercase" },
      ]}
      onchange={(v) => setCfg("text.casing", v)}
    />
  </Setting>
  <Setting title="No period at the end" hint="Chat style: drops the final period of each dictation. Question and exclamation marks stay.">
    <Toggle checked={c.text.drop_final_period} onchange={(v) => setCfg("text.drop_final_period", v)} />
  </Setting>
</div>

<style>
  .try {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 14px;
    padding: 16px;
    align-items: stretch;
    background:
      radial-gradient(400px 200px at 100% 0%, rgba(139, 124, 246, 0.1), transparent 70%),
      var(--surface);
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .lbl {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-3);
  }
  textarea {
    flex: 1;
    resize: none;
    padding: 10px 12px;
    border-radius: 10px;
    border: 1px solid var(--border-2);
    background: var(--surface);
    outline: none;
    line-height: 1.5;
  }
  textarea:focus {
    border-color: var(--accent);
  }
  .arrow {
    display: grid;
    place-items: center;
    color: var(--accent-text);
  }
  .out {
    flex: 1;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--flow-soft);
    border: 1px solid var(--border);
    white-space: pre-wrap;
    line-height: 1.5;
    user-select: text;
    min-height: 70px;
  }
</style>
