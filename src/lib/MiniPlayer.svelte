<script lang="ts">
  import { slide } from "svelte/transition";
  import { Pause, Play, SkipBack, SkipForward } from "@lucide/svelte";
  import { api } from "./api";
  import { app } from "./store.svelte";
  import Cover from "./Cover.svelte";
  import IconButton from "./IconButton.svelte";

  let player = $derived(app.player);
  let progress = $derived(player.duration > 0 ? (player.position / player.duration) * 100 : 0);
</script>

{#if player.track}
  <div class="mini" transition:slide={{ duration: 320 }}>
    <div class="row">
      <button class="open" onclick={() => (app.fullPlayerOpen = true)} aria-label="Open player">
        <Cover path={player.track.path} size={46} />
        <span class="text">
          <span class="title">{player.track.title}</span>
          <span class="artist">{player.track.artist}</span>
        </span>
      </button>

      <div class="controls">
        <IconButton icon={SkipBack} label="Previous" size="sm" solid onclick={() => api.previous()} />
        <IconButton
          icon={player.playing ? Pause : Play}
          label={player.playing ? "Pause" : "Play"}
          variant="filled"
          solid
          onclick={() => api.toggle()}
        />
        <IconButton icon={SkipForward} label="Next" size="sm" solid onclick={() => api.next()} />
      </div>
    </div>

    <div class="progress"><div class="fill" style="transform: scaleX({progress / 100})"></div></div>
  </div>
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.55rem 0.5rem;
  }

  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  .text {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title,
  .artist {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-weight: 800;
  }

  .artist {
    color: var(--fg-muted);
    font-size: 0.85rem;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .progress {
    height: 4px;
    margin: 0 1rem;
    border-radius: var(--pill);
    background: var(--bg-raised);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: var(--pill);
    background: var(--accent);
    transform-origin: left;
    transition: transform 0.25s linear;
  }
</style>
