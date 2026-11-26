import { invoke } from "@tauri-apps/api/core";

export interface Track {
  id: number;
  path: string;
  title: string;
  artist: string;
  album: string;
  genre: string;
  year: number;
  duration: number;
  format: string;
}

export interface Playlist {
  id: number;
  name: string;
  count: number;
  /** Folder the playlist is linked to; new songs there are added on every scan. */
  folder: string | null;
}

export interface Folder {
  path: string;
  label: string;
  name: string;
  count: number;
}

export type Repeat = "off" | "all" | "one";

export interface Theme {
  background: string;
  background_soft: string;
  background_raised: string;
  foreground: string;
  foreground_muted: string;
  accent: string;
  accent_soft: string;
  warm: string;
  danger: string;
  radius: number;
  font: string;
}

export interface CloudConfig {
  enabled: boolean;
  url: string;
  username: string;
  password: string;
  remote_path: string;
  sync_on_start: boolean;
}

export interface Config {
  theme: Theme;
  library: { folders: string[]; scan_on_start: boolean };
  playback: { volume: number; shuffle: boolean; repeat: Repeat };
  eq: { enabled: boolean; gains: number[] };
  cloud: CloudConfig;
}

export interface PlayerState {
  track: Track | null;
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  shuffle: boolean;
  repeat: Repeat;
  queue_length: number;
}

export interface SearchFilter {
  query: string;
  artist: string | null;
  album: string | null;
  genre: string | null;
  format: string | null;
  year_from: number | null;
  year_to: number | null;
}

export interface FilterOptions {
  artists: string[];
  albums: string[];
  genres: string[];
  formats: string[];
}

export interface SyncReport {
  downloaded: number;
  skipped: number;
  failed: number;
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  getConfigPath: () => invoke<string>("get_config_path"),
  saveConfig: (config: Config) => invoke<void>("save_config", { config }),

  scanLibrary: () => invoke<number>("scan_library"),
  getTracks: () => invoke<Track[]>("get_tracks"),
  searchTracks: (filter: SearchFilter) => invoke<Track[]>("search_tracks", { filter }),
  getFilterOptions: () => invoke<FilterOptions>("get_filter_options"),
  getCover: (path: string) => invoke<string | null>("get_cover", { path }),

  getPlaylists: () => invoke<Playlist[]>("get_playlists"),
  createPlaylist: (name: string, folder: string | null) =>
    invoke<number>("create_playlist", { name, folder }),
  getFolders: () => invoke<Folder[]>("get_folders"),
  deletePlaylist: (id: number) => invoke<void>("delete_playlist", { id }),
  getPlaylistTracks: (id: number) => invoke<Track[]>("get_playlist_tracks", { id }),
  addToPlaylist: (playlistId: number, trackIds: number[]) =>
    invoke<void>("add_to_playlist", { playlistId, trackIds }),
  removeFromPlaylist: (playlistId: number, trackId: number) =>
    invoke<void>("remove_from_playlist", { playlistId, trackId }),

  playQueue: (tracks: Track[], start: number) => invoke<void>("play_queue", { tracks, start }),
  enqueueTrack: (track: Track) => invoke<void>("enqueue_track", { track }),
  playNext: (track: Track) => invoke<void>("play_next", { track }),
  toggle: () => invoke<void>("player_toggle"),
  next: () => invoke<void>("player_next"),
  previous: () => invoke<void>("player_previous"),
  seek: (seconds: number) => invoke<void>("player_seek", { seconds }),
  setVolume: (volume: number) => invoke<void>("player_set_volume", { volume }),
  setShuffle: (enabled: boolean) => invoke<void>("player_set_shuffle", { enabled }),
  setRepeat: (mode: Repeat) => invoke<void>("player_set_repeat", { mode }),

  setEq: (enabled: boolean, gains: number[]) => invoke<void>("set_eq", { enabled, gains }),
  eqFrequencies: () => invoke<number[]>("eq_frequencies"),

  cloudTest: (config: CloudConfig) => invoke<number>("cloud_test", { config }),
  cloudSync: () => invoke<SyncReport>("cloud_sync"),
};

export function formatTime(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  const rest = String(total % 60).padStart(2, "0");
  return `${minutes}:${rest}`;
}
