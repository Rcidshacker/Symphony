//! Lyrics Fetcher
//!
//! Fetches lyrics from local cache and online sources.

use super::{Lyrics, LyricsError, LyricsSource, LrcParser};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

/// Lyrics fetcher configuration
#[derive(Debug, Clone)]
pub struct LyricsFetcherConfig {
    /// Cache directory
    pub cache_dir: PathBuf,
    /// Enable online fetching
    pub online_enabled: bool,
    /// Auto-fetch when no local lyrics
    pub auto_fetch: bool,
    /// Cache expiration in days
    pub cache_expiration_days: u32,
}

impl Default for LyricsFetcherConfig {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony")
            .join("lyrics");

        Self {
            cache_dir,
            online_enabled: true,
            auto_fetch: true,
            cache_expiration_days: 30,
        }
    }
}

/// Lyrics fetcher
pub struct LyricsFetcher {
    config: LyricsFetcherConfig,
    http_client: Option<reqwest::Client>,
}

impl LyricsFetcher {
    /// Create a new lyrics fetcher
    pub fn new(config: LyricsFetcherConfig) -> Self {
        let http_client = if config.online_enabled {
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .user_agent("Symphony Music Player")
                .build()
                .ok()
        } else {
            None
        };

        Self { config, http_client }
    }

    /// Create fetcher with default config
    pub fn with_defaults() -> Self {
        Self::new(LyricsFetcherConfig::default())
    }

    /// Fetch lyrics for a track
    pub async fn fetch(&self, title: &str, artist: &str) -> Result<Lyrics, LyricsError> {
        // 1. Try cache
        if let Some(lyrics) = self.fetch_from_cache(title, artist).await? {
            debug!("Found cached lyrics for {} - {}", artist, title);
            return Ok(lyrics);
        }

        // 2. Try local LRC files (same directory as track)
        // This would need the track path, skipped for now

        // 3. Try online sources
        if self.config.online_enabled {
            if let Some(lyrics) = self.fetch_online(title, artist).await? {
                // Cache the result
                self.cache_lyrics(&lyrics, title, artist).await?;
                return Ok(lyrics);
            }
        }

        Err(LyricsError::NotFound)
    }

    /// Fetch lyrics from local cache
    async fn fetch_from_cache(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let cache_path = self.get_cache_path(title, artist);

        if cache_path.exists() {
            let content = fs::read_to_string(&cache_path).await?;
            let mut lyrics = LrcParser::parse(&content)?;
            lyrics.source = LyricsSource::LocalFile;
            return Ok(Some(lyrics));
        }

        Ok(None)
    }

    /// Fetch lyrics from online sources
    async fn fetch_online(&self, title: &str, artist: &str) -> Result<Option<Lyrics>, LyricsError> {
        let client = match &self.http_client {
            Some(c) => c,
            None => return Ok(None),
        };

        // Try multiple sources in order
        // Note: Real implementation would use proper APIs with keys

        // For now, return None as we don't have API keys
        // In production, you would:
        // 1. Try MusicBrainz for track metadata
        // 2. Try LRCLIB (https://lrclib.net/) - free API
        // 3. Try other services as needed

        debug!("Online lyrics fetch not implemented for {} - {}", artist, title);
        Ok(None)
    }

    /// Try to fetch from LRCLIB (free lyrics API)
    #[allow(dead_code)]
    async fn fetch_from_lrclib(
        &self,
        client: &reqwest::Client,
        title: &str,
        artist: &str,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let url = format!(
            "https://lrclib.net/api/get?artist_name={}&track_name={}",
            urlencoding::encode(artist),
            urlencoding::encode(title)
        );

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        // Parse LRCLIB response
        #[derive(serde::Deserialize)]
        struct LrclibResponse {
            plain_lyrics: Option<String>,
            synced_lyrics: Option<String>,
        }

        let data: LrclibResponse = response
            .json()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;

        if let Some(synced) = data.synced_lyrics {
            let mut lyrics = LrcParser::parse(&synced)?;
            lyrics.source = LyricsSource::Online {
                service: "LRCLIB".to_string(),
                url: Some(url),
            };
            return Ok(Some(lyrics));
        }

        Ok(None)
    }

    /// Cache lyrics to local storage
    async fn cache_lyrics(
        &self,
        lyrics: &Lyrics,
        title: &str,
        artist: &str,
    ) -> Result<(), LyricsError> {
        let cache_path = self.get_cache_path(title, artist);

        // Ensure parent directory exists
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let lrc_content = super::lrc::to_lrc(lyrics);
        fs::write(&cache_path, lrc_content).await?;

        info!("Cached lyrics for {} - {}", artist, title);
        Ok(())
    }

    /// Get cache path for a track
    pub fn get_cache_path(&self, title: &str, artist: &str) -> PathBuf {
        let sanitized_artist = sanitize_filename(artist);
        let sanitized_title = sanitize_filename(title);

        self.config
            .cache_dir
            .join(sanitized_artist)
            .join(format!("{}.lrc", sanitized_title))
    }

    /// Clear the lyrics cache
    pub async fn clear_cache(&self) -> Result<(), LyricsError> {
        if self.config.cache_dir.exists() {
            fs::remove_dir_all(&self.config.cache_dir).await?;
            fs::create_dir_all(&self.config.cache_dir).await?;
        }
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        if !self.config.cache_dir.exists() {
            return stats;
        }

        let mut queue = vec![self.config.cache_dir.clone()];

        while let Some(dir) = queue.pop() {
            if let Ok(mut entries) = fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        queue.push(path);
                    } else if path.extension().map(|e| e == "lrc").unwrap_or(false) {
                        stats.total_files += 1;
                        if let Ok(metadata) = entry.metadata().await {
                            stats.total_size += metadata.len();
                        }
                    }
                }
            }
        }

        stats
    }
}

/// Sanitize filename for safe filesystem use
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// URL encoding module
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total cached files
    pub total_files: usize,
    /// Total cache size in bytes
    pub total_size: u64,
}

impl CacheStats {
    /// Format size as human-readable
    pub fn formatted_size(&self) -> String {
        let bytes = self.total_size;
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
        assert_eq!(sanitize_filename("test/slash"), "test_slash");
        assert_eq!(sanitize_filename("Artist: Name"), "Artist_ Name");
    }

    #[tokio::test]
    async fn test_cache_path() {
        let fetcher = LyricsFetcher::with_defaults();
        let path = fetcher.get_cache_path("Test Song", "Test Artist");

        assert!(path.to_string_lossy().contains("Test Artist"));
        assert!(path.to_string_lossy().contains("Test Song"));
        assert!(path.extension().unwrap() == "lrc");
    }

    #[test]
    fn test_cache_stats() {
        let stats = CacheStats {
            total_files: 10,
            total_size: 1024 * 1024,
        };

        assert_eq!(stats.formatted_size(), "1.0 MB");
    }
}
