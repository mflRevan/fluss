<script lang="ts">
  // Determinate when `value` is 0..1, sweeping otherwise. Hairline gold arc.
  let { value = null, size = 40, stroke = 2 }: { value?: number | null; size?: number; stroke?: number } = $props();
  const r = $derived((size - stroke) / 2);
  const c = $derived(2 * Math.PI * r);
</script>

<svg width={size} height={size} viewBox="0 0 {size} {size}" class:spin={value == null}>
  <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--rule-strong)" stroke-width="1" />
  <circle
    cx={size / 2}
    cy={size / 2}
    {r}
    fill="none"
    stroke="var(--gold)"
    stroke-width={stroke}
    stroke-dasharray={c}
    stroke-dashoffset={value == null ? c * 0.78 : c * (1 - Math.max(0, Math.min(1, value)))}
    transform="rotate(-90 {size / 2} {size / 2})"
    style="transition: stroke-dashoffset 200ms var(--q)"
  />
</svg>

<style>
  svg {
    flex: none;
  }
  .spin {
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
