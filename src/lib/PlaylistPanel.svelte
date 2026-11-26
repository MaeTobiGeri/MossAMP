<script lang="ts">
  import { Check, Folder as FolderIcon, ListMusic, Music2, Plus, Trash2, X } from "@lucide/svelte";
  import { fly } from "svelte/transition";
  import { api, type Folder } from "./api";
  import { app, notify, refreshPlaylists } from "./store.svelte";
  import IconButton from "./IconButton.svelte";
  import PillButton from "./PillButton.svelte";

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

  let creating = $state(false);
  let newName = $state("");
  /** Name was typed by the user, so picking a folder no longer overwrites it. */
  let nameEdited = $state(false);
  let folders: Folder[] = $state([]);
  let chosenFolder: Folder | null = $state(null);
  let confirmDelete: number | null = $state(null);

  async function startCreate() {
    creating = true;
    newName = "";
    nameEdited = false;
    chosenFolder = null;
    folders = await api.getFolders();
  }

  function cancelCreate() {
    creating = false;
  }

  function pickFolder(folder: Folder | null) {
    chosenFolder = chosenFolder?.path === folder?.path ? null : folder;
    if (!nameEdited) newName = chosenFolder?.name ?? "";
  }

  function choose(id: number | null) {
    app.selectedPlaylist = id;
    onclose();
  }

  async function createPlaylist() {
    if (!newName.trim()) return;
    try {
      const id = await api.createPlaylist(newName, chosenFolder?.path ?? null);
      await refreshPlaylists();
      creating = false;
      choose(id);
    } catch (error) {
      notify(String(error));
    }
  }

  async function deletePlaylist(id: number) {
    if (confirmDelete !== id) {
      confirmDelete = id;
      setTimeout(() => {
        if (confirmDelete === id) confirmDelete = null;
      }, 3000);
      return;
    }
    confirmDelete = null;
    await api.deletePlaylist(id);
    if (app.selectedPlaylist === id) app.selectedPlaylist = null;
    await refreshPlaylists();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || progress === 0) return;
    if (creating) cancelCreate();
    else onclose();
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
    <div class="header-actions">
      <IconButton icon={Plus} label="New playlist" variant="soft" onclick={startCreate} />
      <IconButton icon={X} label="Close" variant="ghost" onclick={onclose} />
    </div>
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
          <span class="badge">
            {#if playlist.folder}
              <FolderIcon size={22} strokeWidth={2.75} fill="currentColor" />
            {:else}
              <ListMusic size={22} strokeWidth={2.75} />
            {/if}
          </span>
          <span class="text">
            <span class="name">{playlist.name}</span>
            <small>{playlist.count} {playlist.count === 1 ? "song" : "songs"}</small>
          </span>
        </button>
        {#if confirmDelete === playlist.id}
          <button class="confirm" onclick={() => deletePlaylist(playlist.id)}>Delete?</button>
        {:else}
          <IconButton icon={Trash2} label="Delete {playlist.name}" size="sm" onclick={() => deletePlaylist(playlist.id)} />
        {/if}
      </li>
    {/each}
  </ul>

  {#if creating}
    <div class="sheet" transition:fly={{ y: 30, duration: 280 }}>
      <header>
        <h2>New playlist</h2>
        <IconButton icon={X} label="Cancel" variant="soft" onclick={cancelCreate} />
      </header>

      <form onsubmit={(event) => { event.preventDefault(); createPlaylist(); }}>
        <input
          type="text"
          placeholder="Playlist name"
          bind:value={newName}
          oninput={() => (nameEdited = newName.trim() !== "")}
          onkeydown={(event) => event.stopPropagation()}
        />

        <h3>Songs from a folder <span>optional</span></h3>
        <p class="hint">
          Every song in the folder (and its subfolders) joins the playlist, and new ones are added
          automatically. Songs are linked, never copied.
        </p>

        <div class="folders">
          {#each folders as folder (folder.path)}
            {@const picked = chosenFolder?.path === folder.path}
            <button type="button" class="folder" class:picked onclick={() => pickFolder(folder)}>
              <span class="badge small">
                {#if picked}
                  <Check size={18} strokeWidth={3.25} />
                {:else}
                  <FolderIcon size={18} strokeWidth={2.75} fill="currentColor" />
                {/if}
              </span>
              <span class="text">
                <span class="name">{folder.name}</span>
                <small>{folder.label}</small>
              </span>
              <small>{folder.count}</small>
            </button>
          {:else}
            <p class="hint">No folders with songs found. Add a music folder in Settings.</p>
          {/each}
        </div>

        <div class="submit">
          <PillButton type="submit" disabled={!newName.trim()}>
            {chosenFolder ? `Create with ${chosenFolder.count} songs` : "Create empty playlist"}
          </PillButton>
        </div>
      </form>
    </div>
  {/if}
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
    font-size: 0.82rem;
    font-weight: 700;
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .text small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .header-actions {
    display: flex;
    gap: 0.3rem;
  }

  .confirm {
    flex-shrink: 0;
    height: 2.25rem;
    padding: 0 0.9rem;
    border: none;
    border-radius: var(--pill);
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    color: var(--danger);
    font-weight: 800;
  }

  .sheet {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    padding: 1rem 0.8rem;
    background: var(--bg);
  }

  .sheet > * {
    width: 100%;
    max-width: 52rem;
    margin-left: auto;
    margin-right: auto;
  }

  h2 {
    font-size: 1.6rem;
    font-weight: 900;
  }

  form {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0 0.4rem;
  }

  h3 {
    margin-top: 0.6rem;
    font-size: 1rem;
    font-weight: 900;
  }

  h3 span {
    margin-left: 0.3rem;
    color: var(--fg-muted);
    font-size: 0.8rem;
    font-weight: 700;
  }

  .hint {
    color: var(--fg-muted);
    font-size: 0.85rem;
    line-height: 1.35;
  }

  .folders {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0 -0.4rem;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.45rem 0.9rem 0.45rem 0.45rem;
    border: none;
    border-radius: var(--radius);
    background: none;
    font-weight: 800;
    text-align: left;
    transition: background 0.15s;
  }

  .folder.picked {
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-soft));
  }

  .folder.picked .badge {
    background: var(--accent);
    color: var(--bg);
  }

  @media (hover: hover) {
    .folder:hover:not(.picked) {
      background: var(--bg-soft);
    }
  }

  .badge.small {
    width: 2.4rem;
    height: 2.4rem;
  }

  .submit {
    display: flex;
    justify-content: center;
    padding-top: 0.4rem;
  }
</style>
