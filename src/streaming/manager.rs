//! Stream Manager - Unified interface for all streaming operations
//!
//! Provides a facade pattern for:
//! - YouTube streaming
//! - Spotify search/preview
//! - Smart caching
//! - Predictive prefetching
//!
//! # Usage
//! ```rust,no_run
//! let manager = StreamManager::new(config).await?;
//!
//! // Search all sources
//! let results = manager.search_all("pink floyd").await?;
//!
//! // Play a track (with automatic caching)
//! let path = manager.play_track(&track).await?;
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::cache::{CacheConfig, CacheStats, SmartCache};
use super::predictor::{Prediction, PredictiveCache, PredictorConfig, PredictorStats};
use super::provider::{
    DownloadProgress, SearchResult, StreamError, StreamProvider, StreamQuality, StreamSource,
    StreamTrack,
};
use super::spotify::{SpotifyConfig, SpotifyProvider};
use super::youtube::{YouTubeConfig, YouTubeProvider};

/// Stream manager configuration
#[derive(Debug, Clone)]
pub struct StreamManagerConfig {
    /// Enable streaming features
    pub enabled: bool,
    /// YouTube configuration
    pub youtube: YouTubeConfig,
    /// Spotify configuration
    pub spotify: SpotifyConfig,
    /// Cache configuration
    pub cache: CacheConfig,
    /// Predictor configuration
    pub predictor: PredictorConfig,
    /// Default quality for streaming
    pub default_quality: StreamQuality,
    /// Maximum concurrent downloads
    pub max_concurrent_downloads: usize,
    /// Prefer cached content
    pub prefer_cached: bool,
}

impl Default for StreamManagerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            youtube: YouTubeConfig::default(),
            spotify: SpotifyConfig::default(),
            cache: CacheConfig::default(),
            predictor: PredictorConfig::default(),
            default_quality: StreamQuality::Medium,
            max_concurrent_downloads: 2,
            prefer_cached: true,
        }
    }
}

/// Combined search results from all sources
#[derive(Debug, Clone)]
pub struct CombinedSearchResult {
    /// YouTube results
    pub youtube: Option<SearchResult>,
    /// Spotify results
    pub spotify: Option<SearchResult>,
    /// Combined and deduplicated tracks
    pub tracks: Vec<StreamTrack>,
    /// Total count across all sources
    pub total: usize,
    /// Search query
    pub query: String,
}

impl CombinedSearchResult {
    /// Check if results are empty
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Get number of results
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Filter tracks by source
    pub fn filter_by_source(&self, source: StreamSource) -> Vec<&StreamTrack> {
        self.tracks.iter().filter(|t| t.source == source).collect()
    }

    /// Sort by relevance (YouTube views + title match)
    pub fn sorted_by_relevance(&self, query: &str) -> Vec<&StreamTrack> {
        let query_lower = query.to_lowercase();
        let mut tracks: Vec<&StreamTrack> = self.tracks.iter().collect();

        tracks.sort_by(|a, b| {
            // Title match score
            let a_title_match = if a.title.to_lowercase().contains(&query_lower) {
                2
            } else {
                0
            };
            let b_title_match = if b.title.to_lowercase().contains(&query_lower) {
                2
            } else {
                0
            };

            // Artist match score
            let a_artist_match = if a.artist.to_lowercase().contains(&query_lower) {
                1
            } else {
                0
            };
            let b_artist_match = if b.artist.to_lowercase().contains(&query_lower) {
                1
            } else {
                0
            };

            // View count (for YouTube)
            let a_views = a.view_count.unwrap_or(0);
            let b_views = b.view_count.unwrap_or(0);

            // Combined score (higher is better)
            let a_score = a_title_match + a_artist_match + (a_views / 1000000) as i32;
            let b_score = b_title_match + b_artist_match + (b_views / 1000000) as i32;

            b_score.cmp(&a_score)
        });

        tracks
    }
}

/// Download task status
#[derive(Debug, Clone)]
pub struct DownloadTask {
    /// Track being downloaded
    pub track_id: String,
    /// Track title
    pub title: String,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Download speed (bytes/sec)
    pub speed: u64,
    /// Status
    pub status: DownloadStatus,
    /// Source
    pub source: StreamSource,
}

/// Download status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

/// Stream manager - main entry point for streaming operations
pub struct StreamManager {
    config: StreamManagerConfig,
    youtube: Option<YouTubeProvider>,
    spotify: Option<SpotifyProvider>,
    cache: Arc<SmartCache>,
    predictor: Arc<PredictiveCache>,
    active_downloads: Arc<RwLock<Vec<DownloadTask>>>,
    current_track: Arc<RwLock<Option<String>>>,
}

impl StreamManager {
    /// Create a new stream manager
    pub async fn new(config: StreamManagerConfig) -> Result<Self, StreamError> {
        if !config.enabled {
            info!("Streaming is disabled in configuration");
        }

        // Initialize cache
        let cache = Arc::new(SmartCache::new(config.cache.clone()).await?);

        // Initialize predictor
        let predictor_db = config.cache.cache_dir.join("predictor.db");
        let predictor = Arc::new(PredictiveCache::new(
            config.predictor.clone(),
            predictor_db,
            cache.clone(),
        )?);

        // Initialize YouTube provider
        let youtube = if config.enabled {
            let mut yt = YouTubeProvider::new(config.youtube.clone());
            match yt.initialize().await {
                Ok(()) => {
                    info!("YouTube provider initialized successfully");
                    Some(yt)
                }
                Err(e) => {
                    warn!("YouTube provider initialization failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize Spotify provider
        let spotify = if config.enabled && config.spotify.enabled {
            let mut sp = SpotifyProvider::new(config.spotify.clone());
            match sp.initialize().await {
                Ok(()) => {
                    info!("Spotify provider initialized successfully");
                    Some(sp)
                }
                Err(e) => {
                    warn!("Spotify provider initialization failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            youtube,
            spotify,
            cache,
            predictor,
            active_downloads: Arc::new(RwLock::new(Vec::new())),
            current_track: Arc::new(RwLock::new(None)),
        })
    }

    /// Check if YouTube is available
    pub fn is_youtube_available(&self) -> bool {
        self.youtube.is_some()
    }

    /// Check if Spotify is available
    pub fn is_spotify_available(&self) -> bool {
        self.spotify.is_some()
    }

    /// Search all available sources
    pub async fn search_all(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<CombinedSearchResult, StreamError> {
        let mut results = CombinedSearchResult {
            youtube: None,
            spotify: None,
            tracks: Vec::new(),
            total: 0,
            query: query.to_string(),
        };

        // Search YouTube
        if let Some(ref youtube) = self.youtube {
            match youtube.search(query, limit).await {
                Ok(yt_results) => {
                    results.total += yt_results.total;
                    results.tracks.extend(yt_results.tracks.clone());
                    results.youtube = Some(yt_results);
                }
                Err(e) => {
                    warn!("YouTube search failed: {}", e);
                }
            }
        }

        // Search Spotify
        if let Some(ref spotify) = self.spotify {
            match spotify.search(query, limit).await {
                Ok(sp_results) => {
                    results.total += sp_results.total;
                    results.tracks.extend(sp_results.tracks.clone());
                    results.spotify = Some(sp_results);
                }
                Err(e) => {
                    warn!("Spotify search failed: {}", e);
                }
            }
        }

        // Deduplicate by title + artist similarity
        results.tracks = self.deduplicate_tracks(&results.tracks);

        info!(
            "Found {} tracks across all sources for query '{}'",
            results.tracks.len(),
            query
        );

        Ok(results)
    }

    /// Search only YouTube
    pub async fn search_youtube(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResult, StreamError> {
        match &self.youtube {
            Some(youtube) => youtube.search(query, limit).await,
            None => Err(StreamError::Unavailable(
                "YouTube provider is not available".to_string(),
            )),
        }
    }

    /// Search only Spotify
    pub async fn search_spotify(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResult, StreamError> {
        match &self.spotify {
            Some(spotify) => spotify.search(query, limit).await,
            None => Err(StreamError::Unavailable(
                "Spotify provider is not available".to_string(),
            )),
        }
    }

    /// Get a playable URL for a track
    pub async fn get_playable_url(&self, track: &StreamTrack) -> Result<String, StreamError> {
        // Check cache first
        if self.config.prefer_cached {
            if let Some(path) = self.cache.get_cached_path(&track.id).await {
                info!("Using cached version of track {}", track.id);
                return Ok(format!("file://{}", path.to_string_lossy()));
            }
        }

        // Get stream URL based on source
        match track.source {
            StreamSource::YouTube => match &self.youtube {
                Some(youtube) => {
                    youtube
                        .get_stream_url(&track.id, self.config.default_quality)
                        .await
                }
                None => Err(StreamError::Unavailable(
                    "YouTube provider is not available".to_string(),
                )),
            },
            StreamSource::Spotify => match &self.spotify {
                Some(spotify) => {
                    spotify
                        .get_stream_url(&track.id, self.config.default_quality)
                        .await
                }
                None => Err(StreamError::Unavailable(
                    "Spotify provider is not available".to_string(),
                )),
            },
            StreamSource::Local | StreamSource::Cached => {
                // For local/cached, return the stream_url if available
                match &track.stream_url {
                    Some(url) => Ok(url.clone()),
                    None => Err(StreamError::TrackNotFound(format!(
                        "No URL for local track {}",
                        track.id
                    ))),
                }
            }
        }
    }

    /// Play a track (returns playable URL or cached path)
    pub async fn play_track(&self, track: &StreamTrack) -> Result<PlayResult, StreamError> {
        // Update current track
        {
            let mut current = self.current_track.write().await;
            *current = Some(track.id.clone());
        }

        // Check if cached
        if let Some(cached) = self.cache.get_track(&track.id).await {
            info!("Playing cached track: {}", track.title);

            // Record play
            self.cache.record_play(&track.id).await?;

            return Ok(PlayResult {
                url: format!("file://{}", cached.file_path.to_string_lossy()),
                is_cached: true,
                source: track.source,
            });
        }

        // Get stream URL
        let url = self.get_playable_url(track).await?;

        info!("Streaming track: {} from {}", track.title, track.source);

        Ok(PlayResult {
            url,
            is_cached: false,
            source: track.source,
        })
    }

    /// Download a track to cache
    pub async fn download_track(&self, track: &StreamTrack) -> Result<PathBuf, StreamError> {
        // Check if already cached
        if let Some(path) = self.cache.get_cached_path(&track.id).await {
            info!("Track {} is already cached", track.id);
            return Ok(path);
        }

        // Generate cache path
        let cache_path = self
            .cache
            .generate_cache_path(track, self.config.default_quality);

        // Download based on source
        match track.source {
            StreamSource::YouTube => {
                match &self.youtube {
                    Some(youtube) => {
                        // Add to active downloads
                        self.add_download(&track.id, &track.title, track.source)
                            .await;

                        let result = youtube
                            .download_track(&track.id, self.config.default_quality, &cache_path)
                            .await;

                        // Remove from active downloads
                        self.remove_download(&track.id).await;

                        result?;

                        // Add to cache
                        self.cache
                            .add_track(track, &cache_path, self.config.default_quality)
                            .await?;

                        info!("Downloaded {} to {:?}", track.title, cache_path);
                        Ok(cache_path)
                    }
                    None => Err(StreamError::Unavailable(
                        "YouTube provider is not available".to_string(),
                    )),
                }
            }
            StreamSource::Spotify => {
                // Spotify doesn't support full downloads
                Err(StreamError::DownloadFailed(
                    "Spotify does not support full track downloads. Use YouTube for downloading."
                        .to_string(),
                ))
            }
            _ => Err(StreamError::DownloadFailed(format!(
                "Cannot download {} tracks",
                track.source
            ))),
        }
    }

    /// Run prediction and prefetch for current track
    pub async fn prefetch_next(
        &self,
        queue: &[StreamTrack],
    ) -> Result<Vec<Prediction>, StreamError> {
        let current = self.current_track.read().await;

        if let Some(current_id) = current.as_ref() {
            let predictions = self
                .predictor
                .run_prediction_cycle(current_id, queue)
                .await?;

            if !predictions.is_empty() {
                info!("Prefetching {} predicted tracks", predictions.len());
            }

            Ok(predictions)
        } else {
            Ok(Vec::new())
        }
    }

    /// Record a play event for learning
    pub async fn record_play(
        &self,
        track_id: &str,
        source: StreamSource,
        duration_secs: u64,
        completed: bool,
        skipped: bool,
    ) -> Result<(), StreamError> {
        let previous = self.current_track.read().await.clone();

        self.predictor
            .record_play(
                track_id,
                source,
                previous.as_deref(),
                duration_secs,
                completed,
                skipped,
            )
            .await?;

        // Also record in cache if it's a cached track
        if self.cache.is_cached(track_id).await {
            self.cache.record_play(track_id).await?;
        }

        // Update current track
        {
            let mut current = self.current_track.write().await;
            *current = Some(track_id.to_string());
        }

        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        self.cache.get_stats().await
    }

    /// Get predictor statistics
    pub async fn get_predictor_stats(&self) -> Result<PredictorStats, StreamError> {
        self.predictor.get_stats().await.map_err(StreamError::from)
    }

    /// Get active downloads
    pub async fn get_active_downloads(&self) -> Vec<DownloadTask> {
        self.active_downloads.read().await.clone()
    }

    /// Clear all cached content
    pub async fn clear_cache(&self) -> Result<(), StreamError> {
        self.cache.clear_all().await?;
        info!("Cache cleared");
        Ok(())
    }

    /// Remove a specific track from cache
    pub async fn remove_from_cache(&self, track_id: &str) -> Result<(), StreamError> {
        self.cache.remove_track(track_id).await?;
        info!("Removed track {} from cache", track_id);
        Ok(())
    }

    /// Check if a track is cached
    pub async fn is_cached(&self, track_id: &str) -> bool {
        self.cache.is_cached(track_id).await
    }

    /// Get track info from a provider
    pub async fn get_track_info(
        &self,
        track_id: &str,
        source: StreamSource,
    ) -> Result<StreamTrack, StreamError> {
        match source {
            StreamSource::YouTube => match &self.youtube {
                Some(youtube) => youtube.get_track_info(track_id).await,
                None => Err(StreamError::Unavailable(
                    "YouTube not available".to_string(),
                )),
            },
            StreamSource::Spotify => match &self.spotify {
                Some(spotify) => spotify.get_track_info(track_id).await,
                None => Err(StreamError::Unavailable(
                    "Spotify not available".to_string(),
                )),
            },
            _ => Err(StreamError::TrackNotFound(format!(
                "Unknown source: {}",
                source
            ))),
        }
    }

    /// Deduplicate tracks by title/artist similarity
    fn deduplicate_tracks(&self, tracks: &[StreamTrack]) -> Vec<StreamTrack> {
        let mut seen: Vec<(String, String)> = Vec::new();
        let mut result = Vec::new();

        for track in tracks {
            let key = (
                track.title.to_lowercase().trim().to_string(),
                track.artist.to_lowercase().trim().to_string(),
            );

            if !seen.contains(&key) {
                seen.push(key);
                result.push(track.clone());
            }
        }

        result
    }

    /// Add a download to the active list
    async fn add_download(&self, track_id: &str, title: &str, source: StreamSource) {
        let mut downloads = self.active_downloads.write().await;
        downloads.push(DownloadTask {
            track_id: track_id.to_string(),
            title: title.to_string(),
            progress: 0,
            speed: 0,
            status: DownloadStatus::Pending,
            source,
        });
    }

    /// Remove a download from the active list
    async fn remove_download(&self, track_id: &str) {
        let mut downloads = self.active_downloads.write().await;
        downloads.retain(|d| d.track_id != track_id);
    }

    /// Get the default quality
    pub fn default_quality(&self) -> StreamQuality {
        self.config.default_quality
    }

    /// Set the default quality
    pub fn set_default_quality(&mut self, quality: StreamQuality) {
        self.config.default_quality = quality;
    }
}

/// Result of playing a track
#[derive(Debug, Clone)]
pub struct PlayResult {
    /// Playable URL (file:// or http://)
    pub url: String,
    /// Whether the track was served from cache
    pub is_cached: bool,
    /// Source of the track
    pub source: StreamSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combined_search_result() {
        let result = CombinedSearchResult {
            youtube: None,
            spotify: None,
            tracks: vec![
                StreamTrack::new("1", "Song A", "Artist", StreamSource::YouTube),
                StreamTrack::new("2", "Song B", "Artist", StreamSource::Spotify),
            ],
            total: 2,
            query: "test".to_string(),
        };

        assert_eq!(result.len(), 2);
        assert!(!result.is_empty());

        let youtube_only = result.filter_by_source(StreamSource::YouTube);
        assert_eq!(youtube_only.len(), 1);
    }

    #[test]
    fn test_stream_manager_config_default() {
        let config = StreamManagerConfig::default();
        assert!(config.enabled);
        assert_eq!(config.default_quality, StreamQuality::Medium);
        assert!(config.prefer_cached);
    }

    #[test]
    fn test_play_result() {
        let result = PlayResult {
            url: "file:///path/to/file.mp3".to_string(),
            is_cached: true,
            source: StreamSource::YouTube,
        };

        assert!(result.is_cached);
        assert!(result.url.starts_with("file://"));
    }
}
