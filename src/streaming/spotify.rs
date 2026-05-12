//! Spotify streaming provider
//!
//! Provides access to the Spotify Web API for searching and metadata.
//!
//! # Limitations
//! - Only 30-second preview URLs are available via the Web API
//! - Full playback requires Spotify Premium and an official SDK (not available in Rust)
//! - Users should use YouTube provider for full streaming functionality
//!
//! # Setup
//! 1. Go to https://developer.spotify.com/dashboard
//! 2. Create a new application
//! 3. Copy the Client ID and Client Secret
//! 4. Configure in symphony.toml under [streaming.spotify]

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::provider::{
    DownloadProgress, DownloadStatus, SearchResult, StreamError, StreamProvider, StreamQuality,
    StreamSource, StreamTrack,
};

/// Spotify API base URL
const SPOTIFY_API_URL: &str = "https://api.spotify.com/v1";
const SPOTIFY_ACCOUNTS_URL: &str = "https://accounts.spotify.com";

/// Spotify provider configuration
#[derive(Debug, Clone)]
pub struct SpotifyConfig {
    /// Spotify Client ID
    pub client_id: String,
    /// Spotify Client Secret
    pub client_secret: String,
    /// Redirect URI for OAuth
    pub redirect_uri: String,
    /// Enable Spotify provider
    pub enabled: bool,
}

impl Default for SpotifyConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: "http://localhost:8888/callback".to_string(),
            enabled: false,
        }
    }
}

/// OAuth token response from Spotify
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

/// Cached authentication token
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

impl CachedToken {
    fn new(token: TokenResponse) -> Self {
        Self {
            access_token: token.access_token,
            // Refresh 5 minutes before expiry
            expires_at: Instant::now() + Duration::from_secs(token.expires_in.saturating_sub(300)),
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Spotify search response structure
#[derive(Debug, Deserialize)]
struct SearchResponse {
    tracks: TracksResponse,
}

#[derive(Debug, Deserialize)]
struct TracksResponse {
    items: Vec<TrackItem>,
    total: u32,
}

#[derive(Debug, Deserialize)]
struct TrackItem {
    id: String,
    name: String,
    duration_ms: u64,
    preview_url: Option<String>,
    artists: Vec<ArtistItem>,
    album: Option<AlbumItem>,
    external_urls: ExternalUrls,
}

#[derive(Debug, Deserialize)]
struct ArtistItem {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct AlbumItem {
    id: String,
    name: String,
    images: Vec<ImageItem>,
}

#[derive(Debug, Deserialize)]
struct ImageItem {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Debug, Deserialize)]
struct ExternalUrls {
    spotify: String,
}

/// Track details response
#[derive(Debug, Deserialize)]
struct TrackDetailsResponse {
    id: String,
    name: String,
    duration_ms: u64,
    preview_url: Option<String>,
    artists: Vec<ArtistItem>,
    album: Option<AlbumItem>,
    popularity: u32,
}

/// Spotify provider implementation
pub struct SpotifyProvider {
    config: SpotifyConfig,
    client: Client,
    token: Arc<RwLock<Option<CachedToken>>>,
    available: bool,
}

impl SpotifyProvider {
    /// Create a new Spotify provider
    pub fn new(config: SpotifyConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            config,
            client,
            token: Arc::new(RwLock::new(None)),
            available: false,
        }
    }

    /// Create a Spotify provider with default configuration
    pub fn with_defaults() -> Self {
        Self::new(SpotifyConfig::default())
    }

    /// Initialize the provider
    pub async fn initialize(&mut self) -> Result<(), StreamError> {
        if !self.config.enabled {
            info!("Spotify provider is disabled in configuration");
            return Ok(());
        }

        if self.config.client_id.is_empty() || self.config.client_secret.is_empty() {
            warn!("Spotify credentials not configured");
            return Err(StreamError::ConfigError(
                "Spotify client_id and client_secret are required. Get them from https://developer.spotify.com/dashboard".to_string()
            ));
        }

        // Get initial access token using client credentials flow
        self.refresh_token().await?;

        self.available = true;
        info!("Spotify provider initialized successfully");
        Ok(())
    }

    /// Refresh the access token using client credentials flow
    async fn refresh_token(&self) -> Result<(), StreamError> {
        let url = format!("{}/api/token", SPOTIFY_ACCOUNTS_URL);

        let params = [("grant_type", "client_credentials")];

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Failed to get Spotify token: {} - {}", status, body);
            return Err(StreamError::AuthRequired(format!(
                "Spotify authentication failed: {}",
                status
            )));
        }

        let token: TokenResponse = response.json().await?;

        let mut cached = self.token.write().await;
        *cached = Some(CachedToken::new(token));

        debug!("Spotify token refreshed successfully");
        Ok(())
    }

    /// Get a valid access token, refreshing if necessary
    async fn get_access_token(&self) -> Result<String, StreamError> {
        let cached = self.token.read().await;

        if let Some(ref token) = *cached {
            if !token.is_expired() {
                return Ok(token.access_token.clone());
            }
        }

        // Need to refresh
        drop(cached); // Release read lock
        self.refresh_token().await?;

        let cached = self.token.read().await;
        cached
            .as_ref()
            .map(|t| t.access_token.clone())
            .ok_or_else(|| StreamError::AuthRequired("No Spotify token available".to_string()))
    }

    /// Convert a Spotify track item to our StreamTrack
    fn convert_track(&self, item: TrackItem) -> StreamTrack {
        let artist = item
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = item.album.as_ref().map(|a| a.name.clone());

        let thumbnail_url = item
            .album
            .as_ref()
            .and_then(|a| a.images.first())
            .map(|i| i.url.clone());

        let duration_secs = item.duration_ms / 1000;

        let mut track = StreamTrack::new(item.id, item.name, artist, StreamSource::Spotify)
            .with_duration(duration_secs);

        track.album = album;
        track.thumbnail_url = thumbnail_url;
        track.stream_url = item.preview_url; // 30-second preview URL

        track
    }

    /// Search for tracks on Spotify
    async fn search_tracks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<StreamTrack>, StreamError> {
        let token = self.get_access_token().await?;

        let url = format!(
            "{}/search?q={}&type=track&limit={}",
            SPOTIFY_API_URL,
            urlencoding::encode(query),
            limit
        );

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if response.status() == 401 {
            // Token expired, refresh and retry
            debug!("Spotify token expired, refreshing...");
            self.refresh_token().await?;
            let token = self.get_access_token().await?;

            let response = self.client.get(&url).bearer_auth(&token).send().await?;

            if !response.status().is_success() {
                return Err(StreamError::SearchFailed(format!(
                    "Spotify search failed: {}",
                    response.status()
                )));
            }

            let search: SearchResponse = response.json().await?;
            return Ok(search
                .tracks
                .items
                .into_iter()
                .map(|t| self.convert_track(t))
                .collect());
        }

        if !response.status().is_success() {
            return Err(StreamError::SearchFailed(format!(
                "Spotify search failed: {}",
                response.status()
            )));
        }

        let search: SearchResponse = response.json().await?;

        Ok(search
            .tracks
            .items
            .into_iter()
            .map(|t| self.convert_track(t))
            .collect())
    }

    /// Get track details by ID
    async fn get_track_details(&self, track_id: &str) -> Result<StreamTrack, StreamError> {
        let token = self.get_access_token().await?;

        let url = format!("{}/tracks/{}", SPOTIFY_API_URL, track_id);

        let response = self.client.get(&url).bearer_auth(&token).send().await?;

        if response.status() == 404 {
            return Err(StreamError::TrackNotFound(track_id.to_string()));
        }

        if response.status() == 401 {
            self.refresh_token().await?;
            let token = self.get_access_token().await?;

            let response = self.client.get(&url).bearer_auth(&token).send().await?;

            if !response.status().is_success() {
                return Err(StreamError::TrackNotFound(track_id.to_string()));
            }

            let track: TrackDetailsResponse = response.json().await?;
            return Ok(self.convert_track_details(track));
        }

        if !response.status().is_success() {
            return Err(StreamError::TrackNotFound(format!(
                "Failed to get track: {}",
                response.status()
            )));
        }

        let track: TrackDetailsResponse = response.json().await?;
        Ok(self.convert_track_details(track))
    }

    /// Convert track details to StreamTrack
    fn convert_track_details(&self, details: TrackDetailsResponse) -> StreamTrack {
        let artist = details
            .artists
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());

        let album = details.album.as_ref().map(|a| a.name.clone());

        let thumbnail_url = details
            .album
            .as_ref()
            .and_then(|a| a.images.first())
            .map(|i| i.url.clone());

        let duration_secs = details.duration_ms / 1000;

        let mut track = StreamTrack::new(details.id, details.name, artist, StreamSource::Spotify)
            .with_duration(duration_secs);

        track.album = album;
        track.thumbnail_url = thumbnail_url;
        track.stream_url = details.preview_url;

        track
    }
}

#[async_trait]
impl StreamProvider for SpotifyProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<SearchResult, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "Spotify provider not initialized or disabled. Configure client_id and client_secret in symphony.toml".to_string(),
            ));
        }

        info!("Searching Spotify for: {}", query);
        let tracks = self.search_tracks(query, limit).await?;

        Ok(SearchResult::new(tracks, query, StreamSource::Spotify))
    }

    async fn get_stream_url(
        &self,
        track_id: &str,
        _quality: StreamQuality,
    ) -> Result<String, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "Spotify provider not initialized.".to_string(),
            ));
        }

        // Get track info to retrieve preview URL
        let track = self.get_track_details(track_id).await?;

        match track.stream_url {
            Some(url) => {
                info!("Returning 30-second preview URL for Spotify track");
                Ok(url)
            }
            None => {
                warn!("No preview URL available for Spotify track {}", track_id);
                Err(StreamError::DownloadFailed(
                    "No preview URL available. Spotify only provides 30-second previews for most tracks. Use YouTube provider for full playback.".to_string()
                ))
            }
        }
    }

    async fn get_track_info(&self, track_id: &str) -> Result<StreamTrack, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "Spotify provider not initialized.".to_string(),
            ));
        }

        self.get_track_details(track_id).await
    }

    async fn download_track(
        &self,
        track_id: &str,
        _quality: StreamQuality,
        _output_path: &PathBuf,
    ) -> Result<(), StreamError> {
        // Spotify doesn't allow direct downloads via API
        // Preview URLs could be downloaded, but that's limited to 30 seconds
        Err(StreamError::DownloadFailed(
            "Spotify does not support full track downloads via API. Only 30-second previews are available. Use YouTube provider for full track downloads.".to_string()
        ))
    }

    async fn is_available(&self) -> bool {
        self.available
    }

    fn name(&self) -> &str {
        "Spotify"
    }

    fn source(&self) -> StreamSource {
        StreamSource::Spotify
    }
}

/// URL encoding helper (simple implementation)
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spotify_config_default() {
        let config = SpotifyConfig::default();
        assert!(config.client_id.is_empty());
        assert!(config.client_secret.is_empty());
        assert!(!config.enabled);
    }

    #[tokio::test]
    async fn test_spotify_provider_creation() {
        let provider = SpotifyProvider::with_defaults();
        assert_eq!(provider.name(), "Spotify");
        assert_eq!(provider.source(), StreamSource::Spotify);
    }

    #[test]
    fn test_cached_token_expiry() {
        let token = TokenResponse {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: None,
            scope: None,
        };

        let cached = CachedToken::new(token);
        assert!(!cached.is_expired());
    }
}
