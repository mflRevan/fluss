<script lang="ts">
  import { fade, fly, scale } from "svelte/transition";
  import { call } from "../backend";
  import FlowField from "../components/FlowField.svelte";
  import HotkeyRecorder from "../components/HotkeyRecorder.svelte";
  import Icon from "../components/Icon.svelte";
  import Keys from "../components/Keys.svelte";
  import LevelMeter from "../components/LevelMeter.svelte";
  import Logo from "../components/Logo.svelte";
  import ModelCards from "../components/ModelCards.svelte";
  import ProgressRing from "../components/ProgressRing.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Select from "../components/Select.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg, toast } from "../state.svelte";
  import type { DeviceInfo } from "../types";

  let { onfinish }: { onfinish?: () => void } = $props();

  const steps = ["welcome", "mic", "model", "install", "shortcuts", "done"] as const;
  let i = $state(0);
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
  const heard = $derived(app.level.rms > 0.06);
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
  <FlowField energy={step === "welcome" || step === "done" ? 0.45 : 0.15} opacity={step === "welcome" || step === "done" ? 1 : 0.45} lines={9} />

  <div class="dots">
    {#each steps as s, k}
      <span class:on={k === i} class:past={k < i}></span>
    {/each}
  </div>

  {#key step}
    <section class="step" in:fly={{ x: 40 * dir, duration: 380, delay: 80 }} out:fly={{ x: -40 * dir, duration: 200 }}>
      {#if step === "welcome"}
        <div class="center">
          <div class="logo" in:scale={{ start: 0.6, duration: 600, delay: 150 }}><Logo size={92} /></div>
          <h1 class="big">Welcome to <span class="gradient-text">Fluss</span></h1>
          <p class="lead">Talk, and your words appear in any app — as fast as you can think them. Everything runs on your computer.</p>
          <button class="btn primary lg" onclick={() => go(1)}>Get started <Icon name="arrowRight" size={16} /></button>
          <p class="faint tiny">Takes about a minute.</p>
        </div>
      {:else if step === "mic"}
        <div class="panel">
          <div class="num">1</div>
          <h2>Your microphone</h2>
          <p class="lead">Pick the mic you talk into, then say a few words.</p>
          <Select
            value={c.audio.device}
            options={[{ value: "", label: "System default", hint: devices.find((d) => d.default)?.name }, ...devices.map((d) => ({ value: d.name, label: d.name }))]}
            onchange={(v) => setCfg("audio.device", v)}
            width={560}
          />
          <div class="meter card">
            <LevelMeter bars={44} height={52} />
          </div>
          <div class="ok" class:show={everHeard}>
            <Icon name="check" size={16} stroke={2.4} /> Sounds good!
          </div>
        </div>
      {:else if step === "model"}
        <div class="panel wide">
          <div class="num">2</div>
          <h2>Choose a voice model</h2>
          <p class="lead">You can switch any time. Not sure? Keep the recommended one.</p>
          <ModelCards compact />
          <div class="mem">
            <div>
              <b>Keep it ready in memory?</b>
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
        </div>
      {:else if step === "install"}
        <div class="panel">
          <div class="num">3</div>
          {#if c.model.id === "custom"}
            <h2>Connect your endpoint</h2>
            <p class="lead">You chose a custom server. Enter its details on the Model page after setup.</p>
          {:else if ready}
            <div class="done-ring" in:scale={{ start: 0.5, duration: 400 }}><Icon name="check" size={34} stroke={2.4} /></div>
            <h2>{st?.name} is ready</h2>
            <p class="lead">Loaded{st?.device ? ` on ${st.device}` : ""}. Let's try it.</p>
          {:else}
            <div class="ring"><ProgressRing value={st?.phase === "installing" ? (st?.progress ?? null) : null} size={96} stroke={6} /></div>
            <h2>{st?.phase === "loading" ? "Warming up" : "Setting up"} {st?.name ?? ""}</h2>
            <p class="lead">
              {st?.detail || "Preparing…"}{st?.phase === "installing" && st?.progress != null ? ` · ${Math.round(st.progress * 100)}%` : ""}
            </p>
            <p class="faint tiny">You can continue — it keeps going in the background.</p>
          {/if}
        </div>
      {:else if step === "shortcuts"}
        <div class="panel">
          <div class="num">4</div>
          <h2>Try it</h2>
          <p class="lead">Click into the box, hold your shortcut, say something, and let go.</p>
          <div class="sc card">
            <div class="scrow">
              <span><b>Dictate</b> <span class="faint">hold</span></span>
              <HotkeyRecorder path="hotkeys.push_to_talk" value={c.hotkeys.push_to_talk} />
            </div>
            <div class="scrow">
              <span><b>Hands-free</b> <span class="faint">press</span></span>
              <HotkeyRecorder path="hotkeys.hands_free" value={c.hotkeys.hands_free} />
            </div>
          </div>
          <textarea class="practice" bind:value={practice} placeholder="Hold {c.hotkeys.push_to_talk} and say: “Hello Fluss, this is my first dictation.”"></textarea>
          {#if !ready}
            <p class="faint tiny">The model is still loading — your words will appear as soon as it's ready.</p>
          {/if}
        </div>
      {:else}
        <div class="center">
          <div class="logo" in:scale={{ start: 0.6, duration: 500 }}><Logo size={80} /></div>
          <h1 class="big">You're all set</h1>
          <p class="lead">Fluss lives in your system tray. Hold <Keys combo={c.hotkeys.push_to_talk} size="sm" /> anywhere to dictate, and press <Keys
              combo={c.hotkeys.add_rule}
              size="sm"
            /> whenever a word comes out wrong.</p>
          <label class="auto">
            <Toggle checked={c.general.autostart} onchange={(v) => setCfg("general.autostart", v)} />
            Start Fluss when I sign in to {isMac ? "my Mac" : "Windows"}
          </label>
          <button class="btn primary lg" onclick={finish}>Start using Fluss <Icon name="arrowRight" size={16} /></button>
        </div>
      {/if}
    </section>
  {/key}

  {#if step !== "welcome" && step !== "done"}
    <footer in:fade>
      <button class="btn ghost" onclick={() => go(-1)}><Icon name="arrowLeft" size={15} />Back</button>
      <span class="grow"></span>
      {#if step === "model"}
        <button class="btn primary" onclick={toInstall}>Continue <Icon name="arrowRight" size={15} /></button>
      {:else if step === "install"}
        <button class="btn primary" onclick={() => go(1)}>{ready || c.model.id === "custom" ? "Continue" : "Continue while it loads"} <Icon name="arrowRight" size={15} /></button>
      {:else}
        <button class="btn primary" onclick={() => go(1)}>Continue <Icon name="arrowRight" size={15} /></button>
      {/if}
    </footer>
  {/if}
</div>

<style>
  .ob {
    position: relative;
    flex: 1;
    display: grid;
    grid-template-rows: auto 1fr auto;
    overflow: hidden;
    background:
      radial-gradient(800px 400px at 50% 0%, rgba(34, 211, 238, 0.08), transparent 70%),
      var(--bg);
  }
  .dots {
    position: relative;
    display: flex;
    justify-content: center;
    gap: 8px;
    padding-top: 22px;
    z-index: 2;
  }
  .dots span {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: var(--surface-3);
    transition:
      width 400ms var(--spring),
      background 300ms;
  }
  .dots span.past {
    background: rgba(45, 212, 191, 0.5);
  }
  .dots span.on {
    width: 26px;
    background: var(--flow);
  }
  .step {
    grid-area: 2 / 1;
    position: relative;
    display: grid;
    place-items: center;
    padding: 20px 40px;
    overflow-y: auto;
    min-height: 0;
  }
  .center {
    position: relative;
    isolation: isolate;
    display: grid;
    justify-items: center;
    text-align: center;
    gap: 16px;
    max-width: 560px;
  }
  .ob :global(.card),
  .ob :global(.model),
  .ob :global(.trigger),
  .practice,
  .mem,
  .auto {
    background: color-mix(in srgb, var(--surface-solid) 90%, transparent);
    backdrop-filter: blur(14px);
  }
  .center::before {
    content: "";
    position: absolute;
    inset: -60px -120px;
    z-index: -1;
    background: radial-gradient(closest-side, var(--bg) 35%, color-mix(in srgb, var(--bg) 60%, transparent) 65%, transparent);
  }
  .logo {
    filter: drop-shadow(0 20px 40px rgba(34, 211, 238, 0.45));
    margin-bottom: 6px;
  }
  .big {
    font-size: 44px;
    font-weight: 760;
    letter-spacing: -0.045em;
  }
  .lead {
    font-size: 15.5px;
    color: var(--text-2);
    margin: 0;
    line-height: 1.55;
  }
  .tiny {
    font-size: 12px;
    margin: 0;
  }
  .panel {
    width: min(560px, 100%);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .panel.wide {
    width: min(760px, 100%);
  }
  .num {
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: var(--flow-soft);
    border: 1px solid var(--border-2);
    color: var(--accent-text);
    font-weight: 700;
    font-size: 13px;
  }
  h2 {
    font-size: 28px;
    font-weight: 720;
    letter-spacing: -0.035em;
  }
  .meter {
    padding: 16px 18px;
  }
  .ok {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--green);
    font-weight: 600;
    opacity: 0;
    transform: translateY(6px);
    transition:
      opacity 400ms var(--ease),
      transform 400ms var(--spring);
  }
  .ok.show {
    opacity: 1;
    transform: none;
  }
  .mem {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 16px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
  }
  .mem p {
    margin: 2px 0 0;
    font-size: 12.5px;
  }
  .mem > div {
    flex: 1;
  }
  .ring,
  .done-ring {
    align-self: flex-start;
  }
  .done-ring {
    width: 96px;
    height: 96px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--flow);
    color: #031615;
    box-shadow: 0 20px 50px -14px rgba(34, 211, 238, 0.7);
  }
  .sc {
    padding: 6px 16px;
  }
  .scrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 0;
  }
  .scrow + .scrow {
    border-top: 1px solid var(--border);
  }
  .practice {
    min-height: 110px;
    resize: none;
    padding: 14px 16px;
    border-radius: 14px;
    border: 1px solid var(--border-2);
    background: var(--surface);
    outline: none;
    font-size: 15px;
    line-height: 1.55;
    transition:
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }
  .practice:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 4px rgba(45, 212, 191, 0.15);
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    cursor: pointer;
  }
  footer {
    position: relative;
    display: flex;
    padding: 16px 28px 22px;
    z-index: 2;
  }
  .grow {
    flex: 1;
  }
</style>
