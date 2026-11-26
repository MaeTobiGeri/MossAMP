<script lang="ts">
  import { Check, Search, X } from "@lucide/svelte";
  import { fade, fly } from "svelte/transition";
  import { api, formatTime, type Playlist, type Track } from "./api";
  import { notify, refreshPlaylists } from "./store.svelte";
  import IconButton from "./IconButton.svelte";
  import PillButton from "./PillButton.svelte";

  let {
    playlist,
    onclose,
    onadded,
  }: { playlist: Playlist; onclose: () => void; onadded?: () => void } = $props();

  let all: Track[] = $state([]);
  /** Songs already in the playlist; shown ticked and not selectable. */
  let present = $state(new Set<number>());
  let selected = $state(new Set<number>());
  let query = $state("");
  let saving = $state(false);

  $effect(() => {
    const id = playlist.id;
    Promise.all([api.getTracks(), api.getPlaylistTracks(id)]).then(([tracks, inPlaylist]) => {
      all = tracks;
      present = new Set(inPlaylist.map((t) => t.id));
    });
  });

  let visible = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return all;
    return all.filter((t) =>
      [t.title, t.artist, t.album].some((field) => field.toLowerCase().includes(q)),
    );
  });

  let selectableVisible = $derived(visible.filter((t) => !present.has(t.id)));
  let allVisibleSelected = $derived(
    selectableVisible.length > 0 && selectableVisible.every((t) => selected.has(t.id)),
  );

  function toggle(track: Track) {
    if (present.has(track.id)) return;
    const next = new Set(selected);
    if (next.has(track.id)) next.delete(track.id);
    else next.add(track.id);
    selected = next;
  }

  function toggleAllVisible() {
    const next = new Set(selected);
    for (const t of selectableVisible) {
      if (allVisibleSelected) next.delete(t.id);
      else next.add(t.id);
    }
    selected = next;
  }

  async function add() {
    if (selected.size === 0 || saving) return;
    saving = true;
    // Keep library order so the playlist reads the same way as the list here.
    const ids = all.filter((t) => selected.has(t.id)).map((t) => t.id);
    try {
      await api.addToPlaylist(playlist.id, ids);
      await refreshPlaylists();
      notify(`Added ${ids.length} ${ids.length === 1 ? "song" : "songs"} to ${playlist.name}`);
      onadded?.();
      onclose();
    } catch (error) {
      notify(String(error));
    } finally {
      saving = false;
    }
  }

  /** Moves the node to <body> so it covers the whole app, including the dock. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" use:portal transition:fade={{ duration: 200 }}></div>
<div class="picker" use:portal role="dialog" aria-label="Add songs to {playlist.name}" transition:fly={{ y: 40, duration: 320 }}>
  <header>
    <div class="heading">
      <h2>Add songs</h2>
      <p>to {playlist.name}</p>
    </div>
    <IconButton icon={X} label="Close" variant="soft" onclick={onclose} />
  </header>

  <div class="tools">
    <label class="search">
      <Search size={18} strokeWidth={2.75} />
      <input type="search" placeholder="Search songs" bind:value={query} />
    </label>
    <button class="select-all" onclick={toggleAllVisible} disabled={selectableVisible.length === 0}>
      {allVisibleSelected ? "None" : "All"}
    </button>
  </div>

  <ul>
    {#each visible as track (track.id)}
      {@const added = present.has(track.id)}
      {@const checked = added || selected.has(track.id)}
      <li>
        <button class="row" class:checked class:added onclick={() => toggle(track)} disabled={added}>
          <span class="tick">
            {#if checked}<Check size={16} strokeWidth={3.5} />{/if}
          </span>
          <span class="text">
            <span class="title">{track.title}</span>
            <span class="meta">{added ? "Already in playlist" : `${track.artist} · ${track.album}`}</span>
          </span>
          <span class="time">{formatTime(track.duration)}</span>
        </button>
      </li>
    {:else}
      <li class="empty">{all.length === 0 ? "No songs in your library yet." : "Nothing matches."}</li>
    {/each}
  </ul>

  <footer>
    <PillButton onclick={add} disabled={selected.size === 0 || saving}>
      {selected.size === 0
        ? "Select songs"
        : `Add ${selected.size} ${selected.size === 1 ? "song" : "songs"}`}
    </PillButton>
  </footer>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.5);
  }

  .picker {
    position: fixed;
    inset: 0;
    z-index: 101;
    display: flex;
    flex-direction: column;
    max-width: 52rem;
    margin: 0 auto;
    padding: 1rem 0.8rem 0.8rem;
    background: var(--bg);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin: 0.4rem 0.4rem 0.8rem;
  }

  .heading {
    min-width: 0;
  }

  h2 {
    font-size: 1.6rem;
    font-weight: 900;
    line-height: 1.1;
  }

  .heading p {
    overflow: hidden;
    color: var(--fg-muted);
    font-size: 0.9rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tools {
    display: flex;
    gap: 0.5rem;
    margin: 0 0.4rem 0.6rem;
  }

  .search {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    color: var(--fg-muted);
  }

  .search :global(svg) {
    position: absolute;
    left: 0.95rem;
    pointer-events: none;
  }

  .search input {
    width: 100%;
    padding-left: 2.6rem;
  }

  .select-all {
    padding: 0 1.1rem;
    border: none;
    border-radius: var(--pill);
    background: var(--bg-raised);
    color: var(--accent);
    font-weight: 800;
  }

  .select-all:disabled {
    opacity: 0.4;
  }

  ul {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    padding: 0;
    list-style: none;
  }

  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.6rem 0.8rem;
    border: none;
    border-radius: var(--radius);
    background: none;
    text-align: left;
    transition: background 0.15s;
  }

  .row.checked:not(.added) {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-soft));
  }

  .row.added {
    cursor: default;
    opacity: 0.5;
  }

  @media (hover: hover) {
    .row:hover:not(.added) {
      background: var(--bg-soft);
    }
  }

  .tick {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 1.6rem;
    height: 1.6rem;
    border: 2px solid var(--fg-muted);
    border-radius: var(--pill);
    color: var(--bg);
    transition:
      background 0.15s,
      border-color 0.15s;
  }

  .checked .tick {
    border-color: var(--accent);
    background: var(--accent);
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

  .empty {
    padding: 3rem 1rem;
    color: var(--fg-muted);
    text-align: center;
  }

  footer {
    display: flex;
    justify-content: center;
    padding-top: 0.7rem;
  }
</style>
