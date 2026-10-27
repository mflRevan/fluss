<script lang="ts">
  let {
    checked = false,
    onchange,
    disabled = false,
    label = "",
  }: { checked?: boolean; onchange?: (v: boolean) => void; disabled?: boolean; label?: string } = $props();
</script>

<button class="toggle" class:on={checked} role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={() => onchange?.(!checked)}>
  <span class="txt">{checked ? "ON" : "OFF"}</span>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 56px;
    height: 22px;
    flex: none;
    padding: 0;
    border: 1px solid var(--rule-strong);
    background: var(--void);
    transition: border-color var(--t1) var(--q), background var(--t1) var(--q);
  }
  .toggle:hover {
    border-color: var(--gold-dim);
  }
  .toggle.on {
    border-color: var(--gold);
    background: var(--gold-wash);
  }
  .toggle:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .txt {
    position: absolute;
    top: 50%;
    translate: 0 -50%;
    right: 7px;
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    color: var(--text-3);
    transition: right var(--t2) var(--q), color var(--t1);
  }
  .on .txt {
    right: 27px;
    color: var(--gold);
  }
  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 14px;
    height: 14px;
    background: var(--text-3);
    transition: transform var(--t2) var(--q), background var(--t1);
  }
  .on .knob {
    transform: translateX(34px);
    background: var(--gold);
  }
</style>
