<script lang="ts">
  let {
    checked,
    label,
    onchange,
  }: { checked: boolean; label: string; onchange: (checked: boolean) => void } = $props();
</script>

<button class="toggle" class:on={checked} role="switch" aria-checked={checked} onclick={() => onchange(!checked)}>
  <span class="track"><span class="knob"></span></span>
  <span class="label">{label}</span>
</button>

<style>
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .track {
    flex-shrink: 0;
    position: relative;
    width: 3.2rem;
    height: 1.9rem;
    border-radius: var(--pill);
    background: var(--bg-raised);
    transition: background 0.25s;
  }

  .knob {
    position: absolute;
    top: 0.25rem;
    left: 0.25rem;
    width: 1.4rem;
    height: 1.4rem;
    border-radius: 50%;
    background: var(--fg-muted);
    transition:
      transform 0.3s var(--ease-pop),
      background 0.25s;
  }

  .on .track {
    background: var(--accent);
  }

  .on .knob {
    transform: translateX(1.3rem);
    background: var(--bg);
  }
</style>
