//! Smart Playlists
//!
//! AI-generated playlists based on various criteria.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai::provider::MusicIntent;
use crate::app::Track;

/// Playlist generation error
#[derive(Debug, thiserror::Error)]
pub enum PlaylistError {
    #[error("Not enough tracks: {0} available, {1} needed")]
    NotEnoughTracks(usize, usize),

    #[error("AI error: {0}")]
    AIError(String),

    #[error("Invalid criteria: {0}")]
    InvalidCriteria(String),
}

/// Smart playlist criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistCriteria {
    /// Name for the playlist
    pub name: Option<String>,
    /// Genre filter
    pub genre: Option<String>,
    /// Mood filter
    pub mood: Option<String>,
    /// Artist filter (must include at least one)
    pub artists: Vec<String>,
    /// Exclude artists
    pub exclude_artists: Vec<String>,
    /// Minimum duration in minutes
    pub min_duration_mins: Option<u32>,
    /// Maximum duration in minutes
    pub max_duration_mins: Option<u32>,
    /// Target number of tracks
    pub target_tracks: Option<usize>,
    /// Tempo preference
    pub tempo: Option<TempoPreference>,
    /// Era filter
    pub era: Option<String>,
    /// Only favorites
    pub favorites_only: bool,
    /// Minimum rating (if ratings are tracked)
    pub min_rating: Option<u8>,
}

impl Default for PlaylistCriteria {
    fn default() -> Self {
        Self {
            name: None,
            genre: None,
            mood: None,
            artists: Vec::new(),
            exclude_artists: Vec::new(),
            min_duration_mins: None,
            max_duration_mins: None,
            target_tracks: Some(20),
            tempo: None,
            era: None,
            favorites_only: false,
            min_rating: None,
        }
    }
}

impl PlaylistCriteria {
    /// Create from MusicIntent
    pub fn from_intent(intent: &MusicIntent) -> Self {
        match intent {
            MusicIntent::Play {
                genre,
                mood,
                artist,
                tempo,
                era,
                duration_mins,
            } => Self {
                name: None,
                genre: genre.clone(),
                mood: mood.clone(),
                artists: artist.iter().cloned().collect(),
                tempo: tempo.as_ref().and_then(|t| TempoPreference::from_str(t)),
                era: era.clone(),
                target_tracks: duration_mins.map(|_| 20),
                ..Default::default()
            },
            MusicIntent::CreatePlaylist {
                theme,
                duration_mins,
            } => Self {
                name: Some(theme.clone()),
                max_duration_mins: Some(*duration_mins),
                target_tracks: None,
                ..Default::default()
            },
            _ => Self::default(),
        }
    }
}

/// Tempo preference
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TempoPreference {
    Slow,
    Medium,
    Fast,
}

impl TempoPreference {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "slow" | "calm" | "chill" => Some(Self::Slow),
            "fast" | "energetic" | "upbeat" => Some(Self::Fast),
            "medium" | "moderate" => Some(Self::Medium),
            _ => None,
        }
    }
}

/// Mood to genre mapping
pub fn mood_to_genres(mood: &str) -> Vec<&'static str> {
    match mood.to_lowercase().as_str() {
        "happy" | "joyful" => vec!["pop", "electronic", "indie", "rock"],
        "sad" | "melancholic" => vec!["indie", "alternative", "acoustic", "blues"],
        "energetic" | "pumped" => vec!["rock", "electronic", "hip-hop", "metal"],
        "calm" | "relaxed" => vec!["ambient", "jazz", "classical", "acoustic"],
        "focus" | "concentrate" => vec!["ambient", "classical", "lo-fi", "electronic"],
        "romantic" | "love" => vec!["r&b", "soul", "jazz", "pop"],
        "angry" | "aggressive" => vec!["metal", "rock", "punk", "hip-hop"],
        "party" | "dance" => vec!["electronic", "pop", "hip-hop", "dance"],
        "chill" | "laid-back" => vec!["lo-fi", "chill", "ambient", "jazz"],
        "epic" | "dramatic" => vec!["classical", "soundtrack", "rock", "electronic"],
        _ => vec!["pop", "rock", "electronic", "indie"],
    }
}

/// Era to year range mapping
pub fn era_to_years(era: &str) -> Option<(u32, u32)> {
    match era.to_lowercase().as_str() {
        "60s" | "sixties" => Some((1960, 1969)),
        "70s" | "seventies" => Some((1970, 1979)),
        "80s" | "eighties" => Some((1980, 1989)),
        "90s" | "nineties" => Some((1990, 1999)),
        "2000s" | "two-thousands" => Some((2000, 2009)),
        "2010s" => Some((2010, 2019)),
        "2020s" | "recent" | "modern" => Some((2020, 2030)),
        "classic" | "oldies" => Some((1950, 1989)),
        _ => None,
    }
}

/// Smart playlist generator
pub struct SmartPlaylistGenerator {
    /// Weight factors for scoring
    genre_weight: f32,
    mood_weight: f32,
    recency_weight: f32,
}

impl Default for SmartPlaylistGenerator {
    fn default() -> Self {
        Self {
            genre_weight: 0.4,
            mood_weight: 0.3,
            recency_weight: 0.1,
        }
    }
}

impl SmartPlaylistGenerator {
    /// Create new generator
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a playlist from tracks
    pub fn generate(
        &self,
        tracks: &[Track],
        criteria: &PlaylistCriteria,
    ) -> Result<SmartPlaylist, PlaylistError> {
        // Filter tracks based on criteria
        let mut candidates: Vec<(&Track, f32)> = tracks
            .iter()
            .filter(|t| self.matches_criteria(t, criteria))
            .map(|t| (t, self.score_track(t, criteria)))
            .collect();

        if candidates.is_empty() {
            return Err(PlaylistError::NotEnoughTracks(
                0,
                criteria.target_tracks.unwrap_or(1),
            ));
        }

        // Sort by score
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Select tracks
        let mut selected_tracks: Vec<Track> = Vec::new();
        let mut total_duration = 0.0;
        let max_duration = criteria.max_duration_mins.map(|m| m as f64 * 60.0);
        let target_count = criteria.target_tracks.unwrap_or(20);

        for (track, _score) in candidates {
            // Check duration limit
            if let Some(max) = max_duration {
                if total_duration + track.duration > max {
                    break;
                }
            }

            // Check track count
            if selected_tracks.len() >= target_count {
                break;
            }

            total_duration += track.duration;
            selected_tracks.push(track.clone());
        }

        // Create playlist
        let name = criteria.name.clone().unwrap_or_else(|| {
            format!(
                "Smart Playlist - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M")
            )
        });

        Ok(SmartPlaylist {
            id: Uuid::new_v4().to_string(),
            name,
            tracks: selected_tracks,
            total_duration,
            criteria: criteria.clone(),
            created_at: chrono::Utc::now(),
        })
    }

    /// Check if a track matches the criteria
    fn matches_criteria(&self, track: &Track, criteria: &PlaylistCriteria) -> bool {
        // Genre filter
        if let Some(ref genre) = criteria.genre {
            if !track
                .genre
                .as_ref()
                .map(|g| g.to_lowercase().contains(&genre.to_lowercase()))
                .unwrap_or(false)
            {
                return false;
            }
        }

        // Artist filter
        if !criteria.artists.is_empty() {
            let matches = criteria
                .artists
                .iter()
                .any(|a| track.artist.to_lowercase().contains(&a.to_lowercase()));
            if !matches {
                return false;
            }
        }

        // Exclude artists
        if !criteria.exclude_artists.is_empty() {
            let excluded = criteria
                .exclude_artists
                .iter()
                .any(|a| track.artist.to_lowercase().contains(&a.to_lowercase()));
            if excluded {
                return false;
            }
        }

        // Era filter
        if let Some(ref era) = criteria.era {
            if let Some((start, end)) = era_to_years(era) {
                if let Some(year) = track.year {
                    if year < start || year > end {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Score a track based on how well it matches criteria
    fn score_track(&self, track: &Track, criteria: &PlaylistCriteria) -> f32 {
        let mut score = 0.0;

        // Genre match bonus
        if let Some(ref genre) = criteria.genre {
            if track
                .genre
                .as_ref()
                .map(|g| g.to_lowercase().contains(&genre.to_lowercase()))
                .unwrap_or(false)
            {
                score += self.genre_weight;
            }
        }

        // Mood-based genre bonus
        if let Some(ref mood) = criteria.mood {
            let mood_genres = mood_to_genres(mood);
            if let Some(ref track_genre) = track.genre {
                if mood_genres
                    .iter()
                    .any(|g| track_genre.to_lowercase().contains(&g.to_lowercase()))
                {
                    score += self.mood_weight;
                }
            }
        }

        score
    }

    /// Generate playlist from mood
    pub fn from_mood(
        tracks: &[Track],
        mood: &str,
        duration_mins: u32,
    ) -> Result<SmartPlaylist, PlaylistError> {
        let criteria = PlaylistCriteria {
            mood: Some(mood.to_string()),
            genre: mood_to_genres(mood).first().map(|s| s.to_string()),
            max_duration_mins: Some(duration_mins),
            ..Default::default()
        };

        Self::new().generate(tracks, &criteria)
    }

    /// Generate focus playlist
    pub fn focus_playlist(
        tracks: &[Track],
        duration_mins: u32,
    ) -> Result<SmartPlaylist, PlaylistError> {
        let criteria = PlaylistCriteria {
            name: Some("Focus Session".to_string()),
            mood: Some("focus".to_string()),
            genre: Some("ambient".to_string()),
            tempo: Some(TempoPreference::Slow),
            max_duration_mins: Some(duration_mins),
            ..Default::default()
        };

        Self::new().generate(tracks, &criteria)
    }

    /// Generate workout playlist
    pub fn workout_playlist(
        tracks: &[Track],
        duration_mins: u32,
    ) -> Result<SmartPlaylist, PlaylistError> {
        let criteria = PlaylistCriteria {
            name: Some("Workout Mix".to_string()),
            mood: Some("energetic".to_string()),
            tempo: Some(TempoPreference::Fast),
            max_duration_mins: Some(duration_mins),
            ..Default::default()
        };

        Self::new().generate(tracks, &criteria)
    }
}

/// Generated smart playlist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartPlaylist {
    /// Unique ID
    pub id: String,
    /// Playlist name
    pub name: String,
    /// Tracks in playlist
    pub tracks: Vec<Track>,
    /// Total duration in seconds
    pub total_duration: f64,
    /// Criteria used to generate
    pub criteria: PlaylistCriteria,
    /// When created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SmartPlaylist {
    /// Get total duration formatted
    pub fn duration_formatted(&self) -> String {
        let mins = (self.total_duration / 60.0).floor() as i32;
        let secs = (self.total_duration % 60.0) as i32;
        format!("{}:{:02}", mins, secs)
    }

    /// Get track count
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_track(id: &str, title: &str, artist: &str, genre: &str) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: "Test Album".to_string(),
            duration: 180.0,
            path: PathBuf::from("/test.mp3"),
            track_number: None,
            genre: Some(genre.to_string()),
            year: Some(2020),
            source: crate::app::TrackSource::Local,
        }
    }

    #[test]
    fn test_mood_to_genres() {
        let genres = mood_to_genres("happy");
        assert!(genres.contains(&"pop"));

        let genres = mood_to_genres("focus");
        assert!(genres.contains(&"ambient"));
    }

    #[test]
    fn test_era_to_years() {
        assert_eq!(era_to_years("80s"), Some((1980, 1989)));
        assert_eq!(era_to_years("2020s"), Some((2020, 2030)));
        assert_eq!(era_to_years("invalid"), None);
    }

    #[test]
    fn test_generate_playlist() {
        let tracks = vec![
            create_test_track("1", "Song 1", "Artist A", "rock"),
            create_test_track("2", "Song 2", "Artist B", "pop"),
            create_test_track("3", "Song 3", "Artist C", "rock"),
        ];

        let criteria = PlaylistCriteria {
            genre: Some("rock".to_string()),
            target_tracks: Some(10),
            ..Default::default()
        };

        let generator = SmartPlaylistGenerator::new();
        let playlist = generator.generate(&tracks, &criteria).unwrap();

        assert_eq!(playlist.track_count(), 2);
    }

    #[test]
    fn test_playlist_from_mood() {
        let tracks = vec![
            create_test_track("1", "Happy Song", "Artist A", "pop"),
            create_test_track("2", "Sad Song", "Artist B", "indie"),
        ];

        let playlist = SmartPlaylistGenerator::from_mood(&tracks, "happy", 30);
        assert!(playlist.is_ok());
    }
}
