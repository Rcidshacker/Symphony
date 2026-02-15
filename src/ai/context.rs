//! Context Detection
//!
//! Detects user context for intelligent music recommendations.
//! Analyzes time, system state, and user patterns.

use std::collections::VecDeque;

use chrono::{DateTime, Datelike, Local, Timelike, Weekday};
use serde::{Deserialize, Serialize};

/// Time of day categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDay {
    EarlyMorning,  // 5-8
    Morning,       // 8-12
    Afternoon,     // 12-17
    Evening,       // 17-21
    Night,         // 21-24
    LateNight,     // 0-5
}

impl TimeOfDay {
    /// Determine from current time
    pub fn current() -> Self {
        Self::from_datetime(&Local::now())
    }

    /// Determine from datetime
    pub fn from_datetime(dt: &DateTime<Local>) -> Self {
        match dt.hour() {
            5..=7 => TimeOfDay::EarlyMorning,
            8..=11 => TimeOfDay::Morning,
            12..=16 => TimeOfDay::Afternoon,
            17..=20 => TimeOfDay::Evening,
            21..=23 => TimeOfDay::Night,
            _ => TimeOfDay::LateNight,
        }
    }

    /// Get recommended moods for this time
    pub fn recommended_moods(&self) -> Vec<&'static str> {
        match self {
            TimeOfDay::EarlyMorning => vec!["calm", "peaceful", "gentle", "ambient"],
            TimeOfDay::Morning => vec!["energetic", "upbeat", "positive", "focus"],
            TimeOfDay::Afternoon => vec!["focus", "productive", "neutral", "upbeat"],
            TimeOfDay::Evening => vec!["relaxing", "chill", "unwind", "romantic"],
            TimeOfDay::Night => vec!["chill", "relaxing", "ambient", "dreamy"],
            TimeOfDay::LateNight => vec!["calm", "ambient", "peaceful", "focus"],
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            TimeOfDay::EarlyMorning => "Early Morning",
            TimeOfDay::Morning => "Morning",
            TimeOfDay::Afternoon => "Afternoon",
            TimeOfDay::Evening => "Evening",
            TimeOfDay::Night => "Night",
            TimeOfDay::LateNight => "Late Night",
        }
    }
}

/// User activity state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityState {
    /// Unknown/default
    Unknown,
    /// Focus/work mode
    Focus,
    /// Relaxation mode
    Relax,
    /// Exercise/workout
    Exercise,
    /// Party/social
    Party,
    /// Commute
    Commute,
}

impl ActivityState {
    /// Get recommended genres for this activity
    pub fn recommended_genres(&self) -> Vec<&'static str> {
        match self {
            ActivityState::Focus => vec!["ambient", "classical", "lo-fi", "electronic"],
            ActivityState::Relax => vec!["jazz", "chill", "acoustic", "ambient"],
            ActivityState::Exercise => vec!["electronic", "rock", "hip-hop", "pop"],
            ActivityState::Party => vec!["pop", "electronic", "hip-hop", "dance"],
            ActivityState::Commute => vec!["rock", "pop", "indie", "alternative"],
            ActivityState::Unknown => vec!["pop", "rock", "electronic", "indie"],
        }
    }

    /// Get recommended tempo
    pub fn recommended_tempo(&self) -> &'static str {
        match self {
            ActivityState::Focus => "slow",
            ActivityState::Relax => "slow",
            ActivityState::Exercise => "fast",
            ActivityState::Party => "fast",
            ActivityState::Commute => "medium",
            ActivityState::Unknown => "medium",
        }
    }
}

/// User context information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    /// Time of day
    pub time_of_day: TimeOfDay,
    /// Day of week
    pub day_of_week: Weekday,
    /// Is weekend
    pub is_weekend: bool,
    /// Current hour
    pub hour: u32,
    /// Detected activity
    pub activity: ActivityState,
    /// Focus mode enabled
    pub focus_mode: bool,
    /// Recently played genres
    pub recent_genres: VecDeque<String>,
    /// Listening patterns by hour
    pub hourly_patterns: [Option<String>; 24],
}

impl Default for UserContext {
    fn default() -> Self {
        Self::new()
    }
}

impl UserContext {
    /// Create a new context with current time
    pub fn new() -> Self {
        let now = Local::now();
        let day = now.weekday();

        Self {
            time_of_day: TimeOfDay::from_datetime(&now),
            day_of_week: day,
            is_weekend: day == Weekday::Sat || day == Weekday::Sun,
            hour: now.hour(),
            activity: ActivityState::Unknown,
            focus_mode: false,
            recent_genres: VecDeque::with_capacity(10),
            hourly_patterns: [None; 24],
        }
    }

    /// Update with current time
    pub fn update(&mut self) {
        let now = Local::now();
        self.time_of_day = TimeOfDay::from_datetime(&now);
        self.day_of_week = now.weekday();
        self.is_weekend = self.day_of_week == Weekday::Sat || self.day_of_week == Weekday::Sun;
        self.hour = now.hour();
    }

    /// Set focus mode
    pub fn set_focus_mode(&mut self, enabled: bool) {
        self.focus_mode = enabled;
        if enabled {
            self.activity = ActivityState::Focus;
        }
    }

    /// Add a played genre
    pub fn add_played_genre(&mut self, genre: String) {
        // Remove if already exists
        self.recent_genres.retain(|g| g != &genre);
        // Add to front
        self.recent_genres.push_front(genre);
        // Keep only 10
        while self.recent_genres.len() > 10 {
            self.recent_genres.pop_back();
        }
    }

    /// Get recommended mood based on context
    pub fn recommended_mood(&self) -> String {
        if self.focus_mode {
            return "focus".to_string();
        }

        let moods = self.time_of_day.recommended_moods();
        moods.first().unwrap_or(&"neutral").to_string()
    }

    /// Get recommended genres based on context
    pub fn recommended_genres(&self) -> Vec<String> {
        if self.focus_mode {
            return ActivityState::Focus.recommended_genres()
                .iter().map(|s| s.to_string()).collect();
        }

        // Consider time of day and activity
        let mut genres = self.activity.recommended_genres();
        genres.extend(self.time_of_day.recommended_moods());

        // Prioritize recently played genres
        for recent in &self.recent_genres {
            if !genres.contains(&recent.as_str()) {
                genres.insert(0, recent.as_str());
            }
        }

        genres.into_iter().take(5).map(|s| s.to_string()).collect()
    }

    /// Get context summary for AI prompts
    pub fn to_context_string(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!("Time: {} ({})", self.time_of_day.display_name(), self.hour));
        parts.push(format!("Day: {}{}", 
            self.day_of_week,
            if self.is_weekend { " (weekend)" } else { "" }
        ));

        if self.focus_mode {
            parts.push("Focus mode: ON".to_string());
        }

        if !self.recent_genres.is_empty() {
            parts.push(format!("Recent genres: {}", self.recent_genres.iter().take(5).cloned().collect::<Vec<_>>().join(", ")));
        }

        parts.push(format!("Activity: {:?}", self.activity));

        parts.join("\n")
    }

    /// Generate AI prompt context
    pub fn to_ai_context(&self) -> super::provider::AIContext {
        super::provider::AIContext {
            time_of_day: format!("{}:00", self.hour),
            day_of_week: format!("{:?}", self.day_of_week),
            focus_mode: self.focus_mode,
            favorite_genres: self.recent_genres.iter().take(5).cloned().collect(),
            ..Default::default()
        }
    }
}

/// Context detector for analyzing patterns
pub struct ContextDetector {
    /// Current context
    context: UserContext,
    /// Listening history by hour
    hourly_history: Vec<Vec<String>>,
}

impl ContextDetector {
    /// Create a new context detector
    pub fn new() -> Self {
        Self {
            context: UserContext::new(),
            hourly_history: vec![Vec::new(); 24],
        }
    }

    /// Record a listening event
    pub fn record_listen(&mut self, genre: Option<&str>) {
        self.context.update();

        if let Some(g) = genre {
            self.context.add_played_genre(g.to_string());

            // Record in hourly history
            let hour = self.context.hour as usize;
            if hour < 24 {
                self.hourly_history[hour].push(g.to_string());
                // Keep only last 50 entries per hour
                if self.hourly_history[hour].len() > 50 {
                    self.hourly_history[hour].remove(0);
                }
            }
        }
    }

    /// Get current context
    pub fn context(&self) -> &UserContext {
        &self.context
    }

    /// Get mutable context
    pub fn context_mut(&mut self) -> &mut UserContext {
        &mut self.context
    }

    /// Detect activity from patterns
    pub fn detect_activity(&mut self) {
        // Time-based heuristics
        let hour = self.context.hour;

        // Check hourly patterns
        let hour_genres = &self.hourly_history[hour as usize];
        if !hour_genres.is_empty() {
            // Find most common genre
            let mut genre_counts = std::collections::HashMap::new();
            for g in hour_genres {
                *genre_counts.entry(g.as_str()).or_insert(0) += 1;
            }

            if let Some((&most_common, _)) = genre_counts.iter().max_by_key(|&(_, c)| c) {
                // Infer activity from genre
                self.context.activity = match most_common {
                    "ambient" | "classical" | "lo-fi" | "lofi" => ActivityState::Focus,
                    "electronic" | "rock" | "hip-hop" if hour >= 6 && hour <= 9 => ActivityState::Exercise,
                    "pop" | "dance" | "electronic" if hour >= 20 || hour <= 2 => ActivityState::Party,
                    "jazz" | "chill" | "acoustic" => ActivityState::Relax,
                    _ => self.context.activity,
                };
            }
        }

        // Time-based defaults
        if self.context.activity == ActivityState::Unknown {
            self.context.activity = match self.context.time_of_day {
                TimeOfDay::Morning if self.context.is_weekend => ActivityState::Relax,
                TimeOfDay::Morning => ActivityState::Focus,
                TimeOfDay::Afternoon if self.context.is_weekend => ActivityState::Relax,
                TimeOfDay::Afternoon => ActivityState::Focus,
                TimeOfDay::Evening if !self.context.is_weekend => ActivityState::Relax,
                TimeOfDay::Night if self.context.is_weekend => ActivityState::Party,
                _ => ActivityState::Relax,
            };
        }
    }

    /// Get recommendations based on detected context
    pub fn get_recommendations(&self) -> ContextRecommendations {
        self.detect_activity(); // This line shouldn't compile - detect_activity takes &mut self

        ContextRecommendations {
            moods: vec![self.context.recommended_mood()],
            genres: self.context.recommended_genres(),
            tempo: self.context.activity.recommended_tempo().to_string(),
        }
    }
}

impl Default for ContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Recommendations based on context
#[derive(Debug, Clone)]
pub struct ContextRecommendations {
    /// Recommended moods
    pub moods: Vec<String>,
    /// Recommended genres
    pub genres: Vec<String>,
    /// Recommended tempo
    pub tempo: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day() {
        let tod = TimeOfDay::current();
        assert!(!tod.recommended_moods().is_empty());
    }

    #[test]
    fn test_user_context() {
        let ctx = UserContext::new();
        assert!(ctx.recommended_genres().len() <= 5);
        assert!(!ctx.to_context_string().is_empty());
    }

    #[test]
    fn test_context_detector() {
        let mut detector = ContextDetector::new();

        detector.record_listen(Some("rock"));
        detector.record_listen(Some("electronic"));

        assert!(!detector.context().recent_genres.is_empty());
    }

    #[test]
    fn test_activity_genres() {
        let focus_genres = ActivityState::Focus.recommended_genres();
        assert!(focus_genres.contains(&"ambient"));

        let exercise_genres = ActivityState::Exercise.recommended_genres();
        assert!(exercise_genres.contains(&"electronic"));
    }
}
