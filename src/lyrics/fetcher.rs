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
    /// Base URL of the LRCLIB API
    pub lrclib_url: String,
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
            lrclib_url: "https://lrclib.net".to_string(),
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
        self.fetch_track(title, artist, None, None).await
    }

    /// Fetch lyrics, using album and duration (seconds) to pick the right recording
    pub async fn fetch_track(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_secs: Option<f64>,
    ) -> Result<Lyrics, LyricsError> {
        // 1. Cache
        if let Some(lyrics) = self.fetch_from_cache(title, artist).await? {
            debug!("Found cached lyrics for {} - {}", artist, title);
            return Ok(lyrics);
        }

        // 2. Online (LRCLIB)
        if self.config.online_enabled {
            if let Some(lyrics) = self
                .fetch_online(title, artist, album, duration_secs)
                .await?
            {
                self.cache_lyrics(&lyrics, title, artist).await?;
                return Ok(lyrics);
            }
        }

        Err(LyricsError::NotFound)
    }

    /// Fetch lyrics from local cache (entries older than `cache_expiration_days` are ignored)
    async fn fetch_from_cache(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let cache_path = self.get_cache_path(title, artist);
        if !cache_path.exists() {
            return Ok(None);
        }

        let max_age = std::time::Duration::from_secs(u64::from(self.config.cache_expiration_days) * 86_400);
        let age = fs::metadata(&cache_path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|modified| modified.elapsed().ok());
        if age.is_some_and(|age| age > max_age) {
            debug!("Cached lyrics for {} - {} expired", artist, title);
            return Ok(None);
        }

        let content = fs::read_to_string(&cache_path).await?;
        let mut lyrics = LrcParser::parse(&content)?;
        lyrics.source = LyricsSource::LocalFile;
        Ok(Some(lyrics))
    }

    /// Fetch lyrics from LRCLIB: exact lookup first, then a search
    async fn fetch_online(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_secs: Option<f64>,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let Some(client) = &self.http_client else {
            return Ok(None);
        };

        let exact = self
            .lrclib_get(client, title, artist, album, duration_secs)
            .await;
        if let Ok(Some(lyrics)) = exact {
            return Ok(Some(lyrics));
        }

        match self.lrclib_search(client, title, artist, duration_secs).await {
            Ok(Some(lyrics)) => Ok(Some(lyrics)),
            // the search found nothing: report the lookup's network error if there was one
            Ok(None) => exact.map(|_| None),
            Err(search_err) => exact.map(|_| None).or(Err(search_err)),
        }
    }

    /// `GET /api/get`: the exact recording
    async fn lrclib_get(
        &self,
        client: &reqwest::Client,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_secs: Option<f64>,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let mut query = vec![
            ("artist_name", artist.to_string()),
            ("track_name", title.to_string()),
        ];
        if let Some(album) = album.filter(|a| !a.is_empty() && *a != "Unknown") {
            query.push(("album_name", album.to_string()));
        }
        if let Some(duration) = duration_secs.filter(|d| *d > 0.0) {
            query.push(("duration", format!("{}", duration.round() as u64)));
        }

        let url = format!("{}/api/get", self.config.lrclib_url.trim_end_matches('/'));
        let response = client
            .get(&url)
            .query(&query)
            .send()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(LyricsError::NetworkError(format!("LRCLIB returned {status}")));
        }

        let entry: LrclibEntry = response
            .json()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;
        entry.into_lyrics(&url)
    }

    /// `GET /api/search`: the best synced match, preferring the closest duration
    async fn lrclib_search(
        &self,
        client: &reqwest::Client,
        title: &str,
        artist: &str,
        duration_secs: Option<f64>,
    ) -> Result<Option<Lyrics>, LyricsError> {
        let url = format!("{}/api/search", self.config.lrclib_url.trim_end_matches('/'));
        let response = client
            .get(&url)
            .query(&[("track_name", title), ("artist_name", artist)])
            .send()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(LyricsError::NetworkError(format!("LRCLIB returned {status}")));
        }

        let entries: Vec<LrclibEntry> = response
            .json()
            .await
            .map_err(|e| LyricsError::NetworkError(e.to_string()))?;

        let best = entries
            .into_iter()
            .filter(|e| !e.instrumental && e.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty()))
            .min_by(|a, b| {
                let distance = |e: &LrclibEntry| match (duration_secs, e.duration) {
                    (Some(wanted), Some(got)) if wanted > 0.0 => (wanted - got).abs(),
                    _ => 0.0,
                };
                distance(a).total_cmp(&distance(b))
            });

        match best {
            Some(entry) => entry.into_lyrics(&url),
            None => Ok(None),
        }
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

/// One LRCLIB record (`/api/get` returns one, `/api/search` a list)
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibEntry {
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    duration: Option<f64>,
}

impl LrclibEntry {
    /// Synced lyrics only: plain text has no timestamps to follow
    fn into_lyrics(self, url: &str) -> Result<Option<Lyrics>, LyricsError> {
        if self.instrumental {
            return Ok(None);
        }
        let Some(synced) = self.synced_lyrics.filter(|s| !s.trim().is_empty()) else {
            return Ok(None);
        };

        let mut lyrics = LrcParser::parse(&synced)?;
        if lyrics.is_empty() {
            return Ok(None);
        }
        lyrics.source = LyricsSource::Online {
            service: "LRCLIB".to_string(),
            url: Some(url.to_string()),
        };
        Ok(Some(lyrics))
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
