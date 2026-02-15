//! Audio playback engine
//!
//! Core audio playback using Rodio with support for multiple formats.
//! Supports local files, HTTP streaming, and cached streams.

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicF32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::streaming::{StreamSource, StreamTrack};

/// Audio engine errors
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Failed to initialize audio output stream")]
    OutputStreamInit,

    #[error("Failed to create audio sink: {0}")]
    SinkCreation(String),

    #[error("Failed to decode audio file: {0}")]
    DecodeError(String),

    #[error("Failed to open file: {0}")]
    FileOpen(String),

    #[error("Playback error: {0}")]
    PlaybackError(String),

    #[error("No track loaded")]
    NoTrackLoaded,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Buffering error: {0}")]
    BufferingError(String),
}

/// Playback state for the current track
#[derive(Debug, Clone)]
pub struct PlaybackState {
    /// Current track info
    pub track: Option<StreamTrack>,
    /// Source type
    pub source: StreamSource,
    /// Whether the track is from cache
    pub is_cached: bool,
    /// Playback position in seconds
    pub position_secs: u64,
    /// Total duration in seconds
    pub duration_secs: u64,
    /// When playback started
    pub started_at: Option<Instant>,
    /// Whether the track completed
    pub completed: bool,
    /// Whether the track was skipped
    pub skipped: bool,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            track: None,
            source: StreamSource::Local,
            is_cached: false,
            position_secs: 0,
            duration_secs: 0,
            started_at: None,
            completed: false,
            skipped: false,
        }
    }
}

/// Audio playback engine
pub struct AudioEngine {
    /// Output stream (must be kept alive)
    _output_stream: OutputStream,

    /// Stream handle for creating sinks
    stream_handle: OutputStreamHandle,

    /// Current audio sink
    sink: Option<Sink>,

    /// Current volume
    volume: Arc<AtomicF32>,

    /// Muted state
    muted: Arc<AtomicBool>,

    /// Playback position in milliseconds (approximate)
    position_ms: Arc<AtomicU64>,

    /// When playback started
    playback_start: Option<Instant>,

    /// Current playback state
    playback_state: PlaybackState,

    /// HTTP client for streaming
    http_client: Option<reqwest::Client>,
}

impl AudioEngine {
    /// Create a new audio engine
    pub fn new() -> Result<Self, AudioError> {
        let (output_stream, stream_handle) =
            OutputStream::try_default().map_err(|_| AudioError::OutputStreamInit)?;

        // Create HTTP client for streaming
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .ok();

        Ok(Self {
            _output_stream: output_stream,
            stream_handle,
            sink: None,
            volume: Arc::new(AtomicF32::new(0.6)),
            muted: Arc::new(AtomicBool::new(false)),
            position_ms: Arc::new(AtomicU64::new(0)),
            playback_start: None,
            playback_state: PlaybackState::default(),
            http_client,
        })
    }

    /// Play a local file
    pub fn play_file(&mut self, path: &Path) -> Result<(), AudioError> {
        self.play_file_internal(path, None)
    }

    /// Play a local file with track metadata
    pub fn play_file_with_track(&mut self, path: &Path, track: StreamTrack) -> Result<(), AudioError> {
        self.play_file_internal(path, Some(track))
    }

    /// Internal file playback implementation
    fn play_file_internal(&mut self, path: &Path, track: Option<StreamTrack>) -> Result<(), AudioError> {
        // Create new sink
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| AudioError::SinkCreation(e.to_string()))?;

        // Open and decode file
        let file =
            File::open(path).map_err(|e| AudioError::FileOpen(format!("{}: {}", path.display(), e)))?;

        let reader = BufReader::new(file);

        let source =
            Decoder::new(reader).map_err(|e| AudioError::DecodeError(e.to_string()))?;

        // Apply volume
        let volume = self.volume.load(Ordering::SeqCst);
        sink.set_volume(volume);

        // Append source
        sink.append(source);

        // Store sink and update state
        self.sink = Some(sink);
        self.playback_start = Some(Instant::now());
        self.position_ms.store(0, Ordering::SeqCst);

        // Update playback state
        self.playback_state = PlaybackState {
            track,
            source: StreamSource::Local,
            is_cached: path.to_string_lossy().contains(".cache"),
            ..Default::default()
        };
        self.playback_state.started_at = Some(Instant::now());

        info!("Playing local file: {}", path.display());
        Ok(())
    }

    /// Play from a URL (HTTP streaming)
    pub async fn play_url(&mut self, url: &str, track: Option<StreamTrack>) -> Result<(), AudioError> {
        let client = self.http_client.as_ref().ok_or_else(|| {
            AudioError::NetworkError("HTTP client not initialized".to_string())
        })?;

        debug!("Fetching audio from URL: {}", url);

        // Download the audio data
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| AudioError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(AudioError::NetworkError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        // Get the bytes
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AudioError::NetworkError(e.to_string()))?;

        debug!("Downloaded {} bytes", bytes.len());

        // Create cursor for in-memory playback
        let cursor = Cursor::new(bytes.to_vec());

        // Create new sink
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| AudioError::SinkCreation(e.to_string()))?;

        // Decode from cursor
        let source = Decoder::new(cursor)
            .map_err(|e| AudioError::DecodeError(e.to_string()))?;

        // Apply volume
        let volume = self.volume.load(Ordering::SeqCst);
        sink.set_volume(volume);

        // Append source
        sink.append(source);

        // Store sink and update state
        self.sink = Some(sink);
        self.playback_start = Some(Instant::now());
        self.position_ms.store(0, Ordering::SeqCst);

        // Determine source from URL
        let source = if url.contains("youtube") || url.contains("googlevideo") {
            StreamSource::YouTube
        } else if url.contains("spotify") || url.contains("scdn.co") {
            StreamSource::Spotify
        } else if url.starts_with("file://") {
            StreamSource::Cached
        } else {
            // Default to YouTube for unknown HTTP URLs
            StreamSource::YouTube
        };

        self.playback_state = PlaybackState {
            track,
            source,
            is_cached: false,
            ..Default::default()
        };
        self.playback_state.started_at = Some(Instant::now());

        info!("Playing stream from URL");
        Ok(())
    }

    /// Play a stream track (handles both local and streaming)
    pub async fn play_stream_track(
        &mut self,
        track: &StreamTrack,
        url: &str,
        is_cached: bool,
    ) -> Result<(), AudioError> {
        // Update playback state
        self.playback_state = PlaybackState {
            track: Some(track.clone()),
            source: track.source,
            is_cached,
            duration_secs: track.duration,
            ..Default::default()
        };

        if url.starts_with("file://") {
            // Local cached file
            let path = url.trim_start_matches("file://");
            self.play_file_internal(Path::new(path), Some(track.clone()))?;
            self.playback_state.is_cached = true;
        } else if url.starts_with("http://") || url.starts_with("https://") {
            // HTTP stream
            self.play_url(url, Some(track.clone())).await?;
        } else {
            // Assume local path
            self.play_file_internal(Path::new(url), Some(track.clone()))?;
        }

        Ok(())
    }

    /// Get current playback state
    pub fn get_playback_state(&self) -> &PlaybackState {
        &self.playback_state
    }

    /// Get mutable playback state
    pub fn get_playback_state_mut(&mut self) -> &mut PlaybackState {
        &mut self.playback_state
    }

    /// Mark current track as skipped
    pub fn mark_skipped(&mut self) {
        self.playback_state.skipped = true;
    }

    /// Mark current track as completed
    pub fn mark_completed(&mut self) {
        self.playback_state.completed = true;
    }

    /// Get approximate playback duration in seconds
    pub fn elapsed_secs(&self) -> u64 {
        self.playback_start
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Pause playback
    pub fn pause(&self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }

    /// Resume playback
    pub fn play(&self) {
        if let Some(sink) = &self.sink {
            sink.play();
        }
    }

    /// Stop playback
    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }

    /// Toggle play/pause
    pub fn toggle(&self) {
        if let Some(sink) = &self.sink {
            if sink.is_paused() {
                sink.play();
            } else {
                sink.pause();
            }
        }
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.sink
            .as_ref()
            .map(|s| !s.is_paused() && !s.empty())
            .unwrap_or(false)
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.sink.as_ref().map(|s| s.is_paused()).unwrap_or(false)
    }

    /// Set volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume, Ordering::SeqCst);
        if let Some(sink) = &self.sink {
            // If muted, don't actually change volume but store it
            if !self.muted.load(Ordering::SeqCst) {
                sink.set_volume(volume);
            }
        }
    }

    /// Get current volume
    pub fn volume(&self) -> f32 {
        self.volume.load(Ordering::SeqCst)
    }

    /// Set muted state
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::SeqCst);
        if let Some(sink) = &self.sink {
            if muted {
                sink.set_volume(0.0);
            } else {
                sink.set_volume(self.volume.load(Ordering::SeqCst));
            }
        }
    }

    /// Check if muted
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::SeqCst)
    }

    /// Check if sink is empty (no more audio to play)
    pub fn is_empty(&self) -> bool {
        self.sink.as_ref().map(|s| s.empty()).unwrap_or(true)
    }

    /// Get current playback position (if available)
    pub fn position(&self) -> Option<std::time::Duration> {
        // Rodio doesn't provide position tracking out of the box
        // This would need to be implemented with a custom source wrapper
        None
    }

    /// Seek to position (if supported)
    pub fn seek(&self, _position: std::time::Duration) -> Result<(), AudioError> {
        // Rodio's Source trait has try_seek but it's not always available
        // This would need custom implementation
        Err(AudioError::PlaybackError(
            "Seeking not yet implemented".to_string(),
        ))
    }

    /// Get supported file extensions
    pub fn supported_extensions() -> &'static [&'static str] {
        &["mp3", "wav", "ogg", "flac", "aiff", "aac"]
    }

    /// Check if a file extension is supported
    pub fn is_supported(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| Self::supported_extensions().contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new().expect("Failed to initialize default audio engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        let extensions = AudioEngine::supported_extensions();
        assert!(extensions.contains(&"mp3"));
        assert!(extensions.contains(&"flac"));
        assert!(extensions.contains(&"wav"));
        assert!(extensions.contains(&"ogg"));
    }

    #[test]
    fn test_is_supported() {
        assert!(AudioEngine::is_supported(Path::new("test.mp3")));
        assert!(AudioEngine::is_supported(Path::new("test.FLAC")));
        assert!(!AudioEngine::is_supported(Path::new("test.txt")));
        assert!(!AudioEngine::is_supported(Path::new("test")));
    }
}
