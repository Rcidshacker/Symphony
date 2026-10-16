//! Playing from YouTube and Spotify: a stream track is downloaded to the cache (so repeat
//! plays and prefetched tracks are instant and work offline), then played like a local file.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::OnceCell;

use super::{App, BackgroundEvent, Track, TrackSource};
use crate::streaming::{StreamManager, StreamManagerConfig, StreamSource, StreamTrack};

/// Lazily created stream manager, shared with the background tasks
pub(super) type SharedManager = Arc<OnceCell<Arc<StreamManager>>>;

/// How many search results a `symphony stream <query>` looks at
const SEARCH_LIMIT: usize = 5;

/// Library entry for a stream track. The id and path are `youtube:<id>` / `spotify:<id>`
/// so the same track is never added twice.
pub fn track_from_stream(stream: &StreamTrack) -> Track {
    let (source, prefix) = match stream.source {
        StreamSource::Spotify => (TrackSource::Spotify, "spotify"),
        _ => (TrackSource::YouTube, "youtube"),
    };
    let key = format!("{prefix}:{}", stream.id);
    Track {
        id: key.clone(),
        title: stream.title.clone(),
        artist: stream.artist.clone(),
        album: stream.album.clone().unwrap_or_default(),
        duration: stream.duration as f64,
        path: PathBuf::from(key),
        track_number: None,
        genre: None,
        year: None,
        source,
    }
}

/// Resolve a `youtube:` / `spotify:` spec (or link) and save its tracks in the library.
/// Returns the tracks that were added.
pub async fn import_stream_spec(
    manager: &StreamManager,
    db: &crate::db::Database,
    spec: &str,
) -> Result<Vec<Track>, String> {
    let found = manager
        .resolve_spec(spec)
        .await
        .map_err(|e| e.to_string())?;
    let mut added = Vec::new();
    for stream in &found {
        let track = track_from_stream(stream);
        db.upsert_track(&track).map_err(|e| e.to_string())?;
        added.push(track);
    }
    Ok(added)
}

/// The streaming-side view of a library stream track
fn stream_from_track(track: &Track) -> StreamTrack {
    let (source, id) = match track.source {
        TrackSource::Spotify => (
            StreamSource::Spotify,
            track.id.trim_start_matches("spotify:"),
        ),
        _ => (
            StreamSource::YouTube,
            track.id.trim_start_matches("youtube:"),
        ),
    };
    StreamTrack::new(id, track.title.clone(), track.artist.clone(), source)
        .with_duration(track.duration as u64)
}

/// Tracks that must be fetched before they can play
pub(super) fn is_stream(track: &Track) -> bool {
    matches!(track.source, TrackSource::YouTube | TrackSource::Spotify)
}

async fn manager(
    cell: &SharedManager,
    config: StreamManagerConfig,
) -> Result<Arc<StreamManager>, String> {
    cell.get_or_try_init(|| async { StreamManager::new(config).await.map(Arc::new) })
        .await
        .cloned()
        .map_err(|e| e.to_string())
}

impl App {
    fn manager_config(&self) -> StreamManagerConfig {
        StreamManagerConfig::from_config(&self.config.streaming)
    }

    /// Fetch `track` in the background and play it when it is ready
    pub(super) fn start_stream(&mut self, track: Track) {
        let Some(runtime) = &self.runtime else {
            self.ui_state.error_message = Some("Streaming needs the async runtime".to_string());
            return;
        };

        // nothing plays while the download runs
        self.audio_engine.stop();
        self.playback_state.is_playing = false;
        self.stream_pending = Some(track.id.clone());
        self.ui_state.error_message = None;
        self.ui_state.status_message =
            Some(format!("Fetching: {} - {}", track.artist, track.title));

        let (cell, config, tx) = (
            self.stream.clone(),
            self.manager_config(),
            self.ai_tx.clone(),
        );
        let stream = stream_from_track(&track);
        let track_id = track.id;
        runtime.spawn(async move {
            let result = match manager(&cell, config).await {
                Ok(manager) => manager
                    .download_track(&stream)
                    .await
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(match result {
                Ok(path) => BackgroundEvent::StreamReady { track_id, path },
                Err(error) => BackgroundEvent::StreamFailed { track_id, error },
            });
        });
    }

    /// The download finished: play it, unless the user already moved on
    pub(super) fn apply_stream_ready(&mut self, track_id: &str, path: PathBuf) {
        if self.stream_pending.as_deref() != Some(track_id) {
            return;
        }
        self.stream_pending = None;
        let Some(track) = self.library.queue.get(self.library.queue_index).cloned() else {
            return;
        };
        if track.id != track_id {
            return;
        }
        if self.begin_playback(&track, &path) {
            self.prefetch_next_stream();
        }
    }

    pub(super) fn apply_stream_failed(&mut self, track_id: &str, error: String) {
        if self.stream_pending.as_deref() != Some(track_id) {
            return;
        }
        self.stream_pending = None;
        self.ui_state.status_message = None;
        self.ui_state.error_message = Some(format!(
            "Stream failed: {error} (n skips to the next track)"
        ));
    }

    /// Download the next queued stream track so it starts instantly
    fn prefetch_next_stream(&self) {
        let Some(runtime) = &self.runtime else { return };
        if !self.config.streaming.cache.prefetch_enabled {
            return;
        }
        let Some(next) = self.library.queue.get(self.library.queue_index + 1) else {
            return;
        };
        if !is_stream(next) {
            return;
        }
        let (cell, config, stream) = (
            self.stream.clone(),
            self.manager_config(),
            stream_from_track(next),
        );
        runtime.spawn(async move {
            if let Ok(manager) = manager(&cell, config).await {
                // a failed prefetch only means the track downloads when it is played
                if let Err(e) = manager.download_track(&stream).await {
                    tracing::debug!("prefetch of {} failed: {e}", stream.id);
                }
            }
        });
    }

    /// `symphony stream <query>`: search the streaming sources and play the best match
    pub fn play_stream_query(&mut self, query: &str) {
        let Some(runtime) = &self.runtime else { return };
        self.ui_state.status_message = Some(format!("Searching streams for \"{query}\"..."));
        let (cell, config, tx) = (
            self.stream.clone(),
            self.manager_config(),
            self.ai_tx.clone(),
        );
        let query = query.to_string();
        runtime.spawn(async move {
            let result = match manager(&cell, config).await {
                Ok(manager) => manager
                    .search_all(&query, SEARCH_LIMIT)
                    .await
                    .map(|found| found.tracks)
                    .map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            let _ = tx.send(BackgroundEvent::StreamSearch { query, result });
        });
    }

    pub(super) fn apply_stream_search(
        &mut self,
        query: &str,
        result: Result<Vec<StreamTrack>, String>,
    ) {
        let found = match result {
            Ok(found) => found,
            Err(e) => {
                self.ui_state.status_message = None;
                self.ui_state.error_message = Some(format!("Stream search failed: {e}"));
                return;
            }
        };
        let Some(best) = found.first() else {
            self.ui_state.status_message = None;
            self.ui_state.error_message = Some(format!("No streams found for \"{query}\""));
            return;
        };
        let track = track_from_stream(best);
        self.add_to_library(&track);
        self.start_queue(vec![track], false);
    }

    /// Remember a stream track in the library (and database) if it is new
    pub fn add_to_library(&mut self, track: &Track) {
        if let Some(db) = &self.database {
            if let Err(e) = db.upsert_track(track) {
                self.ui_state.error_message = Some(format!("Could not save track: {e}"));
            }
        }
        if !self.library.tracks.iter().any(|t| t.id == track.id) {
            self.library.tracks.push(track.clone());
        }
    }
}
