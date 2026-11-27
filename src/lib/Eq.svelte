<script lang="ts">
  import { Power, RotateCcw } from "@lucide/svelte";
  import { api } from "./api";
  import { app, updateConfig } from "./store.svelte";
  import IconButton from "./IconButton.svelte";
  import Toggle from "./Toggle.svelte";

  const presets: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "Bass boost": [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
    "Treble boost": [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
    Vocal: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1],
    Warm: [3, 3, 2, 1, 0, -1, -1, -2, -2, -3],
    Loudness: [5, 3, 0, -1, -2, -1, 0, 2, 4, 5],
  };

  let frequencies: number[] = $state([]);
  api.eqFrequencies().then((loaded) => (frequencies = loaded));

  let enabled = $derived(app.config?.eq.enabled ?? false);
  let gains = $derived(app.config?.eq.gains ?? presets.Flat);
  let activePreset = $derived(
    Object.entries(presets).find(([, values]) => values.every((v, i) => v === gains[i]))?.[0],
  );

  let leveling = $derived(app.config?.playback.normalize ?? false);
  let pending = $derived(app.loudness.total - app.loudness.analyzed);

  function setLeveling(on: boolean) {
    updateConfig((config) => (config.playback.normalize = on));
  }

  function label(frequency: number): string {
    return frequency >= 1000 ? `${frequency / 1000}k` : String(frequency);
  }

  function setGain(index: number, value: number) {
    const next = [...gains];
    next[index] = value;
    api.setEq(enabled, next);
  }
</script>

<div class="eq">
  <header>
    <div>
      <h1>Equalizer</h1>
      <p>{enabled ? "Shaping your sound" : "Turned off"}</p>
    </div>
    <div class="actions">
      <IconButton icon={RotateCcw} label="Reset" variant="soft" onclick={() => api.setEq(enabled, presets.Flat)} />
      <IconButton
        icon={Power}
        label={enabled ? "Turn off" : "Turn on"}
        variant={enabled ? "filled" : "soft"}
        onclick={() => api.setEq(!enabled, gains)}
      />
    </div>
  </header>

  <div class="presets">
    {#each Object.entries(presets) as [name, values]}
      <button class:active={activePreset === name} onclick={() => api.setEq(true, values)}>{name}</button>
    {/each}
  </div>

  <div class="bands" class:off={!enabled}>
    {#each gains as gain, index}
      <div class="band">
        <span class="value">{gain > 0 ? "+" : ""}{gain.toFixed(1)}</span>
        <input
          type="range"
          min="-12"
          max="12"
          step="0.5"
          value={gain}
          oninput={(e) => setGain(index, Number(e.currentTarget.value))}
          ondblclick={() => setGain(index, 0)}
        />
        <span class="freq">{frequencies[index] ? label(frequencies[index]) : ""}</span>
      </div>
    {/each}
  </div>
  <p class="hint">Double-tap a slider to reset it.</p>

  <section class="leveling">
    <Toggle checked={leveling} label="Volume leveling" onchange={setLeveling} />
    <p>
      Plays every song at about the same loudness, so you can set your volume once. Quiet songs are only
      raised as far as they can go without distorting.
    </p>
    {#if leveling && pending > 0}
      <div class="progress" style="--done: {(app.loudness.analyzed / Math.max(app.loudness.total, 1)) * 100}%">
        <span>Measuring songs: {app.loudness.analyzed} of {app.loudness.total}</span>
        <div class="bar"><div></div></div>
      </div>
    {:else if leveling && app.loudness.total > 0}
      <p class="done">All {app.loudness.total} songs measured.</p>
    {/if}
  </section>
</div>

<style>
  .eq {
    height: 100%;
    max-width: 52rem;
    margin: 0 auto;
    padding: 1rem 0.8rem;
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0.4rem 0.4rem 1.2rem;
  }

  h1 {
    font-size: 1.9rem;
    font-weight: 900;
  }

  header p {
    color: var(--fg-muted);
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
  }

  .presets {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1.2rem;
    padding-bottom: 0.3rem;
    overflow-x: auto;
  }

  .presets button {
    flex-shrink: 0;
    padding: 0.6rem 1.1rem;
    border: none;
    border-radius: var(--pill);
    background: var(--bg-soft);
    color: var(--fg-muted);
    font-weight: 800;
    transition:
      background 0.2s,
      color 0.2s,
      transform 0.18s var(--ease-pop);
  }

  .presets button:active {
    transform: scale(0.94);
  }

  .presets button.active {
    background: var(--accent);
    color: var(--bg);
  }

  .bands {
    display: flex;
    justify-content: space-between;
    gap: 0.2rem;
    padding: 1.4rem 0.8rem;
    border-radius: var(--radius);
    background: var(--bg-soft);
    transition: opacity 0.3s;
  }

  .bands.off {
    opacity: 0.4;
  }

  .band {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.9rem;
  }

  .band input {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 10px;
    height: 14rem;
    background: var(--bg-raised);
  }

  .value {
    color: var(--warm);
    font-size: 0.72rem;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
  }

  .freq {
    color: var(--fg-muted);
    font-size: 0.78rem;
    font-weight: 700;
  }

  .hint {
    margin: 0.8rem 0.4rem;
    color: var(--fg-muted);
    font-size: 0.8rem;
  }

  .leveling {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    margin-top: 1.2rem;
    padding: 1.1rem 1.2rem;
    border-radius: var(--radius);
    background: var(--bg-soft);
  }

  .leveling p {
    color: var(--fg-muted);
    font-size: 0.85rem;
    line-height: 1.4;
  }

  .leveling .done {
    color: var(--accent);
    font-weight: 700;
  }

  .progress {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    color: var(--warm);
    font-size: 0.8rem;
    font-weight: 700;
  }

  .bar {
    height: 6px;
    overflow: hidden;
    border-radius: var(--pill);
    background: var(--bg-raised);
  }

  .bar div {
    width: var(--done);
    height: 100%;
    border-radius: inherit;
    background: var(--warm);
    transition: width 0.4s var(--ease-smooth);
  }

  /* Desktop: the equalizer uses the whole window and the sliders grow tall. */
  @media (min-width: 900px) {
    .eq {
      display: flex;
      flex-direction: column;
      max-width: none;
      padding: 1.4rem 2rem 1rem;
    }

    .bands {
      flex: 1;
      min-height: 16rem;
      padding: 2rem 1.5rem;
    }

    .band input {
      flex: 1;
      height: auto;
      min-height: 0;
    }

    .value {
      font-size: 0.85rem;
    }

    .freq {
      font-size: 0.9rem;
    }
  }
</style>
