<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { inTauri, onEvent } from "./lib/backend";
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
    app.config?.ui.theme;
    app.config?.ui.reduce_motion;
    applyTheme();
  });

  onMount(() => {
    startState();
    const p = new URLSearchParams(location.search).get("page") as Page | null;
    if (p && p in pages) app.page = p;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", applyTheme);
    applyTheme();
    // The native side asks us to switch pages when launched with --page.
    let un: (() => void) | undefined;
    if (inTauri) {
      import("@tauri-apps/api/event").then(({ listen }) =>
        listen<string>("navigate", (e) => {
          if (e.payload in pages) app.page = e.payload as Page;
        }).then((u) => (un = u)),
      );
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().show());
    }
    return () => {
      un?.();
      mq.removeEventListener("change", applyTheme);
    };
  });

  const Current = $derived(pages[app.page]);
</script>

<div class="shell">
  <TitleBar />
  {#if !app.config}
    <div class="boot" in:fade>
      <div class="pulse"></div>
      <div class="muted">{app.connected ? "Loading…" : "Starting Fluss…"}</div>
    </div>
  {:else if onboarding}
    <Onboarding onfinish={() => (forceOnboarding = false)} />
  {:else}
    <Sidebar />
    <main class="content">
      {#key app.page}
        <div class="page" in:fly={{ y: 10, duration: 280, delay: 60 }} out:fade={{ duration: 90 }}>
          <Current />
        </div>
      {/key}
    </main>
  {/if}
  {#if !app.connected && app.config}
    <div class="offline" transition:fly={{ y: -10 }}>Reconnecting to Fluss…</div>
  {/if}
  <Toasts />
</div>

<style>
  .shell {
    position: relative;
    display: flex;
    height: 100%;
    background:
      radial-gradient(1200px 600px at 110% -10%, rgba(139, 124, 246, 0.09), transparent 60%),
      radial-gradient(900px 500px at -10% 110%, rgba(20, 184, 166, 0.08), transparent 60%),
      var(--bg);
  }
  .content {
    flex: 1;
    min-width: 0;
    position: relative;
    display: grid;
  }
  .page {
    grid-area: 1 / 1;
    overflow-y: auto;
    padding: 44px 40px 40px;
    min-height: 0;
  }
  .boot {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 16px;
  }
  .pulse {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--flow);
    animation: b 1.4s ease-in-out infinite;
  }
  @keyframes b {
    50% {
      transform: scale(0.8);
      opacity: 0.6;
    }
  }
  .offline {
    position: absolute;
    top: 46px;
    left: 50%;
    transform: translateX(-50%);
    padding: 6px 14px;
    border-radius: 999px;
    background: rgba(251, 191, 36, 0.14);
    border: 1px solid rgba(251, 191, 36, 0.35);
    color: var(--amber);
    font-size: 12.5px;
    z-index: 60;
  }
</style>
