<script lang="ts">
  // Precision slider: hairline track with a scale, diamond thumb, mono readout.
  // Commits on release only (no setting spam while dragging).
  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    unit = "",
    format,
    onchange,
    width = 200,
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

<div class="slider" style="--w:{width}px">
  <div class="rail">
    <div class="scale"></div>
    <div class="track"></div>
    <div class="fill" style="width:{pct}%"></div>
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
    <div class="thumb" class:active={dragging} style="left:{pct}%"></div>
  </div>
  <span class="value">{label}</span>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 14px;
    width: var(--w);
    height: 26px;
  }
  .rail {
    position: relative;
    flex: 1;
    height: 26px;
  }
  .scale {
    position: absolute;
    left: 0;
    right: 0;
    top: 17px;
    height: 4px;
    background: repeating-linear-gradient(90deg, var(--rule) 0 1px, transparent 1px 10%);
    border-right: 1px solid var(--rule);
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: 12px;
    height: 1px;
    background: var(--rule-strong);
  }
  .fill {
    position: absolute;
    left: 0;
    top: 12px;
    height: 1px;
    background: var(--gold);
  }
  input {
    position: absolute;
    inset: 0;
    width: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .thumb {
    position: absolute;
    top: 7.5px;
    width: 10px;
    height: 10px;
    translate: -50% 0;
    rotate: 45deg;
    background: var(--void);
    border: 1px solid var(--gold);
    pointer-events: none;
    transition: background var(--t1) var(--q), scale var(--t1) var(--q);
  }
  .rail:hover .thumb,
  .thumb.active {
    background: var(--gold);
  }
  .thumb.active {
    scale: 1.2;
  }
  .value {
    flex: none;
    min-width: 62px;
    text-align: right;
    font-family: var(--mono);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    color: var(--gold);
  }
</style>
