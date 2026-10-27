<script lang="ts">
  // Live microphone meter: a scrolling, centered waveform driven by `level` events.
  import { onDestroy } from "svelte";
  import { app } from "../state.svelte";

  let { bars = 32, height = 44 }: { bars?: number; height?: number } = $props();

  let hist = $state<number[]>(Array(96).fill(0));
  let smooth = 0;
  let raf = 0;
  let last = 0;
  const loop = (now: number) => {
    raf = requestAnimationFrame(loop);
    if (now - last < 33) return; // ~30 samples per second
    last = now;
    // Perceptual curve: quiet speech should still move the bars clearly.
    const target = Math.min(1, Math.pow(app.level.rms * 4, 0.6));
    smooth += (target - smooth) * (target > smooth ? 0.6 : 0.25);
    hist = [...hist.slice(1), smooth];
  };
  raf = requestAnimationFrame(loop);
  onDestroy(() => cancelAnimationFrame(raf));

  const shown = $derived(hist.slice(-bars));
  const loud = $derived(shown.slice(-6).reduce((a, b) => Math.max(a, b), 0));
</script>

<div class="meter" style="height:{height}px">
  <div class="bars">
    {#each shown as v, i}
      <span class="bar" style="height:{Math.max(6, v * 100)}%; opacity:{0.3 + 0.7 * (i / shown.length)}"></span>
    {/each}
  </div>
  <span class="label" class:on={loud > 0.15}>{loud > 0.15 ? "Hearing you" : "Quiet"}</span>
</div>

<style>
  .meter {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .bars {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .bar {
    flex: 1;
    min-width: 3px;
    border-radius: 3px;
    background: var(--flow);
    background-size: 600px 100%;
    transition: height 70ms linear;
  }
  .label {
    flex: none;
    width: 84px;
    text-align: right;
    font-size: 12px;
    font-weight: 550;
    color: var(--text-3);
    transition: color var(--t-med);
  }
  .label.on {
    color: var(--accent-text);
  }
</style>
