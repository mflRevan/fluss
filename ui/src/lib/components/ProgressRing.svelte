<script lang="ts">
  // Determinate when `value` is a number 0..1, spinning otherwise.
  let { value = null, size = 40, stroke = 4 }: { value?: number | null; size?: number; stroke?: number } = $props();
  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
  const id = `pr${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg width={size} height={size} viewBox="0 0 {size} {size}" class:spin={value == null}>
  <defs>
    <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#14b8a6" />
      <stop offset="50%" stop-color="#22d3ee" />
      <stop offset="100%" stop-color="#8b7cf6" />
    </linearGradient>
  </defs>
  <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--surface-3)" stroke-width={stroke} />
  <circle
    cx={size / 2}
    cy={size / 2}
    {r}
    fill="none"
    stroke="url(#{id})"
    stroke-width={stroke}
    stroke-linecap="round"
    stroke-dasharray={c}
    stroke-dashoffset={value == null ? c * 0.7 : c * (1 - Math.max(0, Math.min(1, value)))}
    transform="rotate(-90 {size / 2} {size / 2})"
    style="transition: stroke-dashoffset 300ms var(--ease)"
  />
</svg>

<style>
  svg {
    flex: none;
  }
  .spin {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
