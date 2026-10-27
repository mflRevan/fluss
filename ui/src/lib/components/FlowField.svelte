<script lang="ts">
  // Ambient "river" background: layered sine ribbons drifting slowly.
  // `energy` (0..1) speeds them up and raises amplitude, `level` adds live mic motion.
  import { onDestroy, onMount } from "svelte";
  import { app } from "../state.svelte";

  let { energy = 0.2, opacity = 1, lines = 7 }: { energy?: number; opacity?: number; lines?: number } = $props();

  let canvas: HTMLCanvasElement;
  let raf = 0;
  let last = 0;
  let t = Math.random() * 100;
  let e = 0.2;
  let lvl = 0;

  function frame(now: number) {
    raf = requestAnimationFrame(frame);
    if (now - last < 1000 / 40) return; // 40 fps is plenty for ambience
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    if (document.hidden) return;
    const reduced = document.documentElement.dataset.motion === "reduced";
    e += (energy - e) * 0.05;
    lvl += (Math.min(1, Math.sqrt(app.level.rms) * 1.5) * (energy > 0.5 ? 1 : 0) - lvl) * 0.2;
    t += reduced ? 0 : dt * (0.25 + e * 0.9);

    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    const ctx = canvas.getContext("2d")!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    const dark = document.documentElement.dataset.theme !== "light";

    for (let i = 0; i < lines; i++) {
      const k = i / Math.max(1, lines - 1);
      const g = ctx.createLinearGradient(0, 0, w, 0);
      const a = (dark ? 0.55 : 0.45) * (0.35 + 0.65 * (1 - Math.abs(k - 0.5) * 1.6));
      g.addColorStop(0, `rgba(20,184,166,${a * 0.2})`);
      g.addColorStop(0.35, `rgba(34,211,238,${a})`);
      g.addColorStop(0.7, `rgba(139,124,246,${a * 0.85})`);
      g.addColorStop(1, `rgba(139,124,246,0)`);
      ctx.strokeStyle = g;
      ctx.lineWidth = 1.2 + (i === Math.floor(lines / 2) ? 1.2 : 0);
      ctx.beginPath();
      const amp = h * (0.07 + 0.1 * e + 0.18 * lvl) * (0.6 + 0.4 * Math.sin(i * 1.7));
      const base = h * (0.5 + (k - 0.5) * 0.34);
      for (let x = 0; x <= w; x += 6) {
        const u = x / w;
        const y =
          base +
          Math.sin(u * 6.2 + t * 1.4 + i * 0.55) * amp * Math.sin(u * Math.PI) +
          Math.sin(u * 13 - t * 2.1 + i) * amp * 0.25 * (0.4 + lvl);
        x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
      }
      ctx.stroke();
    }
  }

  onMount(() => (raf = requestAnimationFrame(frame)));
  onDestroy(() => cancelAnimationFrame(raf));
</script>

<canvas bind:this={canvas} style="opacity:{opacity}"></canvas>

<style>
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    transition: opacity 600ms var(--ease);
  }
</style>
