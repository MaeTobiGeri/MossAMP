# Musicapp

A cozy, Poweramp-inspired music player built with Rust, Tauri 2 and Svelte. Pastel gruvbox greens by default.

## Run

    npm install
    cargo tauri dev

Needs `cargo install tauri-cli --locked` and the Tauri Linux dependencies (webkit2gtk-4.1, gtk3, alsa-lib).

## Config (desktop)

`~/.config/musicapp/config.toml` is created on first start and watched live. Edit it in any editor and the
app updates instantly: theme colors, radius, font, library folders, playback, EQ and cloud login. A broken
file shows an error in Settings and the last good config stays active. On mobile the same options live in
the Settings page.

## Cloud

Settings, Cloud: enter a WebDAV URL (Nextcloud, ownCloud, most NAS), login and music folder, then Sync.
Songs are downloaded into the app data folder and indexed with the rest of the library. The password is
stored in plain text in `config.toml`, so use an app token.

## Android

Setup (SDK, NDK 30 and platform 36 installed; Android 8.0 / API 26 or newer):

    export ANDROID_HOME=~/Android/Sdk NDK_HOME=$ANDROID_HOME/ndk/<version>
    rustup target add aarch64-linux-android
    cargo tauri android build --debug --target aarch64 --apk
    adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk

- The default library folder on Android is `/storage/emulated/0/Music`. The app asks for audio access
  on first start; press Scan now in Settings after granting it.
- Playback continues in the background through a foreground service (`PlaybackService.kt`) with a
  notification showing the current song. Notification buttons and lock-screen controls are not built yet.
- The Browse button in Settings returns content URIs on Android that plain file access cannot read, so
  type the folder path instead, or use cloud sync.
