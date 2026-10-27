<script lang="ts">
  import { call } from "../backend";
  import LevelMeter from "../components/LevelMeter.svelte";
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

  // Timeline illustration: pre-roll | hold | post-roll, proportional.
  const total = $derived(c.audio.preroll_ms + 2000 + c.audio.postroll_ms);
  const pre = $derived((c.audio.preroll_ms / total) * 100);
  const post = $derived((c.audio.postroll_ms / total) * 100);
</script>

<PageHeader title="Microphone" subtitle="Which microphone Fluss listens to, and how it catches the start and end of what you say." />

<div class="card">
  <Setting icon="mic" title="Microphone" hint={app.status?.mic.error ?? "Fluss follows your system default unless you pick one."}>
    <Select value={c.audio.device} {options} onchange={(v) => setCfg("audio.device", v)} width={300} />
  </Setting>
  <Setting title="Input level" hint="Talk normally — the bars should move clearly.">
    {#snippet below()}
      <LevelMeter bars={48} height={40} />
    {/snippet}
  </Setting>
  <Setting title="Boost" hint="Raise this if you speak quietly or sit far from the mic.">
    <Slider value={c.audio.gain} min={0.5} max={4} step={0.1} format={(v) => `${Number(v).toFixed(1)}×`} onchange={(v) => setCfg("audio.gain", v)} />
  </Setting>
</div>

<div class="section-title">Catching every word</div>
<div class="card">
  <div class="timeline">
    <div class="seg pre" style="width:{pre}%"><span>{c.audio.preroll_ms} ms before</span></div>
    <div class="seg hold"><span>while you hold the shortcut</span></div>
    <div class="seg post" style="width:{post}%"><span>{c.audio.postroll_ms} ms after</span></div>
  </div>
  <Setting title="Listen before you press" hint="Keeps the last moment of audio, so a word you start a little early isn't cut off." disabled={!c.audio.keep_mic_open}>
    <Slider value={c.audio.preroll_ms} min={0} max={1500} step={50} unit=" ms" onchange={(v) => setCfg("audio.preroll_ms", v)} />
  </Setting>
  <Setting title="Keep listening after you release" hint="Catches the last syllable if you let go a bit early.">
    <Slider value={c.audio.postroll_ms} min={0} max={1500} step={50} unit=" ms" onchange={(v) => setCfg("audio.postroll_ms", v)} />
  </Setting>
  <Setting title="Keep the microphone ready" hint="Needed for “listen before you press” and makes starting instant. Windows shows the mic as in use.">
    <Toggle checked={c.audio.keep_mic_open} onchange={(v) => setCfg("audio.keep_mic_open", v)} />
  </Setting>
</div>

<div class="section-title">Pauses</div>
<div class="card">
  <Setting title="Pause before typing" hint="When you pause this long, the sentence so far is written out. Shorter feels snappier, longer avoids splitting sentences.">
    <Slider value={c.audio.pause_ms} min={300} max={2000} step={50} unit=" ms" onchange={(v) => setCfg("audio.pause_ms", v)} />
  </Setting>
  <Setting title="Live preview speed" hint="How often the preview updates while you talk.">
    <Slider value={c.audio.preview_interval_ms} min={200} max={1500} step={50} unit=" ms" onchange={(v) => setCfg("audio.preview_interval_ms", v)} />
  </Setting>
</div>

<style>
  .timeline {
    display: flex;
    height: 34px;
    margin: 16px 18px 4px;
    border-radius: 10px;
    overflow: hidden;
    font-size: 11.5px;
    font-weight: 600;
  }
  .seg {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    transition: width 400ms var(--ease);
  }
  .seg span {
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 0 6px;
  }
  .pre {
    background: repeating-linear-gradient(-45deg, rgba(45, 212, 191, 0.25) 0 6px, rgba(45, 212, 191, 0.12) 6px 12px);
    color: var(--accent-text);
  }
  .hold {
    flex: 1;
    background: var(--flow);
    color: #031615;
  }
  .post {
    background: repeating-linear-gradient(-45deg, rgba(139, 124, 246, 0.28) 0 6px, rgba(139, 124, 246, 0.12) 6px 12px);
    color: var(--violet);
  }
</style>
