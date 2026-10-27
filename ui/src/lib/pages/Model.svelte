<script lang="ts">
  import { slide } from "svelte/transition";
  import { call } from "../backend";
  import Icon from "../components/Icon.svelte";
  import ModelCards from "../components/ModelCards.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Select from "../components/Select.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg, toast } from "../state.svelte";

  const c = $derived(app.config!);
  const st = $derived(app.status?.model);
  const custom = $derived(c.model.id === "custom");
  const multilingual = $derived(c.model.id === "phonon-2.5" || c.model.id === "parakeet-v3" || custom);

  const languages = [
    ["auto", "Detect automatically"],
    ["en", "English"],
    ["de", "Deutsch"],
    ["fr", "Français"],
    ["es", "Español"],
    ["it", "Italiano"],
    ["pt", "Português"],
    ["nl", "Nederlands"],
    ["pl", "Polski"],
    ["sv", "Svenska"],
    ["da", "Dansk"],
    ["fi", "Suomi"],
    ["cs", "Čeština"],
    ["sk", "Slovenčina"],
    ["sl", "Slovenščina"],
    ["hr", "Hrvatski"],
    ["hu", "Magyar"],
    ["ro", "Română"],
    ["bg", "Български"],
    ["el", "Ελληνικά"],
    ["et", "Eesti"],
    ["lv", "Latviešu"],
    ["lt", "Lietuvių"],
    ["mt", "Malti"],
    ["ru", "Русский"],
    ["uk", "Українська"],
  ].map(([value, label]) => ({ value, label }));

  let showKey = $state(false);
  let testing = $state(false);
  let testResult = $state<{ ok: boolean; text: string } | null>(null);

  async function test() {
    testing = true;
    testResult = null;
    try {
      const r = await call<{ ok: boolean; latency_ms: number; text: string }>("endpoint.test", {});
      testResult = { ok: true, text: `Connected in ${r.latency_ms} ms${r.text ? ` — heard “${r.text}”` : ""}` };
    } catch (e) {
      testResult = { ok: false, text: String(e instanceof Error ? e.message : e) };
    } finally {
      testing = false;
    }
  }

  async function act(m: string) {
    try {
      await call(m);
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    }
  }

  const phaseText = $derived.by(() => {
    switch (st?.phase) {
      case "ready":
        return `Loaded${st.device ? ` on ${st.device}` : ""}${st.memory_mb ? ` · ${(st.memory_mb / 1024).toFixed(1)} GB` : ""}`;
      case "loading":
        return "Loading…";
      case "installing":
        return `${st.detail || "Installing"} ${st.progress != null ? Math.round(st.progress * 100) + "%" : ""}`;
      case "unloaded":
        return "Not in memory — it loads on your next dictation";
      case "not-installed":
        return "Not installed yet";
      case "error":
        return st.error ?? "Error";
      default:
        return "";
    }
  });
</script>

<PageHeader title="Model" subtitle="The speech engine that turns your voice into text. Everything runs on this computer unless you pick a custom endpoint.">
  {#snippet actions()}
    {#if st?.phase === "ready"}
      <button class="btn" onclick={() => act("model.unload")}><Icon name="power" size={15} />Unload</button>
    {:else if st?.phase === "unloaded" || st?.phase === "error"}
      <button class="btn" onclick={() => act("model.load")}><Icon name="zap" size={15} />Load now</button>
    {/if}
  {/snippet}
</PageHeader>

<div class="now" class:err={st?.phase === "error"}>
  <span class="dot {st?.phase}"></span>
  <b>{st?.name}</b><span class="muted">{phaseText}</span>
</div>

<ModelCards />

{#if custom}
  <div transition:slide={{ duration: 260 }}>
    <div class="section-title">Custom endpoint</div>
    <div class="card form">
      <p class="muted intro">
        Works with anything that speaks the OpenAI <code>/audio/transcriptions</code> format: OpenAI, Groq, Fireworks, a local whisper.cpp or speaches
        server, or <code>fermion serve</code>.
      </p>
      <label>
        <span>Server address</span>
        <input class="input" value={c.custom.base_url} placeholder="https://api.openai.com/v1" onchange={(e) => setCfg("custom.base_url", e.currentTarget.value.trim())} />
      </label>
      <div class="two">
        <label>
          <span>API key <em class="faint">(leave empty for local servers)</em></span>
          <div class="keybox">
            <input
              class="input"
              type={showKey ? "text" : "password"}
              value={c.custom.api_key}
              placeholder="sk-…"
              onchange={(e) => setCfg("custom.api_key", e.currentTarget.value.trim())}
            />
            <button class="eye" aria-label="Show key" onclick={() => (showKey = !showKey)}><Icon name={showKey ? "eyeOff" : "eye"} size={16} /></button>
          </div>
        </label>
        <label>
          <span>Model name <em class="faint">(optional)</em></span>
          <input class="input" value={c.custom.model} placeholder="whisper-1" onchange={(e) => setCfg("custom.model", e.currentTarget.value.trim())} />
        </label>
      </div>
      <div class="testrow">
        <button class="btn" disabled={testing} onclick={test}><Icon name="link" size={15} />{testing ? "Testing…" : "Test connection"}</button>
        {#if testResult}
          <span class:ok={testResult.ok} class:bad={!testResult.ok} transition:slide={{ axis: "x" }}>{testResult.text}</span>
        {/if}
      </div>
    </div>
    <div class="card" style="margin-top:12px">
      <Setting title="Live preview" hint="Sends extra requests while you speak so you see words appear. Costs more on paid services.">
        <Toggle checked={c.custom.live_preview} onchange={(v) => setCfg("custom.live_preview", v)} />
      </Setting>
      <Setting title="Ask for word timings" hint="Helps Fluss find the exact words to fix. Turn off if your server rejects it.">
        <Toggle checked={c.custom.word_timestamps} onchange={(v) => setCfg("custom.word_timestamps", v)} />
      </Setting>
      <Setting title="Timeout" hint="Give up on a request after this long.">
        <Slider value={c.custom.timeout_s} min={5} max={120} step={5} unit=" s" onchange={(v) => setCfg("custom.timeout_s", v)} />
      </Setting>
    </div>
  </div>
{/if}

{#if !custom}
  <div class="section-title">Memory</div>
  <div class="card">
    <Setting icon="memory" title="Keep the model loaded" hint="Always loaded is instant. Unloading frees memory; the first dictation after that waits a few seconds while it loads.">
      <Segmented
        value={c.model.residency}
        options={[
          { value: "always", label: "Always" },
          { value: "idle-unload", label: "Unload when idle" },
          { value: "on-demand", label: "Only while used" },
        ]}
        onchange={(v) => setCfg("model.residency", v)}
      />
    </Setting>
    {#if c.model.residency === "idle-unload"}
      <div transition:slide={{ duration: 220 }}>
        <Setting title="Unload after" hint="Minutes without dictation before the model is put to sleep.">
          <Slider value={c.model.idle_unload_min} min={1} max={120} step={1} unit=" min" onchange={(v) => setCfg("model.idle_unload_min", v)} />
        </Setting>
      </div>
    {/if}
    <Setting icon="gpu" title="Run on" hint="Auto uses your graphics card when the model supports it, otherwise the processor.">
      <Segmented
        value={c.model.device}
        options={[
          { value: "auto", label: "Auto" },
          { value: "gpu", label: "Graphics card" },
          { value: "cpu", label: "Processor" },
        ]}
        onchange={(v) => setCfg("model.device", v)}
      />
    </Setting>
    <Setting title="Load gently" hint="Loads the model at low priority so starting your computer stays snappy.">
      <Toggle checked={c.model.load_priority === "low"} onchange={(v) => setCfg("model.load_priority", v ? "low" : "normal")} />
    </Setting>
  </div>
{/if}

<div class="section-title">Recognition</div>
<div class="card">
  {#if multilingual}
    <Setting icon="globe" title="Language" hint="Pick yours for the best accuracy, or let the model detect it.">
      <Select value={c.model.language} options={languages} onchange={(v) => setCfg("model.language", v)} width={220} />
    </Setting>
  {/if}
  <Setting icon="book" title="Use my dictionary as hints" hint="Nudges the model toward words in your dictionary, so names and jargon come out right more often.">
    <Toggle checked={c.model.hotwords} onchange={(v) => setCfg("model.hotwords", v)} />
  </Setting>
</div>

<style>
  .now {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: -6px 0 16px;
    font-size: 13px;
  }
  .now .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
  }
  .dot.ready {
    background: var(--green);
    box-shadow: 0 0 8px var(--green);
  }
  .dot.loading,
  .dot.installing {
    background: var(--amber);
  }
  .dot.error {
    background: var(--rose);
  }
  .now.err .muted {
    color: var(--rose);
  }
  .form {
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .intro {
    margin: 0;
    font-size: 13px;
  }
  code {
    font-family: var(--mono);
    font-size: 12px;
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--surface-2);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
  }
  em {
    font-style: normal;
    font-weight: 450;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .keybox {
    position: relative;
  }
  .eye {
    position: absolute;
    right: 6px;
    top: 6px;
    width: 26px;
    height: 24px;
    border: 0;
    background: transparent;
    color: var(--text-3);
    display: grid;
    place-items: center;
  }
  .testrow {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 13px;
  }
  .ok {
    color: var(--green);
  }
  .bad {
    color: var(--rose);
  }
</style>
