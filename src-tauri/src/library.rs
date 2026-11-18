use std::collections::HashSet;
use std::path::{Path, PathBuf};

use base64::Engine;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

pub const AUDIO_EXTENSIONS: [&str; 7] = ["mp3", "flac", "ogg", "wav", "m4a", "aac", "oga"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Track {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: u32,
    pub duration: f64,
    pub format: String,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct SearchFilter {
    pub query: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub format: Option<String>,
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
}

#[derive(Serialize, Default)]
pub struct FilterOptions {
    pub artists: Vec<String>,
    pub albums: Vec<String>,
    pub genres: Vec<String>,
    pub formats: Vec<String>,
}

#[derive(Serialize, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            album TEXT NOT NULL,
            genre TEXT NOT NULL,
            year INTEGER NOT NULL,
            duration REAL NOT NULL,
            format TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL
        );
        CREATE TABLE IF NOT EXISTS playlist_tracks (
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, track_id)
        );
        PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn read_track(path: &Path) -> Option<Track> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let file_stem = path.file_stem()?.to_string_lossy().into_owned();

    let text = |value: Option<std::borrow::Cow<str>>, fallback: &str| {
        value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    };

    Some(Track {
        id: 0,
        path: path.to_string_lossy().into_owned(),
        title: text(tag.and_then(|t| t.title()), &file_stem),
        artist: text(tag.and_then(|t| t.artist()), "Unknown Artist"),
        album: text(tag.and_then(|t| t.album()), "Unknown Album"),
        genre: text(tag.and_then(|t| t.genre()), "Unknown"),
        year: tag.and_then(|t| t.date()).map(|d| d.year as u32).unwrap_or(0),
        duration: tagged.properties().duration().as_secs_f64(),
        format: path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
    })
}

pub fn scan_folders(folders: &[PathBuf]) -> Vec<Track> {
    folders
        .iter()
        .flat_map(|folder| WalkDir::new(folder).follow_links(true).into_iter().flatten())
        .filter(|entry| entry.file_type().is_file() && is_audio(entry.path()))
        .filter_map(|entry| read_track(entry.path()))
        .collect()
}

pub fn store_scan(conn: &mut Connection, tracks: &[Track]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for track in tracks {
        tx.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, year, duration, format)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(path) DO UPDATE SET
                title = ?2, artist = ?3, album = ?4, genre = ?5,
                year = ?6, duration = ?7, format = ?8",
            params![
                track.path,
                track.title,
                track.artist,
                track.album,
                track.genre,
                track.year,
                track.duration,
                track.format
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    let known: HashSet<&str> = tracks.iter().map(|t| t.path.as_str()).collect();
    let stored: Vec<(i64, String)> = {
        let mut stmt = tx
            .prepare("SELECT id, path FROM tracks")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.flatten().collect()
    };
    for (id, path) in stored {
        if !known.contains(path.as_str()) {
            tx.execute("DELETE FROM tracks WHERE id = ?1", [id])
                .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())
}

fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        genre: row.get(5)?,
        year: row.get(6)?,
        duration: row.get(7)?,
        format: row.get(8)?,
    })
}

const TRACK_COLUMNS: &str = "t.id, t.path, t.title, t.artist, t.album, t.genre, t.year, t.duration, t.format";

pub fn all_tracks(conn: &Connection) -> Result<Vec<Track>, String> {
    let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks t ORDER BY t.artist, t.album, t.title");
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], row_to_track).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn search(conn: &Connection, filter: &SearchFilter) -> Result<Vec<Track>, String> {
    let mut conditions: Vec<String> = Vec::new();
    let mut values: Vec<rusqlite::types::Value> = Vec::new();

    let query = filter.query.trim();
    if !query.is_empty() {
        conditions.push("(t.title LIKE ? OR t.artist LIKE ? OR t.album LIKE ?)".into());
        for _ in 0..3 {
            values.push(format!("%{query}%").into());
        }
    }

    let exact = [
        ("t.artist", &filter.artist),
        ("t.album", &filter.album),
        ("t.genre", &filter.genre),
        ("t.format", &filter.format),
    ];
    for (column, value) in exact {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty()) {
            conditions.push(format!("{column} = ?"));
            values.push(value.clone().into());
        }
    }

    if let Some(from) = filter.year_from {
        conditions.push("t.year >= ?".into());
        values.push((from as i64).into());
    }
    if let Some(to) = filter.year_to {
        conditions.push("t.year <= ?".into());
        values.push((to as i64).into());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t {where_clause} ORDER BY t.artist, t.album, t.title"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(values), row_to_track)
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn filter_options(conn: &Connection) -> Result<FilterOptions, String> {
    let distinct = |column: &str| -> Result<Vec<String>, String> {
        let sql = format!("SELECT DISTINCT {column} FROM tracks ORDER BY {column}");
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    };

    Ok(FilterOptions {
        artists: distinct("artist")?,
        albums: distinct("album")?,
        genres: distinct("genre")?,
        formats: distinct("format")?,
    })
}

pub fn playlists(conn: &Connection) -> Result<Vec<Playlist>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.name, COUNT(pt.track_id) FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
             GROUP BY p.id ORDER BY p.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn create_playlist(conn: &Connection, name: &str) -> Result<(), String> {
    conn.execute("INSERT INTO playlists (name) VALUES (?1)", [name.trim()])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn playlist_tracks(conn: &Connection, id: i64) -> Result<Vec<Track>, String> {
    let sql = format!(
        "SELECT {TRACK_COLUMNS} FROM tracks t
         JOIN playlist_tracks pt ON pt.track_id = t.id
         WHERE pt.playlist_id = ?1 ORDER BY pt.position"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([id], row_to_track).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn add_to_playlist(conn: &Connection, playlist_id: i64, track_id: i64) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1))",
        params![playlist_id, track_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_from_playlist(conn: &Connection, playlist_id: i64, track_id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        params![playlist_id, track_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn cover_data_url(path: &Path) -> Option<String> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag.pictures().first()?;
    let mime = picture.mime_type().map(|m| m.to_string()).unwrap_or_else(|| "image/jpeg".into());
    let encoded = base64::engine::general_purpose::STANDARD.encode(picture.data());
    Some(format!("data:{mime};base64,{encoded}"))
}
