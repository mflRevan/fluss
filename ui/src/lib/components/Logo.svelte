<script lang="ts">
  // The Fluss mark: a gold diamond inside a hairline diamond — the iris at rest.
  let { size = 22 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-label="Fluss">
  <path d="M12 1.5 22.5 12 12 22.5 1.5 12Z" fill="none" stroke="#8c6b2a" stroke-width="1" />
  <path d="M12 7 17 12 12 17 7 12Z" fill="#e3b04b" />
</svg>
