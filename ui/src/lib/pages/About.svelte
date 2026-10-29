<script lang="ts">
  import { call, invoke } from "../backend";
  import Iris from "../components/Iris.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { app, setCfg, toast, versionSkew } from "../state.svelte";
  import type { UpdateStatus } from "../types";

  const open = (url: string) => invoke("open_path", { path: url });
  const err = (e: unknown) => toast(String(e instanceof Error ? e.message : e), "error", 6000);

  const u = $derived<UpdateStatus | undefined>(app.status?.update);
  const skew = $derived(versionSkew());
  let restarting = $state(false);

  function ago(ms: number): string {
    const m = Math.round((Date.now() - ms) / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    return h < 48 ? `${h} h ago` : `${Math.round(h / 24)} days ago`;
  }

  const line = $derived.by(() => {
    if (!u) return "";
    if (skew) return `This window is v${app.uiVersion}, but the background app is still v${app.status?.version}.`;
    if (u.downloading != null) return `Downloading Fluss ${u.latest}…`;
    if (u.checking) return "Checking for updates…";
    if (u.required) return `This version is no longer supported. Fluss ${u.latest} is available.`;
    if (u.available) return `Fluss ${u.latest} is available${u.skipped ? " (skipped)" : ""}.`;
    if (u.error && !u.checked_at) return `Couldn't check for updates: ${u.error}`;
    if (u.checked_at) return `Up to date · checked ${ago(u.checked_at)}`;
    return "Not checked yet.";
  });

  const where = $derived.by(() => {
    const i = app.status?.install;
    if (!i?.kind) return "";
    if (i.kind === "installed") return "Installed for this user";
    if (i.kind === "dev") return "Development build";
    return "Portable copy";
  });

  async function check() {
    try {
      const r = await call<UpdateStatus>("update.check");
      if (r.error) toast(`Couldn't check for updates: ${r.error}`, "error", 6000);
      else if (!r.available) toast(`Fluss ${r.current} is up to date`, "success");
    } catch (e) {
      err(e);
    }
  }

  async function install() {
    try {
      await call("update.install", { reopen: true });
    } catch (e) {
      err(e);
    }
  }

  async function skip(version: string | null | undefined) {
    await call("update.skip", { version }).catch(err);
  }

  async function restart() {
    restarting = true;
    try {
      await invoke("restart_daemon");
      toast("Fluss restarted", "success");
    } catch (e) {
      err(e);
    } finally {
      restarting = false;
    }
  }
</script>

<div class="about">
  <div class="hero">
    <div class="txt">
      <div class="label"><b>09</b> — About</div>
      <h1 class="display">Fluss</h1>
      <p class="tag">Speak, and it flows into any app.<br />Local, fast, private.</p>
      <div class="meta mono">
        <span>v{app.status?.version ?? "—"}</span>
        {#if where}<span title={app.status?.install.dir}>{where}</span>{/if}
        <span>MIT</span>
        <span>German for “river”</span>
      </div>

      {#if u}
        <div class="update" class:alert={skew || u.required} class:offer={!skew && !u.required && u.available && !u.skipped}>
          <div class="uline">{line}</div>
          {#if u.message && (u.available || u.required) && !skew}<div class="umsg">{u.message}</div>{/if}
          {#if u.downloading != null}
            <div class="bar"><div class="fill" style:width="{Math.round(u.downloading * 100)}%"></div></div>
          {/if}
          <div class="actions">
            {#if skew}
              <button class="btn sm primary" disabled={restarting} onclick={restart}>{restarting ? "Restarting…" : "Restart Fluss"}</button>
            {:else if u.downloading != null}
              <span class="mono pct">{Math.round(u.downloading * 100)}%</span>
            {:else if u.available}
              <button class="btn sm primary" onclick={install}>Install {u.latest}</button>
              {#if u.notes_url}<button class="btn sm ghost" onclick={() => open(u.notes_url!)}>What's new ↗</button>{/if}
              {#if u.skipped}
                <button class="btn sm ghost" onclick={() => skip(null)}>Remind me again</button>
              {:else if !u.required}
                <button class="btn sm ghost" onclick={() => skip(u.latest)}>Skip this version</button>
              {/if}
            {:else}
              <button class="btn sm" disabled={u.checking} onclick={check}>{u.checking ? "Checking…" : "Check for updates"}</button>
            {/if}
            <span class="grow"></span>
            {#if app.config}
              <span class="auto mono">Auto-check</span>
              <Toggle label="Check for updates automatically" checked={app.config.general.check_updates} onchange={(v) => setCfg("general.check_updates", v)} />
            {/if}
          </div>
        </div>
      {/if}

      <button class="btn ghost repo" onclick={() => open("https://github.com/mflRevan/fluss")}>github.com/mflRevan/fluss ↗</button>
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
  .txt {
    flex: 1;
    max-width: 560px;
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
    flex-wrap: wrap;
    gap: 18px;
    margin: 22px 0 20px;
    font-size: 11px;
    color: var(--gold-dim);
    letter-spacing: 0.06em;
  }
  .update {
    padding: 12px 0 12px 14px;
    border-left: 1px solid var(--rule-strong);
    margin-bottom: 16px;
  }
  .update.offer {
    border-left-color: var(--gold);
  }
  .update.alert {
    border-left-color: var(--danger);
  }
  .uline {
    font-size: 13.5px;
    color: var(--text);
  }
  .update.alert .uline {
    color: var(--danger);
  }
  .umsg {
    margin-top: 4px;
    font-size: 12.5px;
    color: var(--text-2);
  }
  .bar {
    height: 2px;
    margin-top: 10px;
    background: var(--rule);
  }
  .fill {
    height: 100%;
    background: var(--gold);
    transition: width var(--t2) var(--q);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .grow {
    flex: 1;
  }
  .pct {
    font-size: 11px;
    color: var(--gold);
  }
  .auto {
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .repo {
    margin-left: -16px;
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
