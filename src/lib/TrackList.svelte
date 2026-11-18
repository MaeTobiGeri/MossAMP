<script lang="ts">
  import { Ellipsis, ListEnd, ListMinus, ListPlus, ListStart } from "@lucide/svelte";
  import { api, formatTime, type Track } from "./api";
  import { app, notify, playAndOpen, refreshPlaylists } from "./store.svelte";
  import Cover from "./Cover.svelte";
  import IconButton from "./IconButton.svelte";
  import PlayingBars from "./PlayingBars.svelte";

  let {
    tracks,
    playlistId = null,
    onchange,
  }: { tracks: Track[]; playlistId?: number | null; onchange?: () => void } = $props();

  let menuTrack: Track | null = $state(null);

  function toggleMenu(event: MouseEvent, track: Track) {
    event.stopPropagation();
    menuTrack = menuTrack?.id === track.id ? null : track;
  }

  async function addToPlaylist(playlist: number, track: Track) {
    await api.addToPlaylist(playlist, track.id);
    await refreshPlaylists();
    notify(`Added “${track.title}”`);
  }

  async function removeFromPlaylist(track: Track) {
    if (playlistId === null) return;
    await api.removeFromPlaylist(playlistId, track.id);
    await refreshPlaylists();
    onchange?.();
  }

  function queueNext(track: Track) {
    api.playNext(track);
    notify("Plays next");
  }

  function queueLast(track: Track) {
    api.enqueueTrack(track);
    notify("Added to queue");
  }
</script>

<svelte:window onclick={() => (menuTrack = null)} />

<ul>
  {#each tracks as track, index (track.id)}
    {@const playing = app.player.track?.id === track.id}
    <li class:playing>
      <button class="row" onclick={() => playAndOpen(tracks, index)}>
        <div class="art">
          <Cover path={track.path} size={50} />
          {#if playing}
            <div class="bars"><PlayingBars paused={!app.player.playing} /></div>
          {/if}
        </div>
        <span class="text">
          <span class="title">{track.title}</span>
          <span class="meta">{track.artist} · {track.album}</span>
        </span>
        <span class="time">{formatTime(track.duration)}</span>
      </button>
      <IconButton icon={Ellipsis} label="More" size="sm" onclick={(e) => toggleMenu(e, track)} />

      {#if menuTrack?.id === track.id}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="menu" onclick={(e) => e.stopPropagation()}>
          <button onclick={() => queueNext(track)}><ListStart size={18} strokeWidth={2.75} />Play next</button>
          <button onclick={() => queueLast(track)}><ListEnd size={18} strokeWidth={2.75} />Add to queue</button>
          {#each app.playlists as playlist}
            <button onclick={() => addToPlaylist(playlist.id, track)}>
              <ListPlus size={18} strokeWidth={2.75} />{playlist.name}
            </button>
          {/each}
          {#if playlistId !== null}
            <button class="danger" onclick={() => removeFromPlaylist(track)}>
              <ListMinus size={18} strokeWidth={2.75} />Remove from playlist
            </button>
          {/if}
        </div>
      {/if}
    </li>
  {:else}
    <li class="empty">No songs here yet.</li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  li {
    position: relative;
    display: flex;
    align-items: center;
    padding-right: 0.4rem;
    border-radius: var(--radius);
    transition: background 0.2s;
  }

  li:hover {
    background: var(--bg-soft);
  }

  li.playing {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-soft));
  }

  li.playing .title {
    color: var(--accent);
  }

  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.5rem 0.6rem;
    border: none;
    background: none;
    text-align: left;
  }

  .art {
    position: relative;
  }

  .bars {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: var(--radius-small);
    background: rgba(0, 0, 0, 0.55);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title,
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-weight: 800;
  }

  .meta,
  .time {
    color: var(--fg-muted);
    font-size: 0.85rem;
  }

  .time {
    font-variant-numeric: tabular-nums;
  }

  .menu {
    position: absolute;
    right: 0.5rem;
    top: calc(100% - 0.4rem);
    z-index: 10;
    min-width: 13rem;
    display: flex;
    flex-direction: column;
    padding: 0.4rem;
    border-radius: var(--radius);
    background: var(--bg-raised);
    box-shadow: var(--shadow);
    animation: menu-in 0.25s var(--ease-pop);
    transform-origin: top right;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.65rem 0.9rem;
    border: none;
    border-radius: var(--pill);
    background: none;
    font-weight: 700;
    text-align: left;
  }

  .menu button:hover {
    background: var(--bg-soft);
    color: var(--accent);
  }

  .menu .danger {
    color: var(--danger);
  }

  .empty {
    justify-content: center;
    padding: 3rem 1rem;
    color: var(--fg-muted);
  }

  @keyframes menu-in {
    from {
      opacity: 0;
      transform: scale(0.85);
    }
  }
</style>
