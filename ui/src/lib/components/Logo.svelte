<script lang="ts">
  // The Fluss mark: three flowing strokes in a rounded tile.
  let { size = 28 }: { size?: number } = $props();
  const id = `lg${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-label="Fluss">
  <defs>
    <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#0fb5a4" />
      <stop offset="55%" stop-color="#22d3ee" />
      <stop offset="100%" stop-color="#8b7cf6" />
    </linearGradient>
  </defs>
  <rect x="2" y="2" width="60" height="60" rx="17" fill="url(#{id})" />
  <g fill="none" stroke="#fff" stroke-linecap="round" stroke-width="5">
    <path d="M14 24c6-6 12-6 18 0s12 6 18 0" opacity="0.55" />
    <path d="M14 34c6-6 12-6 18 0s12 6 18 0" />
    <path d="M14 44c6-6 12-6 18 0s12 6 18 0" opacity="0.55" />
  </g>
</svg>
