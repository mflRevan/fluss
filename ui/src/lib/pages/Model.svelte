<script lang="ts">
  import { fade, slide } from "svelte/transition";
  import { call } from "../backend";
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
        return `loaded${st.device ? ` · ${st.device}` : ""}${st.memory_mb ? ` · ${(st.memory_mb / 1024).toFixed(1)} GB` : ""}`;
      case "loading":
        return "loading";
      case "installing":
        return `${st.detail || "Installing"} ${st.progress != null ? Math.round(st.progress * 100) + "%" : ""}`;
      case "unloaded":
        return "asleep · loads on your next dictation";
      case "not-installed":
        return "not installed";
      case "error":
        return st.error ?? "Error";
      default:
        return "";
    }
  });
</script>

<PageHeader index="04" title="Model" subtitle="The engine that turns your voice into text. It runs on this computer unless you choose a custom endpoint.">
  {#snippet actions()}
    <span class="now mono" class:err={st?.phase === "error"}>{phaseText}</span>
    {#if st?.phase === "ready"}
      <button class="btn" onclick={() => act("model.unload")}>Unload</button>
    {:else if st?.phase === "unloaded" || st?.phase === "error"}
      <button class="btn" onclick={() => act("model.load")}>Load now</button>
    {/if}
  {/snippet}
</PageHeader>

{#snippet recognition()}
  <div class="group rec">
    <span class="label">Recognition</span>
    {#if multilingual}
      <Setting title="Language" hint="Yours, or detect automatically.">
        <Select value={c.model.language} options={languages} onchange={(v) => setCfg("model.language", v)} width={190} />
      </Setting>
    {/if}
    <Setting title="Dictionary as hints" hint="Nudges the model toward your words.">
      <Toggle checked={c.model.hotwords} onchange={(v) => setCfg("model.hotwords", v)} />
    </Setting>
  </div>
{/snippet}

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Engine</span>
      <ModelCards compact={custom} />
    </div>
    {#if custom}{@render recognition()}{/if}
  </div>

  <div class="col">
    {#if custom}
      <div class="group" in:fade={{ duration: 120 }}>
        <span class="label">Custom endpoint</span>
        <p class="intro muted">Any server that speaks the OpenAI <code>/audio/transcriptions</code> format — OpenAI, Groq, whisper.cpp, speaches, <code>fermion serve</code>.</p>
        <label class="field">
          <span class="label">Server address</span>
          <input class="input mono" value={c.custom.base_url} placeholder="https://api.openai.com/v1" onchange={(e) => setCfg("custom.base_url", e.currentTarget.value.trim())} />
        </label>
        <div class="two">
          <label class="field">
            <span class="label">API key</span>
            <div class="keybox">
              <input class="input mono" type={showKey ? "text" : "password"} value={c.custom.api_key} placeholder="empty for local" onchange={(e) => setCfg("custom.api_key", e.currentTarget.value.trim())} />
              <button class="eye label" onclick={() => (showKey = !showKey)}>{showKey ? "hide" : "show"}</button>
            </div>
          </label>
          <label class="field">
            <span class="label">Model name</span>
            <input class="input mono" value={c.custom.model} placeholder="whisper-1" onchange={(e) => setCfg("custom.model", e.currentTarget.value.trim())} />
          </label>
        </div>
        <div class="testrow">
          <button class="btn sm" disabled={testing} onclick={test}>{testing ? "Testing…" : "Test connection"}</button>
          {#if testResult}
            <span class="mono res" class:ok={testResult.ok} class:bad={!testResult.ok} in:fade={{ duration: 100 }}>{testResult.text}</span>
          {/if}
        </div>
        <Setting title="Live preview" hint="Extra requests while you speak. Costs more on paid APIs.">
          <Toggle checked={c.custom.live_preview} onchange={(v) => setCfg("custom.live_preview", v)} />
        </Setting>
        <Setting title="Ask for word timings" hint="Turn off if your server rejects it.">
          <Toggle checked={c.custom.word_timestamps} onchange={(v) => setCfg("custom.word_timestamps", v)} />
        </Setting>
        <Setting title="Timeout">
          <Slider value={c.custom.timeout_s} min={5} max={120} step={5} unit=" s" width={170} onchange={(v) => setCfg("custom.timeout_s", v)} />
        </Setting>
      </div>
    {:else}
      <div class="group" in:fade={{ duration: 120 }}>
        <span class="label">Memory</span>
        <Setting title="Keep the model loaded" hint="Always is instant. Unloading frees memory; the next dictation waits while it loads." stack>
          <Segmented
            value={c.model.residency}
            options={[
              { value: "always", label: "Always" },
              { value: "idle-unload", label: "Unload idle" },
              { value: "on-demand", label: "While used" },
            ]}
            onchange={(v) => setCfg("model.residency", v)}
          />
        </Setting>
        {#if c.model.residency === "idle-unload"}
          <div transition:slide={{ duration: 140 }}>
            <Setting title="Unload after" hint="Minutes without dictation.">
              <Slider value={c.model.idle_unload_min} min={1} max={120} step={1} unit=" min" width={170} onchange={(v) => setCfg("model.idle_unload_min", v)} />
            </Setting>
          </div>
        {/if}
        <Setting title="Run on" hint="Auto uses the graphics card when the model supports it." stack>
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
        <Setting title="Load gently" hint="Low priority, so sign-in stays snappy.">
          <Toggle checked={c.model.load_priority === "low"} onchange={(v) => setCfg("model.load_priority", v ? "low" : "normal")} />
        </Setting>
      </div>
      {@render recognition()}
    {/if}
  </div>
</div>
<style>
  .now {
    align-self: center;
    margin-right: 8px;
    font-size: 11px;
    color: var(--gold-dim);
    white-space: nowrap;
  }
  .now.err {
    color: var(--danger);
  }
  .rec {
    margin-top: 22px;
  }
  .intro {
    margin: 12px 0 4px;
    font-size: 12.5px;
  }
  code {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--gold);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .keybox {
    position: relative;
  }
  .keybox .input {
    padding-right: 48px;
  }
  .eye {
    position: absolute;
    right: 1px;
    top: 1px;
    height: 36px;
    padding: 0 10px;
    border: 0;
    background: transparent;
  }
  .eye:hover {
    color: var(--gold);
  }
  .input {
    font-size: 12px;
  }
  .testrow {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 14px 0 6px;
  }
  .res {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ok {
    color: var(--gold);
  }
  .bad {
    color: var(--danger);
  }
</style>