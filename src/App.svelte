<script lang="ts">
  import { pages, type PageId } from "./lib/pages";
  import { app, initStore } from "./lib/store.svelte";
  import Eq from "./lib/Eq.svelte";
  import FullPlayer from "./lib/FullPlayer.svelte";
  import Library from "./lib/Library.svelte";
  import MiniPlayer from "./lib/MiniPlayer.svelte";
  import Search from "./lib/Search.svelte";
  import Settings from "./lib/Settings.svelte";

  let current: PageId = $state("library");
  /** +1 when moving to a page further right in the nav, -1 when moving left. */
  let direction = $state(1);
  let currentIndex = $derived(pages.findIndex((page) => page.id === current));

  function go(id: PageId) {
    const index = pages.findIndex((page) => page.id === id);
    if (index === currentIndex) return;
    direction = index > currentIndex ? 1 : -1;
    current = id;
  }

  /** Slides the page sideways and fades it, in the direction of travel. */
  function slide(node: HTMLElement, { entering }: { entering: boolean }) {
    const offset = (entering ? direction : -direction) * 18;
    return {
      duration: 380,
      easing: (t: number) => 1 - Math.pow(1 - t, 4),
      css: (t: number) => `
        opacity: ${t};
        transform: translateX(${(1 - t) * offset}%);
      `,
    };
  }

  initStore();
</script>

<main>
  <div class="content">
    {#if app.config}
      {#key current}
        <div class="page" in:slide={{ entering: true }} out:slide={{ entering: false }}>
          {#if current === "library"}
            <Library />
          {:else if current === "search"}
            <Search />
          {:else if current === "eq"}
            <Eq />
          {:else}
            <Settings />
          {/if}
        </div>
      {/key}
    {/if}
  </div>

  {#if app.message}
    <div class="toast">{app.message}</div>
  {/if}

  <div class="dock">
    <div class="box">
      <MiniPlayer />

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

{#if app.fullPlayerOpen}
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
  }

  .page {
    position: absolute;
    inset: 0;
    will-change: transform, opacity;
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
