<script lang="ts">
  import { call, invoke } from "../backend";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Setting from "../components/Setting.svelte";
  import Slider from "../components/Slider.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, refresh, setCfg, toast } from "../state.svelte";

  const c = $derived(app.config!);
  let confirmReset = $state(false);

  async function autostart(v: boolean) {
    await setCfg("general.autostart", v);
    try {
      await call("autostart.set", { enabled: v });
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), "error");
    }
  }

  async function openLogs() {
    const p = await call<string>("logs.path");
    await invoke("open_path", { path: p });
  }

  async function reset() {
    if (!confirmReset) {
      confirmReset = true;
      setTimeout(() => (confirmReset = false), 3500);
      return;
    }
    confirmReset = false;
    await call("config.reset");
    await refresh();
    toast("Settings reset to defaults", "success");
  }

  async function quit() {
    await call("daemon.quit").catch(() => {});
    await invoke("app_quit");
  }
</script>

<PageHeader title="Advanced" subtitle="Startup, appearance and the nuts and bolts." />

<div class="section-title" style="margin-top:0">Startup</div>
<div class="card">
  <Setting icon="power" title="Start with Windows" hint="Fluss sits quietly in the tray, ready when you need it.">
    <Toggle checked={c.general.autostart} onchange={autostart} />
  </Setting>
  <Setting title="Wait before loading the model" hint="Gives your computer a moment after sign-in before the model loads in the background." disabled={!c.general.autostart}>
    <Slider value={c.general.autostart_delay_s} min={0} max={60} step={1} unit=" s" onchange={(v) => setCfg("general.autostart_delay_s", v)} />
  </Setting>
</div>

<div class="section-title">Appearance</div>
<div class="card">
  <Setting icon="eye" title="Theme">
    <Segmented
      value={c.ui.theme}
      options={[
        { value: "system", label: "System" },
        { value: "light", label: "Light" },
        { value: "dark", label: "Dark" },
      ]}
      onchange={(v) => setCfg("ui.theme", v)}
    />
  </Setting>
  <Setting title="Listening indicator" hint="The small pill that shows Fluss is listening.">
    <Toggle checked={c.ui.overlay} onchange={(v) => setCfg("ui.overlay", v)} />
  </Setting>
  <Setting title="Indicator position" disabled={!c.ui.overlay}>
    <Segmented
      value={c.ui.overlay_position}
      options={[
        { value: "bottom", label: "Bottom" },
        { value: "top", label: "Top" },
      ]}
      onchange={(v) => setCfg("ui.overlay_position", v)}
    />
  </Setting>
  <Setting title="Show live words in the indicator" disabled={!c.ui.overlay}>
    <Toggle checked={c.ui.overlay_preview} onchange={(v) => setCfg("ui.overlay_preview", v)} />
  </Setting>
  <Setting title="Reduce motion" hint="Fewer animations everywhere.">
    <Toggle checked={c.ui.reduce_motion} onchange={(v) => setCfg("ui.reduce_motion", v)} />
  </Setting>
</div>

<div class="section-title">Sound</div>
<div class="card">
  <Setting icon="wave" title="Start and stop sounds" hint="A soft tone when Fluss starts and stops listening.">
    <Toggle checked={c.general.sounds} onchange={(v) => setCfg("general.sounds", v)} />
  </Setting>
  <Setting title="Volume" disabled={!c.general.sounds}>
    <Slider value={c.general.sound_volume} min={0} max={100} step={5} unit="%" onchange={(v) => setCfg("general.sound_volume", v)} />
  </Setting>
</div>

<div class="section-title">Performance & privacy</div>
<div class="card">
  <Setting icon="cpu" title="Processor threads" hint="0 lets Fluss decide. Only change this if dictation competes with heavy work.">
    <Slider value={c.model.threads} min={0} max={32} step={1} format={(v) => (v === 0 ? "Auto" : `${v}`)} onchange={(v) => setCfg("model.threads", v)} />
  </Setting>
  <Setting icon="zap" title="Keep this window ready" hint="The fix-a-word box opens instantly, at the cost of ~60 MB of memory.">
    <Toggle checked={c.ui.keep_warm} onchange={(v) => setCfg("ui.keep_warm", v)} />
  </Setting>
  <Setting icon="shield" title="Recent dictations" hint="Kept in memory only, never written to disk. Turn off to keep nothing at all.">
    <Toggle checked={c.general.history} onchange={(v) => setCfg("general.history", v)} />
  </Setting>
</div>

<div class="section-title">Maintenance</div>
<div class="card tools">
  <button class="btn" onclick={openLogs}><Icon name="folder" size={15} />Open logs</button>
  <button class="btn" onclick={() => app.status && invoke("open_path", { path: app.status.config_path })}><Icon name="file" size={15} />Open config file</button>
  <button class="btn" class:danger={confirmReset} onclick={reset}><Icon name="undo" size={15} />{confirmReset ? "Click again to reset" : "Reset settings"}</button>
  <span class="grow"></span>
  <button class="btn danger" onclick={quit}><Icon name="power" size={15} />Quit Fluss</button>
</div>

<style>
  .tools {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 14px;
  }
  .grow {
    flex: 1;
  }
</style>
