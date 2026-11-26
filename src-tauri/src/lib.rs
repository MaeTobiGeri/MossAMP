mod cloud;
mod config;
mod eq;
mod library;
mod loudness;
mod media_keys;
mod player;

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, State};

use config::{Config, Repeat, EQ_BANDS};
use eq::EqShared;
use library::{FilterOptions, Folder, Playlist, SearchFilter, Track};
use player::Command;

pub(crate) struct AppState {
    pub(crate) config: Mutex<Config>,
    config_path: PathBuf,
    cache_dir: PathBuf,
    pub(crate) db: Mutex<Connection>,
    player: Sender<Command>,
    analyzer: Sender<loudness::Request>,
    eq: Arc<EqShared>,
    watcher: Mutex<Option<Box<dyn std::any::Any + Send>>>,
}

impl AppState {
    fn scan_folders(&self) -> Vec<PathBuf> {
        let config = self.config.lock().unwrap();
        let mut folders: Vec<PathBuf> = config.library.folders.iter().map(PathBuf::from).collect();
        folders.push(self.cache_dir.clone());
        folders
    }

    fn apply(&self, config: &Config) {
        self.eq.set(config.eq.enabled, config.eq.gains);
        let _ = self.player.send(Command::Volume(config.playback.volume));
        let _ = self.player.send(Command::Shuffle(config.playback.shuffle));
        let _ = self.player.send(Command::Repeat(config.playback.repeat));
        let _ = self.player.send(Command::Normalize(config.playback.normalize));
        if config.playback.normalize {
            let _ = self.analyzer.send(loudness::Request::Wake);
        }
    }
}

fn update_config(
    app: &AppHandle,
    state: &AppState,
    change: impl FnOnce(&mut Config),
) -> Result<Config, String> {
    let updated = {
        let mut config = state.config.lock().unwrap();
        change(&mut config);
        config.save(&state.config_path)?;
        config.clone()
    };
    state.apply(&updated);
    let _ = app.emit("config-changed", &updated);
    Ok(updated)
}

async fn rescan(app: &AppHandle) -> Result<usize, String> {
    let folders = app.state::<AppState>().scan_folders();
    let tracks = tauri::async_runtime::spawn_blocking(move || library::scan_folders(&folders))
        .await
        .map_err(|e| e.to_string())?;

    let count = tracks.len();
    let state = app.state::<AppState>();
    library::store_scan(&mut state.db.lock().unwrap(), &tracks)?;
    let _ = app.emit("library-updated", count);
    // New songs need measuring (does nothing while leveling is off).
    let _ = state.analyzer.send(loudness::Request::Wake);
    Ok(count)
}

#[tauri::command]
fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn get_config_path(state: State<AppState>) -> String {
    state.config_path.to_string_lossy().into_owned()
}

#[tauri::command]
fn save_config(app: AppHandle, state: State<AppState>, config: Config) -> Result<(), String> {
    update_config(&app, &state, |current| *current = config).map(|_| ())
}

#[tauri::command]
async fn scan_library(app: AppHandle) -> Result<usize, String> {
    rescan(&app).await
}

#[tauri::command]
fn get_tracks(state: State<AppState>) -> Result<Vec<Track>, String> {
    library::all_tracks(&state.db.lock().unwrap())
}

#[tauri::command]
fn search_tracks(state: State<AppState>, filter: SearchFilter) -> Result<Vec<Track>, String> {
    library::search(&state.db.lock().unwrap(), &filter)
}

#[tauri::command]
fn get_filter_options(state: State<AppState>) -> Result<FilterOptions, String> {
    library::filter_options(&state.db.lock().unwrap())
}

#[tauri::command]
async fn get_cover(path: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || library::cover_data_url(std::path::Path::new(&path)))
        .await
        .ok()
        .flatten()
}

#[tauri::command]
fn get_loudness_progress(state: State<AppState>) -> Result<loudness::Progress, String> {
    library::loudness_progress(&state.db.lock().unwrap())
}

#[tauri::command]
fn get_playlists(state: State<AppState>) -> Result<Vec<Playlist>, String> {
    library::playlists(&state.db.lock().unwrap())
}

#[tauri::command]
fn create_playlist(state: State<AppState>, name: String, folder: Option<String>) -> Result<i64, String> {
    library::create_playlist(&state.db.lock().unwrap(), &name, folder.as_deref())
}

#[tauri::command]
fn get_folders(state: State<AppState>) -> Result<Vec<Folder>, String> {
    let roots = state.scan_folders();
    library::folders(&state.db.lock().unwrap(), &roots)
}

#[tauri::command]
fn delete_playlist(state: State<AppState>, id: i64) -> Result<(), String> {
    library::delete_playlist(&state.db.lock().unwrap(), id)
}

#[tauri::command]
fn get_playlist_tracks(state: State<AppState>, id: i64) -> Result<Vec<Track>, String> {
    library::playlist_tracks(&state.db.lock().unwrap(), id)
}

#[tauri::command]
fn add_to_playlist(state: State<AppState>, playlist_id: i64, track_ids: Vec<i64>) -> Result<(), String> {
    library::add_to_playlist(&mut state.db.lock().unwrap(), playlist_id, &track_ids)
}

#[tauri::command]
fn remove_from_playlist(state: State<AppState>, playlist_id: i64, track_id: i64) -> Result<(), String> {
    library::remove_from_playlist(&state.db.lock().unwrap(), playlist_id, track_id)
}

#[tauri::command]
fn play_queue(state: State<AppState>, tracks: Vec<Track>, start: usize) {
    let _ = state.player.send(Command::PlayQueue(tracks, start));
}

#[tauri::command]
fn enqueue_track(state: State<AppState>, track: Track) {
    let _ = state.player.send(Command::Enqueue(track));
}

#[tauri::command]
fn play_next(state: State<AppState>, track: Track) {
    let _ = state.player.send(Command::PlayNext(track));
}

#[tauri::command]
fn player_resume(state: State<AppState>) {
    let _ = state.player.send(Command::Resume);
}

#[tauri::command]
fn player_pause(state: State<AppState>) {
    let _ = state.player.send(Command::Pause);
}

#[tauri::command]
fn player_toggle(state: State<AppState>) {
    let _ = state.player.send(Command::Toggle);
}

#[tauri::command]
fn player_next(state: State<AppState>) {
    let _ = state.player.send(Command::Next);
}

#[tauri::command]
fn player_previous(state: State<AppState>) {
    let _ = state.player.send(Command::Previous);
}

#[tauri::command]
fn player_seek(state: State<AppState>, seconds: f64) {
    let _ = state.player.send(Command::Seek(seconds));
}

#[tauri::command]
fn player_set_volume(app: AppHandle, state: State<AppState>, volume: f32) -> Result<(), String> {
    update_config(&app, &state, |config| config.playback.volume = volume.clamp(0.0, 1.0)).map(|_| ())
}

#[tauri::command]
fn player_set_shuffle(app: AppHandle, state: State<AppState>, enabled: bool) -> Result<(), String> {
    update_config(&app, &state, |config| config.playback.shuffle = enabled).map(|_| ())
}

#[tauri::command]
fn player_set_repeat(app: AppHandle, state: State<AppState>, mode: Repeat) -> Result<(), String> {
    update_config(&app, &state, |config| config.playback.repeat = mode).map(|_| ())
}

#[tauri::command]
fn set_eq(app: AppHandle, state: State<AppState>, enabled: bool, gains: [f32; EQ_BANDS]) -> Result<(), String> {
    let gains = gains.map(|gain| gain.clamp(-12.0, 12.0));
    update_config(&app, &state, |config| {
        config.eq.enabled = enabled;
        config.eq.gains = gains;
    })
    .map(|_| ())
}

#[tauri::command]
fn eq_frequencies() -> [f32; EQ_BANDS] {
    eq::BAND_FREQUENCIES
}

#[tauri::command]
async fn cloud_test(config: config::CloudConfig) -> Result<usize, String> {
    cloud::test_connection(&config).await
}

#[tauri::command]
async fn cloud_sync(app: AppHandle) -> Result<cloud::SyncReport, String> {
    let (cloud_config, cache_dir) = {
        let state = app.state::<AppState>();
        let config = state.config.lock().unwrap().cloud.clone();
        (config, state.cache_dir.clone())
    };
    if !cloud_config.enabled {
        return Err("Cloud is not enabled".into());
    }

    let report = cloud::sync(&app, &cloud_config, &cache_dir).await?;
    rescan(&app).await?;
    Ok(report)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn watch_config(app: &AppHandle) -> Option<Box<dyn std::any::Any + Send>> {
    use notify::{RecursiveMode, Watcher};

    let handle = app.clone();
    let path = app.state::<AppState>().config_path.clone();
    let directory = path.parent()?.to_path_buf();

    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !event.paths.iter().any(|changed| changed.file_name() == path.file_name()) {
            return;
        }

        let state = handle.state::<AppState>();
        match Config::load(&path) {
            Ok(loaded) => {
                let _ = handle.emit("config-error", Option::<String>::None);
                let changed = {
                    let mut current = state.config.lock().unwrap();
                    let changed = *current != loaded;
                    *current = loaded.clone();
                    changed
                };
                if changed {
                    state.apply(&loaded);
                    let _ = handle.emit("config-changed", &loaded);
                }
            }
            Err(message) => {
                let _ = handle.emit("config-error", Some(message));
            }
        }
    })
    .ok()?;

    std::fs::create_dir_all(&directory).ok()?;
    watcher.watch(&directory, RecursiveMode::NonRecursive).ok()?;
    Some(Box::new(watcher))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn watch_config(_app: &AppHandle) -> Option<Box<dyn std::any::Any + Send>> {
    None
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    let config_path = paths.config_dir()?.join("mossamp").join("config.toml");
    let data_dir = paths.app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;

    let music_dir = if cfg!(target_os = "android") {
        Some(PathBuf::from("/storage/emulated/0/Music"))
    } else {
        paths.audio_dir().ok()
    };
    let config = Config::load_or_create(&config_path, music_dir);
    let eq = EqShared::new(config.eq.enabled, config.eq.gains);
    let (analyzer, analyzer_requests) = std::sync::mpsc::channel();
    let player = player::spawn(app.handle().clone(), eq.clone(), config.playback.clone(), analyzer.clone());
    loudness::spawn(app.handle().clone(), analyzer_requests, player.clone());
    let startup = config.clone();

    app.manage(AppState {
        config: Mutex::new(config),
        config_path,
        cache_dir: data_dir.join("cloud"),
        db: Mutex::new(library::open(&data_dir.join("library.db"))?),
        player,
        analyzer: analyzer.clone(),
        eq,
        watcher: Mutex::new(None),
    });

    // Picks up songs left unmeasured last time (does nothing while leveling is off).
    let _ = analyzer.send(loudness::Request::Wake);

    let watcher = watch_config(app.handle());
    *app.state::<AppState>().watcher.lock().unwrap() = watcher;

    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        if startup.cloud.enabled && startup.cloud.sync_on_start {
            let cache_dir = handle.state::<AppState>().cache_dir.clone();
            let _ = cloud::sync(&handle, &startup.cloud, &cache_dir).await;
        }
        if startup.library.scan_on_start {
            let _ = rescan(&handle).await;
        }
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_config_path,
            save_config,
            scan_library,
            get_tracks,
            search_tracks,
            get_filter_options,
            get_cover,
            get_loudness_progress,
            get_playlists,
            create_playlist,
            get_folders,
            delete_playlist,
            get_playlist_tracks,
            add_to_playlist,
            remove_from_playlist,
            play_queue,
            enqueue_track,
            play_next,
            player_resume,
            player_pause,
            player_toggle,
            player_next,
            player_previous,
            player_seek,
            player_set_volume,
            player_set_shuffle,
            player_set_repeat,
            set_eq,
            eq_frequencies,
            cloud_test,
            cloud_sync,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run mossamp");
}
