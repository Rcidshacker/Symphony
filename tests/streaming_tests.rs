//! Streaming module tests
//!
//! Tests for the streaming infrastructure including providers, cache, and predictions.

use std::path::PathBuf;
use tempfile::tempdir;

// Note: These tests require the streaming module to be properly set up.
// Some tests may require network access or external dependencies (yt-dlp).

/// Test cache tier default sizes
#[test]
fn test_cache_tier_defaults() {
    use symphony::streaming::CacheTier;

    assert_eq!(CacheTier::Hot.default_max_size(), 100 * 1024 * 1024);
    assert_eq!(CacheTier::Warm.default_max_size(), 2 * 1024 * 1024 * 1024);
    assert_eq!(CacheTier::Cold.default_max_size(), 20 * 1024 * 1024 * 1024);
}

/// Test stream source display
#[test]
fn test_stream_source_display() {
    use symphony::streaming::StreamSource;

    assert_eq!(StreamSource::YouTube.label(), "YOUTUBE");
    assert_eq!(StreamSource::Spotify.label(), "SPOTIFY");
    assert_eq!(StreamSource::Local.label(), "LOCAL");
    assert_eq!(StreamSource::Cached.label(), "CACHED");

    assert!(!StreamSource::YouTube.icon().is_empty());
}

/// Test stream quality bitrates
#[test]
fn test_stream_quality_bitrate() {
    use symphony::streaming::StreamQuality;

    assert_eq!(StreamQuality::Low.bitrate_kbps(), 64);
    assert_eq!(StreamQuality::Medium.bitrate_kbps(), 128);
    assert_eq!(StreamQuality::High.bitrate_kbps(), 256);
    assert_eq!(StreamQuality::Best.bitrate_kbps(), 320);
}

/// Test stream track creation
#[test]
fn test_stream_track() {
    use symphony::streaming::{StreamQuality, StreamSource, StreamTrack};

    let track = StreamTrack::new("test123", "Test Song", "Test Artist", StreamSource::YouTube)
        .with_duration(180)
        .with_album("Test Album")
        .with_thumbnail("http://example.com/thumb.jpg");

    assert_eq!(track.id, "test123");
    assert_eq!(track.title, "Test Song");
    assert_eq!(track.artist, "Test Artist");
    assert_eq!(track.duration, 180);
    assert_eq!(track.album, Some("Test Album".to_string()));
    assert_eq!(
        track.thumbnail_url,
        Some("http://example.com/thumb.jpg".to_string())
    );
    assert_eq!(track.formatted_duration(), "03:00");
}

/// Test search result
#[test]
fn test_search_result() {
    use symphony::streaming::{SearchResult, StreamSource, StreamTrack};

    let track1 = StreamTrack::new("id1", "Song 1", "Artist 1", StreamSource::YouTube);
    let track2 = StreamTrack::new("id2", "Song 2", "Artist 2", StreamSource::Spotify);

    let result = SearchResult::new(vec![track1, track2], "test query", StreamSource::YouTube);

    assert_eq!(result.len(), 2);
    assert_eq!(result.total, 2);
    assert!(!result.is_empty());
    assert_eq!(result.query, "test query");
}

/// Test download progress
#[test]
fn test_download_progress() {
    use symphony::streaming::DownloadProgress;

    let mut progress = DownloadProgress::new("track123");
    assert_eq!(progress.track_id, "track123");
    assert_eq!(progress.percentage, 0);

    progress.update(1024 * 1024, Some(5 * 1024 * 1024), Some(512 * 1024));

    assert_eq!(progress.bytes_downloaded, 1024 * 1024);
    assert_eq!(progress.percentage, 20);
    assert!(progress.formatted_speed().contains("KB"));
}

/// Test stream error recoverability
#[test]
fn test_stream_error() {
    use symphony::streaming::StreamError;

    let network_error = StreamError::NetworkError("Connection failed".to_string());
    assert!(network_error.is_recoverable());
    assert!(network_error.retry_delay_secs().is_some());

    let not_found = StreamError::TrackNotFound("xyz".to_string());
    assert!(!not_found.is_recoverable());
    assert!(not_found.retry_delay_secs().is_none());
}

/// Test cache configuration defaults
#[test]
fn test_cache_config_defaults() {
    use symphony::streaming::cache::CacheConfig;

    let config = CacheConfig::default();

    assert!(config.cache_dir.to_string_lossy().contains("symphony"));
    assert_eq!(config.max_hot_size, 100 * 1024 * 1024);
    assert_eq!(config.max_warm_size, 2 * 1024 * 1024 * 1024);
    assert!(config.prefetch_enabled);
    assert_eq!(config.prefetch_count, 3);
}

/// Test prediction reason descriptions
#[test]
fn test_prediction_reason() {
    use symphony::streaming::PredictionReason;

    let transition = PredictionReason::Transition { probability: 0.75 };
    assert!(transition.description().contains("75%"));

    let time_pattern = PredictionReason::TimePattern {
        hour: 14,
        probability: 0.5,
    };
    assert!(time_pattern.description().contains("14:00"));

    let queue = PredictionReason::QueuePosition { position: 2 };
    assert!(queue.description().contains("position 2"));
}

/// Test combined search result
#[test]
fn test_combined_search_result() {
    use symphony::streaming::{CombinedSearchResult, StreamSource, StreamTrack};

    let track_yt = StreamTrack::new("yt1", "Song", "Artist", StreamSource::YouTube);
    let track_sp = StreamTrack::new("sp1", "Song", "Artist", StreamSource::Spotify);

    let mut result = CombinedSearchResult {
        youtube: None,
        spotify: None,
        tracks: vec![track_yt, track_sp],
        total: 2,
        query: "test".to_string(),
    };

    assert_eq!(result.len(), 2);
    assert!(!result.is_empty());

    let youtube_only = result.filter_by_source(StreamSource::YouTube);
    assert_eq!(youtube_only.len(), 1);

    let spotify_only = result.filter_by_source(StreamSource::Spotify);
    assert_eq!(spotify_only.len(), 1);
}

/// Test format bytes function
#[test]
fn test_format_bytes() {
    use symphony::streaming::cache::format_bytes;

    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.00 KB");
    assert!(format_bytes(1024 * 1024).contains("MB"));
    assert!(format_bytes(1024 * 1024 * 1024).contains("GB"));
}

/// Test YouTube config defaults
#[test]
fn test_youtube_config_defaults() {
    use symphony::streaming::youtube::YouTubeConfig;

    let config = YouTubeConfig::default();

    assert_eq!(config.ytdlp_path, "yt-dlp");
    assert_eq!(
        config.default_quality,
        symphony::streaming::StreamQuality::Medium
    );
    assert!(config.include_thumbnails);
    assert_eq!(config.max_results, 20);
}

/// Test Spotify config defaults
#[test]
fn test_spotify_config_defaults() {
    use symphony::streaming::spotify::SpotifyConfig;

    let config = SpotifyConfig::default();

    assert!(config.client_id.is_empty());
    assert!(config.client_secret.is_empty());
    assert!(!config.enabled);
    assert!(config.redirect_uri.contains("localhost"));
}

/// Test stream manager config defaults
#[test]
fn test_stream_manager_config_defaults() {
    use symphony::streaming::StreamManagerConfig;

    let config = StreamManagerConfig::default();

    assert!(config.enabled);
    assert_eq!(
        config.default_quality,
        symphony::streaming::StreamQuality::Medium
    );
    assert!(config.prefer_cached);
    assert_eq!(config.max_concurrent_downloads, 2);
}
