<script lang="ts">
  import { call, invoke } from "../backend";
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

<PageHeader index="08" title="Advanced" subtitle="Startup, the on-screen indicator, sound, and the nuts and bolts." />

<div class="cols">
  <div class="col">
    <div class="group">
      <span class="label">Startup</span>
      <Setting title="Start with Windows" hint="Waits quietly in the tray.">
        <Toggle checked={c.general.autostart} onchange={autostart} />
      </Setting>
      <Setting title="Delay model load" hint="After sign-in, before loading." disabled={!c.general.autostart}>
        <Slider value={c.general.autostart_delay_s} min={0} max={60} step={1} unit=" s" width={150} onchange={(v) => setCfg("general.autostart_delay_s", v)} />
      </Setting>
    </div>
    <div class="group">
      <span class="label">Sound</span>
      <Setting title="Start and stop tones">
        <Toggle checked={c.general.sounds} onchange={(v) => setCfg("general.sounds", v)} />
      </Setting>
      <Setting title="Volume" disabled={!c.general.sounds}>
        <Slider value={c.general.sound_volume} min={0} max={100} step={5} unit="%" width={150} onchange={(v) => setCfg("general.sound_volume", v)} />
      </Setting>
    </div>
    <div class="group">
      <span class="label">Maintenance</span>
      <div class="tools">
        <button class="btn sm" onclick={openLogs}>Open logs</button>
        <button class="btn sm" onclick={() => app.status && invoke("open_path", { path: app.status.config_path })}>Config file</button>
        <button class="btn sm danger" class:armed={confirmReset} onclick={reset}>{confirmReset ? "Confirm reset" : "Reset settings"}</button>
        <button class="btn sm danger" onclick={quit}>Quit Fluss</button>
      </div>
    </div>
  </div>

  <div class="col">
    <div class="group">
      <span class="label">Indicator</span>
      <Setting title="Show the indicator" hint="The small plate that shows Fluss is listening.">
        <Toggle checked={c.ui.overlay} onchange={(v) => setCfg("ui.overlay", v)} />
      </Setting>
      <Setting title="Position" disabled={!c.ui.overlay}>
        <Segmented
          value={c.ui.overlay_position}
          options={[
            { value: "bottom", label: "Bottom" },
            { value: "top", label: "Top" },
          ]}
          onchange={(v) => setCfg("ui.overlay_position", v)}
        />
      </Setting>
      <Setting title="Live words" hint="Show the preview while you speak." disabled={!c.ui.overlay}>
        <Toggle checked={c.ui.overlay_preview} onchange={(v) => setCfg("ui.overlay_preview", v)} />
      </Setting>
      <Setting title="Reduce motion" hint="Fewer animations everywhere.">
        <Toggle checked={c.ui.reduce_motion} onchange={(v) => setCfg("ui.reduce_motion", v)} />
      </Setting>
    </div>
    <div class="group">
      <span class="label">Performance & privacy</span>
      <Setting title="Processor threads" hint="0 lets Fluss decide.">
        <Slider value={c.model.threads} min={0} max={32} step={1} width={150} format={(v) => (v === 0 ? "auto" : `${v}`)} onchange={(v) => setCfg("model.threads", v)} />
      </Setting>
      <Setting title="Keep this window ready" hint="Instant fix-a-word box, ~60 MB.">
        <Toggle checked={c.ui.keep_warm} onchange={(v) => setCfg("ui.keep_warm", v)} />
      </Setting>
      <Setting title="Recent dictations" hint="Memory only, never on disk.">
        <Toggle checked={c.general.history} onchange={(v) => setCfg("general.history", v)} />
      </Setting>
    </div>
  </div>
</div>

<style>
  .tools {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding-top: 14px;
  }
</style>