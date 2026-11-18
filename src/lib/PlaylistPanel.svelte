<script lang="ts">
  import { Check, ListMusic, Music2, Plus, Trash2, X } from "@lucide/svelte";
  import { api } from "./api";
  import { app, notify, refreshPlaylists } from "./store.svelte";
  import IconButton from "./IconButton.svelte";

  let {
    progress,
    dragging,
    onclose,
  }: {
    /** 0 = hidden, 1 = fully open. */
    progress: number;
    /** While true the panel follows the finger without easing. */
    dragging: boolean;
    onclose: () => void;
  } = $props();

  let newName = $state("");
  let creating = $state(false);

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
  }

  function cancelCreate() {
    creating = false;
    newName = "";
  }

  function choose(id: number | null) {
    app.selectedPlaylist = id;
    onclose();
  }

  async function createPlaylist() {
    if (!newName.trim()) return;
    try {
      await api.createPlaylist(newName);
      cancelCreate();
      await refreshPlaylists();
    } catch {
      notify("A playlist with that name already exists");
    }
  }

  async function deletePlaylist(id: number) {
    await api.deletePlaylist(id);
    if (app.selectedPlaylist === id) app.selectedPlaylist = null;
    await refreshPlaylists();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && progress > 0) onclose();
  }
</script>

<svelte:window {onkeydown} />

<div
  class="panel"
  class:dragging
  class:open={progress > 0}
  class:full={progress === 1}
  style="--progress: {progress}"
  aria-hidden={progress === 0}
>
  <header>
    <h1>Playlists</h1>
    <IconButton icon={X} label="Close" variant="soft" onclick={onclose} />
  </header>

  <ul>
    <li style="--i: 0">
      <button class="row" class:active={app.selectedPlaylist === null} onclick={() => choose(null)}>
        <span class="badge"><Music2 size={22} strokeWidth={2.75} fill="currentColor" /></span>
        <span class="name">All songs</span>
      </button>
    </li>
    {#each app.playlists as playlist, index (playlist.id)}
      <li style="--i: {index + 1}">
        <button class="row" class:active={app.selectedPlaylist === playlist.id} onclick={() => choose(playlist.id)}>
          <span class="badge"><ListMusic size={22} strokeWidth={2.75} /></span>
          <span class="name">{playlist.name}</span>
          <small>{playlist.count}</small>
        </button>
        <IconButton icon={Trash2} label="Delete {playlist.name}" size="sm" onclick={() => deletePlaylist(playlist.id)} />
      </li>
    {/each}
    <li style="--i: {app.playlists.length + 1}">
      {#if creating}
        <form class="new" onsubmit={(event) => { event.preventDefault(); createPlaylist(); }}>
          <input
            type="text"
            placeholder="Playlist name"
            bind:value={newName}
            use:focusOnMount
            onkeydown={(event) => {
              event.stopPropagation();
              if (event.key === "Escape") cancelCreate();
            }}
          />
          <IconButton icon={Check} label="Create" variant="filled" size="sm" />
          <IconButton icon={X} label="Cancel" size="sm" onclick={cancelCreate} />
        </form>
      {:else}
        <button class="row create" onclick={() => (creating = true)}>
          <span class="badge"><Plus size={22} strokeWidth={3} /></span>
          <span class="name">New playlist</span>
        </button>
      {/if}
    </li>
  </ul>
</div>

<style>
  .panel {
    position: absolute;
    inset: 0;
    z-index: 10;
    padding: 1rem 0.8rem;
    overflow-y: auto;
    background: var(--bg);
    border-radius: calc(var(--radius) * 1.5);
    /* Grows out of the top-left corner as the swipe progresses. Only transform and
       opacity change, so the GPU can animate it without repainting. */
    opacity: clamp(0, calc(var(--progress) * 2.5), 1);
    transform: scale(calc(0.15 + var(--progress) * 0.85));
    transform-origin: 0 0;
    will-change: transform, opacity;
    pointer-events: none;
    transition:
      transform 0.45s var(--ease-smooth),
      opacity 0.3s var(--ease-smooth);
  }

  .panel.open {
    pointer-events: auto;
  }

  .panel.full {
    border-radius: 0;
  }

  .panel.dragging {
    transition: none;
  }

  .panel > * {
    max-width: 52rem;
    margin-left: auto;
    margin-right: auto;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 0.4rem;
    margin-bottom: 1rem;
    padding: 0 0.4rem;
  }

  h1 {
    font-size: 1.9rem;
    font-weight: 900;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    opacity: clamp(0, calc(var(--progress) * 3 - 1.2 - var(--i) * 0.15), 1);
    transform: translate(
      calc((1 - var(--progress)) * -2rem),
      calc((1 - var(--progress)) * -1rem)
    );
    transition:
      opacity 0.35s,
      transform 0.45s var(--ease-smooth);
  }

  .dragging li {
    transition: none;
  }

  .row {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.5rem 0.9rem 0.5rem 0.5rem;
    border: none;
    border-radius: var(--radius);
    background: none;
    font-weight: 800;
    font-size: 1.05rem;
    text-align: left;
    transition:
      background 0.2s,
      transform 0.18s var(--ease-pop);
  }

  .row:active {
    transform: scale(0.97);
  }

  @media (hover: hover) {
    .row:hover {
      background: var(--bg-soft);
    }
  }

  .badge {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    width: 3rem;
    height: 3rem;
    border-radius: var(--radius-small);
    background: var(--bg-raised);
    color: var(--accent);
    transition:
      background 0.2s,
      color 0.2s;
  }

  .row.active .badge {
    background: var(--accent);
    color: var(--bg);
  }

  .row.active .name {
    color: var(--accent);
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  small {
    color: var(--fg-muted);
  }

  .create .name {
    color: var(--fg-muted);
  }

  .new {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0;
  }

  .new input {
    flex: 1;
    min-width: 0;
  }
</style>
