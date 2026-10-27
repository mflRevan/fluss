<script lang="ts">
  import { call } from "../backend";
  import Iris from "../components/Iris.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Select from "../components/Select.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg } from "../state.svelte";
  import type { DeviceInfo } from "../types";

  const c = $derived(app.config!);
  let devices = $state<DeviceInfo[]>([]);
  $effect(() => {
    call<DeviceInfo[]>("devices.list").then((d) => (devices = d));
  });
  const options = $derived([
    { value: "", label: "System default", hint: devices.find((d) => d.default)?.name },
    ...devices.map((d) => ({ value: d.name, label: d.name })),
  ]);

  const db = $derived(app.level.rms > 0 ? Math.max(-60, 20 * Math.log10(app.level.rms)) : -60);

  // Timing diagram: pre-roll | hold | post-roll, proportional.
  const total = $derived(c.audio.preroll_ms + 2000 + c.audio.postroll_ms);
  const pre = $derived((c.audio.preroll_ms / total) * 100);
  const post = $derived((c.audio.postroll_ms / total) * 100);
</script>

<PageHeader index="05" title="Microphone" subtitle="What Fluss listens to, and how it catches the first and last word you say." />

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Input</span>
      <Setting title="Device" hint={app.status?.mic.error ?? "Follows your system default unless you pick one."} stack>
        <Select value={c.audio.device} {options} onchange={(v) => setCfg("audio.device", v)} width="100%" />
      </Setting>
    </div>
    <div class="monitor">
      <Iris size={176} mode="idle" reactive />
      <div class="readout">
        <div class="label">Level</div>
        <div class="db mono">{db.toFixed(0)}<small>dB</small></div>
        <div class="label hint">Talk normally — the ring should open clearly.</div>
      </div>
    </div>
    <Setting title="Boost" hint="For quiet voices or distant mics.">
      <Slider value={c.audio.gain} min={0.5} max={4} step={0.1} width={170} format={(v) => `${Number(v).toFixed(1)}×`} onchange={(v) => setCfg("audio.gain", v)} />
    </Setting>
  </div>

  <div class="col">
    <div class="group">
      <span class="label">Catching every word</span>
      <div class="timeline mono">
        <div class="seg pre" style="width:{pre}%"><span>−{c.audio.preroll_ms}</span></div>
        <div class="seg hold"><span>hold</span></div>
        <div class="seg post" style="width:{post}%"><span>+{c.audio.postroll_ms}</span></div>
      </div>
      <Setting title="Listen before you press" hint="Keeps the moment before the shortcut." disabled={!c.audio.keep_mic_open}>
        <Slider value={c.audio.preroll_ms} min={0} max={1500} step={50} unit=" ms" width={170} onchange={(v) => setCfg("audio.preroll_ms", v)} />
      </Setting>
      <Setting title="Keep listening after release" hint="Catches a last syllable.">
        <Slider value={c.audio.postroll_ms} min={0} max={1500} step={50} unit=" ms" width={170} onchange={(v) => setCfg("audio.postroll_ms", v)} />
      </Setting>
      <Setting title="Keep the microphone ready" hint="Needed for listening before you press.">
        <Toggle checked={c.audio.keep_mic_open} onchange={(v) => setCfg("audio.keep_mic_open", v)} />
      </Setting>
    </div>
    <div class="group">
      <span class="label">Pauses</span>
      <Setting title="Pause before typing" hint="A pause this long writes out the sentence.">
        <Slider value={c.audio.pause_ms} min={300} max={2000} step={50} unit=" ms" width={170} onchange={(v) => setCfg("audio.pause_ms", v)} />
      </Setting>
      <Setting title="Live preview rate" hint="How often the preview refreshes.">
        <Slider value={c.audio.preview_interval_ms} min={200} max={1500} step={50} unit=" ms" width={170} onchange={(v) => setCfg("audio.preview_interval_ms", v)} />
      </Setting>
    </div>
  </div>
</div>

<style>
  .monitor {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 18px 0;
    border-bottom: 1px solid var(--rule);
  }
  .readout {
    display: grid;
    gap: 6px;
  }
  .db {
    font-size: 34px;
    line-height: 1;
    color: var(--gold);
  }
  .db small {
    font-size: 12px;
    color: var(--text-3);
    margin-left: 4px;
  }
  .hint {
    letter-spacing: 0.08em;
    max-width: 200px;
    line-height: 1.5;
  }
  .timeline {
    display: flex;
    height: 26px;
    margin: 14px 0 4px;
    font-size: 10px;
    letter-spacing: 0.08em;
  }
  .seg {
    display: flex;
    align-items: center;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    transition: width var(--t3) var(--q);
  }
  .seg span {
    padding: 0 7px;
  }
  .pre,
  .post {
    border: 1px dashed var(--gold-dim);
    color: var(--gold);
  }
  .pre {
    border-right: 0;
  }
  .post {
    border-left: 0;
    justify-content: flex-end;
  }
  .hold {
    flex: 1;
    background: var(--gold);
    color: var(--gold-ink);
    justify-content: center;
    text-transform: uppercase;
  }
</style>
