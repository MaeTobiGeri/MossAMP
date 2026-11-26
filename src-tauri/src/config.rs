use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const EQ_BANDS: usize = 10;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Theme {
    pub background: String,
    pub background_soft: String,
    pub background_raised: String,
    pub foreground: String,
    pub foreground_muted: String,
    pub accent: String,
    pub accent_soft: String,
    pub warm: String,
    pub danger: String,
    pub radius: u32,
    pub font: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#1d1e1c".into(),
            background_soft: "#262825".into(),
            background_raised: "#30332f".into(),
            foreground: "#ebe8de".into(),
            foreground_muted: "#9c9d94".into(),
            accent: "#b4d47c".into(),
            accent_soft: "#56663d".into(),
            warm: "#e3b766".into(),
            danger: "#ec7a6f".into(),
            radius: 22,
            font: "Nunito Variable, Nunito, system-ui, sans-serif".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct LibraryConfig {
    pub folders: Vec<String>,
    pub scan_on_start: bool,
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            scan_on_start: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct PlaybackConfig {
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
    /// Volume leveling: play every song at about the same loudness.
    pub normalize: bool,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            volume: 0.8,
            shuffle: false,
            repeat: Repeat::Off,
            normalize: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct EqConfig {
    pub enabled: bool,
    pub gains: [f32; EQ_BANDS],
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gains: [0.0; EQ_BANDS],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct CloudConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
    pub sync_on_start: bool,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            remote_path: "/".into(),
            sync_on_start: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub theme: Theme,
    pub library: LibraryConfig,
    pub playback: PlaybackConfig,
    pub eq: EqConfig,
    pub cloud: CloudConfig,
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        toml::from_str(&text).map_err(|e| e.to_string())
    }

    pub fn load_or_create(path: &Path, default_music_dir: Option<PathBuf>) -> Config {
        if let Ok(config) = Config::load(path) {
            return config;
        }

        let mut config = Config::default();
        if let Some(dir) = default_music_dir {
            config.library.folders.push(dir.to_string_lossy().into_owned());
        }
        let _ = config.save(path);
        config
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }
}
