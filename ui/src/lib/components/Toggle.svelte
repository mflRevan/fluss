<script lang="ts">
  let {
    checked = false,
    onchange,
    disabled = false,
    label = "",
  }: { checked?: boolean; onchange?: (v: boolean) => void; disabled?: boolean; label?: string } = $props();
</script>

<button
  class="toggle"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange?.(!checked)}
>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 40px;
    height: 24px;
    flex: none;
    border-radius: 999px;
    border: 1px solid var(--border-2);
    background: var(--surface-3);
    padding: 0;
    transition:
      background var(--t-med) var(--ease),
      border-color var(--t-med),
      box-shadow var(--t-med);
  }
  .toggle.on {
    background: var(--flow);
    border-color: transparent;
    box-shadow: 0 4px 14px -6px rgba(34, 211, 238, 0.7);
  }
  .toggle:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    transition:
      transform 360ms var(--spring),
      width 160ms var(--ease);
  }
  .toggle:active .knob {
    width: 22px;
  }
  .toggle.on .knob {
    transform: translateX(16px);
  }
  .toggle.on:active .knob {
    transform: translateX(12px);
  }
</style>
