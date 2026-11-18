use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::config::CloudConfig;
use crate::library::AUDIO_EXTENSIONS;

const PROPFIND_BODY: &str = r#"<?xml version="1.0"?>
<d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/><d:getcontentlength/></d:prop></d:propfind>"#;

#[derive(Debug)]
struct RemoteEntry {
    href: String,
    size: u64,
    is_dir: bool,
}

#[derive(Serialize, Clone)]
struct Progress {
    done: usize,
    total: usize,
    current: String,
}

#[derive(Serialize)]
pub struct SyncReport {
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
}

fn base_url(config: &CloudConfig) -> String {
    let url = config.url.trim_end_matches('/');
    let path = config.remote_path.trim_matches('/');
    if path.is_empty() {
        format!("{url}/")
    } else {
        format!("{url}/{path}/")
    }
}

fn origin(url: &str) -> String {
    match url.find("://") {
        Some(scheme_end) => match url[scheme_end + 3..].find('/') {
            Some(slash) => url[..scheme_end + 3 + slash].to_string(),
            None => url.to_string(),
        },
        None => url.to_string(),
    }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                decoded.push(value);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn parse_listing(xml: &str) -> Vec<RemoteEntry> {
    let mut reader = Reader::from_str(xml);
    let mut entries = Vec::new();
    let mut current: Option<RemoteEntry> = None;
    let mut field = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.local_name().into_inner().to_lowercase();
                match name.as_str() {
                    "response" => {
                        current = Some(RemoteEntry {
                            href: String::new(),
                            size: 0,
                            is_dir: false,
                        })
                    }
                    "collection" => {
                        if let Some(entry) = &mut current {
                            entry.is_dir = true;
                        }
                    }
                    _ => field = name,
                }
            }
            Ok(Event::Empty(e)) => {
                let name = e.local_name().into_inner().to_lowercase();
                if name == "collection" {
                    if let Some(entry) = &mut current {
                        entry.is_dir = true;
                    }
                }
            }
            Ok(Event::Text(text)) => {
                let value: &str = text.as_ref();
                if let Some(entry) = &mut current {
                    match field.as_str() {
                        "href" => entry.href.push_str(&value.replace("&amp;", "&")),
                        "getcontentlength" => entry.size = value.trim().parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.local_name().into_inner().to_lowercase();
                field.clear();
                if name == "response" {
                    entries.extend(current.take());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    entries
}

async fn list_directory(client: &Client, config: &CloudConfig, url: &str) -> Result<Vec<RemoteEntry>, String> {
    let response = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), url)
        .basic_auth(&config.username, Some(&config.password))
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(PROPFIND_BODY)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    match response.status() {
        StatusCode::MULTI_STATUS => {
            let text = response.text().await.map_err(|e| e.to_string())?;
            Ok(parse_listing(&text))
        }
        StatusCode::UNAUTHORIZED => Err("Login failed: wrong username or password".into()),
        StatusCode::NOT_FOUND => Err("Folder not found on the server".into()),
        status => Err(format!("Server answered with {status}")),
    }
}

async fn list_all_files(client: &Client, config: &CloudConfig) -> Result<Vec<(String, RemoteEntry)>, String> {
    let base = base_url(config);
    let server = origin(&base);
    let base_path = percent_decode(&base[server.len()..]);

    let mut pending = vec![base.clone()];
    let mut files = Vec::new();

    while let Some(directory) = pending.pop() {
        let directory_path = percent_decode(&directory[server.len()..]);
        for entry in list_directory(client, config, &directory).await? {
            let entry_path = percent_decode(&entry.href);
            if entry_path.trim_end_matches('/') == directory_path.trim_end_matches('/') {
                continue;
            }
            if entry.is_dir {
                pending.push(format!("{server}{}", entry.href));
            } else if let Some(relative) = entry_path.strip_prefix(&base_path) {
                files.push((relative.to_string(), entry));
            }
        }
    }

    Ok(files)
}

pub async fn test_connection(config: &CloudConfig) -> Result<usize, String> {
    let client = Client::new();
    let files = list_all_files(&client, config).await?;
    Ok(files.iter().filter(|(path, _)| is_audio_path(path)).count())
}

fn is_audio_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn local_path(cache_dir: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    let safe = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    safe.then(|| cache_dir.join(relative))
}

async fn download(client: &Client, config: &CloudConfig, href: &str, target: &Path) -> Result<(), String> {
    let server = origin(&config.url);
    let mut response = client
        .get(format!("{server}{href}"))
        .basic_auth(&config.username, Some(&config.password))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let partial = target.with_extension("part");
    let mut file = File::create(&partial).map_err(|e| e.to_string())?;
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    fs::rename(&partial, target).map_err(|e| e.to_string())
}

pub async fn sync(app: &AppHandle, config: &CloudConfig, cache_dir: &Path) -> Result<SyncReport, String> {
    let client = Client::new();
    let files: Vec<_> = list_all_files(&client, config)
        .await?
        .into_iter()
        .filter(|(relative, _)| is_audio_path(relative))
        .collect();

    let total = files.len();
    let mut report = SyncReport {
        downloaded: 0,
        skipped: 0,
        failed: 0,
    };

    for (done, (relative, entry)) in files.iter().enumerate() {
        let _ = app.emit(
            "cloud-progress",
            Progress {
                done,
                total,
                current: relative.clone(),
            },
        );

        let Some(target) = local_path(cache_dir, relative) else {
            report.failed += 1;
            continue;
        };

        let up_to_date = fs::metadata(&target)
            .map(|meta| meta.len() == entry.size)
            .unwrap_or(false);
        if up_to_date {
            report.skipped += 1;
            continue;
        }

        match download(&client, config, &entry.href, &target).await {
            Ok(()) => report.downloaded += 1,
            Err(_) => report.failed += 1,
        }
    }

    let _ = app.emit(
        "cloud-progress",
        Progress {
            done: total,
            total,
            current: String::new(),
        },
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/dav/files/me/Music/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/files/me/Music/My%20Song.mp3</d:href>
    <d:propstat><d:prop><d:resourcetype/><d:getcontentlength>1234</d:getcontentlength></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn parses_files_and_folders() {
        let entries = parse_listing(LISTING);

        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir);
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].size, 1234);
        assert_eq!(percent_decode(&entries[1].href), "/dav/files/me/Music/My Song.mp3");
    }

    #[test]
    fn rejects_paths_that_escape_the_cache() {
        let cache = Path::new("/cache");
        assert!(local_path(cache, "../etc/passwd").is_none());
        assert!(local_path(cache, "album/song.mp3").is_some());
    }
}
