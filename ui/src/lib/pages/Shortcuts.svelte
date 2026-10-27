<script lang="ts">
  import HotkeyRecorder from "../components/HotkeyRecorder.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg } from "../state.svelte";

  const c = $derived(app.config!);
</script>

<PageHeader title="Shortcuts" subtitle="They work everywhere — in any app, any text field. Click Change and press the keys you want." />

<div class="card">
  <Setting icon="mic" title="Dictate" hint="Hold while you speak. Release and the text appears where your cursor is.">
    <HotkeyRecorder path="hotkeys.push_to_talk" value={c.hotkeys.push_to_talk} />
  </Setting>
  <Setting icon="hand" title="Hands-free" hint="Keeps listening without holding anything. Press Dictate again to stop. Also works by tapping Space while holding Dictate.">
    <HotkeyRecorder path="hotkeys.hands_free" value={c.hotkeys.hands_free} />
  </Setting>
  <Setting icon="sparkles" title="Fix a word" hint="Opens a tiny box already filled with what you just said — type the right spelling and press Enter.">
    <HotkeyRecorder path="hotkeys.add_rule" value={c.hotkeys.add_rule} />
  </Setting>
  <Setting icon="x" title="Cancel" hint="Throws away the dictation in progress. Only active while Fluss is listening.">
    <HotkeyRecorder path="hotkeys.cancel" value={c.hotkeys.cancel} />
  </Setting>
</div>

<div class="section-title">Hands-free</div>
<div class="card">
  <Setting title="Double-tap to lock" hint="Tap Dictate twice quickly to start hands-free mode.">
    <Toggle checked={c.hotkeys.double_tap_lock} onchange={(v) => setCfg("hotkeys.double_tap_lock", v)} />
  </Setting>
  <Setting title="Double-tap speed" hint="How quickly the two taps must follow each other." disabled={!c.hotkeys.double_tap_lock}>
    <Slider value={c.hotkeys.double_tap_ms} min={150} max={600} step={10} unit=" ms" onchange={(v) => setCfg("hotkeys.double_tap_ms", v)} />
  </Setting>
</div>
