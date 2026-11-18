<script lang="ts">
  import type { Play } from "@lucide/svelte";

  type Variant = "filled" | "soft" | "ghost";
  type Size = "sm" | "md" | "lg" | "xl";

  let {
    icon: Icon,
    label,
    onclick,
    variant = "ghost",
    size = "md",
    active = false,
    solid = false,
    disabled = false,
  }: {
    icon: typeof Play;
    label: string;
    onclick?: (event: MouseEvent) => void;
    variant?: Variant;
    size?: Size;
    active?: boolean;
    solid?: boolean;
    disabled?: boolean;
  } = $props();

  const iconSizes: Record<Size, number> = { sm: 18, md: 22, lg: 26, xl: 34 };
</script>

<button class="icon-button {variant} {size}" class:active aria-label={label} title={label} {onclick} {disabled}>
  <Icon size={iconSizes[size]} strokeWidth={2.75} fill={solid ? "currentColor" : "none"} />
</button>

<style>
  .icon-button {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border: none;
    border-radius: var(--pill);
    transition:
      transform 0.18s var(--ease-pop),
      background 0.2s,
      color 0.2s;
  }

  .icon-button:hover:not(:disabled) {
    transform: scale(1.08);
  }

  .icon-button:active:not(:disabled) {
    transform: scale(0.92);
  }

  .icon-button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .sm {
    width: 2.25rem;
    height: 2.25rem;
  }

  .md {
    width: 2.9rem;
    height: 2.9rem;
  }

  .lg {
    width: 3.6rem;
    height: 3.6rem;
  }

  .xl {
    width: 5.2rem;
    height: 5.2rem;
  }

  .filled {
    background: var(--accent);
    color: var(--bg);
    box-shadow: 0 6px 18px color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .soft {
    background: var(--bg-raised);
    color: var(--accent);
  }

  .ghost {
    background: transparent;
    color: var(--fg-muted);
  }

  .ghost:hover:not(:disabled) {
    color: var(--accent);
  }

  .ghost.active,
  .soft.active {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--accent);
  }
</style>
