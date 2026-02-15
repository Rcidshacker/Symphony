//! Music Recommendations
//!
//! Recommendation engine for suggesting music based on various signals.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::app::Track;

/// Recommendation error
#[derive(Debug, thiserror::Error)]
pub enum RecommendationError {
    #[error("Not enough data for recommendations")]
    InsufficientData,

    #[error("No tracks available")]
    NoTracks,
}

/// Recommendation source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationSource {
    /// Similar to currently playing
    Similar,
    /// Based on listening history
    History,
    /// Based on context (time, activity)
    Context,
    /// Based on AI analysis
    AI,
    /// Random discovery
    Discovery,
    /// Based on favorites
    Favorites,
}

/// A single recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// The recommended track
    pub track: Track,
    /// Recommendation score (0.0 to 1.0)
    pub score: f32,
    /// Source of recommendation
    pub source: RecommendationSource,
    /// Reason for recommendation
    pub reason: String,
}

/// Recommendation engine
pub struct RecommendationEngine {
    /// Minimum score threshold
    min_score: f32,
    /// Weights for different signals
    weights: RecommendationWeights,
}

/// Weights for recommendation signals
#[derive(Debug, Clone)]
struct RecommendationWeights {
    artist_similarity: f32,
    genre_similarity: f32,
    recency: f32,
    diversity: f32,
}

impl Default for RecommendationWeights {
    fn default() -> Self {
        Self {
            artist_similarity: 0.3,
            genre_similarity: 0.3,
            recency: 0.2,
            diversity: 0.2,
        }
    }
}

impl Default for RecommendationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RecommendationEngine {
    /// Create a new recommendation engine
    pub fn new() -> Self {
        Self {
            min_score: 0.3,
            weights: RecommendationWeights::default(),
        }
    }

    /// Get recommendations based on current track
    pub fn get_similar(
        &self,
        current: &Track,
        library: &[Track],
        limit: usize,
    ) -> Vec<Recommendation> {
        let mut recommendations: Vec<Recommendation> = library
            .iter()
            .filter(|t| t.id != current.id)
            .map(|t| {
                let score = self.calculate_similarity(current, t);
                let reason = self.get_similarity_reason(current, t);
                (t.clone(), score, reason)
            })
            .filter(|(_, score, _)| *score >= self.min_score)
            .map(|(track, score, reason)| Recommendation {
                track,
                score,
                source: RecommendationSource::Similar,
                reason,
            })
            .collect();

        // Sort by score
        recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Apply diversity
        recommendations = self.apply_diversity(recommendations, limit);

        recommendations.into_iter().take(limit).collect()
    }

    /// Get recommendations based on listening history
    pub fn get_from_history(
        &self,
        history: &[Track],
        library: &[Track],
        limit: usize,
    ) -> Vec<Recommendation> {
        if history.is_empty() {
            return Vec::new();
        }

        // Count artist and genre occurrences
        let mut artist_counts: HashMap<String, u32> = HashMap::new();
        let mut genre_counts: HashMap<String, u32> = HashMap::new();

        for track in history {
            *artist_counts.entry(track.artist.to_lowercase()).or_insert(0) += 1;
            if let Some(ref genre) = track.genre {
                *genre_counts.entry(genre.to_lowercase()).or_insert(0) += 1;
            }
        }

        // Score library tracks
        let history_ids: HashSet<String> = history.iter().map(|t| t.id.clone()).collect();

        let mut recommendations: Vec<Recommendation> = library
            .iter()
            .filter(|t| !history_ids.contains(&t.id))
            .map(|t| {
                let artist_score = artist_counts
                    .get(&t.artist.to_lowercase())
                    .map(|&c| c as f32 / history.len() as f32)
                    .unwrap_or(0.0);

                let genre_score = t.genre
                    .as_ref()
                    .map(|g| genre_counts.get(&g.to_lowercase()).map(|&c| c as f32 / history.len() as f32).unwrap_or(0.0))
                    .unwrap_or(0.0);

                let score = artist_score * self.weights.artist_similarity
                    + genre_score * self.weights.genre_similarity;

                let reason = if artist_score > 0.0 {
                    format!("You've listened to {} before", t.artist)
                } else if genre_score > 0.0 {
                    format!("Matches your preferred genre")
                } else {
                    "Based on your history".to_string()
                };

                Recommendation {
                    track: t.clone(),
                    score,
                    source: RecommendationSource::History,
                    reason,
                }
            })
            .filter(|r| r.score >= self.min_score)
            .collect();

        recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        recommendations.into_iter().take(limit).collect()
    }

    /// Get discovery recommendations (random but weighted)
    pub fn get_discovery(
        &self,
        library: &[Track],
        played_ids: &HashSet<String>,
        limit: usize,
    ) -> Vec<Recommendation> {
        use rand::seq::SliceRandom;

        let unplayed: Vec<&Track> = library
            .iter()
            .filter(|t| !played_ids.contains(&t.id))
            .collect();

        if unplayed.is_empty() {
            return Vec::new();
        }

        let mut rng = rand::thread_rng();
        let mut selected: Vec<&Track> = unplayed
            .choose_multiple(&mut rng, limit * 2);

        selected.shuffle(&mut rng);

        selected
            .into_iter()
            .take(limit)
            .map(|t| Recommendation {
                track: t.clone(),
                score: 0.5,
                source: RecommendationSource::Discovery,
                reason: "Something new for you".to_string(),
            })
            .collect()
    }

    /// Calculate similarity between two tracks
    fn calculate_similarity(&self, a: &Track, b: &Track) -> f32 {
        let mut score = 0.0;

        // Same artist
        if a.artist.to_lowercase() == b.artist.to_lowercase() {
            score += self.weights.artist_similarity;
        }

        // Same genre
        if let (Some(ga), Some(gb)) = (&a.genre, &b.genre) {
            if ga.to_lowercase() == gb.to_lowercase() {
                score += self.weights.genre_similarity;
            }
        }

        // Similar era
        if let (Some(ya), Some(yb)) = (a.year, b.year) {
            let year_diff = (ya as i32 - yb as i32).abs();
            if year_diff <= 2 {
                score += 0.1;
            } else if year_diff <= 5 {
                score += 0.05;
            }
        }

        score.min(1.0)
    }

    /// Get reason for similarity
    fn get_similarity_reason(&self, current: &Track, other: &Track) -> String {
        if current.artist.to_lowercase() == other.artist.to_lowercase() {
            return format!("More from {}", current.artist);
        }

        if let (Some(ga), Some(gb)) = (&current.genre, &other.genre) {
            if ga.to_lowercase() == gb.to_lowercase() {
                return format!("Similar {} style", ga);
            }
        }

        if let (Some(ya), Some(yb)) = (current.year, other.year) {
            if (ya as i32 - yb as i32).abs() <= 5 {
                return "From the same era".to_string();
            }
        }

        "Similar vibes".to_string()
    }

    /// Apply diversity to recommendations
    fn apply_diversity(&self, mut recs: Vec<Recommendation>, limit: usize) -> Vec<Recommendation> {
        let mut result = Vec::new();
        let mut seen_artists: HashSet<String> = HashSet::new();
        let mut seen_genres: HashSet<String> = HashSet::new();

        for rec in recs.drain(..) {
            let artist_key = rec.track.artist.to_lowercase();
            let genre_key = rec.track.genre.as_ref().map(|g| g.to_lowercase()).unwrap_or_default();

            // Allow if we haven't seen too many from same artist/genre
            let artist_count = seen_artists.iter().filter(|a| **a == artist_key).count();
            let genre_count = seen_genres.iter().filter(|g| **g == genre_key).count();

            if artist_count < 2 && genre_count < 3 {
                seen_artists.insert(artist_key);
                if !genre_key.is_empty() {
                    seen_genres.insert(genre_key);
                }
                result.push(rec);

                if result.len() >= limit {
                    break;
                }
            }
        }

        result
    }

    /// Generate a mix of recommendations from all sources
    pub fn get_mixed(
        &self,
        current: Option<&Track>,
        history: &[Track],
        library: &[Track],
        limit: usize,
    ) -> Vec<Recommendation> {
        let mut all_recs = Vec::new();
        let played_ids: HashSet<String> = history.iter().map(|t| t.id.clone()).collect();

        // Similar to current (40%)
        if let Some(current_track) = current {
            let similar = self.get_similar(current_track, library, limit * 40 / 100);
            all_recs.extend(similar);
        }

        // From history (30%)
        let from_history = self.get_from_history(history, library, limit * 30 / 100);
        all_recs.extend(from_history);

        // Discovery (30%)
        let discovery = self.get_discovery(library, &played_ids, limit * 30 / 100);
        all_recs.extend(discovery);

        // Remove duplicates
        let mut seen_ids: HashSet<String> = HashSet::new();
        all_recs.retain(|r| seen_ids.insert(r.track.id.clone()));

        // Sort by score
        all_recs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        all_recs.into_iter().take(limit).collect()
    }
}

/// Quick recommendation helpers
impl RecommendationEngine {
    /// Get "more like this" recommendations
    pub fn more_like_this(track: &Track, library: &[Track], limit: usize) -> Vec<Recommendation> {
        Self::new().get_similar(track, library, limit)
    }

    /// Get daily mix recommendations
    pub fn daily_mix(history: &[Track], library: &[Track], limit: usize) -> Vec<Recommendation> {
        let engine = Self::new();
        engine.get_mixed(None, history, library, limit)
    }

    /// Get radio-style recommendations
    pub fn radio(seed: &Track, library: &[Track], limit: usize) -> Vec<Recommendation> {
        let engine = Self::new();
        engine.get_similar(seed, library, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_track(id: &str, artist: &str, genre: Option<&str>) -> Track {
        Track {
            id: id.to_string(),
            title: format!("Song {}", id),
            artist: artist.to_string(),
            album: "Album".to_string(),
            duration: 180.0,
            path: PathBuf::from("/test.mp3"),
            track_number: None,
            genre: genre.map(|s| s.to_string()),
            year: Some(2020),
            source: crate::app::TrackSource::Local,
        }
    }

    #[test]
    fn test_get_similar() {
        let engine = RecommendationEngine::new();

        let current = create_test_track("0", "Artist A", Some("rock"));
        let library = vec![
            current.clone(),
            create_test_track("1", "Artist A", Some("rock")),  // Same artist & genre
            create_test_track("2", "Artist B", Some("rock")),  // Same genre
            create_test_track("3", "Artist C", Some("pop")),   // Different
        ];

        let recs = engine.get_similar(&current, &library, 5);

        assert!(!recs.is_empty());
        assert!(recs[0].score >= recs[recs.len() - 1].score);
    }

    #[test]
    fn test_get_from_history() {
        let engine = RecommendationEngine::new();

        let history = vec![
            create_test_track("1", "Artist A", Some("rock")),
            create_test_track("2", "Artist A", Some("rock")),
            create_test_track("3", "Artist B", Some("pop")),
        ];

        let library = vec![
            create_test_track("4", "Artist A", Some("rock")),
            create_test_track("5", "Artist B", Some("pop")),
            create_test_track("6", "Artist C", Some("jazz")),
        ];

        let recs = engine.get_from_history(&history, &library, 5);

        // Artist A should be recommended first (appeared twice in history)
        assert!(!recs.is_empty());
    }

    #[test]
    fn test_get_discovery() {
        let engine = RecommendationEngine::new();

        let library = vec![
            create_test_track("1", "A", Some("rock")),
            create_test_track("2", "B", Some("pop")),
            create_test_track("3", "C", Some("jazz")),
        ];

        let played = HashSet::new();
        let recs = engine.get_discovery(&library, &played, 2);

        assert!(recs.len() <= 2);
        assert!(recs.iter().all(|r| r.source == RecommendationSource::Discovery));
    }
}
