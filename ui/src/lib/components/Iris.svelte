<script module lang="ts">
  export type IrisMode = "idle" | "listening" | "hands-free" | "loading" | "thinking";
</script>

<script lang="ts">
  // The voice visual: the aperture, a six-blade camera iris. It opens with the
  // voice and twists as it opens, closes to a pinhole (showing the gold diamond)
  // in silence and spins while transcribing or loading. Hands-free dashes the rim.
  // Same geometry as the overlay (crates/fluss-daemon/src/overlay/render.rs) and
  // the website. See docs/DESIGN.md ("aperture").
  import { onMount } from "svelte";
  import { app } from "../state.svelte";

  let {
    size = 200,
    mode = "idle",
    progress = null,
    reactive = false,
  }: {
    size?: number;
    mode?: IrisMode;
    /** 0..1 draws a progress arc on the rim and opens the blades with it. */
    progress?: number | null;
    /** React to the mic level even when idle (mic test). */
    reactive?: boolean;
  } = $props();

  let canvas: HTMLCanvasElement;

  const TAU = Math.PI * 2;
  const GOLD = "rgb(227,176,75)";
  const DIM = "rgb(140,107,42)";
  const gold = (a: number) => `rgba(227,176,75,${a})`;

  onMount(() => {
    const ctx = canvas.getContext("2d")!;
    const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let raf = 0;
    let last = performance.now();
    let visible = true;
    let peak = 0.05;
    let level = 0;
    let open = 0.13;
    let rot = 0;

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
      const dpr = window.devicePixelRatio || 1;
      const px = Math.round(size * dpr);
      if (canvas.width !== px) {
        canvas.width = px;
        canvas.height = px;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, size, size);

      const live = mode === "listening" || mode === "hands-free" || reactive;
      const busy = mode === "loading" || mode === "thinking";
      const raw = app.level?.rms ?? 0;
      peak = Math.max(peak * Math.pow(0.8, elapsed), raw, 0.04);
      const norm = Math.min(1, Math.pow(raw / peak, 0.8));
      level += ((live ? norm : 0) - level) * Math.min(1, dt * (norm > level ? 28 : 7));

      const target =
        progress != null && mode === "loading"
          ? 0.12 + 0.55 * Math.min(1, Math.max(0, progress))
          : busy
            ? 0.2 + 0.05 * Math.sin(t * 10)
            : live
              ? 0.13 + 0.72 * level
              : 0.13;
      open = reduce ? target : open + (target - open) * Math.min(1, dt * 18);
      if (!reduce) rot += dt * (busy ? 4.2 : mode === "hands-free" ? 0.7 : 0.2);

      const c = size / 2;
      const R = size * 0.42;
      const B = 6;
      const half = Math.PI / B;
      const twist = rot + open * 1.3;
      const vr = (R * open) / Math.cos(half);
      const lw = Math.max(1, size * 0.008);
      const hex: [number, number][] = [];
      for (let i = 0; i < B; i++) {
        const a = twist + (i * TAU) / B + half;
        hex.push([c + Math.cos(a) * vr, c + Math.sin(a) * vr]);
      }
      const hexPath = () => {
        ctx.beginPath();
        hex.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
        ctx.closePath();
      };

      // Blade disc, with the opening cut out so the page shows through.
      ctx.fillStyle = gold(0.09);
      ctx.beginPath();
      ctx.arc(c, c, R, 0, TAU);
      ctx.fill();
      ctx.globalCompositeOperation = "destination-out";
      hexPath();
      ctx.fill();
      ctx.globalCompositeOperation = "source-over";

      // Blade edges: rays leaving each hexagon vertex tangentially, ending on the rim.
      ctx.lineWidth = lw;
      ctx.strokeStyle = DIM;
      for (let i = 0; i < B; i++) {
        const th = twist + (i * TAU) / B;
        const [vx, vy] = hex[(i + B - 1) % B];
        const dx = -Math.sin(th);
        const dy = Math.cos(th);
        const ox = vx - c;
        const oy = vy - c;
        const b = ox * dx + oy * dy;
        const u = -b + Math.sqrt(Math.max(0, b * b - (ox * ox + oy * oy - R * R)));
        ctx.beginPath();
        ctx.moveTo(vx, vy);
        ctx.lineTo(vx + dx * u, vy + dy * u);
        ctx.stroke();
      }
      ctx.strokeStyle = live || busy ? GOLD : DIM;
      hexPath();
      ctx.stroke();

      // Rim: dashed, turning and gold in hands-free.
      ctx.strokeStyle = mode === "hands-free" ? GOLD : DIM;
      if (mode === "hands-free") {
        ctx.setLineDash([R * 0.08, R * 0.06]);
        ctx.lineDashOffset = reduce ? 0 : -t * R * 0.3;
      }
      ctx.beginPath();
      ctx.arc(c, c, R, 0, TAU);
      ctx.stroke();
      ctx.setLineDash([]);

      // Lens markings.
      ctx.strokeStyle = gold(0.35);
      ctx.lineWidth = Math.max(1, size * 0.005);
      for (let i = 0; i < 24; i++) {
        const a = (i * TAU) / 24;
        const l = i % 2 ? 0.05 : 0.1;
        ctx.beginPath();
        ctx.moveTo(c + Math.cos(a) * R * 1.04, c + Math.sin(a) * R * 1.04);
        ctx.lineTo(c + Math.cos(a) * R * (1.04 + l), c + Math.sin(a) * R * (1.04 + l));
        ctx.stroke();
      }

      if (progress != null) {
        ctx.strokeStyle = GOLD;
        ctx.lineWidth = Math.max(1.5, size * 0.016);
        ctx.beginPath();
        ctx.arc(c, c, R, -Math.PI / 2, -Math.PI / 2 + TAU * Math.max(0.002, Math.min(1, progress)));
        ctx.stroke();
      }

      // The diamond shows through the pinhole when nearly closed.
      const show = Math.max(0, 1 - open / 0.3);
      if (show > 0) {
        const d = Math.max(1.2, R * open * 0.45);
        ctx.fillStyle = gold(0.95 * show);
        ctx.beginPath();
        ctx.moveTo(c, c - d);
        ctx.lineTo(c + d, c);
        ctx.lineTo(c, c + d);
        ctx.lineTo(c - d, c);
        ctx.closePath();
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
