//! LLM Provider Abstraction
//!
//! Defines the trait and types for AI provider implementations.
//! Supports both local (Ollama) and cloud (OpenRouter) providers.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AIProviderConfig {
    /// Ollama local LLM
    Ollama {
        /// Base URL for Ollama API (default: http://localhost:11434)
        base_url: String,
        /// Model name (e.g., llama3.2:3b, mistral:7b)
        model: String,
    },
    /// OpenRouter cloud LLM
    OpenRouter {
        /// API key from openrouter.ai
        api_key: String,
        /// Model name (e.g., anthropic/claude-3.5-sonnet)
        model: String,
        /// Site URL for OpenRouter headers
        site_url: Option<String>,
        /// App name for OpenRouter headers
        app_name: Option<String>,
    },
}

impl Default for AIProviderConfig {
    fn default() -> Self {
        Self::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model: "llama3.2:3b".to_string(),
        }
    }
}

/// Music intent parsed from natural language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MusicIntent {
    /// Play music with specified criteria
    Play {
        /// Genre filter
        genre: Option<String>,
        /// Mood filter (happy, sad, energetic, calm, focus, etc.)
        mood: Option<String>,
        /// Specific artist
        artist: Option<String>,
        /// Tempo preference (slow, medium, fast)
        tempo: Option<String>,
        /// Era filter (60s, 70s, 80s, etc.)
        era: Option<String>,
        /// Duration in minutes (for playlists)
        duration_mins: Option<u32>,
    },
    /// Search for music
    Search {
        /// Search query
        query: String,
    },
    /// Get suggestions
    Suggest {
        /// Base suggestion on this (track, artist, mood)
        based_on: Option<String>,
        /// Number of suggestions
        count: usize,
    },
    /// Create a playlist
    CreatePlaylist {
        /// Theme or name for playlist
        theme: String,
        /// Total duration in minutes
        duration_mins: u32,
    },
    /// Skip current track
    Skip {
        /// Reason for skipping
        reason: Option<String>,
    },
    /// Get information about current track
    Info {
        /// What info to get (lyrics, artist, album)
        about: Option<String>,
    },
    /// Set playback state
    SetState {
        /// Volume level (0-100)
        volume: Option<u8>,
        /// Shuffle mode
        shuffle: Option<bool>,
        /// Repeat mode
        repeat: Option<String>,
    },
    /// Unknown or unsupported command
    Unknown {
        /// Original query
        query: String,
    },
}

impl MusicIntent {
    /// Get a human-readable description of the intent
    pub fn description(&self) -> String {
        match self {
            MusicIntent::Play {
                genre,
                mood,
                artist,
                tempo,
                era,
                ..
            } => {
                let mut parts = Vec::new();
                if let Some(a) = artist {
                    parts.push(format!("artist: {}", a));
                }
                if let Some(g) = genre {
                    parts.push(format!("genre: {}", g));
                }
                if let Some(m) = mood {
                    parts.push(format!("mood: {}", m));
                }
                if let Some(t) = tempo {
                    parts.push(format!("tempo: {}", t));
                }
                if let Some(e) = era {
                    parts.push(format!("era: {}", e));
                }
                if parts.is_empty() {
                    "Play music".to_string()
                } else {
                    format!("Play music ({})", parts.join(", "))
                }
            }
            MusicIntent::Search { query } => format!("Search for: {}", query),
            MusicIntent::Suggest { based_on, count } => {
                if let Some(base) = based_on {
                    format!("Suggest {} tracks based on {}", count, base)
                } else {
                    format!("Suggest {} tracks", count)
                }
            }
            MusicIntent::CreatePlaylist {
                theme,
                duration_mins,
            } => {
                format!("Create '{}' playlist ({} mins)", theme, duration_mins)
            }
            MusicIntent::Skip { reason } => {
                if let Some(r) = reason {
                    format!("Skip track ({})", r)
                } else {
                    "Skip track".to_string()
                }
            }
            MusicIntent::Info { about } => {
                if let Some(a) = about {
                    format!("Get info about: {}", a)
                } else {
                    "Get track info".to_string()
                }
            }
            MusicIntent::SetState {
                volume,
                shuffle,
                repeat,
            } => {
                let mut parts = Vec::new();
                if let Some(v) = volume {
                    parts.push(format!("volume: {}%", v));
                }
                if let Some(s) = shuffle {
                    parts.push(format!("shuffle: {}", if *s { "on" } else { "off" }));
                }
                if let Some(r) = repeat {
                    parts.push(format!("repeat: {}", r));
                }
                format!("Set state ({})", parts.join(", "))
            }
            MusicIntent::Unknown { query } => format!("Unknown command: {}", query),
        }
    }
}

/// LLM provider trait
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Parse natural language command into structured intent
    async fn parse_command(&self, query: &str, context: &str) -> Result<MusicIntent, AIError>;

    /// Generate a conversational response
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError>;

    /// Generate a playlist based on description
    async fn generate_playlist(
        &self,
        description: &str,
        available_tracks: &[String],
        count: usize,
    ) -> Result<Vec<String>, AIError>;

    /// Check if provider is available/online
    async fn health_check(&self) -> Result<bool, AIError>;

    /// Get provider name
    fn name(&self) -> &str;

    /// Get current model name
    fn model(&self) -> &str;
}

/// AI errors
#[derive(Debug, thiserror::Error)]
pub enum AIError {
    #[error("Provider not available: {0}")]
    Unavailable(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Parsing error: {0}")]
    ParseError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Rate limited")]
    RateLimited,
}

/// AI response with metadata
#[derive(Debug, Clone)]
pub struct AIResponse {
    /// The response content
    pub content: String,
    /// Whether this came from cache
    pub from_cache: bool,
    /// Time taken to generate (ms)
    pub latency_ms: u64,
    /// Tokens used (if available)
    pub tokens_used: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone)]
pub struct TokenUsage {
    /// Input tokens
    pub prompt_tokens: u32,
    /// Output tokens
    pub completion_tokens: u32,
    /// Total tokens
    pub total_tokens: u32,
}

/// Context for AI commands
#[derive(Debug, Clone, Default)]
pub struct AIContext {
    /// Current time
    pub time_of_day: String,
    /// Day of week
    pub day_of_week: String,
    /// Currently playing track (if any)
    pub current_track: Option<String>,
    /// Current artist (if any)
    pub current_artist: Option<String>,
    /// Recently played tracks
    pub recent_tracks: Vec<String>,
    /// User's favorite genres
    pub favorite_genres: Vec<String>,
    /// Library size
    pub library_size: usize,
    /// Whether user is working/focusing
    pub focus_mode: bool,
}

/// System prompt that makes a model answer with one JSON intent
pub const INTENT_SYSTEM_PROMPT: &str = r#"You are the assistant of Symphony, a terminal music player.
Turn the user's request into ONE JSON object and reply with nothing else.

"intent" is one of:
- "play": play music. Optional fields: "genre", "mood" (happy, sad, energetic, calm, focus, chill, party, romantic, angry, epic), "artist", "tempo" (slow, medium, fast), "era" (60s ... 2020s), "duration_mins"
- "search": find tracks by meaning. Field: "query"
- "suggest": recommend tracks. Optional: "based_on" (artist or track to be similar to), "count"
- "create_playlist": build and play a playlist. Fields: "theme" (e.g. "focus session", "workout"), "duration_mins"
- "skip": skip the current track
- "info": about the current track. Optional: "about" (artist, album, lyrics, ...)
- "set_state": change playback. Optional: "volume" (0-100), "shuffle" (true/false), "repeat" ("off", "all" or "one")
- "unknown": a question or anything else. Field: "query" (the user's words)

Examples:
{"intent":"play","mood":"energetic","tempo":"fast"}
{"intent":"suggest","based_on":"Radiohead","count":10}
{"intent":"create_playlist","theme":"focus session","duration_mins":120}
{"intent":"set_state","volume":30}
{"intent":"unknown","query":"what should I listen to while debugging?"}"#;

/// Parse a model's reply into an intent. Tolerates code fences and prose around the JSON.
pub fn parse_intent_json(reply: &str) -> Result<MusicIntent, AIError> {
    #[derive(Deserialize)]
    struct IntentJson {
        intent: String,
        genre: Option<String>,
        mood: Option<String>,
        artist: Option<String>,
        tempo: Option<String>,
        era: Option<String>,
        duration_mins: Option<u32>,
        query: Option<String>,
        theme: Option<String>,
        count: Option<usize>,
        based_on: Option<String>,
        about: Option<String>,
        reason: Option<String>,
        volume: Option<u8>,
        shuffle: Option<bool>,
        repeat: Option<String>,
    }

    // the JSON object, whatever surrounds it
    let json = match (reply.find('{'), reply.rfind('}')) {
        (Some(start), Some(end)) if start < end => &reply[start..=end],
        _ => reply.trim(),
    };
    let parsed: IntentJson = serde_json::from_str(json)
        .map_err(|e| AIError::ParseError(format!("Failed to parse JSON: {e}. Input: {json}")))?;

    Ok(match parsed.intent.to_lowercase().as_str() {
        "play" => MusicIntent::Play {
            genre: parsed.genre,
            mood: parsed.mood,
            artist: parsed.artist,
            tempo: parsed.tempo,
            era: parsed.era,
            duration_mins: parsed.duration_mins,
        },
        "search" => MusicIntent::Search {
            query: parsed.query.unwrap_or_else(|| json.to_string()),
        },
        "suggest" => MusicIntent::Suggest {
            based_on: parsed.based_on.or(parsed.artist),
            count: parsed.count.unwrap_or(10),
        },
        "create_playlist" => MusicIntent::CreatePlaylist {
            theme: parsed.theme.unwrap_or_else(|| "New Playlist".to_string()),
            duration_mins: parsed.duration_mins.unwrap_or(60),
        },
        "skip" => MusicIntent::Skip {
            reason: parsed.reason,
        },
        "info" => MusicIntent::Info {
            about: parsed.about,
        },
        "set_state" => MusicIntent::SetState {
            volume: parsed.volume,
            shuffle: parsed.shuffle,
            repeat: parsed.repeat,
        },
        _ => MusicIntent::Unknown {
            query: parsed.query.unwrap_or_else(|| json.to_string()),
        },
    })
}

impl AIContext {
    /// Build context string for AI prompt
    pub fn to_context_string(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!("Current time: {}", self.time_of_day));
        parts.push(format!("Day: {}", self.day_of_week));
        parts.push(format!("Library size: {} tracks", self.library_size));

        if let Some(ref track) = self.current_track {
            parts.push(format!("Currently playing: {}", track));
        }

        if !self.favorite_genres.is_empty() {
            parts.push(format!(
                "Favorite genres: {}",
                self.favorite_genres.join(", ")
            ));
        }

        if self.focus_mode {
            parts.push("User is in focus mode".to_string());
        }

        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_intent_reads_every_field_the_prompt_documents() {
        match parse_intent_json(r#"{"intent":"suggest","based_on":"Radiohead","count":5}"#).unwrap()
        {
            MusicIntent::Suggest { based_on, count } => {
                assert_eq!((based_on.as_deref(), count), (Some("Radiohead"), 5));
            }
            other => panic!("{other:?}"),
        }
        match parse_intent_json(
            r#"{"intent":"set_state","volume":30,"shuffle":true,"repeat":"all"}"#,
        )
        .unwrap()
        {
            MusicIntent::SetState {
                volume,
                shuffle,
                repeat,
            } => {
                assert_eq!(
                    (volume, shuffle, repeat.as_deref()),
                    (Some(30), Some(true), Some("all"))
                );
            }
            other => panic!("{other:?}"),
        }
        match parse_intent_json(r#"{"intent":"info","about":"lyrics"}"#).unwrap() {
            MusicIntent::Info { about } => assert_eq!(about.as_deref(), Some("lyrics")),
            other => panic!("{other:?}"),
        }
        match parse_intent_json(r#"{"intent":"unknown","query":"what is jazz"}"#).unwrap() {
            MusicIntent::Unknown { query } => assert_eq!(query, "what is jazz"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parse_intent_tolerates_fences_and_chatter() {
        let reply = "Sure! Here you go:\n```json\n{\"intent\": \"play\", \"artist\": \"Queen\"}\n```\nEnjoy.";
        match parse_intent_json(reply).unwrap() {
            MusicIntent::Play { artist, .. } => assert_eq!(artist.as_deref(), Some("Queen")),
            other => panic!("{other:?}"),
        }
        assert!(parse_intent_json("no json here").is_err());
    }

    #[test]
    fn test_music_intent_description() {
        let intent = MusicIntent::Play {
            genre: Some("rock".to_string()),
            mood: Some("energetic".to_string()),
            artist: None,
            tempo: None,
            era: None,
            duration_mins: None,
        };
        assert!(intent.description().contains("rock"));
        assert!(intent.description().contains("energetic"));
    }

    #[test]
    fn test_ai_context() {
        let context = AIContext {
            time_of_day: "14:30".to_string(),
            day_of_week: "Monday".to_string(),
            library_size: 1000,
            ..Default::default()
        };

        let ctx_str = context.to_context_string();
        assert!(ctx_str.contains("14:30"));
        assert!(ctx_str.contains("1000 tracks"));
    }

    #[test]
    fn test_provider_config_default() {
        let config = AIProviderConfig::default();
        match config {
            AIProviderConfig::Ollama { base_url, model } => {
                assert!(base_url.contains("localhost"));
                assert!(model.contains("llama"));
            }
            _ => panic!("Default should be Ollama"),
        }
    }
}
