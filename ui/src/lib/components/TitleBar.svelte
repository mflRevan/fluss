<script lang="ts">
  import { inTauri } from "../backend";
  import Icon from "./Icon.svelte";

  async function win(action: "minimize" | "toggleMaximize" | "close") {
    if (!inTauri) return;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const w = getCurrentWindow();
    if (action === "close") await w.close();
    else await w[action]();
  }
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="spacer" data-tauri-drag-region></div>
  <div class="controls">
    <button aria-label="Minimize" onclick={() => win("minimize")}><Icon name="minimize" size={14} /></button>
    <button aria-label="Maximize" onclick={() => win("toggleMaximize")}><Icon name="maximize" size={12} /></button>
    <button aria-label="Close" class="close" onclick={() => win("close")}><Icon name="close" size={14} /></button>
  </div>
</header>

<style>
  .titlebar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 36px;
    display: flex;
    z-index: 40;
  }
  .spacer {
    flex: 1;
  }
  .controls {
    display: flex;
  }
  button {
    width: 44px;
    height: 34px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--text-3);
    transition: background var(--t1), color var(--t1);
  }
  button:hover {
    background: var(--plate-2);
    color: var(--gold);
  }
  .close:hover {
    background: var(--danger);
    color: #fff;
  }
</style>