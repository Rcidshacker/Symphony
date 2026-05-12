//! Stream provider abstraction
//!
//! Defines the common interface for all streaming providers (YouTube, Spotify, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/// Stream source type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamSource {
    /// YouTube video
    YouTube,
    /// Spotify track
    Spotify,
    /// Local file
    Local,
    /// Cached stream
    Cached,
}

impl StreamSource {
    /// Get the display icon for this source
    pub fn icon(&self) -> &'static str {
        match self {
            StreamSource::YouTube => "🔴", // YouTube red
            StreamSource::Spotify => "🟢", // Spotify green
            StreamSource::Local => "💿",   // Local disc
            StreamSource::Cached => "⚡",  // Cached lightning
        }
    }

    /// Get the display label for this source
    pub fn label(&self) -> &'static str {
        match self {
            StreamSource::YouTube => "YOUTUBE",
            StreamSource::Spotify => "SPOTIFY",
            StreamSource::Local => "LOCAL",
            StreamSource::Cached => "CACHED",
        }
    }
}

impl std::fmt::Display for StreamSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Stream quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamQuality {
    /// Low quality (~64kbps) - minimal bandwidth
    Low,
    /// Medium quality (~128kbps) - balanced
    Medium,
    /// High quality (~256kbps) - good experience
    High,
    /// Best quality available
    Best,
}

impl StreamQuality {
    /// Get approximate bitrate in kbps
    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            StreamQuality::Low => 64,
            StreamQuality::Medium => 128,
            StreamQuality::High => 256,
            StreamQuality::Best => 320,
        }
    }

    /// Get the YouTube format selector for this quality
    pub fn yt_dlp_format(&self) -> &'static str {
        match self {
            StreamQuality::Low => "bestaudio[abr<=64]/bestaudio/worst",
            StreamQuality::Medium => "bestaudio[abr<=128]/bestaudio",
            StreamQuality::High => "bestaudio[abr<=256]/bestaudio",
            StreamQuality::Best => "bestaudio/best",
        }
    }
}

impl Default for StreamQuality {
    fn default() -> Self {
        StreamQuality::Medium
    }
}

impl std::fmt::Display for StreamQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamQuality::Low => write!(f, "Low (64kbps)"),
            StreamQuality::Medium => write!(f, "Medium (128kbps)"),
            StreamQuality::High => write!(f, "High (256kbps)"),
            StreamQuality::Best => write!(f, "Best (320kbps)"),
        }
    }
}

/// Streaming track metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTrack {
    /// Unique identifier (provider-specific)
    pub id: String,
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Album name (if available)
    pub album: Option<String>,
    /// Duration in seconds
    pub duration: u64,
    /// Source type
    pub source: StreamSource,
    /// Thumbnail URL (if available)
    pub thumbnail_url: Option<String>,
    /// Direct stream URL (if available)
    pub stream_url: Option<String>,
    /// View count (for YouTube)
    pub view_count: Option<u64>,
    /// Upload date
    pub upload_date: Option<String>,
}

impl StreamTrack {
    /// Create a new stream track
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        artist: impl Into<String>,
        source: StreamSource,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album: None,
            duration: 0,
            source,
            thumbnail_url: None,
            stream_url: None,
            view_count: None,
            upload_date: None,
        }
    }

    /// Set the album
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Set the duration
    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = duration;
        self
    }

    /// Set the thumbnail URL
    pub fn with_thumbnail(mut self, url: impl Into<String>) -> Self {
        self.thumbnail_url = Some(url.into());
        self
    }

    /// Set the stream URL
    pub fn with_stream_url(mut self, url: impl Into<String>) -> Self {
        self.stream_url = Some(url.into());
        self
    }

    /// Format duration as MM:SS
    pub fn formatted_duration(&self) -> String {
        let minutes = self.duration / 60;
        let seconds = self.duration % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }

    /// Format for display in UI
    pub fn display_line(&self, width: usize) -> String {
        let source_tag = format!("[{}]", self.source);
        let duration_str = self.formatted_duration();

        // Calculate available space for title/artist
        let used = source_tag.len() + duration_str.len() + 4; // 4 for spacing
        let available = width.saturating_sub(used);

        let title_artist = if self.title.len() + self.artist.len() + 3 <= available {
            format!("{} - {}", self.title, self.artist)
        } else {
            // Truncate if too long
            let max_title = available.saturating_sub(self.artist.len() + 6);
            if max_title > 10 {
                format!(
                    "{}... - {}",
                    &self.title[..max_title.min(self.title.len())],
                    self.artist
                )
            } else {
                format!(
                    "{} - {}",
                    &self.title[..available.min(self.title.len())],
                    self.artist
                )
            }
        };

        format!("{} {} {}", source_tag, title_artist, duration_str)
    }
}

/// Search result from streaming provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// List of found tracks
    pub tracks: Vec<StreamTrack>,
    /// Total number of results available
    pub total: usize,
    /// Query that produced these results
    pub query: String,
    /// Source that produced these results
    pub source: StreamSource,
}

impl SearchResult {
    /// Create an empty search result
    pub fn empty(query: impl Into<String>, source: StreamSource) -> Self {
        Self {
            tracks: Vec::new(),
            total: 0,
            query: query.into(),
            source,
        }
    }

    /// Create a search result with tracks
    pub fn new(tracks: Vec<StreamTrack>, query: impl Into<String>, source: StreamSource) -> Self {
        let total = tracks.len();
        Self {
            tracks,
            total,
            query: query.into(),
            source,
        }
    }

    /// Check if results are empty
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Get number of results
    pub fn len(&self) -> usize {
        self.tracks.len()
    }
}

/// Download progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    /// Track being downloaded
    pub track_id: String,
    /// Bytes downloaded so far
    pub bytes_downloaded: u64,
    /// Total bytes (if known)
    pub total_bytes: Option<u64>,
    /// Download speed in bytes/sec
    pub speed_bps: Option<u64>,
    /// Current status
    pub status: DownloadStatus,
    /// Percentage complete (0-100)
    pub percentage: u8,
}

impl DownloadProgress {
    /// Create a new progress tracker
    pub fn new(track_id: impl Into<String>) -> Self {
        Self {
            track_id: track_id.into(),
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bps: None,
            status: DownloadStatus::Pending,
            percentage: 0,
        }
    }

    /// Update progress
    pub fn update(&mut self, downloaded: u64, total: Option<u64>, speed: Option<u64>) {
        self.bytes_downloaded = downloaded;
        self.total_bytes = total;
        self.speed_bps = speed;

        if let Some(total) = total {
            if total > 0 {
                self.percentage = ((downloaded as f64 / total as f64) * 100.0) as u8;
            }
        }
    }

    /// Format download speed
    pub fn formatted_speed(&self) -> String {
        match self.speed_bps {
            Some(speed) if speed < 1024 => format!("{} B/s", speed),
            Some(speed) if speed < 1024 * 1024 => format!("{:.1} KB/s", speed as f64 / 1024.0),
            Some(speed) => format!("{:.1} MB/s", speed as f64 / (1024.0 * 1024.0)),
            None => "Unknown".to_string(),
        }
    }

    /// Format bytes
    pub fn formatted_bytes(&self) -> String {
        let bytes = self.bytes_downloaded;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Download status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    /// Download is pending
    Pending,
    /// Download is in progress
    Downloading,
    /// Download completed successfully
    Completed,
    /// Download failed
    Failed,
    /// Download was cancelled
    Cancelled,
}

/// Stream provider trait
///
/// All streaming providers (YouTube, Spotify, etc.) implement this trait
/// to provide a unified interface for searching and playing content.
#[async_trait]
pub trait StreamProvider: Send + Sync {
    /// Search for tracks
    async fn search(&self, query: &str, limit: usize) -> Result<SearchResult, StreamError>;

    /// Get a direct stream URL for a track
    async fn get_stream_url(
        &self,
        track_id: &str,
        quality: StreamQuality,
    ) -> Result<String, StreamError>;

    /// Get detailed track information
    async fn get_track_info(&self, track_id: &str) -> Result<StreamTrack, StreamError>;

    /// Download a track to local storage
    async fn download_track(
        &self,
        track_id: &str,
        quality: StreamQuality,
        output_path: &PathBuf,
    ) -> Result<(), StreamError>;

    /// Check if the provider is available
    async fn is_available(&self) -> bool;

    /// Get the provider name
    fn name(&self) -> &str;

    /// Get the source type
    fn source(&self) -> StreamSource;
}

/// Streaming errors
#[derive(Debug, Error)]
pub enum StreamError {
    /// Provider is not available
    #[error("Provider not available: {0}")]
    Unavailable(String),

    /// Search failed
    #[error("Search failed: {0}")]
    SearchFailed(String),

    /// Track was not found
    #[error("Track not found: {0}")]
    TrackNotFound(String),

    /// Download failed
    #[error("Download failed: {0}")]
    DownloadFailed(String),

    /// Authentication required
    #[error("Authentication required for {0}")]
    AuthRequired(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Process execution error
    #[error("Process error: {0}")]
    ProcessError(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Rate limited
    #[error("Rate limited, please try again later")]
    RateLimited,

    /// Quota exceeded
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    /// Content not available in region
    #[error("Content not available in your region")]
    RegionRestricted,

    /// Content is age-restricted
    #[error("Content is age-restricted")]
    AgeRestricted,
}

impl StreamError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            StreamError::NetworkError(_) | StreamError::RateLimited | StreamError::ProcessError(_)
        )
    }

    /// Get suggested retry delay in seconds
    pub fn retry_delay_secs(&self) -> Option<u64> {
        match self {
            StreamError::RateLimited => Some(60),
            StreamError::NetworkError(_) => Some(5),
            StreamError::ProcessError(_) => Some(2),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_source_display() {
        assert_eq!(StreamSource::YouTube.label(), "YOUTUBE");
        assert_eq!(StreamSource::Spotify.label(), "SPOTIFY");
        assert_eq!(StreamSource::Local.label(), "LOCAL");
        assert_eq!(StreamSource::Cached.label(), "CACHED");
    }

    #[test]
    fn test_stream_quality_bitrate() {
        assert_eq!(StreamQuality::Low.bitrate_kbps(), 64);
        assert_eq!(StreamQuality::Medium.bitrate_kbps(), 128);
        assert_eq!(StreamQuality::High.bitrate_kbps(), 256);
        assert_eq!(StreamQuality::Best.bitrate_kbps(), 320);
    }

    #[test]
    fn test_stream_track() {
        let track = StreamTrack::new("test123", "Test Song", "Test Artist", StreamSource::YouTube)
            .with_duration(180)
            .with_album("Test Album");

        assert_eq!(track.id, "test123");
        assert_eq!(track.title, "Test Song");
        assert_eq!(track.artist, "Test Artist");
        assert_eq!(track.duration, 180);
        assert_eq!(track.formatted_duration(), "03:00");
    }

    #[test]
    fn test_download_progress() {
        let mut progress = DownloadProgress::new("track123");
        progress.update(1024 * 1024, Some(5 * 1024 * 1024), Some(512 * 1024));

        assert_eq!(progress.percentage, 20);
        assert_eq!(progress.formatted_speed(), "512.0 KB/s");
    }

    #[test]
    fn test_search_result() {
        let track = StreamTrack::new("id", "Title", "Artist", StreamSource::YouTube);
        let result = SearchResult::new(vec![track], "query", StreamSource::YouTube);

        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
    }
}
