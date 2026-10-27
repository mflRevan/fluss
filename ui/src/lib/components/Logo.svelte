<script lang="ts">
  // The Fluss mark: the aperture at rest (six blades, a hexagonal pinhole, the gold diamond).
  let { size = 22 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-label="Fluss">
  <circle cx="12" cy="12" r="10.5" fill="#e3b04b" fill-opacity="0.12" stroke="#8c6b2a" stroke-width="1" />
  <path d="M16.12 12.11L9.88 22.28M13.97 15.62L2.03 15.31M9.85 15.51L4.15 5.02M7.88 11.89L14.12 1.72M10.03 8.38L21.97 8.69M14.15 8.49L19.85 18.98" stroke="#8c6b2a" stroke-width="0.8" />
  <path d="M13.97 15.62 9.85 15.51 7.88 11.89 10.03 8.38 14.15 8.49 16.12 12.11Z" fill="#000" stroke="#e3b04b" stroke-width="1" stroke-linejoin="miter" />
  <path d="M12 9.9 14.1 12 12 14.1 9.9 12Z" fill="#e3b04b" />
</svg>
