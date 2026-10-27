<script lang="ts">
  // Range slider that only commits on release (no setting spam while dragging).
  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    unit = "",
    format,
    onchange,
    width = 220,
  }: {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    unit?: string;
    format?: (v: number) => string;
    onchange?: (v: number) => void;
    width?: number;
  } = $props();

  let local = $state(0);
  let dragging = $state(false);
  $effect(() => {
    if (!dragging) local = value;
  });
  const pct = $derived(((local - min) / (max - min)) * 100);
  const label = $derived(format ? format(local) : `${local}${unit}`);
</script>

<div class="slider" style="width:{width}px">
  <div class="rail">
  <div class="track">
    <div class="fill" style="width:{pct}%"></div>
  </div>
  <input
    type="range"
    {min}
    {max}
    {step}
    bind:value={local}
    oninput={() => (dragging = true)}
    onchange={() => {
      dragging = false;
      onchange?.(Number(local));
    }}
  />
  <div class="thumb" class:active={dragging} style="left:calc({pct}% - {(pct / 100) * 18}px)"></div>
  </div>
  <span class="value">{label}</span>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 28px;
  }
  .rail {
    position: relative;
    flex: 1;
    height: 28px;
    display: flex;
    align-items: center;
  }
  .track {
    width: 100%;
    height: 6px;
    border-radius: 6px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--flow);
    border-radius: 6px;
  }
  input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 28px;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .thumb {
    position: absolute;
    top: 5px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
    pointer-events: none;
    transition: box-shadow var(--t-fast), transform 200ms var(--spring);
  }
  .slider:hover .thumb,
  .thumb.active {
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.35),
      0 0 0 5px rgba(45, 212, 191, 0.22);
  }
  .thumb.active {
    transform: scale(1.12);
  }
  .value {
    flex: none;
    min-width: 58px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-size: 12.5px;
    color: var(--text-2);
    font-weight: 550;
  }
</style>
