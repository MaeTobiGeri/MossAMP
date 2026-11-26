use std::collections::{BTreeMap, HashSet};
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
    /// Folder the playlist is linked to; its songs are added automatically.
    pub folder: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Folder {
    pub path: String,
    /// Path shown to the user, relative to the library folder's parent.
    pub label: String,
    pub name: String,
    /// Songs in this folder and all its subfolders.
    pub count: usize,
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

    // Migrations for databases created before folder playlists existed. The
    // ALTERs fail harmlessly when the column is already there.
    let _ = conn.execute("ALTER TABLE playlists ADD COLUMN folder TEXT", []);
    // Songs removed from a folder playlist stay excluded instead of being
    // re-added by the next scan.
    let _ = conn.execute(
        "ALTER TABLE playlist_tracks ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0",
        [],
    );
    Ok(conn)
}

/// Path prefix shared by every file inside `folder`.
fn folder_prefix(folder: &str) -> String {
    format!("{}{}", folder.trim_end_matches(['/', '\\']), std::path::MAIN_SEPARATOR)
}

/// Adds every known song inside the playlist's folder that is not in it yet.
fn sync_folder_playlist(conn: &Connection, playlist_id: i64, folder: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position)
         SELECT ?1, t.id,
                (SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE playlist_id = ?1)
                + ROW_NUMBER() OVER (ORDER BY t.path)
         FROM tracks t
         WHERE substr(t.path, 1, length(?2)) = ?2",
        params![playlist_id, folder_prefix(folder)],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn sync_folder_playlists(conn: &Connection) -> Result<(), String> {
    let linked: Vec<(i64, String)> = {
        let mut stmt = conn
            .prepare("SELECT id, folder FROM playlists WHERE folder IS NOT NULL")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.flatten().collect()
    };
    for (id, folder) in linked {
        sync_folder_playlist(conn, id, &folder)?;
    }
    Ok(())
}

/// Every folder that holds songs (directly or in subfolders), below the
/// library roots.
pub fn folders(conn: &Connection, roots: &[PathBuf]) -> Result<Vec<Folder>, String> {
    let mut stmt = conn.prepare("SELECT path FROM tracks").map_err(|e| e.to_string())?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();

    let mut counts: BTreeMap<PathBuf, (usize, PathBuf)> = BTreeMap::new();
    for path in &paths {
        let path = Path::new(path);
        let Some(root) = roots.iter().find(|root| path.starts_with(root)) else {
            continue;
        };
        let mut dir = path.parent();
        while let Some(current) = dir {
            if !current.starts_with(root) {
                break;
            }
            let entry = counts
                .entry(current.to_path_buf())
                .or_insert_with(|| (0, root.clone()));
            entry.0 += 1;
            dir = current.parent();
        }
    }

    Ok(counts
        .into_iter()
        .map(|(path, (count, root))| {
            let base = root.parent().unwrap_or(&root);
            let label = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            Folder {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| label.clone()),
                path: path.to_string_lossy().into_owned(),
                label,
                count,
            }
        })
        .collect())
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

    sync_folder_playlists(&tx)?;
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
            "SELECT p.id, p.name, COUNT(pt.track_id), p.folder FROM playlists p
             LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id AND pt.hidden = 0
             GROUP BY p.id ORDER BY p.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                count: row.get(2)?,
                folder: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn create_playlist(conn: &Connection, name: &str, folder: Option<&str>) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Playlist name is empty".into());
    }
    conn.execute(
        "INSERT INTO playlists (name, folder) VALUES (?1, ?2)",
        params![name, folder],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
            "A playlist with that name already exists".to_string()
        }
        other => other.to_string(),
    })?;
    let id = conn.last_insert_rowid();
    if let Some(folder) = folder {
        sync_folder_playlist(conn, id, folder)?;
    }
    Ok(id)
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
         WHERE pt.playlist_id = ?1 AND pt.hidden = 0 ORDER BY pt.position"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([id], row_to_track).map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

/// Adds references to existing songs; the audio files themselves are never
/// copied, so a song can be in any number of playlists.
pub fn add_to_playlist(conn: &mut Connection, playlist_id: i64, track_ids: &[i64]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for track_id in track_ids {
        tx.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1))
             ON CONFLICT(playlist_id, track_id) DO UPDATE SET
                hidden = 0,
                position = CASE WHEN hidden = 1 THEN excluded.position ELSE position END",
            params![playlist_id, track_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn remove_from_playlist(conn: &Connection, playlist_id: i64, track_id: i64) -> Result<(), String> {
    // In a folder playlist the row is kept but marked, so the next scan does
    // not put the song back.
    conn.execute(
        "UPDATE playlist_tracks SET hidden = 1
         WHERE playlist_id = ?1 AND track_id = ?2
           AND (SELECT folder FROM playlists WHERE id = ?1) IS NOT NULL",
        params![playlist_id, track_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM playlist_tracks
         WHERE playlist_id = ?1 AND track_id = ?2
           AND (SELECT folder FROM playlists WHERE id = ?1) IS NULL",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn track(path: &str) -> Track {
        Track {
            id: 0,
            path: path.into(),
            title: path.into(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            year: 0,
            duration: 0.0,
            format: "mp3".into(),
        }
    }

    fn titles(conn: &Connection, id: i64) -> Vec<String> {
        playlist_tracks(conn, id).unwrap().into_iter().map(|t| t.path).collect()
    }

    #[test]
    fn folder_playlists_reference_songs_and_follow_scans() {
        let mut conn = open(Path::new(":memory:")).unwrap();
        let scan = vec![track("/m/Rock/a.mp3"), track("/m/Rock/sub/b.mp3"), track("/m/rock2/c.mp3"), track("/m/Pop/d.mp3")];
        store_scan(&mut conn, &scan).unwrap();

        let rock = create_playlist(&conn, "Rock", Some("/m/Rock")).unwrap();
        assert_eq!(titles(&conn, rock), ["/m/Rock/a.mp3", "/m/Rock/sub/b.mp3"]);
        assert!(create_playlist(&conn, "Rock", None).is_err());

        // Removing from a folder playlist survives a rescan.
        let a = playlist_tracks(&conn, rock).unwrap()[0].id;
        remove_from_playlist(&conn, rock, a).unwrap();
        store_scan(&mut conn, &scan).unwrap();
        assert_eq!(titles(&conn, rock), ["/m/Rock/sub/b.mp3"]);

        // New files in the folder are picked up; manual adds are references.
        let mut scan2 = scan.clone();
        scan2.push(track("/m/Rock/e.mp3"));
        store_scan(&mut conn, &scan2).unwrap();
        let pop = all_tracks(&conn).unwrap().into_iter().find(|t| t.path == "/m/Pop/d.mp3").unwrap();
        add_to_playlist(&mut conn, rock, &[pop.id, a]).unwrap();
        assert_eq!(titles(&conn, rock), ["/m/Rock/sub/b.mp3", "/m/Rock/e.mp3", "/m/Pop/d.mp3", "/m/Rock/a.mp3"]);
        assert_eq!(playlists(&conn).unwrap()[0].count, 4);

        let mix = create_playlist(&conn, "Mix", None).unwrap();
        add_to_playlist(&mut conn, mix, &[pop.id]).unwrap();
        remove_from_playlist(&conn, mix, pop.id).unwrap();
        assert!(titles(&conn, mix).is_empty());

        let roots = [PathBuf::from("/m")];
        let found: Vec<(String, usize)> = folders(&conn, &roots).unwrap().into_iter().map(|f| (f.label, f.count)).collect();
        assert_eq!(found, [("m".into(), 5), ("m/Pop".into(), 1), ("m/Rock".into(), 3), ("m/Rock/sub".into(), 1), ("m/rock2".into(), 1)]);
    }
}
