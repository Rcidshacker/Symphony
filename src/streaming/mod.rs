//! Streaming module (Phase 4)
//!
//! This module provides streaming capabilities for Symphony, enabling playback
//! from YouTube and Spotify in addition to local files.
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │          StreamManager (Facade)                 │
//! ├─────────────────────────────────────────────────┤
//! │  ┌─────────────────────────────────────┐       │
//! │  │   StreamProvider (Trait)            │       │
//! │  └─────────────────────────────────────┘       │
//! │           ↑              ↑                      │
//! │           │              │                      │
//! │  ┌────────────┐  ┌──────────────┐              │
//! │  │  YouTube   │  │   Spotify    │              │
//! │  │ (yt-dlp)   │  │  (Web API)   │              │
//! │  └────────────┘  └──────────────┘              │
//! │                                                  │
//! │  ┌─────────────────────────────────┐           │
//! │  │      SmartCache                 │           │
//! │  │  - Hot tier (SSD, 100MB)        │           │
//! │  │  - Warm tier (DB, 2GB)          │           │
//! │  │  - Cold tier (HDD, 20GB)        │           │
//! │  │  - ML-based prediction          │           │
//! │  └─────────────────────────────────┘           │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//! ```rust,no_run
//! use symphony::streaming::{StreamManager, StreamManagerConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create stream manager
//!     let config = StreamManagerConfig::default();
//!     let manager = StreamManager::new(config).await?;
//!
//!     // Search all sources
//!     let results = manager.search_all("pink floyd", 20).await?;
//!     
//!     // Play a track
//!     if let Some(track) = results.tracks.first() {
//!         let play_result = manager.play_track(track).await?;
//!         println!("Playing from: {}", play_result.url);
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! # Features
//! - **YouTube**: Full streaming via yt-dlp, supports download and caching
//! - **Spotify**: Search and metadata, 30-second previews via Web API
//! - **Smart Caching**: Multi-tier LRU cache with automatic promotion/demotion
//! - **Predictive Prefetching**: ML-based prediction of what to play next

pub mod cache;
pub mod manager;
pub mod predictor;
pub mod provider;
pub mod spotify;
pub mod youtube;

// Re-export main types
pub use cache::{CacheConfig, CacheStats, CacheTier, CachedTrack, SmartCache};
pub use manager::{
    CombinedSearchResult, DownloadStatus, DownloadTask, PlayResult, StreamManager,
    StreamManagerConfig,
};
pub use predictor::{
    Prediction, PredictionReason, PredictiveCache, PredictorConfig, PredictorStats,
};
pub use provider::{
    DownloadProgress, DownloadStatus as ProviderDownloadStatus, SearchResult, StreamError,
    StreamProvider, StreamQuality, StreamSource, StreamTrack,
};
pub use spotify::{SpotifyConfig, SpotifyProvider};
pub use youtube::{YouTubeConfig, YouTubeProvider};
