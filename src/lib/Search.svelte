<script lang="ts">
  import { Search, X } from "@lucide/svelte";
  import { api, type FilterOptions, type SearchFilter, type Track } from "./api";
  import { app } from "./store.svelte";
  import TrackList from "./TrackList.svelte";

  const emptyOptions: FilterOptions = { artists: [], albums: [], genres: [], formats: [] };

  let query = $state("");
  let artist = $state("");
  let album = $state("");
  let genre = $state("");
  let format = $state("");
  let yearFrom = $state("");
  let yearTo = $state("");
  let options = $state(emptyOptions);
  let results: Track[] = $state([]);

  let filtersActive = $derived(Boolean(artist || album || genre || format || yearFrom || yearTo));

  function buildFilter(): SearchFilter {
    return {
      query,
      artist: artist || null,
      album: album || null,
      genre: genre || null,
      format: format || null,
      year_from: yearFrom ? Number(yearFrom) : null,
      year_to: yearTo ? Number(yearTo) : null,
    };
  }

  $effect(() => {
    app.libraryVersion;
    api.getFilterOptions().then((loaded) => (options = loaded));
  });

  $effect(() => {
    const filter = buildFilter();
    api.searchTracks(filter).then((found) => (results = found));
  });

  function reset() {
    query = artist = album = genre = format = yearFrom = yearTo = "";
  }
</script>

<div class="search">
  <label class="query">
    <Search size={22} strokeWidth={2.75} />
    <input type="search" placeholder="Songs, artists, albums" bind:value={query} />
  </label>

  <div class="filters">
    <select bind:value={artist} class:set={artist}>
      <option value="">Artist</option>
      {#each options.artists as value}<option>{value}</option>{/each}
    </select>
    <select bind:value={album} class:set={album}>
      <option value="">Album</option>
      {#each options.albums as value}<option>{value}</option>{/each}
    </select>
    <select bind:value={genre} class:set={genre}>
      <option value="">Genre</option>
      {#each options.genres as value}<option>{value}</option>{/each}
    </select>
    <select bind:value={format} class:set={format}>
      <option value="">Format</option>
      {#each options.formats as value}<option>{value}</option>{/each}
    </select>
    <input class="year" type="number" placeholder="From year" bind:value={yearFrom} />
    <input class="year" type="number" placeholder="To year" bind:value={yearTo} />
    {#if filtersActive || query}
      <button class="clear" onclick={reset}><X size={17} strokeWidth={3} />Clear</button>
    {/if}
  </div>

  <p class="count">{results.length} songs</p>
  <TrackList tracks={results} />
</div>

<style>
  .search {
    height: 100%;
    max-width: 52rem;
    margin: 0 auto;
    padding: 1rem 0.8rem;
    overflow-y: auto;
  }

  .query {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    padding: 0.3rem 0.3rem 0.3rem 1.2rem;
    border-radius: var(--pill);
    background: var(--bg-soft);
    color: var(--accent);
    box-shadow: var(--shadow);
  }

  .query input {
    flex: 1;
    min-width: 0;
    padding: 0.8rem 0.4rem;
    background: none;
    font-size: 1.05rem;
    font-weight: 700;
  }

  .filters {
    display: flex;
    gap: 0.5rem;
    margin: 1rem 0 0.6rem;
    padding-bottom: 0.3rem;
    overflow-x: auto;
  }

  select,
  .year {
    flex-shrink: 0;
    max-width: 11rem;
    padding: 0.55rem 1rem;
    background: var(--bg-soft);
    font-weight: 700;
  }

  select {
    appearance: none;
  }

  select.set {
    background: var(--accent);
    color: var(--bg);
  }

  .year {
    width: 7.5rem;
  }

  .clear {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.55rem 1rem;
    border: none;
    border-radius: var(--pill);
    background: color-mix(in srgb, var(--warm) 18%, transparent);
    color: var(--warm);
    font-weight: 800;
  }

  .count {
    margin: 0 0.4rem 0.5rem;
    color: var(--fg-muted);
    font-size: 0.85rem;
  }
</style>
