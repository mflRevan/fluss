<script lang="ts">
  import HotkeyRecorder from "../components/HotkeyRecorder.svelte";
  import Keys from "../components/Keys.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg } from "../state.svelte";

  const c = $derived(app.config!);
</script>

<PageHeader index="03" title="Shortcuts" subtitle="They work in every app and every text field. Click Change, then press the keys you want." />

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Bindings</span>
      <Setting title="Dictate" hint="Hold while you speak. Release to type.">
        <HotkeyRecorder path="hotkeys.push_to_talk" value={c.hotkeys.push_to_talk} />
      </Setting>
      <Setting title="Hands-free" hint="Listens until you press Dictate again.">
        <HotkeyRecorder path="hotkeys.hands_free" value={c.hotkeys.hands_free} />
      </Setting>
      <Setting title="Fix a word" hint="Opens the correction box, pre-filled.">
        <HotkeyRecorder path="hotkeys.add_rule" value={c.hotkeys.add_rule} />
      </Setting>
      <Setting title="Cancel" hint="Discards the dictation in progress.">
        <HotkeyRecorder path="hotkeys.cancel" value={c.hotkeys.cancel} />
      </Setting>
    </div>
  </div>

  <div class="col">
    <div class="group">
      <span class="label">Hands-free</span>
      <Setting title="Double-tap to lock" hint="Tap Dictate twice to start hands-free.">
        <Toggle checked={c.hotkeys.double_tap_lock} onchange={(v) => setCfg("hotkeys.double_tap_lock", v)} />
      </Setting>
      <Setting title="Double-tap window" hint="How fast the second tap must follow." disabled={!c.hotkeys.double_tap_lock}>
        <Slider value={c.hotkeys.double_tap_ms} min={150} max={600} step={10} unit=" ms" width={170} onchange={(v) => setCfg("hotkeys.double_tap_ms", v)} />
      </Setting>
    </div>

    <div class="group">
      <span class="label">Reference</span>
      <dl class="ref">
        <dt><Keys combo={c.hotkeys.push_to_talk} size="sm" /></dt>
        <dd>hold to dictate</dd>
        <dt><Keys combo={`${c.hotkeys.push_to_talk}+Space`} size="sm" /></dt>
        <dd>while holding: lock hands-free</dd>
        <dt><Keys combo={`${c.hotkeys.push_to_talk}+R`} size="sm" /></dt>
        <dd>while holding: fix what you just said</dd>
        <dt><Keys combo="Escape" size="sm" /></dt>
        <dd>while listening: cancel</dd>
      </dl>
    </div>
  </div>
</div>

<style>
  .cols {
    grid-template-columns: 1.4fr 1fr;
  }
  .ref {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 12px 18px;
    margin: 16px 0 0;
  }
  dt {
    margin: 0;
  }
  dd {
    margin: 0;
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-2);
    letter-spacing: 0.02em;
  }
</style>
