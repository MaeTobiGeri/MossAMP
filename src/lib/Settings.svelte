<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    Cloud,
    CloudDownload,
    FileCog,
    FolderOpen,
    FolderSearch,
    Palette,
    Plug,
    Plus,
    RotateCcw,
    TriangleAlert,
    X,
  } from "@lucide/svelte";
  import { api } from "./api";
  import { colorLabels, defaultTheme } from "./defaults";
  import { app, notify, updateConfig } from "./store.svelte";
  import IconButton from "./IconButton.svelte";
  import PillButton from "./PillButton.svelte";
  import Toggle from "./Toggle.svelte";

  let configPath = $state("");
  let newFolder = $state("");
  let busy = $state("");
  let progress = $state("");

  api.getConfigPath().then((path) => (configPath = path));

  $effect(() => {
    const stop = listen<{ done: number; total: number; current: string }>("cloud-progress", (event) => {
      const { done, total, current } = event.payload;
      progress = done < total ? `Downloading ${done + 1}/${total}: ${current}` : "";
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  });

  async function browse() {
    const chosen = await open({ directory: true });
    if (typeof chosen === "string") await addFolder(chosen);
  }

  async function addFolder(folder: string) {
    const path = folder.trim();
    if (!path) return;
    await updateConfig((config) => {
      if (!config.library.folders.includes(path)) config.library.folders.push(path);
    });
    newFolder = "";
  }

  async function removeFolder(folder: string) {
    await updateConfig((config) => {
      config.library.folders = config.library.folders.filter((f) => f !== folder);
    });
  }

  async function run(task: string, action: () => Promise<string>) {
    busy = task;
    try {
      notify(await action());
    } catch (error) {
      notify(String(error));
    }
    busy = "";
  }

  const scan = () => run("scan", async () => `Found ${await api.scanLibrary()} songs`);

  const testCloud = () =>
    run("test", async () => {
      const count = await api.cloudTest($state.snapshot(app.config!.cloud));
      return `Connected, ${count} songs on the server`;
    });

  const syncCloud = () =>
    run("sync", async () => {
      const report = await api.cloudSync();
      return `Downloaded ${report.downloaded}, up to date ${report.skipped}, failed ${report.failed}`;
    });
</script>

{#if app.config}
  {@const config = app.config}
  <div class="settings">
    <h1>Settings</h1>

    {#if app.configError}
      <p class="error"><TriangleAlert size={20} strokeWidth={2.75} />Config file has an error: {app.configError}</p>
    {/if}

    <section>
      <h2><Palette size={22} strokeWidth={2.75} />Appearance</h2>
      <div class="colors">
        {#each colorLabels as [key, label]}
          <label class="color">
            <span class="swatch" style="background: {config.theme[key]}">
              <input
                type="color"
                value={config.theme[key]}
                oninput={(e) => updateConfig((c) => ((c.theme[key] as string) = e.currentTarget.value))}
              />
            </span>
            <span>{label}</span>
          </label>
        {/each}
      </div>
      <label class="field">
        <span>Roundness <b>{config.theme.radius}px</b></span>
        <input
          type="range"
          min="0"
          max="32"
          value={config.theme.radius}
          style="--progress: {(config.theme.radius / 32) * 100}%"
          oninput={(e) => updateConfig((c) => (c.theme.radius = Number(e.currentTarget.value)))}
        />
      </label>
      <label class="field">
        <span>Font</span>
        <input
          type="text"
          value={config.theme.font}
          onchange={(e) => updateConfig((c) => (c.theme.font = e.currentTarget.value))}
        />
      </label>
      <PillButton icon={RotateCcw} variant="soft" onclick={() => updateConfig((c) => (c.theme = { ...defaultTheme }))}>
        Reset theme
      </PillButton>
    </section>

    <section>
      <h2><FolderOpen size={22} strokeWidth={2.75} />Library</h2>
      <ul class="folders">
        {#each config.library.folders as folder}
          <li>
            <span>{folder}</span>
            <IconButton icon={X} label="Remove folder" size="sm" onclick={() => removeFolder(folder)} />
          </li>
        {/each}
      </ul>
      <form class="add" onsubmit={(e) => { e.preventDefault(); addFolder(newFolder); }}>
        <input type="text" placeholder="/path/to/music" bind:value={newFolder} />
        <IconButton icon={Plus} label="Add folder" variant="filled" size="sm" />
        <IconButton icon={FolderSearch} label="Browse" variant="soft" size="sm" onclick={(e) => { e.preventDefault(); browse(); }} />
      </form>
      <Toggle
        label="Scan on startup"
        checked={config.library.scan_on_start}
        onchange={(value) => updateConfig((c) => (c.library.scan_on_start = value))}
      />
      <PillButton icon={FolderSearch} onclick={scan} disabled={busy !== ""}>
        {busy === "scan" ? "Scanning…" : "Scan now"}
      </PillButton>
    </section>

    <section>
      <h2><Cloud size={22} strokeWidth={2.75} />Cloud</h2>
      <p class="hint">WebDAV, so Nextcloud, ownCloud and most NAS systems work. Songs are downloaded to this device.</p>
      <Toggle
        label="Enable cloud"
        checked={config.cloud.enabled}
        onchange={(value) => updateConfig((c) => (c.cloud.enabled = value))}
      />
      <label class="field">
        <span>Server URL</span>
        <input
          type="url"
          placeholder="https://cloud.example.com/remote.php/dav/files/me"
          value={config.cloud.url}
          onchange={(e) => updateConfig((c) => (c.cloud.url = e.currentTarget.value.trim()))}
        />
      </label>
      <label class="field">
        <span>Music folder on the server</span>
        <input
          type="text"
          value={config.cloud.remote_path}
          onchange={(e) => updateConfig((c) => (c.cloud.remote_path = e.currentTarget.value.trim()))}
        />
      </label>
      <div class="pair">
        <label class="field">
          <span>Username</span>
          <input
            type="text"
            value={config.cloud.username}
            onchange={(e) => updateConfig((c) => (c.cloud.username = e.currentTarget.value))}
          />
        </label>
        <label class="field">
          <span>Password or app token</span>
          <input
            type="password"
            value={config.cloud.password}
            onchange={(e) => updateConfig((c) => (c.cloud.password = e.currentTarget.value))}
          />
        </label>
      </div>
      <Toggle
        label="Sync on startup"
        checked={config.cloud.sync_on_start}
        onchange={(value) => updateConfig((c) => (c.cloud.sync_on_start = value))}
      />
      <div class="buttons">
        <PillButton icon={Plug} variant="soft" onclick={testCloud} disabled={busy !== ""}>
          {busy === "test" ? "Testing…" : "Test connection"}
        </PillButton>
        <PillButton icon={CloudDownload} onclick={syncCloud} disabled={busy !== "" || !config.cloud.enabled}>
          {busy === "sync" ? "Syncing…" : "Sync now"}
        </PillButton>
      </div>
      {#if progress}<p class="hint">{progress}</p>{/if}
    </section>

    <section>
      <h2><FileCog size={22} strokeWidth={2.75} />Config file</h2>
      <p class="hint">
        Everything here lives in <code>{configPath}</code>. On desktop you can edit that file directly and
        changes apply instantly.
      </p>
    </section>
  </div>
{/if}

<style>
  .settings {
    height: 100%;
    max-width: 44rem;
    margin: 0 auto;
    padding: 1rem 0.8rem;
    overflow-y: auto;
  }

  h1 {
    margin: 0.4rem 0.4rem 1rem;
    font-size: 1.9rem;
    font-weight: 900;
  }

  h2 {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    color: var(--accent);
    font-size: 1.15rem;
    font-weight: 900;
  }

  section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1rem;
    margin-bottom: 0.8rem;
    padding: 1.3rem;
    border-radius: var(--radius);
    background: var(--bg-soft);
  }

  .colors {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
    gap: 0.8rem;
    width: 100%;
  }

  .color {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.9rem;
    font-weight: 700;
    cursor: pointer;
  }

  .swatch {
    flex-shrink: 0;
    width: 2.2rem;
    height: 2.2rem;
    border: 3px solid var(--bg-raised);
    border-radius: 50%;
    overflow: hidden;
  }

  .swatch input {
    opacity: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    width: 100%;
    color: var(--fg-muted);
    font-size: 0.85rem;
    font-weight: 700;
  }

  .field b {
    color: var(--warm);
  }

  .field input[type="text"],
  .field input[type="url"],
  .field input[type="password"] {
    width: 100%;
    color: var(--fg);
    font-size: 1rem;
  }

  .pair {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: 1rem;
    width: 100%;
  }

  .folders {
    list-style: none;
    width: 100%;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .folders li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.3rem 0.3rem 0.3rem 1rem;
    border-radius: var(--pill);
    background: var(--bg-raised);
    font-size: 0.9rem;
    overflow-wrap: anywhere;
  }

  .add {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
  }

  .add input {
    flex: 1;
    min-width: 0;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .hint {
    color: var(--fg-muted);
    font-size: 0.88rem;
    overflow-wrap: anywhere;
  }

  code {
    color: var(--accent);
  }

  .error {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin-bottom: 0.8rem;
    padding: 0.9rem 1.2rem;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    color: var(--danger);
    font-weight: 700;
  }
</style>
