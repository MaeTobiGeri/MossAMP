<script lang="ts">
  import {
    Pause,
    Play,
    Repeat,
    Repeat1,
    Shuffle,
    SkipBack,
    SkipForward,
    Volume1,
    Volume2,
    VolumeX,
  } from "@lucide/svelte";
  import { api, formatTime, type Repeat as RepeatMode, type Track } from "./api";
  import { app } from "./store.svelte";
  import Cover from "./Cover.svelte";
  import IconButton from "./IconButton.svelte";

  /**
   * Cover, title, seek bar and controls of the current song. Shared by the
   * full-screen player (phone) and the side panel (desktop).
   *
   * With nothing playing, `idle` is shown instead and the play button calls
   * `onplayidle`.
   */
  let { idle = null, onplayidle }: { idle?: Track | null; onplayidle?: () => void } = $props();

  const nextRepeat: Record<RepeatMode, RepeatMode> = { off: "all", all: "one", one: "off" };

  let player = $derived(app.player);
  let track = $derived(player.track ?? idle);
  let active = $derived(player.track !== null);
  let duration = $derived(active ? player.duration : (idle?.duration ?? 0));

  let dragging: number | null = $state(null);
  let position = $derived(active ? (dragging ?? player.position) : 0);
  let progress = $derived(duration > 0 ? (position / duration) * 100 : 0);

  /** On Android the phone's own volume buttons control loudness. */
  const showVolume = !("MossampAndroid" in window);
  /** Value while the volume slider is being dragged; the player's own updates are ignored meanwhile. */
  let volumeDrag: number | null = $state(null);
  let volume = $derived(volumeDrag ?? player.volume);
  let volumeIcon = $derived(volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2);

  function dragVolume(value: number) {
    volumeDrag = value;
    api.previewVolume(value);
  }

  async function commitVolume(value: number) {
    await api.setVolume(value);
    volumeDrag = null;
  }

  function commitSeek(value: number) {
    api.seek(value);
    dragging = null;
  }

  function playPause() {
    if (active) api.toggle();
    else onplayidle?.();
  }
</script>

{#if track}
  <div class="art">
    <Cover path={track.path} rounded="large" />
  </div>

  <div class="details">
    <h1>{track.title}</h1>
    <p>{track.artist} · {track.album}</p>
  </div>

  <div class="seek">
    <input
      type="range"
      min="0"
      max={duration || 1}
      step="0.5"
      value={position}
      disabled={!active}
      style="--progress: {progress}%"
      oninput={(e) => (dragging = Number(e.currentTarget.value))}
      onchange={(e) => commitSeek(Number(e.currentTarget.value))}
    />
    <div class="times">
      <span>{formatTime(position)}</span>
      <span>{formatTime(duration)}</span>
    </div>
  </div>

  <div class="controls">
    <IconButton
      icon={Shuffle}
      label="Shuffle"
      active={player.shuffle}
      onclick={() => api.setShuffle(!player.shuffle)}
    />
    <IconButton
      icon={SkipBack}
      label="Previous"
      variant="soft"
      size="lg"
      solid
      disabled={!active}
      onclick={() => api.previous()}
    />
    <IconButton
      icon={player.playing ? Pause : Play}
      label={player.playing ? "Pause" : "Play"}
      variant="filled"
      size="xl"
      solid
      onclick={playPause}
    />
    <IconButton
      icon={SkipForward}
      label="Next"
      variant="soft"
      size="lg"
      solid
      disabled={!active}
      onclick={() => api.next()}
    />
    <IconButton
      icon={player.repeat === "one" ? Repeat1 : Repeat}
      label="Repeat: {player.repeat}"
      active={player.repeat !== "off"}
      onclick={() => api.setRepeat(nextRepeat[player.repeat])}
    />
  </div>

  {#if showVolume}
    <div class="volume">
      <IconButton
        icon={volumeIcon}
        label="Mute"
        size="sm"
        onclick={() => api.setVolume(volume > 0 ? 0 : 0.8)}
      />
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={volume}
        style="--progress: {volume * 100}%"
        oninput={(e) => dragVolume(Number(e.currentTarget.value))}
        onchange={(e) => commitVolume(Number(e.currentTarget.value))}
      />
    </div>
  {/if}
{:else}
  <div class="empty">
    <p>No songs yet. Add a music folder in Settings.</p>
  </div>
{/if}

<style>
  .art {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    container-type: size;
  }

  .art :global(.cover) {
    width: min(100cqw, 100cqh) !important;
  }

  .details {
    text-align: center;
  }

  h1 {
    font-size: 1.6rem;
    font-weight: 900;
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .details p {
    margin-top: 0.3rem;
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .seek input {
    width: 100%;
  }

  .seek input:disabled {
    opacity: 0.5;
  }

  .times {
    display: flex;
    justify-content: space-between;
    margin-top: 0.4rem;
    color: var(--fg-muted);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .volume {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .volume input {
    flex: 1;
  }

  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--fg-muted);
    text-align: center;
  }
</style>
