//! Smart caching system for streaming content
//!
//! Implements a multi-tier cache with LRU eviction for storing downloaded
//! streaming content locally.
//!
//! # Cache Tiers
//! - **Hot** (100MB): Most frequently played tracks, fast access
//! - **Warm** (2GB): Recently played tracks, normal access
//! - **Cold** (20GB): Downloaded but rarely played, slower access
//!
//! # Eviction Policy
//! Uses LRU (Least Recently Used) eviction when a tier is full.
//! Tracks can be promoted/demoted between tiers based on access patterns.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;

use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::provider::{StreamQuality, StreamSource, StreamTrack};

/// Cache tier enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheTier {
    /// Hot tier: 100MB, most frequently played
    Hot,
    /// Warm tier: 2GB, recently played
    Warm,
    /// Cold tier: 20GB, rarely played
    Cold,
}

impl CacheTier {
    /// Get the default max size for this tier in bytes
    pub fn default_max_size(&self) -> u64 {
        match self {
            CacheTier::Hot => 100 * 1024 * 1024,        // 100 MB
            CacheTier::Warm => 2 * 1024 * 1024 * 1024,  // 2 GB
            CacheTier::Cold => 20 * 1024 * 1024 * 1024, // 20 GB
        }
    }

    /// Get tier label
    pub fn label(&self) -> &'static str {
        match self {
            CacheTier::Hot => "Hot",
            CacheTier::Warm => "Warm",
            CacheTier::Cold => "Cold",
        }
    }
}

impl std::fmt::Display for CacheTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Configuration for the smart cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Root directory for cache storage
    pub cache_dir: PathBuf,
    /// Maximum size for hot tier in bytes
    pub max_hot_size: u64,
    /// Maximum size for warm tier in bytes
    pub max_warm_size: u64,
    /// Maximum size for cold tier in bytes
    pub max_cold_size: u64,
    /// Minimum plays to promote to hot tier
    pub hot_tier_threshold: u32,
    /// Days without play to demote to cold tier
    pub cold_tier_days: u32,
    /// Enable predictive prefetching
    pub prefetch_enabled: bool,
    /// Number of tracks to prefetch
    pub prefetch_count: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from(".cache"))
            .join("symphony")
            .join("streams");

        Self {
            cache_dir,
            max_hot_size: 100 * 1024 * 1024,        // 100 MB
            max_warm_size: 2 * 1024 * 1024 * 1024,  // 2 GB
            max_cold_size: 20 * 1024 * 1024 * 1024, // 20 GB
            hot_tier_threshold: 5,                  // 5 plays to become hot
            cold_tier_days: 30,                     // 30 days to become cold
            prefetch_enabled: true,
            prefetch_count: 3,
        }
    }
}

/// Cached track metadata
#[derive(Debug, Clone)]
pub struct CachedTrack {
    /// Unique identifier (source-specific)
    pub track_id: String,
    /// Source type
    pub source: StreamSource,
    /// Local file path
    pub file_path: PathBuf,
    /// Quality of cached file
    pub quality: StreamQuality,
    /// Size in bytes
    pub size_bytes: u64,
    /// When the track was cached
    pub cached_at: DateTime<Utc>,
    /// When the track was last played
    pub last_played: Option<DateTime<Utc>>,
    /// Number of times played
    pub play_count: u32,
    /// Current cache tier
    pub tier: CacheTier,
    /// Track title
    pub title: String,
    /// Track artist
    pub artist: String,
}

impl CachedTrack {
    /// Get the age of this cache entry
    pub fn age(&self) -> Duration {
        (Utc::now() - self.cached_at)
            .to_std()
            .unwrap_or(Duration::ZERO)
    }

    /// Get time since last play
    pub fn time_since_play(&self) -> Option<Duration> {
        self.last_played
            .map(|t| (Utc::now() - t).to_std().unwrap_or(Duration::ZERO))
    }

    /// Calculate a score for LRU eviction (higher = more likely to evict)
    pub fn eviction_score(&self) -> f64 {
        let age_score = self.age().as_secs_f64() / 86400.0; // Days
        let play_score = 1.0 / (self.play_count as f64 + 1.0);
        let recency_score = self
            .time_since_play()
            .map(|d| d.as_secs_f64() / 86400.0)
            .unwrap_or(365.0);

        age_score * play_score * recency_score
    }

    /// Check if track should be promoted to a higher tier
    pub fn should_promote(&self, config: &CacheConfig) -> bool {
        match self.tier {
            CacheTier::Hot => false,
            CacheTier::Warm => self.play_count >= config.hot_tier_threshold,
            CacheTier::Cold => self
                .time_since_play()
                .map(|d| d < Duration::from_secs(86400 * 7)) // Played within 7 days
                .unwrap_or(false),
        }
    }

    /// Check if track should be demoted to a lower tier
    pub fn should_demote(&self, config: &CacheConfig) -> bool {
        match self.tier {
            CacheTier::Hot => self
                .time_since_play()
                .map(|d| d > Duration::from_secs(86400 * 7)) // Not played in 7 days
                .unwrap_or(true),
            CacheTier::Warm => self
                .time_since_play()
                .map(|d| d > Duration::from_secs(86400 * config.cold_tier_days as u64))
                .unwrap_or(true),
            CacheTier::Cold => false,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total tracks in cache
    pub total_tracks: usize,
    /// Total size in bytes
    pub total_size: u64,
    /// Size by tier
    pub size_by_tier: HashMap<CacheTier, u64>,
    /// Tracks by tier
    pub tracks_by_tier: HashMap<CacheTier, usize>,
    /// Hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
}

impl CacheStats {
    /// Format total size for display
    pub fn formatted_size(&self) -> String {
        format_bytes(self.total_size)
    }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Smart cache implementation
pub struct SmartCache {
    config: CacheConfig,
    db: Arc<RwLock<Connection>>,
    stats: Arc<RwLock<CacheStats>>,
}

impl SmartCache {
    /// Create a new smart cache
    pub async fn new(config: CacheConfig) -> Result<Self, CacheError> {
        // Ensure cache directory exists
        fs::create_dir_all(&config.cache_dir).await?;

        // Initialize database
        let db_path = config.cache_dir.join("cache.db");
        let db = Connection::open(&db_path)?;

        // Create tables
        Self::init_database(&db)?;

        // Load initial stats
        let stats = Self::load_stats(&db)?;

        Ok(Self {
            config,
            db: Arc::new(RwLock::new(db)),
            stats: Arc::new(RwLock::new(stats)),
        })
    }

    /// Initialize database schema
    fn init_database(db: &Connection) -> Result<(), CacheError> {
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cached_tracks (
                track_id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                file_path TEXT NOT NULL,
                quality TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                cached_at TEXT NOT NULL,
                last_played TEXT,
                play_count INTEGER DEFAULT 0,
                tier TEXT NOT NULL,
                title TEXT,
                artist TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tier ON cached_tracks(tier);
            CREATE INDEX IF NOT EXISTS idx_last_played ON cached_tracks(last_played);
            CREATE INDEX IF NOT EXISTS idx_play_count ON cached_tracks(play_count);

            CREATE TABLE IF NOT EXISTS cache_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (track_id) REFERENCES cached_tracks(track_id)
            );

            CREATE TABLE IF NOT EXISTS prefetch_queue (
                track_id TEXT PRIMARY KEY,
                priority INTEGER DEFAULT 0,
                added_at TEXT NOT NULL,
                source TEXT NOT NULL,
                title TEXT,
                artist TEXT
            );
            "#,
        )?;

        Ok(())
    }

    /// Load statistics from database
    fn load_stats(db: &Connection) -> Result<CacheStats, CacheError> {
        let mut stats = CacheStats::default();

        // Count tracks by tier
        let mut stmt =
            db.prepare("SELECT tier, COUNT(*), SUM(size_bytes) FROM cached_tracks GROUP BY tier")?;

        let tier_iter = stmt.query_map([], |row| {
            let tier: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            let size: i64 = row.get(2)?;
            Ok((tier, count as usize, size as u64))
        })?;

        for result in tier_iter {
            let (tier, count, size) = result?;
            let tier = match tier.as_str() {
                "hot" => CacheTier::Hot,
                "warm" => CacheTier::Warm,
                "cold" => CacheTier::Cold,
                _ => continue,
            };
            stats.tracks_by_tier.insert(tier, count);
            stats.size_by_tier.insert(tier, size);
            stats.total_tracks += count;
            stats.total_size += size;
        }

        Ok(stats)
    }

    /// Check if a track is cached
    pub async fn is_cached(&self, track_id: &str) -> bool {
        let db = self.db.read().await;
        db.query_row(
            "SELECT 1 FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
            |_| Ok(()),
        )
        .is_ok()
    }

    /// Get a cached track's file path
    pub async fn get_cached_path(&self, track_id: &str) -> Option<PathBuf> {
        let db = self.db.read().await;
        db.query_row(
            "SELECT file_path FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(PathBuf::from)
    }

    /// Get a cached track's full info
    pub async fn get_track(&self, track_id: &str) -> Option<CachedTrack> {
        let db = self.db.read().await;
        db.query_row(
            "SELECT track_id, source, file_path, quality, size_bytes, cached_at, 
                    last_played, play_count, tier, title, artist
             FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
            |row| {
                Ok(CachedTrack {
                    track_id: row.get(0)?,
                    source: match row.get::<_, String>(1)?.as_str() {
                        "youtube" => StreamSource::YouTube,
                        "spotify" => StreamSource::Spotify,
                        _ => StreamSource::Local,
                    },
                    file_path: PathBuf::from(row.get::<_, String>(2)?),
                    quality: match row.get::<_, String>(3)?.as_str() {
                        "low" => StreamQuality::Low,
                        "high" => StreamQuality::High,
                        "best" => StreamQuality::Best,
                        _ => StreamQuality::Medium,
                    },
                    size_bytes: row.get::<_, i64>(4)? as u64,
                    cached_at: row.get::<_, String>(5)?.parse().unwrap_or(Utc::now()),
                    last_played: row
                        .get::<_, Option<String>>(6)?
                        .and_then(|s| s.parse().ok()),
                    play_count: row.get::<_, i64>(7)? as u32,
                    tier: match row.get::<_, String>(8)?.as_str() {
                        "hot" => CacheTier::Hot,
                        "cold" => CacheTier::Cold,
                        _ => CacheTier::Warm,
                    },
                    title: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                    artist: row.get::<_, Option<String>>(10)?.unwrap_or_default(),
                })
            },
        )
        .ok()
    }

    /// Add a track to the cache
    pub async fn add_track(
        &self,
        track: &StreamTrack,
        file_path: &Path,
        quality: StreamQuality,
    ) -> Result<(), CacheError> {
        // Get file size
        let metadata = fs::metadata(file_path).await?;
        let size_bytes = metadata.len();

        // Determine initial tier
        let tier = CacheTier::Warm;

        let db = self.db.write().await;
        let now = Utc::now().to_rfc3339();

        db.execute(
            r#"INSERT OR REPLACE INTO cached_tracks 
               (track_id, source, file_path, quality, size_bytes, cached_at, last_played, play_count, tier, title, artist)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
            params![
                track.id,
                track.source.to_string().to_lowercase(),
                file_path.to_string_lossy().to_string(),
                match quality {
                    StreamQuality::Low => "low",
                    StreamQuality::Medium => "medium",
                    StreamQuality::High => "high",
                    StreamQuality::Best => "best",
                },
                size_bytes as i64,
                now,
                Option::<String>::None,
                0,
                tier.to_string().to_lowercase(),
                track.title,
                track.artist,
            ],
        )?;

        info!(
            "Cached track {} ({} bytes) in {} tier",
            track.id, size_bytes, tier
        );

        // Update stats
        drop(db);
        self.refresh_stats().await?;

        Ok(())
    }

    /// Record a play event (updates play count and last played)
    pub async fn record_play(&self, track_id: &str) -> Result<(), CacheError> {
        let db = self.db.write().await;
        let now = Utc::now().to_rfc3339();

        db.execute(
            "UPDATE cached_tracks SET play_count = play_count + 1, last_played = ?1 WHERE track_id = ?2",
            params![now, track_id],
        )?;

        // Log event
        db.execute(
            "INSERT INTO cache_events (track_id, event_type, timestamp) VALUES (?1, 'play', ?2)",
            params![track_id, now],
        )?;

        drop(db);

        // Check for tier promotion
        self.check_tier_promotion(track_id).await?;

        Ok(())
    }

    /// Check if a track should be promoted/demoted
    async fn check_tier_promotion(&self, track_id: &str) -> Result<(), CacheError> {
        if let Some(track) = self.get_track(track_id).await {
            if track.should_promote(&self.config) {
                self.promote_track(track_id).await?;
            } else if track.should_demote(&self.config) {
                self.demote_track(track_id).await?;
            }
        }
        Ok(())
    }

    /// Promote a track to a higher tier
    async fn promote_track(&self, track_id: &str) -> Result<(), CacheError> {
        let db = self.db.write().await;

        // Get current tier
        let current_tier: String = db.query_row(
            "SELECT tier FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
            |row| row.get(0),
        )?;

        let new_tier = match current_tier.as_str() {
            "cold" => "warm",
            "warm" => "hot",
            _ => return Ok(()),
        };

        db.execute(
            "UPDATE cached_tracks SET tier = ?1 WHERE track_id = ?2",
            params![new_tier, track_id],
        )?;

        info!("Promoted track {} to {} tier", track_id, new_tier);
        Ok(())
    }

    /// Demote a track to a lower tier
    async fn demote_track(&self, track_id: &str) -> Result<(), CacheError> {
        let db = self.db.write().await;

        let current_tier: String = db.query_row(
            "SELECT tier FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
            |row| row.get(0),
        )?;

        let new_tier = match current_tier.as_str() {
            "hot" => "warm",
            "warm" => "cold",
            _ => return Ok(()),
        };

        db.execute(
            "UPDATE cached_tracks SET tier = ?1 WHERE track_id = ?2",
            params![new_tier, track_id],
        )?;

        info!("Demoted track {} to {} tier", track_id, new_tier);
        Ok(())
    }

    /// Get the tier's current size
    async fn get_tier_size(&self, tier: CacheTier) -> Result<u64, CacheError> {
        let db = self.db.read().await;
        let size: i64 = db.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM cached_tracks WHERE tier = ?1",
            params![tier.to_string().to_lowercase()],
            |row| row.get(0),
        )?;
        Ok(size as u64)
    }

    /// Evict tracks using LRU policy until we have enough space
    async fn evict_for_space(&self, tier: CacheTier, needed_bytes: u64) -> Result<(), CacheError> {
        let max_size = match tier {
            CacheTier::Hot => self.config.max_hot_size,
            CacheTier::Warm => self.config.max_warm_size,
            CacheTier::Cold => self.config.max_cold_size,
        };

        let current_size = self.get_tier_size(tier).await?;

        if current_size + needed_bytes <= max_size {
            return Ok(()); // No eviction needed
        }

        let bytes_to_free = current_size + needed_bytes - max_size;
        info!(
            "Need to free {} bytes from {} tier",
            format_bytes(bytes_to_free),
            tier
        );

        let db = self.db.write().await;

        // Get tracks ordered by eviction score (highest first)
        let tracks: Vec<(String, String, i64)> = db
            .prepare(
                "SELECT track_id, file_path, size_bytes FROM cached_tracks 
                 WHERE tier = ?1 
                 ORDER BY last_played ASC, play_count ASC, cached_at ASC",
            )?
            .query_map(params![tier.to_string().to_lowercase()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut freed = 0u64;

        for (track_id, file_path, size) in tracks {
            if freed >= bytes_to_free {
                break;
            }

            // Delete file
            if let Err(e) = fs::remove_file(&file_path).await {
                warn!("Failed to delete cached file {}: {}", file_path, e);
            }

            // Remove from database
            db.execute(
                "DELETE FROM cached_tracks WHERE track_id = ?1",
                params![track_id],
            )?;

            freed += size as u64;
            info!("Evicted track {} from {} tier", track_id, tier);
        }

        Ok(())
    }

    /// Remove a track from the cache
    pub async fn remove_track(&self, track_id: &str) -> Result<(), CacheError> {
        let db = self.db.write().await;

        // Get file path before removing
        let file_path: Option<String> = db
            .query_row(
                "SELECT file_path FROM cached_tracks WHERE track_id = ?1",
                params![track_id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(path) = file_path {
            // Delete file
            if let Err(e) = fs::remove_file(&path).await {
                warn!("Failed to delete cached file {}: {}", path, e);
            }
        }

        // Remove from database
        db.execute(
            "DELETE FROM cached_tracks WHERE track_id = ?1",
            params![track_id],
        )?;

        info!("Removed track {} from cache", track_id);
        Ok(())
    }

    /// Clear all cached content
    pub async fn clear_all(&self) -> Result<(), CacheError> {
        let db = self.db.write().await;

        // Get all file paths
        let paths: Vec<String> = db
            .prepare("SELECT file_path FROM cached_tracks")?
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        // Delete all files
        for path in paths {
            if let Err(e) = fs::remove_file(&path).await {
                warn!("Failed to delete cached file {}: {}", path, e);
            }
        }

        // Clear database
        db.execute("DELETE FROM cached_tracks", [])?;
        db.execute("DELETE FROM cache_events", [])?;
        db.execute("DELETE FROM prefetch_queue", [])?;

        info!("Cleared all cached content");
        self.refresh_stats().await?;

        Ok(())
    }

    /// Refresh statistics
    async fn refresh_stats(&self) -> Result<(), CacheError> {
        let db = self.db.read().await;
        let stats = Self::load_stats(&db)?;
        drop(db);

        let mut current_stats = self.stats.write().await;
        *current_stats = stats;

        Ok(())
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> CacheStats {
        self.stats.read().await.clone()
    }

    /// Generate file path for a new cache entry
    pub fn generate_cache_path(&self, track: &StreamTrack, quality: StreamQuality) -> PathBuf {
        let quality_dir = match quality {
            StreamQuality::Low => "low",
            StreamQuality::Medium => "medium",
            StreamQuality::High => "high",
            StreamQuality::Best => "best",
        };

        let source_dir = match track.source {
            StreamSource::YouTube => "youtube",
            StreamSource::Spotify => "spotify",
            StreamSource::Local => "local",
            StreamSource::Cached => "cached",
        };

        // Create safe filename from title
        let safe_title: String = track
            .title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        self.config
            .cache_dir
            .join(source_dir)
            .join(quality_dir)
            .join(format!("{}_{}.mp3", safe_title, track.id))
    }

    /// Add a track to the prefetch queue
    pub async fn queue_prefetch(
        &self,
        track_id: &str,
        source: StreamSource,
        title: &str,
        artist: &str,
        priority: i32,
    ) -> Result<(), CacheError> {
        if !self.config.prefetch_enabled {
            return Ok(());
        }

        // Don't queue if already cached
        if self.is_cached(track_id).await {
            return Ok(());
        }

        let db = self.db.write().await;
        let now = Utc::now().to_rfc3339();

        db.execute(
            "INSERT OR REPLACE INTO prefetch_queue (track_id, priority, added_at, source, title, artist) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                track_id,
                priority,
                now,
                source.to_string().to_lowercase(),
                title,
                artist
            ],
        )?;

        debug!(
            "Queued track {} for prefetch (priority {})",
            track_id, priority
        );
        Ok(())
    }

    /// Get the next tracks to prefetch
    pub async fn get_prefetch_queue(&self, limit: usize) -> Vec<PrefetchItem> {
        let db = self.db.read().await;

        db.prepare(
            "SELECT track_id, source, title, artist, priority FROM prefetch_queue 
             ORDER BY priority DESC, added_at ASC LIMIT ?1",
        )
        .and_then(|mut stmt| {
            let items = stmt
                .query_map(params![limit as i64], |row| {
                    Ok(PrefetchItem {
                        track_id: row.get(0)?,
                        source: match row.get::<_, String>(1)?.as_str() {
                            "youtube" => StreamSource::YouTube,
                            "spotify" => StreamSource::Spotify,
                            _ => StreamSource::Local,
                        },
                        title: row.get(2)?,
                        artist: row.get(3)?,
                        priority: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(items)
        })
        .unwrap_or_default()
    }

    /// Remove a track from the prefetch queue
    pub async fn remove_from_prefetch_queue(&self, track_id: &str) -> Result<(), CacheError> {
        let db = self.db.write().await;
        db.execute(
            "DELETE FROM prefetch_queue WHERE track_id = ?1",
            params![track_id],
        )?;
        Ok(())
    }
}

/// Item in the prefetch queue
#[derive(Debug, Clone)]
pub struct PrefetchItem {
    pub track_id: String,
    pub source: StreamSource,
    pub title: String,
    pub artist: String,
    pub priority: i32,
}

/// Cache error type
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Cache is full")]
    CacheFull,

    #[error("Track not found in cache: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_tier_defaults() {
        assert_eq!(CacheTier::Hot.default_max_size(), 100 * 1024 * 1024);
        assert_eq!(CacheTier::Warm.default_max_size(), 2 * 1024 * 1024 * 1024);
        assert_eq!(CacheTier::Cold.default_max_size(), 20 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_eviction_score() {
        let track = CachedTrack {
            track_id: "test".to_string(),
            source: StreamSource::YouTube,
            file_path: PathBuf::from("/test"),
            quality: StreamQuality::Medium,
            size_bytes: 1024,
            cached_at: Utc::now(),
            last_played: Some(Utc::now()),
            play_count: 10,
            tier: CacheTier::Warm,
            title: "Test".to_string(),
            artist: "Artist".to_string(),
        };

        // A recently played track with high play count should have low eviction score
        let score = track.eviction_score();
        assert!(score < 10.0);
    }

    #[tokio::test]
    async fn test_smart_cache_creation() {
        let dir = tempdir().unwrap();
        let config = CacheConfig {
            cache_dir: dir.path().to_path_buf(),
            ..Default::default()
        };

        let cache = SmartCache::new(config).await;
        assert!(cache.is_ok());
    }
}
