import { listen } from "@tauri-apps/api/event";
import {
  api,
  type Config,
  type LoudnessProgress,
  type PlayerState,
  type Playlist,
  type Theme,
  type Track,
} from "./api";

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
  /** Desktop-sized window: the player sits next to the song list instead of opening full screen. */
  wide: false,
  /** Playlist shown on the library page; null means all songs. */
  selectedPlaylist: null as number | null,
  /** How many songs volume leveling has measured so far. */
  loudness: { analyzed: 0, total: 0 } as LoudnessProgress,
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
  update(title: string, artist: string, playing: boolean, position: number, duration: number, path: string): void;
}

let lastNotified = { key: "", position: 0, at: 0 };

/**
 * Keeps Android's media controls (notification, lock screen) in sync. Android
 * moves the seek bar on its own while playing, so updates are only sent when
 * the song or play state changes, or the position jumps (a seek).
 */
function syncAndroidNotification(player: PlayerState) {
  const bridge = (window as unknown as { MossampAndroid?: AndroidBridge }).MossampAndroid;
  if (!bridge || !player.track) return;

  const now = performance.now();
  const key = `${player.track.id}:${player.playing}`;
  const expected = lastNotified.position + (player.playing ? (now - lastNotified.at) / 1000 : 0);
  if (key === lastNotified.key && Math.abs(player.position - expected) < 1.5) return;

  lastNotified = { key, position: player.position, at: now };
  bridge.update(
    player.track.title,
    player.track.artist,
    player.playing,
    player.position,
    player.duration,
    player.track.path,
  );
}

/** Commands from Android's media controls and Bluetooth/headset buttons. */
function handleMediaCommand(command: string, seconds: number) {
  switch (command) {
    case "play":
      return api.resume();
    case "pause":
      return api.pause();
    case "toggle":
      return api.toggle();
    case "next":
      return api.next();
    case "previous":
      return api.previous();
    case "seek":
      return api.seek(seconds);
  }
}

(window as unknown as { mossampMedia: typeof handleMediaCommand }).mossampMedia = handleMediaCommand;

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
  app.loudness = await api.getLoudnessProgress();

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
  await listen("library-updated", async () => {
    app.libraryVersion += 1;
    refreshPlaylists();
    app.loudness = await api.getLoudnessProgress();
  });
  await listen<LoudnessProgress>("loudness-progress", (event) => {
    app.loudness = event.payload;
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

const wideQuery = window.matchMedia("(min-width: 900px)");
app.wide = wideQuery.matches;
wideQuery.addEventListener("change", (event) => {
  app.wide = event.matches;
  if (event.matches) app.fullPlayerOpen = false;
});

export function playAndOpen(tracks: Track[], start: number) {
  api.playQueue(tracks, start);
  // On wide windows the player is already visible beside the list.
  if (!app.wide) app.fullPlayerOpen = true;
}
