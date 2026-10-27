<script lang="ts">
  // The status orb: one shape whose motion tells you the state at a glance.
  import { app } from "../state.svelte";
  import type { DictationPhase, ModelPhase } from "../types";

  let { model = "ready", phase = "idle", progress = null, size = 132 }: {
    model?: ModelPhase;
    phase?: DictationPhase;
    progress?: number | null;
    size?: number;
  } = $props();

  const mode = $derived(
    phase === "listening" || phase === "hands-free"
      ? "listening"
      : phase === "finishing"
        ? "finishing"
        : model === "installing" || model === "loading"
          ? "busy"
          : model === "ready"
            ? "ready"
            : model === "error"
              ? "error"
              : "off",
  );
  const lvl = $derived(mode === "listening" ? Math.min(1, Math.sqrt(app.level.rms) * 1.4) : 0);
  const r = 46;
  const c = 2 * Math.PI * r;
</script>

<div class="orb {mode}" style="--s:{size}px;--lvl:{lvl}">
  <div class="halo"></div>
  {#if mode === "listening"}
    <span class="ring r1"></span>
    <span class="ring r2"></span>
    <span class="ring r3"></span>
  {/if}
  <div class="core">
    <div class="swirl"></div>
    <div class="gloss"></div>
  </div>
  <svg class="arc" viewBox="0 0 100 100">
    {#if mode === "busy"}
      <circle
        cx="50"
        cy="50"
        {r}
        fill="none"
        stroke="url(#orbg)"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-dasharray={c}
        stroke-dashoffset={progress == null ? c * 0.72 : c * (1 - progress)}
        class:spin={progress == null}
        transform="rotate(-90 50 50)"
      />
    {/if}
    <defs>
      <linearGradient id="orbg" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stop-color="#2dd4bf" />
        <stop offset="100%" stop-color="#a78bfa" />
      </linearGradient>
    </defs>
  </svg>
</div>

<style>
  .orb {
    position: relative;
    width: var(--s);
    height: var(--s);
    flex: none;
    display: grid;
    place-items: center;
  }
  .halo {
    position: absolute;
    inset: -30%;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(34, 211, 238, 0.28), rgba(139, 124, 246, 0.1) 45%, transparent 70%);
    filter: blur(6px);
    opacity: 0.7;
    transition: opacity 600ms var(--ease), transform 600ms var(--ease);
    animation: breathe 5s ease-in-out infinite;
  }
  .core {
    position: relative;
    width: 64%;
    height: 64%;
    border-radius: 50%;
    overflow: hidden;
    box-shadow:
      0 0 0 1px rgba(255, 255, 255, 0.12) inset,
      0 20px 50px -10px rgba(34, 211, 238, 0.55);
    transform: scale(calc(1 + var(--lvl) * 0.12));
    transition:
      transform 90ms linear,
      filter 600ms var(--ease),
      box-shadow 600ms var(--ease);
  }
  .swirl {
    position: absolute;
    inset: -30%;
    background: conic-gradient(from 0deg, #0fb5a4, #22d3ee, #7c8cf8, #a78bfa, #22d3ee, #0fb5a4);
    animation: rot 9s linear infinite;
    filter: blur(8px) saturate(1.15);
  }
  .gloss {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background:
      radial-gradient(circle at 32% 26%, rgba(255, 255, 255, 0.55), transparent 32%),
      radial-gradient(circle at 70% 85%, rgba(7, 11, 16, 0.45), transparent 55%);
  }
  .arc {
    position: absolute;
    inset: 4%;
    width: 92%;
    height: 92%;
    overflow: visible;
  }
  .arc circle {
    transition: stroke-dashoffset 300ms var(--ease);
  }
  .arc .spin {
    transform-origin: 50% 50%;
    animation: rot 1.1s linear infinite;
  }
  .ring {
    position: absolute;
    width: 64%;
    height: 64%;
    border-radius: 50%;
    border: 1.5px solid rgba(45, 212, 191, 0.55);
    animation: ripple 2.4s var(--ease) infinite;
  }
  .r2 {
    animation-delay: 0.8s;
  }
  .r3 {
    animation-delay: 1.6s;
  }

  .off .core,
  .error .core {
    filter: grayscale(0.9) brightness(0.6);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.1) inset;
  }
  .off .halo,
  .error .halo {
    opacity: 0.15;
  }
  .error .core {
    filter: grayscale(0.4) hue-rotate(150deg) brightness(0.8);
  }
  .busy .swirl {
    animation-duration: 2.8s;
  }
  .busy .core {
    filter: saturate(0.7) brightness(0.85);
  }
  .listening .halo {
    opacity: 1;
    transform: scale(calc(1.05 + var(--lvl) * 0.25));
  }
  .listening .swirl {
    animation-duration: 3s;
  }
  .finishing .swirl {
    animation-duration: 1.2s;
  }

  @keyframes rot {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes breathe {
    50% {
      transform: scale(1.06);
      opacity: 0.85;
    }
  }
  @keyframes ripple {
    from {
      transform: scale(1);
      opacity: 0.9;
    }
    to {
      transform: scale(1.75);
      opacity: 0;
    }
  }
</style>
