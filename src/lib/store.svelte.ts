import { listen } from "@tauri-apps/api/event";
import { api, type Config, type PlayerState, type Playlist, type Theme, type Track } from "./api";

const emptyPlayer: PlayerState = {
  track: null,
  playing: false,
  position: 0,
  duration: 0,
  volume: 0.8,
  shuffle: false,
  repeat: "off",
  queue_length: 0,
};

export const app = $state({
  config: null as Config | null,
  configError: null as string | null,
  player: emptyPlayer,
  playlists: [] as Playlist[],
  libraryVersion: 0,
  message: "",
  fullPlayerOpen: false,
  /** Playlist shown on the library page; null means all songs. */
  selectedPlaylist: null as number | null,
});

const themeVariables: Record<keyof Theme, string> = {
  background: "--bg",
  background_soft: "--bg-soft",
  background_raised: "--bg-raised",
  foreground: "--fg",
  foreground_muted: "--fg-muted",
  accent: "--accent",
  accent_soft: "--accent-soft",
  warm: "--warm",
  danger: "--danger",
  radius: "--radius",
  font: "--font",
};

function applyTheme(theme: Theme) {
  const root = document.documentElement.style;
  for (const [key, variable] of Object.entries(themeVariables)) {
    const value = theme[key as keyof Theme];
    root.setProperty(variable, key === "radius" ? `${value}px` : String(value));
  }
}

interface AndroidBridge {
  update(title: string, artist: string, playing: boolean): void;
}

let lastNotified = "";

function syncAndroidNotification(player: PlayerState) {
  const bridge = (window as unknown as { MossampAndroid?: AndroidBridge }).MossampAndroid;
  if (!bridge || !player.track) return;

  const key = `${player.track.id}:${player.playing}`;
  if (key === lastNotified) return;
  lastNotified = key;
  bridge.update(player.track.title, player.track.artist, player.playing);
}

export function notify(text: string) {
  app.message = text;
  setTimeout(() => {
    if (app.message === text) app.message = "";
  }, 4000);
}

export async function refreshPlaylists() {
  app.playlists = await api.getPlaylists();
}

export async function initStore() {
  app.config = await api.getConfig();
  applyTheme(app.config.theme);
  await refreshPlaylists();

  await listen<Config>("config-changed", (event) => {
    app.config = event.payload;
    applyTheme(event.payload.theme);
  });
  await listen<string | null>("config-error", (event) => {
    app.configError = event.payload;
  });
  await listen<PlayerState>("player-state", (event) => {
    app.player = event.payload;
    syncAndroidNotification(event.payload);
  });
  await listen<string>("player-error", (event) => notify(event.payload));
  await listen("library-updated", () => {
    app.libraryVersion += 1;
    refreshPlaylists();
  });
}

export async function updateConfig(change: (config: Config) => void) {
  if (!app.config) return;
  const next = structuredClone($state.snapshot(app.config)) as Config;
  change(next);
  app.config = next;
  applyTheme(next.theme);
  await api.saveConfig(next);
}

export function playAndOpen(tracks: Track[], start: number) {
  api.playQueue(tracks, start);
  app.fullPlayerOpen = true;
}
