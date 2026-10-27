<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { inTauri } from "./lib/backend";
  import Iris from "./lib/components/Iris.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import TitleBar from "./lib/components/TitleBar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import Onboarding from "./lib/onboarding/Onboarding.svelte";
  import About from "./lib/pages/About.svelte";
  import Advanced from "./lib/pages/Advanced.svelte";
  import Audio from "./lib/pages/Audio.svelte";
  import Dictionary from "./lib/pages/Dictionary.svelte";
  import Home from "./lib/pages/Home.svelte";
  import Model from "./lib/pages/Model.svelte";
  import Output from "./lib/pages/Output.svelte";
  import Shortcuts from "./lib/pages/Shortcuts.svelte";
  import Text from "./lib/pages/Text.svelte";
  import { app, applyTheme, startState, type Page } from "./lib/state.svelte";

  const pages = { home: Home, dictionary: Dictionary, shortcuts: Shortcuts, model: Model, audio: Audio, text: Text, output: Output, advanced: Advanced, about: About };

  let forceOnboarding = $state(new URLSearchParams(location.search).has("onboarding"));
  const onboarding = $derived(forceOnboarding || (app.config ? !app.config.general.onboarded : false));

  $effect(() => {
    app.config?.ui.reduce_motion;
    applyTheme();
  });

  onMount(() => {
    startState();
    const p = new URLSearchParams(location.search).get("page") as Page | null;
    if (p && p in pages) app.page = p;
    applyTheme();
    // The native side asks us to switch pages when launched with --page.
    let un: (() => void) | undefined;
    if (inTauri) {
      import("@tauri-apps/api/event").then(({ listen }) =>
        listen<string>("navigate", (e) => {
          if (e.payload in pages) app.page = e.payload as Page;
        }).then((u) => (un = u)),
      );
      // Show only after the fonts are in, so the first frame is the real one.
      document.fonts.ready.then(() => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().show()));
    }
    return () => un?.();
  });

  const Current = $derived(pages[app.page]);
</script>

<div class="shell">
  <TitleBar />
  {#if !app.config}
    <div class="boot" in:fade={{ duration: 140 }}>
      <Iris size={120} ticks={60} mode="loading" />
      <div class="label">{app.connected ? "Loading" : "Starting Fluss"}</div>
    </div>
  {:else if onboarding}
    <Onboarding onfinish={() => (forceOnboarding = false)} />
  {:else}
    <Sidebar />
    <main class="content">
      {#key app.page}
        <div class="page" in:fly={{ y: 4, duration: 140, easing: (t) => 1 - Math.pow(1 - t, 3) }} out:fade={{ duration: 50 }}>
          <Current />
        </div>
      {/key}
    </main>
  {/if}
  {#if !app.connected && app.config}
    <div class="offline label" transition:fade={{ duration: 120 }}>◆ Reconnecting to Fluss</div>
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    position: relative;
    display: flex;
    height: 100%;
    background: var(--void);
  }
  .content {
    flex: 1;
    min-width: 0;
    position: relative;
    display: grid;
  }
  .page {
    grid-area: 1 / 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 40px 48px 32px;
    overflow: hidden;
  }
  /* Very large windows: keep line lengths and column widths sane. */
  .page > :global(*) {
    width: 100%;
    max-width: 1280px;
  }
  .boot {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 18px;
  }
  .offline {
    position: absolute;
    top: 8px;
    left: 50%;
    translate: -50% 0;
    padding: 6px 12px;
    border: 1px solid var(--gold-dim);
    background: var(--void);
    color: var(--gold);
    z-index: 60;
  }
</style>
