<p align="center"><img src="assets/logo.svg" alt="MossAMP logo" width="128"></p>

# MossAMP

A cozy local music player for Linux desktop and Android, built with Rust, Tauri 2 and Svelte 5.
Pastel moss greens by default, fully themeable, and no accounts, ads or tracking. It plays the
music files you already have.

## Features

- **Local library**: scans your music folders and reads tags and cover art. Supports MP3, FLAC,
  Ogg Vorbis, WAV, AAC/M4A and ALAC.
- **Playlists**
  - Create a playlist from a folder. Every song in it (and its subfolders) is added, and new files
    dropped into the folder are picked up on the next scan.
  - Add any song from your library through a searchable picker with multi-select.
  - Playlists only *reference* songs. Nothing is ever copied, so a song can be in any number of
    playlists while existing once on disk.
- **Search** across title, artist and album, with filters for artist, album, genre, format and year range.
- **10-band equalizer** with presets. Automatic headroom and a soft limiter keep boosts from clipping.
- **Volume leveling**: every song plays at about the same loudness (EBU R128, −16 LUFS), so you set
  your volume once. Songs are measured once in the background; quiet ones are only raised as far as
  they can go without clipping.
- **Playback**: queue, play next, shuffle, repeat one/all and seeking.
- **Survives device changes**: switching to Bluetooth, headphones or the car mid-song reopens the
  audio output automatically and continues where it was.
- **Cloud sync**: pull music from any WebDAV server (Nextcloud, ownCloud, most NAS boxes).
- **Desktop media keys** through MPRIS, plus a live-reloading TOML config.
- **A proper Android music app**: media controls in the notification shade and on the lock screen
  (cover, seek bar, play/pause, skip), Bluetooth, car and headset buttons, pausing for phone calls
  and other players, and pausing when headphones or Bluetooth disconnect.

## Building

### Requirements

- Rust 1.90 or newer
- Node.js 20 or newer with npm
- Tauri CLI: `cargo install tauri-cli --locked`
- On Linux: the [Tauri system dependencies](https://tauri.app/start/prerequisites/)
  (webkit2gtk-4.1, gtk3, alsa-lib, …)

### Desktop

```sh
npm install
cargo tauri dev            # run in development mode
cargo tauri build          # build a release bundle
```

### Android

You need the Android SDK with platform 36 and NDK 30. The app runs on Android 8.0 (API 26) or newer,
the first version with AAudio, which the audio engine uses.

```sh
export ANDROID_HOME=~/Android/Sdk
export ANDROID_SDK_ROOT=$ANDROID_HOME   # must match ANDROID_HOME if set at all
export NDK_HOME=$ANDROID_HOME/ndk/<version>

rustup target add aarch64-linux-android
npm install
cargo tauri android build --debug --target aarch64 --apk
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

Notes:

- The default library folder is `/storage/emulated/0/Music`. The app asks for audio access on
  first start; afterwards press **Scan now** in Settings.
- The folder picker in Settings returns content URIs on Android, which plain file access cannot
  read. Type the folder path instead, or use cloud sync.
- Volume leveling measures each song once (the first run over a big library takes a few minutes,
  longer in debug builds) and remembers the result.

### Tests

```sh
cd src-tauri && cargo test     # library, playlists, equalizer, cloud
npm run check                  # Svelte / TypeScript type check
```

## Configuration

On desktop, `~/.config/mossamp/config.toml` is created on first start and **watched live**. Edit it
in any editor and the app updates instantly: theme colors, corner radius, font, library folders,
playback, EQ and cloud login. A broken file shows an error in Settings, and the last good config
stays active. On Android the same options live in the Settings page.

## Cloud sync

In **Settings → Cloud**, enter a WebDAV URL, your login and the remote music folder, then press
**Sync**. Songs are downloaded into the app's data folder and indexed with the rest of the library.

> The password is stored in plain text in `config.toml`. Use an app password or token, not your
> main account password.

## Project layout

```
src/                  Svelte frontend
  lib/                pages, components, store and the typed API to the backend
src-tauri/
  src/
    player.rs         playback thread, queue, output-device recovery
    eq.rs             10-band biquad equalizer with headroom and limiter
    loudness.rs       volume leveling (EBU R128 measurement in the background)
    library.rs        scanning, SQLite library, playlists, folders
    cloud.rs          WebDAV sync
    config.rs         TOML config
    media_keys.rs     MPRIS media keys (desktop)
    lib.rs            Tauri commands and app setup
  gen/android/        Android project (playback service, media session, bridge)
```

## Contributing

Contributions are very welcome, and I'd love for you to get involved! Whether it's a bug fix, a new
feature, a theme, a translation, better docs or just an idea, feel free to open an issue or a pull
request.

- **Found a bug or have an idea?** [Open an issue](https://github.com/MaeTobiGeri/MossAMP/issues).
  For bugs, say what you did, what you expected and what happened, plus your OS or phone model.
- **Want to change something?** Fork the repo, make your change on a branch and open a pull request.
  For bigger changes, open an issue first so we can talk it through.
- **Before submitting**, please run `cargo test` in `src-tauri` and `npm run check`, and keep the
  existing code style.

By contributing you agree that your contribution is licensed under the GPL, like the rest of the
project.

## License

MossAMP is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

It is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See [LICENSE](LICENSE)
for the full text.

Copyright © 2026 MaeTobiGeri
