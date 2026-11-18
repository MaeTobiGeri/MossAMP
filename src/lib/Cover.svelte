<script lang="ts">
  import { Music } from "@lucide/svelte";
  import { coverFor } from "./covers";

  let { path, size, rounded = "small" }: { path: string; size?: number; rounded?: "small" | "large" } = $props();

  let source: string | null = $state(null);

  $effect(() => {
    const current = path;
    source = null;
    coverFor(current).then((cover) => {
      if (current === path) source = cover;
    });
  });

  let dimensions = $derived(size ? `width: ${size}px; height: ${size}px` : "width: 100%; aspect-ratio: 1");
</script>

<div class="cover {rounded}" style={dimensions}>
  {#if source}
    <img src={source} alt="" decoding="async" />
  {:else}
    <Music size={size ? size * 0.42 : 72} strokeWidth={2.5} />
  {/if}
</div>

<style>
  .cover {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    overflow: hidden;
    background: var(--bg-raised);
    color: var(--accent-soft);
  }

  .small {
    border-radius: var(--radius-small);
  }

  .large {
    border-radius: calc(var(--radius) * 1.4);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.5);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
