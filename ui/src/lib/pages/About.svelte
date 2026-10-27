<script lang="ts">
  import { invoke } from "../backend";
  import Iris from "../components/Iris.svelte";
  import { app } from "../state.svelte";

  const open = (url: string) => invoke("open_path", { path: url });
</script>

<div class="about">
  <div class="hero">
    <div class="txt">
      <div class="label"><b>09</b> — About</div>
      <h1 class="display">Fluss</h1>
      <p class="tag">Speak, and it flows into any app.<br />Local, fast, private.</p>
      <div class="meta mono">
        <span>v{app.status?.version ?? "—"}</span>
        <span>MIT</span>
        <span>German for “river”</span>
      </div>
      <button class="btn" onclick={() => open("https://github.com/mflRevan/fluss")}>github.com/mflRevan/fluss ↗</button>
    </div>
    <Iris size={250} />
  </div>

  <div class="credits">
    <div class="label">Credits</div>
    <dl>
      <dt>Phonon 2.5</dt>
      <dd>Fermion Research · weights CC-BY-4.0 · runtime Apache-2.0</dd>
      <dt>Parakeet TDT 0.6B v3</dt>
      <dd>NVIDIA · CC-BY-4.0 · ONNX export by istupakov</dd>
      <dt>Licht Serif</dt>
      <dd>zon.sh/licht · used with permission</dd>
      <dt>Geist, Geist Mono</dt>
      <dd>Vercel · SIL Open Font License</dd>
    </dl>
  </div>
</div>

<style>
  .about {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 0;
  }
  .hero {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 40px;
    padding-bottom: 12px;
  }
  h1 {
    font-size: 104px;
    margin-top: 14px;
    line-height: 0.95;
  }
  .tag {
    margin-top: 16px;
    font-size: 15px;
    color: var(--text-2);
  }
  .meta {
    display: flex;
    gap: 18px;
    margin: 22px 0 20px;
    font-size: 11px;
    color: var(--gold-dim);
    letter-spacing: 0.06em;
  }
  .credits {
    padding-top: 18px;
  }
  dl {
    display: grid;
    grid-template-columns: 220px 1fr;
    margin: 12px 0 0;
  }
  dt,
  dd {
    margin: 0;
    padding: 9px 0;
    border-bottom: 1px solid var(--rule);
    font-size: 13px;
  }
  dd {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--text-3);
    display: flex;
    align-items: center;
  }
</style>
