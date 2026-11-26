use std::sync::mpsc::Sender;

use crate::player::{Command, PlayerState};

#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod desktop {
    use std::time::Duration;

    use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig};

    use super::*;

    pub struct MediaKeys {
        controls: Option<MediaControls>,
        last_track: Option<i64>,
        last_playing: Option<bool>,
    }

    impl MediaKeys {
        pub fn new(tx: Sender<Command>) -> Self {
            let config = PlatformConfig {
                dbus_name: "mossamp",
                display_name: "MossAMP",
                hwnd: None,
            };

            let controls = MediaControls::new(config).ok().and_then(|mut controls| {
                controls
                    .attach(move |event| {
                        let command = match event {
                            MediaControlEvent::Play => Command::Resume,
                            MediaControlEvent::Pause => Command::Pause,
                            MediaControlEvent::Toggle => Command::Toggle,
                            MediaControlEvent::Next => Command::Next,
                            MediaControlEvent::Previous => Command::Previous,
                            MediaControlEvent::SetPosition(MediaPosition(position)) => {
                                Command::Seek(position.as_secs_f64())
                            }
                            _ => return,
                        };
                        let _ = tx.send(command);
                    })
                    .ok()?;
                Some(controls)
            });

            MediaKeys {
                controls,
                last_track: None,
                last_playing: None,
            }
        }

        pub fn update(&mut self, state: &PlayerState) {
            let Some(controls) = &mut self.controls else { return };

            let track_id = state.track().map(|t| t.id);
            if track_id != self.last_track {
                self.last_track = track_id;
                if let Some(track) = state.track() {
                    let _ = controls.set_metadata(MediaMetadata {
                        title: Some(&track.title),
                        artist: Some(&track.artist),
                        album: Some(&track.album),
                        cover_url: None,
                        duration: Some(Duration::from_secs_f64(track.duration)),
                    });
                }
            }

            if Some(state.playing()) != self.last_playing || track_id.is_none() {
                self.last_playing = Some(state.playing());
                let progress = Some(MediaPosition(Duration::from_secs_f64(state.position())));
                let playback = if track_id.is_none() {
                    MediaPlayback::Stopped
                } else if state.playing() {
                    MediaPlayback::Playing { progress }
                } else {
                    MediaPlayback::Paused { progress }
                };
                let _ = controls.set_playback(playback);
            }
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use desktop::MediaKeys;

#[cfg(any(target_os = "android", target_os = "ios"))]
pub struct MediaKeys;

#[cfg(any(target_os = "android", target_os = "ios"))]
impl MediaKeys {
    pub fn new(_tx: Sender<Command>) -> Self {
        MediaKeys
    }

    pub fn update(&mut self, _state: &PlayerState) {}
}
