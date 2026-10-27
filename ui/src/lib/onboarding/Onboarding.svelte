<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { call } from "../backend";
  import HotkeyRecorder from "../components/HotkeyRecorder.svelte";
  import Iris from "../components/Iris.svelte";
  import Keys from "../components/Keys.svelte";
  import Logo from "../components/Logo.svelte";
  import ModelCards from "../components/ModelCards.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Select from "../components/Select.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg, toast } from "../state.svelte";
  import type { DeviceInfo } from "../types";

  let { onfinish }: { onfinish?: () => void } = $props();

  const steps = ["welcome", "mic", "model", "install", "shortcuts", "done"] as const;
  const names: Record<(typeof steps)[number], string> = { welcome: "Welcome", mic: "Microphone", model: "Model", install: "Install", shortcuts: "Try it", done: "Done" };
  // Dev: `?onboarding&step=3` opens a given step.
  let i = $state(Math.max(0, Math.min(5, Number(new URLSearchParams(location.search).get("step") ?? 0) || 0)));
  let dir = $state(1);
  const step = $derived(steps[i]);
  const c = $derived(app.config!);
  const st = $derived(app.status?.model);

  function go(d: number) {
    dir = d;
    i = Math.max(0, Math.min(steps.length - 1, i + d));
  }

  let devices = $state<DeviceInfo[]>([]);
  $effect(() => {
    if (step === "mic") call<DeviceInfo[]>("devices.list").then((d) => (devices = d));
  });
  const heard = $derived(app.level.rms > 0.05);
  let everHeard = $state(false);
  $effect(() => {
    if (step === "mic" && heard) everHeard = true;
  });

  // Start the install as soon as the model step is passed.
  async function toInstall() {
    const m = c.model.id;
    go(1);
    if (m === "custom") return;
    try {
      if (st?.phase === "not-installed" || st?.id !== m) await call("model.install", { id: m });
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    }
  }

  const ready = $derived(st?.phase === "ready");
  let practice = $state("");

  async function finish() {
    await setCfg("general.onboarded", true);
    try {
      await call("autostart.set", { enabled: c.general.autostart });
    } catch {}
    onfinish?.();
  }

  const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);
</script>

<div class="ob">
  <aside class="rail" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region><Logo size={20} /><span class="display">Fluss</span></div>
    <div class="label setup">Setup</div>
    <ol>
      {#each steps as s, k}
        <li class:on={k === i} class:past={k < i}>
          <span class="idx">{k < i ? "◆" : String(k + 1).padStart(2, "0")}</span>
          {names[s]}
        </li>
      {/each}
    </ol>
    <div class="grow"></div>
    <div class="label foot">About a minute</div>
  </aside>

  <div class="stage">
    {#key step}
      <section class="step" in:fly={{ x: 12 * dir, duration: 160, easing: (t) => 1 - Math.pow(1 - t, 3) }} out:fade={{ duration: 60 }}>
        {#if step === "welcome"}
          <div class="welcome">
            <Iris size={200} />
            <div>
              <div class="label"><b>01</b> — Welcome</div>
              <h1 class="display big">Speak.<br />It flows.</h1>
              <p class="lead">Hold a key, talk, let go — your words appear in whatever app you're in. Everything runs on this computer.</p>
              <button class="btn primary lg" onclick={() => go(1)}>Get started →</button>
            </div>
          </div>
        {:else if step === "mic"}
          <div class="label"><b>02</b> — Microphone</div>
          <h2 class="display">Your microphone.</h2>
          <p class="lead">Pick the one you talk into, then say a few words.</p>
          <div class="microw">
            <Iris size={190} mode="idle" reactive ticks={64} />
            <div class="micctl">
              <Select
                value={c.audio.device}
                options={[{ value: "", label: "System default", hint: devices.find((d) => d.default)?.name }, ...devices.map((d) => ({ value: d.name, label: d.name }))]}
                onchange={(v) => setCfg("audio.device", v)}
                width={340}
              />
              <div class="heard mono" class:show={everHeard}>◆ Signal received — sounds good.</div>
            </div>
          </div>
        {:else if step === "model"}
          <div class="label"><b>03</b> — Model</div>
          <h2 class="display">Choose a voice model.</h2>
          <p class="lead">You can switch any time. Not sure? Keep the recommended one.</p>
          <div class="models"><ModelCards /></div>
          <div class="mem">
            <div>
              <div class="mt">Keep it ready in memory?</div>
              <p class="faint">Always ready is instant. Otherwise it sleeps after {c.model.idle_unload_min} minutes and takes a moment to wake.</p>
            </div>
            <Segmented
              value={c.model.residency === "always" ? "always" : "idle-unload"}
              options={[
                { value: "always", label: "Always ready" },
                { value: "idle-unload", label: "Save memory" },
              ]}
              onchange={(v) => setCfg("model.residency", v)}
            />
          </div>
        {:else if step === "install"}
          <div class="label"><b>04</b> — Install</div>
          <div class="installrow">
            <Iris size={190} mode={ready || c.model.id === "custom" ? "idle" : "loading"} progress={st?.phase === "installing" ? (st?.progress ?? null) : ready ? 1 : null} />
            <div>
              {#if c.model.id === "custom"}
                <h2 class="display">Connect your endpoint.</h2>
                <p class="lead">You chose a custom server. Enter its details on the Model page after setup.</p>
              {:else if ready}
                <h2 class="display">{st?.name} is ready.</h2>
                <p class="lead mono gold">loaded{st?.device ? ` · ${st.device}` : ""}</p>
              {:else}
                <h2 class="display">{st?.phase === "loading" ? "Warming up." : "Setting up."}</h2>
                <p class="lead mono">
                  {st?.detail || "preparing"}{st?.phase === "installing" && st?.progress != null ? ` · ${Math.round(st.progress * 100)}%` : ""}
                </p>
                <p class="faint small">You can continue — it keeps going in the background.</p>
              {/if}
            </div>
          </div>
        {:else if step === "shortcuts"}
          <div class="label"><b>05</b> — Try it</div>
          <h2 class="display">Try it.</h2>
          <p class="lead">Click into the box, hold your shortcut, say something, let go.</p>
          <div class="sc">
            <div class="scrow">
              <span>Dictate <span class="faint">— hold</span></span>
              <HotkeyRecorder path="hotkeys.push_to_talk" value={c.hotkeys.push_to_talk} />
            </div>
            <div class="scrow">
              <span>Hands-free <span class="faint">— press</span></span>
              <HotkeyRecorder path="hotkeys.hands_free" value={c.hotkeys.hands_free} />
            </div>
          </div>
          <textarea class="practice" bind:value={practice} placeholder="Hold {c.hotkeys.push_to_talk} and say: “Hello Fluss, this is my first dictation.”"></textarea>
          {#if !ready}
            <p class="faint small">The model is still loading — your words appear as soon as it's ready.</p>
          {/if}
        {:else}
          <div class="welcome">
            <Iris size={200} mode="idle" />
            <div>
              <div class="label"><b>06</b> — Done</div>
              <h1 class="display big">You're set.</h1>
              <p class="lead">
                Fluss lives in the tray. Hold <Keys combo={c.hotkeys.push_to_talk} size="sm" /> anywhere to dictate; press <Keys combo={c.hotkeys.add_rule} size="sm" /> when a word comes out wrong.
              </p>
              <div class="auto">
                <Toggle checked={c.general.autostart} onchange={(v) => setCfg("general.autostart", v)} />
                <span>Start Fluss when I sign in to {isMac ? "my Mac" : "Windows"}</span>
              </div>
              <button class="btn primary lg" onclick={finish}>Start using Fluss →</button>
            </div>
          </div>
        {/if}
      </section>
    {/key}

    {#if step !== "welcome" && step !== "done"}
      <footer>
        <button class="btn ghost" onclick={() => go(-1)}>← Back</button>
        <span class="grow"></span>
        {#if step === "model"}
          <button class="btn primary" onclick={toInstall}>Continue →</button>
        {:else if step === "install"}
          <button class="btn primary" onclick={() => go(1)}>{ready || c.model.id === "custom" ? "Continue →" : "Continue while it loads →"}</button>
        {:else}
          <button class="btn primary" onclick={() => go(1)}>Continue →</button>
        {/if}
      </footer>
    {/if}
  </div>
</div>

<style>
  .ob {
    flex: 1;
    display: grid;
    grid-template-columns: 232px 1fr;
    min-height: 0;
  }
  .rail {
    display: flex;
    flex-direction: column;
    padding: 18px 0 18px;
    border-right: 1px solid var(--rule);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 2px 24px 30px;
  }
  .brand .display {
    font-size: 27px;
    font-weight: 400;
    line-height: 1;
  }
  .setup {
    padding: 0 24px 10px;
  }
  ol {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    position: relative;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 34px;
    padding: 0 24px;
    font-size: 13.5px;
    color: var(--text-3);
    transition: color var(--t2) var(--q);
  }
  .idx {
    width: 16px;
    font-family: var(--mono);
    font-size: 10px;
  }
  li.past {
    color: var(--text-2);
  }
  li.past .idx {
    color: var(--gold-dim);
  }
  li.on {
    color: var(--gold);
    background: var(--gold-wash);
  }
  li.on::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 2px;
    background: var(--gold);
  }
  .grow {
    flex: 1;
  }
  .foot {
    padding: 0 24px;
  }
  .stage {
    position: relative;
    display: grid;
    grid-template-rows: 1fr auto;
    min-height: 0;
  }
  .step {
    grid-area: 1 / 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 40px 64px 12px;
    min-height: 0;
    max-width: 860px;
  }
  .welcome {
    display: flex;
    align-items: center;
    gap: 56px;
  }
  h1.big {
    font-size: 76px;
    margin: 16px 0 18px;
  }
  h2 {
    font-size: 46px;
    margin: 14px 0 12px;
  }
  .lead {
    font-size: 15px;
    color: var(--text-2);
    line-height: 1.55;
    max-width: 520px;
    margin-bottom: 26px;
  }
  .lead.gold {
    color: var(--gold);
    font-size: 12px;
  }
  .small {
    font-size: 12.5px;
  }
  .microw,
  .installrow {
    display: flex;
    align-items: center;
    gap: 40px;
  }
  .installrow {
    margin-top: 22px;
  }
  .installrow h2 {
    margin-top: 0;
  }
  .micctl {
    display: grid;
    gap: 16px;
  }
  .heard {
    font-size: 11.5px;
    color: var(--gold);
    opacity: 0;
    translate: 0 4px;
    transition: opacity var(--t2) var(--q), translate var(--t2) var(--q);
  }
  .heard.show {
    opacity: 1;
    translate: 0 0;
  }
  .models {
    margin-top: -6px;
  }
  .mem {
    display: flex;
    align-items: center;
    gap: 20px;
    padding-top: 16px;
  }
  .mem > div {
    flex: 1;
  }
  .mt {
    font-size: 13.5px;
  }
  .mem p {
    font-size: 12px;
    margin-top: 2px;
  }
  .sc {
    border-top: 1px solid var(--rule);
    margin-bottom: 16px;
  }
  .scrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    border-bottom: 1px solid var(--rule);
    font-size: 13.5px;
  }
  .practice {
    min-height: 104px;
    resize: none;
    padding: 14px 16px;
    border: 1px solid var(--rule-strong);
    border-left: 2px solid var(--gold);
    background: var(--plate);
    outline: none;
    font-family: var(--display);
    font-weight: 300;
    font-size: 22px;
    line-height: 1.35;
    transition: border-color var(--t1);
  }
  .practice:focus {
    border-color: var(--gold);
  }
  .practice::placeholder {
    color: var(--text-3);
    font-family: var(--font);
    font-size: 14px;
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: -6px 0 26px;
    font-size: 13.5px;
    color: var(--text-2);
  }
  footer {
    display: flex;
    padding: 14px 64px 26px 48px;
  }
</style>