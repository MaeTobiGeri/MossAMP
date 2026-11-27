<script lang="ts">
  /** "Now playing" heading; a soft highlight sweeps through it while music plays. */
  let { live, text = "Now playing" }: { live: boolean; text?: string } = $props();
</script>

<span class="now" class:live>{text}</span>

<style>
  .now {
    color: var(--fg-muted);
    font-size: 0.8rem;
    font-weight: 800;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  /* The text keeps its usual muted colour; only the light band moves. */
  .now.live {
    background: linear-gradient(
        90deg,
        var(--fg-muted) 0%,
        var(--fg-muted) 40%,
        color-mix(in srgb, var(--fg-muted) 40%, var(--fg)) 50%,
        var(--fg-muted) 60%,
        var(--fg-muted) 100%
      )
      0 0 / 250% 100% no-repeat;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: now-sweep 3.5s ease-in-out infinite;
  }

  @keyframes now-sweep {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: 0% 0;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .now.live {
      animation: none;
      background: none;
      color: inherit;
    }
  }
</style>
