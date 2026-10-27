<script lang="ts">
  import { invoke } from "../backend";
  import FlowField from "../components/FlowField.svelte";
  import Icon from "../components/Icon.svelte";
  import Logo from "../components/Logo.svelte";
  import { app } from "../state.svelte";

  const open = (url: string) => invoke("open_path", { path: url });
</script>

<div class="about card">
  <FlowField energy={0.2} opacity={0.8} />
  <div class="in">
    <Logo size={72} />
    <h1>Fluss</h1>
    <p class="muted">Speak, and it flows into any app. Local, fast, private.</p>
    <span class="badge">Version {app.status?.version ?? "—"}</span>
    <button class="btn" onclick={() => open("https://github.com/mflRevan/fluss")}><Icon name="link" size={15} />github.com/mflRevan/fluss</button>
  </div>
</div>

<div class="section-title">Speech models</div>
<div class="card credits">
  <div class="cr">
    <b>Phonon 2.5</b>
    <span class="muted">by Fermion Research · weights under CC-BY-4.0, runtime under Apache-2.0</span>
  </div>
  <div class="cr">
    <b>Parakeet TDT 0.6B v3</b>
    <span class="muted">by NVIDIA · CC-BY-4.0 · ONNX export by istupakov</span>
  </div>
  <div class="cr">
    <b>Inter</b>
    <span class="muted">typeface by Rasmus Andersson · SIL Open Font License</span>
  </div>
</div>

<p class="faint foot">Fluss is open source under the MIT license. “Fluss” is German for river.</p>

<style>
  .about {
    position: relative;
    overflow: hidden;
    border-radius: 22px;
    min-height: 300px;
    display: grid;
    place-items: center;
  }
  .in {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    padding: 32px;
  }
  h1 {
    font-size: 40px;
    font-weight: 750;
    letter-spacing: -0.045em;
  }
  p {
    margin: 0;
  }
  .credits {
    padding: 4px 18px;
  }
  .cr {
    display: flex;
    flex-direction: column;
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
    gap: 2px;
  }
  .cr:last-child {
    border-bottom: 0;
  }
  .foot {
    text-align: center;
    font-size: 12px;
    margin-top: 24px;
  }
</style>
