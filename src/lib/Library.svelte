<script lang="ts">
  import { Play, Shuffle } from "@lucide/svelte";
  import { api, type Track } from "./api";
  import { app, playAndOpen } from "./store.svelte";
  import IconButton from "./IconButton.svelte";
  import PlaylistPanel from "./PlaylistPanel.svelte";
  import TrackList from "./TrackList.svelte";

  /** Size of the corner area a swipe has to start in (top-left to open, bottom-right to close). */
  const CORNER = { width: 110, height: 150 };
  /** Swipe distance (px) that fully opens the playlist panel. */
  const FULL_SWIPE = 320;

  let tracks: Track[] = $state([]);
  let panelProgress = $state(0);
  let dragging = $state(false);

  let title = $derived(
    app.selectedPlaylist === null
      ? "All songs"
      : (app.playlists.find((p) => p.id === app.selectedPlaylist)?.name ?? ""),
  );

  async function load() {
    const id = app.selectedPlaylist;
    tracks = id === null ? await api.getTracks() : await api.getPlaylistTracks(id);
  }

  $effect(() => {
    app.libraryVersion;
    app.selectedPlaylist;
    load();
  });

  // A playlist deleted elsewhere falls back to all songs.
  $effect(() => {
    const id = app.selectedPlaylist;
    if (id !== null && !app.playlists.some((p) => p.id === id)) app.selectedPlaylist = null;
  });

  function shuffleAll() {
    if (tracks.length === 0) return;
    api.setShuffle(true);
    playAndOpen(tracks, Math.floor(Math.random() * tracks.length));
  }

  function openPanel() {
    panelProgress = 1;
  }

  function closePanel() {
    panelProgress = 0;
  }

  /**
   * Diagonal swipe from the top-left corner towards the bottom right opens the playlist panel;
   * the reverse swipe, from the bottom-right corner towards the top left, closes it again.
   * The panel follows the finger while dragging and snaps open or closed on release.
   */
  function cornerSwipe(node: HTMLElement) {
    let start: { x: number; y: number } | null = null;
    let claimed = false;
    /** +1 while opening (moving down-right), -1 while closing (moving up-left). */
    let sign = 1;

    function begin(x: number, y: number) {
      const rect = node.getBoundingClientRect();
      if (panelProgress === 0) {
        if (x - rect.left > CORNER.width || y - rect.top > CORNER.height) return false;
        sign = 1;
      } else if (panelProgress === 1) {
        if (rect.right - x > CORNER.width || rect.bottom - y > CORNER.height) return false;
        sign = -1;
      } else {
        return false;
      }
      start = { x, y };
      claimed = false;
      return true;
    }

    /** Returns true once the gesture belongs to us. */
    function move(x: number, y: number) {
      if (!start) return false;
      // Normalised so that the expected direction is always down-right.
      const dx = (x - start.x) * sign;
      const dy = (y - start.y) * sign;

      if (!claimed) {
        if (Math.hypot(dx, dy) < 12) return false;
        // Roughly 20° to 70° below the horizontal counts as diagonal.
        const diagonal = dx > 0 && dy > 0 && dy / dx > 0.35 && dy / dx < 2.8;
        if (!diagonal) {
          start = null;
          return false;
        }
        claimed = true;
        dragging = true;
      }

      const travelled = Math.min(1, Math.hypot(Math.max(dx, 0), Math.max(dy, 0)) / FULL_SWIPE);
      panelProgress = sign === 1 ? travelled : 1 - travelled;
      return true;
    }

    function end() {
      if (claimed) {
        dragging = false;
        // A third of the way is enough to commit, in either direction.
        panelProgress = sign === 1 ? (panelProgress > 0.3 ? 1 : 0) : panelProgress < 0.7 ? 0 : 1;
      }
      start = null;
      claimed = false;
    }

    function touchstart(event: TouchEvent) {
      if (event.touches.length === 1) begin(event.touches[0].clientX, event.touches[0].clientY);
    }

    function touchmove(event: TouchEvent) {
      const touch = event.touches[0];
      if (touch && move(touch.clientX, touch.clientY)) event.preventDefault();
    }

    function mousedown(event: MouseEvent) {
      if (event.button !== 0 || !begin(event.clientX, event.clientY)) return;
      const onmove = (e: MouseEvent) => {
        if (move(e.clientX, e.clientY)) e.preventDefault();
      };
      const onup = () => {
        end();
        window.removeEventListener("mousemove", onmove);
        window.removeEventListener("mouseup", onup);
      };
      window.addEventListener("mousemove", onmove);
      window.addEventListener("mouseup", onup);
    }

    node.addEventListener("touchstart", touchstart, { passive: true });
    node.addEventListener("touchmove", touchmove, { passive: false });
    node.addEventListener("touchend", end);
    node.addEventListener("touchcancel", end);
    node.addEventListener("mousedown", mousedown);

    return {
      destroy() {
        node.removeEventListener("touchstart", touchstart);
        node.removeEventListener("touchmove", touchmove);
        node.removeEventListener("touchend", end);
        node.removeEventListener("touchcancel", end);
        node.removeEventListener("mousedown", mousedown);
      },
    };
  }
</script>

<div class="library" use:cornerSwipe>
  <div class="scroll" class:pushed={panelProgress > 0} style="--progress: {panelProgress}" class:dragging>
    <header>
      <button class="title" onclick={openPanel} title="Playlists">
        <h1>{title}</h1>
        <p>{tracks.length} songs</p>
      </button>
      <div class="actions">
        <IconButton icon={Shuffle} label="Shuffle" variant="soft" onclick={shuffleAll} disabled={tracks.length === 0} />
        <IconButton
          icon={Play}
          label="Play"
          variant="filled"
          solid
          onclick={() => playAndOpen(tracks, 0)}
          disabled={tracks.length === 0}
        />
      </div>
    </header>

    <TrackList {tracks} playlistId={app.selectedPlaylist} onchange={load} />
  </div>

  <PlaylistPanel progress={panelProgress} {dragging} onclose={closePanel} />
</div>

<style>
  .library {
    position: relative;
    height: 100%;
    overflow: hidden;
  }

  .scroll {
    height: 100%;
    max-width: 52rem;
    margin: 0 auto;
    padding: 1rem 0.8rem;
    overflow-y: auto;
    transform-origin: 100% 100%;
    will-change: transform, opacity;
    transition:
      transform 0.45s var(--ease-smooth),
      opacity 0.45s var(--ease-smooth);
  }

  /* The list recedes slightly while the playlist panel grows over it. */
  .scroll.pushed {
    transform: scale(calc(1 - var(--progress) * 0.06));
    opacity: calc(1 - var(--progress) * 0.5);
  }

  .scroll.dragging {
    transition: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin: 0.4rem 0.4rem 0.8rem;
  }

  .title {
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    text-align: left;
  }

  h1 {
    overflow: hidden;
    font-size: 1.9rem;
    font-weight: 900;
    line-height: 1.1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  header p {
    color: var(--fg-muted);
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
</style>
