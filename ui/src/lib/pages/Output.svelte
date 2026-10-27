<script lang="ts">
  import { slide } from "svelte/transition";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg } from "../state.svelte";

  const c = $derived(app.config!);
  const fmtWindow = (s: number) => (s >= 3600 ? `${(s / 3600).toFixed(1)} h` : s >= 60 ? `${Math.round(s / 60)} min` : `${s} s`);
</script>

<PageHeader index="07" title="Typing" subtitle="How the words get into your apps, and how follow-up dictations join the text that's already there." />

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Insertion</span>
      <Setting title="Insert text by" hint="Typing leaves your clipboard alone. Pasting suits terminals and remote desktops." stack>
        <Segmented
          value={c.output.method}
          options={[
            { value: "type", label: "Typing" },
            { value: "paste", label: "Pasting" },
          ]}
          onchange={(v) => setCfg("output.method", v)}
        />
      </Setting>
      <Setting title="“New line” presses" hint="Shift+Enter breaks the line in chat apps instead of sending." stack>
        <Segmented
          value={c.output.newline}
          options={[
            { value: "shift-enter", label: "Shift+Enter" },
            { value: "enter", label: "Enter" },
          ]}
          onchange={(v) => setCfg("output.newline", v)}
        />
      </Setting>
      <Setting title="Smart spacing" hint="Adds the space between earlier text and new words.">
        <Toggle checked={c.output.smart_spacing} onchange={(v) => setCfg("output.smart_spacing", v)} />
      </Setting>
    </div>
  </div>

  <div class="col">
    <div class="group">
      <span class="label">Continuation</span>
      <Setting title="Treat follow-ups as one text" hint="Dictate again into the same field without touching it, and Fluss continues your sentence — fixing a period it added too early.">
        <Toggle checked={c.output.continuation} onchange={(v) => setCfg("output.continuation", v)} />
      </Setting>
      {#if c.output.continuation}
        <div transition:slide={{ duration: 140 }}>
          <Setting title="Within" hint="How long a follow-up still counts.">
            <Slider value={c.output.continuation_window_s} min={10} max={3600} step={10} format={fmtWindow} width={170} onchange={(v) => setCfg("output.continuation_window_s", v)} />
          </Setting>
          <Setting title="Re-check the seam" hint="Re-reads the end of the last part with the new one, so punctuation between them is right. A few milliseconds.">
            <Toggle checked={c.output.seam_repair} onchange={(v) => setCfg("output.seam_repair", v)} />
          </Setting>
        </div>
      {/if}
    </div>

    <div class="seam">
      <div class="label">Example</div>
      <p class="mono"><span class="faint">…the tests look good</span><s>.</s><span class="gold">,</span> <span class="gold">and then we deploy on Friday.</span></p>
    </div>
  </div>
</div>

<style>
  .seam {
    margin-top: 22px;
    padding: 14px 16px;
    border-left: 1px solid var(--gold-dim);
  }
  .seam p {
    margin-top: 8px;
    font-size: 12px;
    line-height: 1.6;
  }
  s {
    color: var(--danger);
  }
</style>
