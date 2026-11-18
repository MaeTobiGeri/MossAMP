<script lang="ts">
  let { paused = false, size = 18 }: { paused?: boolean; size?: number } = $props();
</script>

<svg class:paused width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <rect x="3" y="6" width="4.5" height="14" rx="2.25" />
  <rect x="9.75" y="3" width="4.5" height="17" rx="2.25" />
  <rect x="16.5" y="9" width="4.5" height="11" rx="2.25" />
</svg>

<style>
  svg {
    fill: var(--accent);
  }

  rect {
    transform-box: fill-box;
    transform-origin: bottom;
    animation: bounce 0.9s ease-in-out infinite alternate;
  }

  rect:nth-child(2) {
    animation-delay: -0.3s;
  }

  rect:nth-child(3) {
    animation-delay: -0.6s;
  }

  .paused rect {
    animation-play-state: paused;
  }

  @keyframes bounce {
    from {
      transform: scaleY(0.35);
    }
    to {
      transform: scaleY(1);
    }
  }
</style>
