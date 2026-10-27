<script module lang="ts">
  export type IrisMode = "idle" | "listening" | "hands-free" | "loading" | "thinking";
</script>

<script lang="ts">
  // The voice visual: a ring of radial ticks around a gold diamond.
  // Listening: tick lengths are the recent voice level, newest at 12 o'clock,
  // history sweeping clockwise. See docs/DESIGN.md ("iris").
  import { onMount } from "svelte";
  import { app } from "../state.svelte";

  let {
    size = 200,
    mode = "idle",
    progress = null,
    ticks = 72,
    reactive = false,
  }: {
    size?: number;
    mode?: IrisMode;
    /** 0..1 draws a progress arc on the outer ring. */
    progress?: number | null;
    ticks?: number;
    /** React to the mic level even when idle (mic test). */
    reactive?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement;

  const GOLD = [227, 176, 75];
  const DIM = [140, 107, 42];
  const RULE = "#2a2418";

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const hist = new Float32Array(512);
    const len = new Float32Array(512);
    let raf = 0;
    let last = performance.now();
    let visible = true;
    let peak = 0.05;
    let smooth = 0;
    let prevSmooth = 0;
    let acc = 0;
    let rot = 0;
    let diamond = 0;

    const io = new IntersectionObserver(([e]) => (visible = e.isIntersecting));
    io.observe(canvas);

    function frame(now: number) {
      raf = requestAnimationFrame(frame);
      if (!visible || document.hidden) {
        last = now;
        return;
      }
      const elapsed = Math.min(1, (now - last) / 1000);
      const dt = Math.min(0.05, elapsed);
      last = now;
      const t = now / 1000;
      const n = ticks;
      const dpr = window.devicePixelRatio || 1;
      const px = Math.round(size * dpr);
      if (canvas.width !== px) {
        canvas.width = px;
        canvas.height = px;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size, size);

      const live = mode === "listening" || mode === "hands-free" || reactive;
      const raw = app.level?.rms ?? 0;
      peak = Math.max(peak * Math.pow(0.8, elapsed), raw, 0.04);
      const norm = Math.min(1, Math.pow(raw / peak, 0.8));
      smooth += ((live ? norm : 0) - smooth) * Math.min(1, dt * 20);

      // One history slot per ~28 ms: a full revolution is ~2 s of voice.
      // Advance by real time, so throttled frames still keep the history honest.
      acc += elapsed;
      const step = 2 / n;
      // Several slots per frame when frames are sparse: interpolate so the ring stays textured.
      const slots = Math.floor(acc / step);
      for (let k = 0; k < slots; k++) {
        hist.copyWithin(1, 0, n - 1);
        hist[0] = prevSmooth + (smooth - prevSmooth) * ((k + 1) / slots) * (0.85 + 0.3 * Math.random());
      }
      acc -= slots * step;
      prevSmooth = smooth;

      if (mode === "idle" && !reactive) rot += (dt * Math.PI * 2) / 40;
      else if (mode === "hands-free") rot += (dt * Math.PI * 2) / 24;

      const R = size / 2;
      const r0 = R * 0.4;
      const maxL = R * 0.42;
      const sweep = (t * Math.PI * 2) / 1.4;

      for (let i = 0; i < n; i++) {
        const a = -Math.PI / 2 + (i / n) * Math.PI * 2;
        let target: number;
        if (mode === "loading" || mode === "thinking") {
          let d = Math.abs(((a + Math.PI / 2 - (mode === "thinking" ? sweep * 1.6 : sweep)) % (Math.PI * 2) + Math.PI * 3) % (Math.PI * 2) - Math.PI);
          const lit = Math.max(0, 1 - d / 0.6);
          target = 0.07 + lit * (mode === "thinking" ? 0.42 : 0.3);
        } else if (live) {
          target = 0.09 + 0.91 * Math.pow(hist[i], 1.35);
        } else {
          target = i % 18 === 0 ? 0.42 : i % 6 === 0 ? 0.24 : 0.11;
        }
        len[i] += (target - len[i]) * Math.min(1, dt * 24);
      }

      ctx.lineWidth = 1;
      ctx.lineCap = "butt";
      for (let i = 0; i < n; i++) {
        const l = len[i];
        const a = -Math.PI / 2 + (i / n) * Math.PI * 2 + (mode === "listening" || reactive ? 0 : rot);
        const k = Math.min(1, l * 1.6);
        const c = DIM.map((v, j) => Math.round(v + (GOLD[j] - v) * k));
        ctx.strokeStyle = `rgba(${c[0]},${c[1]},${c[2]},${0.55 + 0.45 * k})`;
        const cos = Math.cos(a);
        const sin = Math.sin(a);
        const a0 = r0;
        const a1 = r0 + Math.max(1.5, l * maxL);
        ctx.beginPath();
        ctx.moveTo(R + cos * a0, R + sin * a0);
        ctx.lineTo(R + cos * a1, R + sin * a1);
        ctx.stroke();
      }

      // Inner hairline and outer HUD ring: four quadrant arcs, cardinal marks.
      ctx.strokeStyle = RULE;
      ctx.beginPath();
      ctx.arc(R, R, r0 - 5, 0, Math.PI * 2);
      ctx.stroke();
      const ro = R - 1.5;
      for (let q = 0; q < 4; q++) {
        const mid = q * (Math.PI / 2) + Math.PI / 4;
        ctx.beginPath();
        ctx.arc(R, R, ro, mid - 0.55, mid + 0.55);
        ctx.stroke();
      }
      ctx.strokeStyle = `rgb(${DIM.join(",")})`;
      for (let q = 0; q < 4; q++) {
        const a = q * (Math.PI / 2);
        ctx.beginPath();
        ctx.moveTo(R + Math.cos(a) * (ro - 5), R + Math.sin(a) * (ro - 5));
        ctx.lineTo(R + Math.cos(a) * (ro + 0.5), R + Math.sin(a) * (ro + 0.5));
        ctx.stroke();
      }
      if (progress != null) {
        ctx.strokeStyle = `rgb(${GOLD.join(",")})`;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(R, R, ro, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.max(0.002, Math.min(1, progress)));
        ctx.stroke();
        ctx.lineWidth = 1;
      }

      // The diamond.
      const target = R * 0.075 * (1 + 0.45 * smooth);
      diamond += (target - diamond) * Math.min(1, dt * 26);
      const d = diamond;
      ctx.beginPath();
      ctx.moveTo(R, R - d);
      ctx.lineTo(R + d, R);
      ctx.lineTo(R, R + d);
      ctx.lineTo(R - d, R);
      ctx.closePath();
      if (mode === "hands-free") {
        ctx.strokeStyle = `rgb(${GOLD.join(",")})`;
        ctx.stroke();
      } else {
        ctx.fillStyle = mode === "idle" && !reactive ? `rgb(${DIM.join(",")})` : `rgb(${GOLD.join(",")})`;
        ctx.fill();
      }
    }
    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      io.disconnect();
    };
  });
</script>

<canvas bind:this={canvas} style="width:{size}px;height:{size}px" aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
    flex: none;
  }
</style>
