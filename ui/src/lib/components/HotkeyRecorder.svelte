<script lang="ts">
  // Records a shortcut through the daemon (it sees Win/Cmd and global keys the
  // webview can't), then writes it to the config.
  import { fade, scale } from "svelte/transition";
  import { call } from "../backend";
  import { setCfg, toast } from "../state.svelte";
  import Icon from "./Icon.svelte";
  import Keys from "./Keys.svelte";

  let { path, value }: { path: string; value: string } = $props();

  let recording = $state(false);
  let flash = $state(false);

  async function record() {
    if (recording) return;
    recording = true;
    try {
      const r = await call<{ hotkey: string }>("hotkeys.record", {});
      if (r?.hotkey) {
        await setCfg(path, r.hotkey);
        flash = true;
        setTimeout(() => (flash = false), 700);
      }
    } catch (e) {
      const msg = String(e instanceof Error ? e.message : e);
      if (!/timeout|cancel/i.test(msg)) toast(msg, "error");
    } finally {
      recording = false;
    }
  }

  async function cancel() {
    recording = false;
    try {
      await call("hotkeys.pause", { paused: false });
    } catch {}
  }
</script>

<div class="rec" class:recording class:flash>
  {#if recording}
    <span class="listening" in:fade={{ duration: 120 }}>
      <span class="dots"><i></i><i></i><i></i></span>
      Press your shortcut…
    </span>
    <button class="btn ghost sm" onclick={cancel}>Cancel</button>
  {:else}
    <span in:scale={{ start: 0.9, duration: 200 }}><Keys combo={value} pressed={flash} /></span>
    <button class="btn sm" onclick={record}><Icon name="keyboard" size={14} />Change</button>
  {/if}
</div>

<style>
  .rec {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 4px 4px 4px 10px;
    border-radius: 12px;
    border: 1px dashed transparent;
    transition: border-color var(--t-med), background var(--t-med);
  }
  .rec.recording {
    border-color: rgba(45, 212, 191, 0.5);
    background: rgba(45, 212, 191, 0.07);
  }
  .listening {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--accent-text);
    font-weight: 560;
    min-width: 170px;
  }
  .dots {
    display: flex;
    gap: 3px;
  }
  .dots i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: d 1s ease-in-out infinite;
  }
  .dots i:nth-child(2) {
    animation-delay: 0.15s;
  }
  .dots i:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes d {
    50% {
      transform: translateY(-4px);
      opacity: 0.5;
    }
  }
  .sm {
    height: 30px;
    padding: 0 10px;
    font-size: 12.5px;
  }
</style>
