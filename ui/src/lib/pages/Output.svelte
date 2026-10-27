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

<PageHeader title="Typing" subtitle="How the text gets into your apps." />

<div class="card">
  <Setting icon="keyboard" title="Insert text by" hint="Typing works almost everywhere and leaves your clipboard alone. Pasting is better for terminals and remote desktops.">
    <Segmented
      value={c.output.method}
      options={[
        { value: "type", label: "Typing" },
        { value: "paste", label: "Pasting" },
      ]}
      onchange={(v) => setCfg("output.method", v)}
    />
  </Setting>
  <Setting icon="output" title="“New line” presses" hint="Shift+Enter makes a line break in chat apps instead of sending the message.">
    <Segmented
      value={c.output.newline}
      options={[
        { value: "shift-enter", label: "Shift+Enter" },
        { value: "enter", label: "Enter" },
      ]}
      onchange={(v) => setCfg("output.newline", v)}
    />
  </Setting>
</div>

<div class="section-title">Continuing a thought</div>
<div class="card">
  <Setting
    icon="sparkles"
    title="Treat follow-ups as one text"
    hint="If you dictate again into the same field without clicking or typing in between, Fluss continues your sentence — fixing a period it added too early."
  >
    <Toggle checked={c.output.continuation} onchange={(v) => setCfg("output.continuation", v)} />
  </Setting>
  {#if c.output.continuation}
    <div transition:slide={{ duration: 220 }}>
      <Setting title="Within" hint="How long after the last dictation a new one still continues it.">
        <Slider value={c.output.continuation_window_s} min={10} max={3600} step={10} format={fmtWindow} onchange={(v) => setCfg("output.continuation_window_s", v)} />
      </Setting>
      <Setting title="Re-check the seam" hint="Listens to the end of the previous part again so punctuation between the two is exactly right. Costs a few milliseconds.">
        <Toggle checked={c.output.seam_repair} onchange={(v) => setCfg("output.seam_repair", v)} />
      </Setting>
    </div>
  {/if}
  <Setting title="Smart spacing" hint="Adds the space between your previous text and the new words.">
    <Toggle checked={c.output.smart_spacing} onchange={(v) => setCfg("output.smart_spacing", v)} />
  </Setting>
</div>
