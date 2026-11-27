<script lang="ts">
  import { api, type Track } from "./api";
  import { coverFor } from "./covers";
  import { app } from "./store.svelte";
  import NowLabel from "./NowLabel.svelte";
  import NowPlaying from "./NowPlaying.svelte";

  /**
   * Desktop: the player as a permanent column next to the song list. With
   * nothing playing it shows the first song of the library, ready to start.
   */
  let library: Track[] = $state([]);
  let backdrop: string | null = $state(null);

  let player = $derived(app.player);
  let shown = $derived(player.track ?? library[0] ?? null);

  $effect(() => {
    app.libraryVersion;
    api.getTracks().then((tracks) => (library = tracks));
  });

  $effect(() => {
    const path = shown?.path;
    backdrop = null;
    if (path) coverFor(path).then((cover) => (backdrop = cover));
  });

  function playFirst() {
    if (library.length > 0) api.playQueue(library, 0);
  }
</script>

<div class="panel">
  {#if backdrop}
    <img class="backdrop" src={backdrop} alt="" decoding="async" />
  {/if}
  <div class="shade"></div>

  <div class="content">
    <header>
      <NowLabel live={player.playing} text={player.track ? "Now playing" : "Ready to play"} />
    </header>
    <NowPlaying idle={library[0] ?? null} onplayidle={playFirst} />
  </div>
</div>

<style>
  /* The box wraps the player exactly; on short windows it is capped at the
     column height and the cover shrinks to fit. */
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    max-height: 100%;
    overflow: hidden;
    border-radius: calc(var(--radius) * 1.2);
    background: var(--bg-soft);
  }

  .backdrop {
    position: absolute;
    inset: -10%;
    width: 120%;
    height: 120%;
    object-fit: cover;
    filter: blur(40px) saturate(1.2);
    opacity: 0.45;
    transform: translateZ(0);
    animation: fade-in 0.6s ease-out;
  }

  .shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      to bottom,
      color-mix(in srgb, var(--bg) 50%, transparent),
      color-mix(in srgb, var(--bg) 90%, transparent) 75%
    );
  }

  /* The cover keeps its square shape instead of stretching, so the player
     stays one compact group. */
  .content {
    position: relative;
    flex: 0 1 auto;
    min-height: 0;
    width: 100%;
    max-width: 30rem;
    margin: 0 auto;
    padding: 1.2rem 1.6rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .content :global(.art) {
    flex: 0 1 auto;
    aspect-ratio: 1;
  }

  header {
    display: flex;
    justify-content: center;
  }

  /* Short windows: tighter spacing, a one-line title and a smaller play
     button leave more room for the cover. */
  @media (max-height: 760px) {
    .content {
      padding: 0.9rem 1.4rem 0.8rem;
      gap: 0.7rem;
    }

    .content :global(h1) {
      font-size: 1.3rem;
      -webkit-line-clamp: 1;
      line-clamp: 1;
    }

    .content :global(.icon-button.xl) {
      width: 4.2rem;
      height: 4.2rem;
    }

    .content :global(.icon-button.lg) {
      width: 3rem;
      height: 3rem;
    }
  }

  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
</style>
