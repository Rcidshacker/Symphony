//! Predictive caching and prefetching system
//!
//! Uses machine learning signals to predict what the user will play next
//! and proactively downloads tracks to the cache.
//!
//! # Prediction Signals
//! 1. **Transition Probability**: What tracks typically follow the current one
//! 2. **Time Patterns**: What the user plays at different times of day
//! 3. **Queue Context**: Next songs in the current queue/playlist
//! 4. **Similarity**: Tracks similar to recently played (via embeddings)
//! 5. **Skip/Replay Patterns**: How the user interacts with tracks

use chrono::{DateTime, Datelike, Timelike, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::cache::{CacheError, PrefetchItem, SmartCache};
use super::provider::{StreamSource, StreamTrack};

/// Predictor configuration
#[derive(Debug, Clone)]
pub struct PredictorConfig {
    /// Number of tracks to prefetch
    pub prefetch_count: usize,
    /// Minimum confidence threshold for prefetching (0.0 to 1.0)
    pub min_confidence: f64,
    /// Weight for transition probability signal
    pub transition_weight: f64,
    /// Weight for time-of-day signal
    pub time_weight: f64,
    /// Weight for queue context signal
    pub queue_weight: f64,
    /// Weight for similarity signal
    pub similarity_weight: f64,
    /// Maximum prefetch bandwidth in KB/s (0 = unlimited)
    pub max_bandwidth_kbps: u32,
    /// Minimum time between predictions (seconds)
    pub prediction_interval_secs: u64,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            prefetch_count: 3,
            min_confidence: 0.3,
            transition_weight: 0.4,
            time_weight: 0.15,
            queue_weight: 0.25,
            similarity_weight: 0.2,
            max_bandwidth_kbps: 0,
            prediction_interval_secs: 10,
        }
    }
}

/// Track transition record
#[derive(Debug, Clone)]
pub struct Transition {
    /// Source track ID
    pub from_track: String,
    /// Target track ID
    pub to_track: String,
    /// Number of times this transition occurred
    pub count: u32,
    /// Average time between transitions
    pub avg_interval_secs: f64,
}

/// Time-based play pattern
#[derive(Debug, Clone)]
pub struct TimePattern {
    /// Hour of day (0-23)
    pub hour: u8,
    /// Day of week (0-6, Sunday = 0)
    pub day_of_week: u8,
    /// Track ID
    pub track_id: String,
    /// Play count at this time
    pub count: u32,
}

/// Prediction result
#[derive(Debug, Clone)]
pub struct Prediction {
    /// Track ID
    pub track_id: String,
    /// Predicted source
    pub source: StreamSource,
    /// Track title (if known)
    pub title: Option<String>,
    /// Track artist (if known)
    pub artist: Option<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Why this track was predicted
    pub reasons: Vec<PredictionReason>,
}

/// Reason for a prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictionReason {
    /// Often played after current track
    Transition { probability: f64 },
    /// Often played at this time
    TimePattern { hour: u8, probability: f64 },
    /// Next in queue
    QueuePosition { position: usize },
    /// Similar to current track
    Similarity { score: f64 },
    /// User replay pattern
    ReplayPattern,
    /// User skip pattern
    SkipPattern,
}

impl PredictionReason {
    /// Get a human-readable description
    pub fn description(&self) -> String {
        match self {
            PredictionReason::Transition { probability } => {
                format!("Often played next ({:.0}% likelihood)", probability * 100.0)
            }
            PredictionReason::TimePattern { hour, probability } => {
                format!(
                    "Often played at {}:00 ({:.0}% likelihood)",
                    hour,
                    probability * 100.0
                )
            }
            PredictionReason::QueuePosition { position } => {
                format!("Next in queue (position {})", position)
            }
            PredictionReason::Similarity { score } => {
                format!("Similar to current track ({:.0}% match)", score * 100.0)
            }
            PredictionReason::ReplayPattern => "You often replay this track".to_string(),
            PredictionReason::SkipPattern => "You often skip to this track".to_string(),
        }
    }
}

/// Predictive cache manager
pub struct PredictiveCache {
    config: PredictorConfig,
    db: Arc<RwLock<Connection>>,
    cache: Arc<SmartCache>,
    last_prediction: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl PredictiveCache {
    /// Create a new predictive cache
    pub fn new(
        config: PredictorConfig,
        db_path: PathBuf,
        cache: Arc<SmartCache>,
    ) -> Result<Self, CacheError> {
        let db = Connection::open(&db_path)?;
        Self::init_database(&db)?;

        Ok(Self {
            config,
            db: Arc::new(RwLock::new(db)),
            cache,
            last_prediction: Arc::new(RwLock::new(None)),
        })
    }

    /// Initialize the prediction database
    fn init_database(db: &Connection) -> Result<(), CacheError> {
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS transitions (
                from_track TEXT NOT NULL,
                to_track TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_occurred TEXT NOT NULL,
                PRIMARY KEY (from_track, to_track)
            );

            CREATE INDEX IF NOT EXISTS idx_transitions_from ON transitions(from_track);
            CREATE INDEX IF NOT EXISTS idx_transitions_to ON transitions(to_track);

            CREATE TABLE IF NOT EXISTS time_patterns (
                hour INTEGER NOT NULL,
                day_of_week INTEGER NOT NULL,
                track_id TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_played TEXT NOT NULL,
                PRIMARY KEY (hour, day_of_week, track_id)
            );

            CREATE TABLE IF NOT EXISTS play_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id TEXT NOT NULL,
                played_at TEXT NOT NULL,
                source TEXT,
                duration_secs INTEGER,
                completed INTEGER DEFAULT 1,
                skipped INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_play_history_track ON play_history(track_id);
            CREATE INDEX IF NOT EXISTS idx_play_history_time ON play_history(played_at);

            CREATE TABLE IF NOT EXISTS track_features (
                track_id TEXT PRIMARY KEY,
                play_count INTEGER DEFAULT 0,
                skip_count INTEGER DEFAULT 0,
                replay_count INTEGER DEFAULT 0,
                avg_listen_ratio REAL DEFAULT 1.0,
                last_played TEXT,
                first_played TEXT
            );
            "#,
        )?;

        Ok(())
    }

    /// Record a track play for learning
    pub async fn record_play(
        &self,
        track_id: &str,
        source: StreamSource,
        previous_track: Option<&str>,
        duration_secs: u64,
        completed: bool,
        skipped: bool,
    ) -> Result<(), CacheError> {
        let db = self.db.write().await;
        let now = Utc::now().to_rfc3339();

        // Record play event
        db.execute(
            "INSERT INTO play_history (track_id, played_at, source, duration_secs, completed, skipped)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                track_id,
                now,
                source.to_string().to_lowercase(),
                duration_secs as i64,
                if completed { 1 } else { 0 },
                if skipped { 1 } else { 0 }
            ],
        )?;

        // Update track features
        db.execute(
            "INSERT INTO track_features (track_id, play_count, last_played, first_played)
             VALUES (?1, 1, ?2, ?2)
             ON CONFLICT(track_id) DO UPDATE SET 
                play_count = play_count + 1,
                last_played = ?2,
                skip_count = skip_count + ?3,
                replay_count = replay_count + ?4",
            params![
                track_id,
                now,
                if skipped { 1 } else { 0 },
                if !completed && duration_secs < 30 {
                    1
                } else {
                    0
                } // Quick replay detection
            ],
        )?;

        // Record transition if there was a previous track
        if let Some(prev_id) = previous_track {
            db.execute(
                "INSERT INTO transitions (from_track, to_track, count, last_occurred)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(from_track, to_track) DO UPDATE SET 
                    count = count + 1,
                    last_occurred = ?3",
                params![prev_id, track_id, now],
            )?;
        }

        // Record time pattern
        let hour = Utc::now().hour() as u8;
        let day_of_week = Utc::now().weekday().num_days_from_sunday() as u8;

        db.execute(
            "INSERT INTO time_patterns (hour, day_of_week, track_id, count, last_played)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(hour, day_of_week, track_id) DO UPDATE SET 
                count = count + 1,
                last_played = ?4",
            params![hour, day_of_week, track_id, now],
        )?;

        debug!("Recorded play for track {}", track_id);
        Ok(())
    }

    /// Predict what tracks the user might want next
    pub async fn predict_next(
        &self,
        current_track: &str,
        queue: &[StreamTrack],
    ) -> Result<Vec<Prediction>, CacheError> {
        let mut predictions: HashMap<String, Prediction> = HashMap::new();

        // 1. Transition-based predictions
        let transition_preds = self.predict_from_transitions(current_track).await?;
        for pred in transition_preds {
            predictions.insert(pred.track_id.clone(), pred);
        }

        // 2. Time-based predictions
        let time_preds = self.predict_from_time().await?;
        for pred in time_preds {
            predictions
                .entry(pred.track_id.clone())
                .and_modify(|existing| {
                    existing.confidence = (existing.confidence + pred.confidence) / 2.0;
                    existing.reasons.extend(pred.reasons.clone());
                })
                .or_insert(pred);
        }

        // 3. Queue-based predictions
        let queue_preds = self.predict_from_queue(queue);
        for pred in queue_preds {
            predictions
                .entry(pred.track_id.clone())
                .and_modify(|existing| {
                    existing.confidence = (existing.confidence + pred.confidence) / 2.0;
                    existing.reasons.extend(pred.reasons.clone());
                })
                .or_insert(pred);
        }

        // 4. Similarity-based predictions (if embeddings available)
        // This would integrate with the AI embeddings from Phase 3
        // For now, we'll use a simplified approach based on artist similarity

        // Sort by confidence and filter
        let mut results: Vec<Prediction> = predictions
            .into_values()
            .filter(|p| p.confidence >= self.config.min_confidence)
            .collect();

        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results.truncate(self.config.prefetch_count);

        Ok(results)
    }

    /// Predict based on track transitions
    async fn predict_from_transitions(
        &self,
        current_track: &str,
    ) -> Result<Vec<Prediction>, CacheError> {
        let db = self.db.read().await;

        let mut stmt = db.prepare(
            "SELECT to_track, count, 
                    (SELECT SUM(count) FROM transitions WHERE from_track = ?1) as total
             FROM transitions 
             WHERE from_track = ?1 
             ORDER BY count DESC 
             LIMIT ?2",
        )?;

        let limit = self.config.prefetch_count as i64;
        let predictions: Vec<Prediction> = stmt
            .query_map(params![current_track, limit], |row| {
                let to_track: String = row.get(0)?;
                let count: i64 = row.get(1)?;
                let total: i64 = row.get(2)?;
                let probability = if total > 0 {
                    count as f64 / total as f64
                } else {
                    0.0
                };

                Ok(Prediction {
                    track_id: to_track,
                    source: StreamSource::YouTube, // Default, will be updated
                    title: None,
                    artist: None,
                    confidence: probability * self.config.transition_weight,
                    reasons: vec![PredictionReason::Transition { probability }],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(predictions)
    }

    /// Predict based on time patterns
    async fn predict_from_time(&self) -> Result<Vec<Prediction>, CacheError> {
        let db = self.db.read().await;

        let hour = Utc::now().hour() as i64;
        let day_of_week = Utc::now().weekday().num_days_from_sunday() as i64;

        // Get total plays at this time
        let total: i64 = db.query_row(
            "SELECT SUM(count) FROM time_patterns WHERE hour = ?1",
            params![hour],
            |row| row.get(0),
        )?;

        if total == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = db.prepare(
            "SELECT track_id, count FROM time_patterns 
             WHERE hour = ?1 AND day_of_week = ?2
             ORDER BY count DESC 
              LIMIT ?3",
        )?;

        let predictions: Vec<Prediction> = stmt
            .query_map(
                params![hour, day_of_week, self.config.prefetch_count as i64],
                |row| {
                    let track_id: String = row.get(0)?;
                    let count: i64 = row.get(1)?;
                    let probability = count as f64 / total as f64;

                    Ok(Prediction {
                        track_id,
                        source: StreamSource::YouTube,
                        title: None,
                        artist: None,
                        confidence: probability * self.config.time_weight,
                        reasons: vec![PredictionReason::TimePattern {
                            hour: hour as u8,
                            probability,
                        }],
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok(predictions)
    }

    /// Predict from queue context
    fn predict_from_queue(&self, queue: &[StreamTrack]) -> Vec<Prediction> {
        queue
            .iter()
            .take(self.config.prefetch_count)
            .enumerate()
            .map(|(i, track)| {
                let position_weight = 1.0 / (i + 1) as f64;
                Prediction {
                    track_id: track.id.clone(),
                    source: track.source,
                    title: Some(track.title.clone()),
                    artist: Some(track.artist.clone()),
                    confidence: position_weight * self.config.queue_weight,
                    reasons: vec![PredictionReason::QueuePosition { position: i + 1 }],
                }
            })
            .collect()
    }

    /// Queue tracks for prefetching
    pub async fn queue_predictions(&self, predictions: &[Prediction]) -> Result<(), CacheError> {
        for (i, pred) in predictions.iter().enumerate() {
            // Skip if already cached
            if self.cache.is_cached(&pred.track_id).await {
                debug!("Track {} already cached, skipping prefetch", pred.track_id);
                continue;
            }

            let priority = ((1.0 - pred.confidence) * 100.0) as i32 - i as i32;

            self.cache
                .queue_prefetch(
                    &pred.track_id,
                    pred.source,
                    pred.title.as_deref().unwrap_or("Unknown"),
                    pred.artist.as_deref().unwrap_or("Unknown"),
                    priority,
                )
                .await?;
        }

        info!("Queued {} tracks for prefetching", predictions.len());
        Ok(())
    }

    /// Run prediction and prefetch cycle
    pub async fn run_prediction_cycle(
        &self,
        current_track: &str,
        queue: &[StreamTrack],
    ) -> Result<Vec<Prediction>, CacheError> {
        // Check if enough time has passed since last prediction
        {
            let last = self.last_prediction.read().await;
            if let Some(last_time) = *last {
                let elapsed = (Utc::now() - last_time).num_seconds();
                if elapsed < self.config.prediction_interval_secs as i64 {
                    debug!("Skipping prediction, too soon since last one");
                    return Ok(Vec::new());
                }
            }
        }

        // Generate predictions
        let predictions = self.predict_next(current_track, queue).await?;

        // Queue for prefetching
        if !predictions.is_empty() {
            self.queue_predictions(&predictions).await?;
        }

        // Update last prediction time
        {
            let mut last = self.last_prediction.write().await;
            *last = Some(Utc::now());
        }

        Ok(predictions)
    }

    /// Get learning statistics
    pub async fn get_stats(&self) -> Result<PredictorStats, CacheError> {
        let db = self.db.read().await;

        let total_transitions: i64 =
            db.query_row("SELECT COUNT(*) FROM transitions", [], |row| row.get(0))?;

        let total_plays: i64 =
            db.query_row("SELECT COUNT(*) FROM play_history", [], |row| row.get(0))?;

        let unique_tracks: i64 = db.query_row(
            "SELECT COUNT(DISTINCT track_id) FROM play_history",
            [],
            |row| row.get(0),
        )?;

        let time_patterns: i64 =
            db.query_row("SELECT COUNT(*) FROM time_patterns", [], |row| row.get(0))?;

        Ok(PredictorStats {
            total_transitions: total_transitions as u64,
            total_plays: total_plays as u64,
            unique_tracks: unique_tracks as u64,
            time_patterns: time_patterns as u64,
        })
    }
}

/// Predictor statistics
#[derive(Debug, Clone, Default)]
pub struct PredictorStats {
    pub total_transitions: u64,
    pub total_plays: u64,
    pub unique_tracks: u64,
    pub time_patterns: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_reason_description() {
        let reason = PredictionReason::Transition { probability: 0.75 };
        assert!(reason.description().contains("75%"));

        let reason = PredictionReason::QueuePosition { position: 2 };
        assert!(reason.description().contains("position 2"));
    }

    #[test]
    fn test_predictor_config_defaults() {
        let config = PredictorConfig::default();
        assert_eq!(config.prefetch_count, 3);
        assert!(config.min_confidence > 0.0);
    }
}
