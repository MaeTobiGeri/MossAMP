<script lang="ts">
  import type { Snippet } from "svelte";
  import type { Play } from "@lucide/svelte";

  let {
    icon: Icon,
    onclick,
    variant = "filled",
    disabled = false,
    type = "button",
    children,
  }: {
    icon?: typeof Play;
    onclick?: (event: MouseEvent) => void;
    variant?: "filled" | "soft" | "danger";
    disabled?: boolean;
    type?: "button" | "submit";
    children: Snippet;
  } = $props();
</script>

<button class="pill {variant}" {type} {onclick} {disabled}>
  {#if Icon}
    <Icon size={19} strokeWidth={2.75} />
  {/if}
  <span>{@render children()}</span>
</button>

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.7rem 1.25rem;
    border: none;
    border-radius: var(--pill);
    font-weight: 800;
    white-space: nowrap;
    transition:
      transform 0.18s var(--ease-pop),
      background 0.2s;
  }

  .pill:hover:not(:disabled) {
    transform: scale(1.05);
  }

  .pill:active:not(:disabled) {
    transform: scale(0.95);
  }

  .pill:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .filled {
    background: var(--accent);
    color: var(--bg);
  }

  .soft {
    background: var(--bg-raised);
    color: var(--accent);
  }

  .danger {
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
  }
</style>
