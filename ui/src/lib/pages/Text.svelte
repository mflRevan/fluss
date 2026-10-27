<script lang="ts">
  import { fade } from "svelte/transition";
  import { call } from "../backend";
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

<PageHeader index="06" title="Text" subtitle="How Fluss cleans up what you say before it types it." />

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Cleanup</span>
      <Setting title="Spoken punctuation" hint="“comma”, “period”, “question mark”, “new line”, “open paren”… become symbols.">
        <Toggle checked={c.text.spoken_punctuation} onchange={(v) => setCfg("text.spoken_punctuation", v)} />
      </Setting>
      <Setting title="Remove filler words" hint="Drops um, uh and erm.">
        <Toggle checked={c.text.remove_fillers} onchange={(v) => setCfg("text.remove_fillers", v)} />
      </Setting>
      <Setting title="Apply my dictionary" hint="Your corrections from the Dictionary.">
        <Toggle checked={c.text.apply_rules} onchange={(v) => setCfg("text.apply_rules", v)} />
      </Setting>
    </div>
    <div class="group">
      <span class="label">Style</span>
      <Setting title="Capitalization" hint="Lowercase keeps acronyms and dictionary words as they are.">
        <Segmented
          value={c.text.casing}
          options={[
            { value: "natural", label: "Normal" },
            { value: "lowercase", label: "lower" },
          ]}
          onchange={(v) => setCfg("text.casing", v)}
        />
      </Setting>
      <Setting title="No period at the end" hint="Chat style. Question and exclamation marks stay.">
        <Toggle checked={c.text.drop_final_period} onchange={(v) => setCfg("text.drop_final_period", v)} />
      </Setting>
    </div>
  </div>

  <div class="col try">
    <span class="label">Live test</span>
    <div class="io">
      <div class="lbl label">In — what you say</div>
      <textarea bind:value={sample} spellcheck="false"></textarea>
    </div>
    <div class="io out-io">
      <div class="lbl label"><b>Out</b> — what Fluss types</div>
      {#key out}
        <div class="out display" in:fade={{ duration: 90 }}>{out}</div>
      {/key}
    </div>
  </div>
</div>

<style>
  .cols {
    flex: 1;
  }
  .try {
    gap: 0;
  }
  .try > .label {
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule);
  }
  .io {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    padding-top: 14px;
  }
  .lbl {
    margin-bottom: 8px;
  }
  textarea {
    flex: 1;
    resize: none;
    padding: 12px 14px;
    border: 1px solid var(--rule);
    background: var(--plate);
    outline: none;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-2);
    transition: border-color var(--t1);
  }
  textarea:focus {
    border-color: var(--gold);
  }
  .out-io {
    padding-bottom: 2px;
  }
  .out {
    flex: 1;
    padding: 12px 14px;
    border: 1px solid var(--gold-dim);
    border-left-width: 2px;
    border-left-color: var(--gold);
    white-space: pre-wrap;
    font-size: 22px;
    line-height: 1.3;
    font-weight: 300;
    user-select: text;
    overflow: auto;
  }
</style>