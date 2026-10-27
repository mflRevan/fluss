<script lang="ts">
  // Records a shortcut through the daemon (it sees Win/Cmd and global keys the
  // webview can't), then writes it to the config.
  import { fade } from "svelte/transition";
  import { call } from "../backend";
  import { setCfg, toast } from "../state.svelte";
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
    <span class="listening" in:fade={{ duration: 90 }}>
      <span class="caret"></span>
      Press your shortcut
    </span>
    <button class="btn ghost sm" onclick={cancel}>Cancel</button>
  {:else}
    <span in:fade={{ duration: 120 }}><Keys combo={value} pressed={flash} /></span>
    <button class="btn sm" onclick={record}>Change</button>
  {/if}
</div>

<style>
  .rec {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 34px;
    padding: 0 0 0 10px;
    border-left: 1px solid transparent;
    transition: border-color var(--t1) var(--q);
  }
  .rec.recording {
    border-left-color: var(--gold);
  }
  .listening {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 170px;
    font-family: var(--mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--gold);
  }
  .caret {
    width: 7px;
    height: 13px;
    background: var(--gold);
    animation: blink 0.9s steps(1) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
</style>