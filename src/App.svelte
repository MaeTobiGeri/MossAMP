<script lang="ts">
  import { pages, type PageId } from "./lib/pages";
  import { app, initStore } from "./lib/store.svelte";
  import Eq from "./lib/Eq.svelte";
  import FullPlayer from "./lib/FullPlayer.svelte";
  import Library from "./lib/Library.svelte";
  import MiniPlayer from "./lib/MiniPlayer.svelte";
  import PlayerPanel from "./lib/PlayerPanel.svelte";
  import Search from "./lib/Search.svelte";
  import Settings from "./lib/Settings.svelte";

  let current: PageId = $state("library");
  let currentIndex = $derived(pages.findIndex((page) => page.id === current));
  /**
   * Pages are built the first time they are opened and then kept alive:
   * switching only fades and slides them (GPU-only), instead of rebuilding
   * the song list and reloading every cover each time.
   */
  let visited: PageId[] = $state(["library"]);
  /** Wide windows show the player beside the song list on these pages. */
  let split = $derived(app.wide && (current === "library" || current === "search"));

  function go(id: PageId) {
    if (id === current) return;
    if (!visited.includes(id)) visited.push(id);
    current = id;
  }

  /** -1 for pages left of the current one in the nav, +1 for pages right of it. */
  function side(id: PageId): number {
    return Math.sign(pages.findIndex((page) => page.id === id) - currentIndex);
  }

  initStore();
</script>

<main>
  <div class="content" class:split>
    {#if app.config && app.wide}
      <!-- Hidden rather than removed on EQ / Settings, so it is not rebuilt. -->
      <aside class="side" class:hidden={!split}>
        <PlayerPanel />
      </aside>
    {/if}
    <div class="pages">
      {#if app.config}
        {#each pages as page (page.id)}
          {#if visited.includes(page.id)}
            <div
              class="page"
              class:active={page.id === current}
              style="--side: {side(page.id)}"
              inert={page.id !== current}
            >
              {#if page.id === "library"}
                <Library />
              {:else if page.id === "search"}
                <Search />
              {:else if page.id === "eq"}
                <Eq />
              {:else}
                <Settings />
              {/if}
            </div>
          {/if}
        {/each}
      {/if}
    </div>
  </div>

  {#if app.message}
    <div class="toast">{app.message}</div>
  {/if}

  <div class="dock">
    <div class="box">
      {#if !split}
        <MiniPlayer />
      {/if}

      <nav style="--count: {pages.length}; --index: {currentIndex}">
        <span class="indicator" aria-hidden="true"></span>
        {#each pages as page}
          {@const Icon = page.icon}
          <button
            class:active={page.id === current}
            onclick={() => go(page.id)}
            aria-label={page.label}
            title={page.label}
          >
            <Icon size={24} strokeWidth={2.75} fill={page.solid ? "currentColor" : "none"} />
          </button>
        {/each}
      </nav>
    </div>
  </div>
</main>

{#if app.fullPlayerOpen && !app.wide}
  <FullPlayer />
{/if}

<style>
  main {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .content {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
  }

  .content.split {
    gap: 0.8rem;
    padding: 0.8rem 0 0 0.8rem;
  }

  /* The player box is centred vertically in its column. */
  .side {
    flex: 0 0 clamp(20rem, 36%, 32rem);
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    animation: side-in 0.35s var(--ease-smooth);
  }

  .side.hidden {
    display: none;
  }

  @keyframes side-in {
    from {
      opacity: 0;
      transform: translateX(-16px);
    }
  }

  .pages {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
  }

  /* Inactive pages wait just beside the view (left or right, matching the
     nav order) and invisible; the active one slides and fades in. Only
     opacity and transform animate, which the GPU handles without re-layout. */
  .page {
    position: absolute;
    inset: 0;
    opacity: 0;
    visibility: hidden;
    transform: translateX(calc(var(--side) * 4%));
    transition:
      opacity 0.22s ease-out,
      transform 0.32s var(--ease-smooth),
      visibility 0s linear 0.32s;
  }

  .page.active {
    opacity: 1;
    visibility: visible;
    transform: none;
    transition:
      opacity 0.26s ease-out 0.04s,
      transform 0.32s var(--ease-smooth),
      visibility 0s;
  }

  @media (prefers-reduced-motion: reduce) {
    .page,
    .page.active {
      transform: none;
      transition: none;
    }
  }

  /* Beside the player, the song list starts right next to it (a small gap)
     and uses the rest of the window instead of floating centred in it. */
  .content.split .page :global(:is(.scroll, .search)) {
    max-width: none;
    margin: 0;
    padding-left: 0.4rem;
  }

  .dock {
    width: 100%;
    max-width: 52rem;
    margin: 0 auto;
    padding: 0.4rem 0.8rem 0.8rem;
  }

  .box {
    border-radius: var(--radius);
    background: var(--bg-soft);
    box-shadow: var(--shadow);
    overflow: hidden;
  }

  nav {
    position: relative;
    display: grid;
    grid-template-columns: repeat(var(--count), 1fr);
    padding: 0.45rem;
  }

  /* One pill shared by all buttons, sliding to the active one. The indicator spans one
     grid column and moves with transform only; the pill itself is drawn by ::before. */
  .indicator {
    position: absolute;
    top: 0.45rem;
    bottom: 0.45rem;
    left: 0.45rem;
    width: calc((100% - 0.9rem) / var(--count));
    transform: translateX(calc(var(--index) * 100%));
    will-change: transform;
    /* Slight overshoot, then settles. */
    transition: transform 0.45s cubic-bezier(0.3, 1.18, 0.6, 1);
  }

  .indicator::before {
    content: "";
    position: absolute;
    inset: 0;
    width: 3.4rem;
    margin: 0 auto;
    border-radius: var(--pill);
    background: var(--accent);
  }

  nav button {
    position: relative;
    justify-self: center;
    display: grid;
    place-items: center;
    width: 3.4rem;
    height: 3rem;
    border: none;
    border-radius: var(--pill);
    background: none;
    color: var(--fg-muted);
    transition:
      color 0.3s,
      transform 0.18s var(--ease-pop);
  }

  nav button :global(svg) {
    transition: transform 0.35s var(--ease-smooth);
  }

  nav button:active {
    transform: scale(0.88);
  }

  nav button.active {
    color: var(--bg);
  }

  nav button.active :global(svg) {
    transform: scale(1.06);
    animation: icon-pop 0.4s var(--ease-smooth);
  }

  @keyframes icon-pop {
    0% {
      transform: scale(0.9);
    }
  }

  .toast {
    position: absolute;
    left: 50%;
    bottom: 10rem;
    z-index: 20;
    max-width: 90%;
    padding: 0.7rem 1.3rem;
    border-radius: var(--pill);
    background: var(--accent);
    color: var(--bg);
    font-weight: 800;
    box-shadow: var(--shadow);
    transform: translateX(-50%);
    animation: toast-in 0.35s var(--ease-pop);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translate(-50%, 12px) scale(0.9);
    }
  }
</style>
