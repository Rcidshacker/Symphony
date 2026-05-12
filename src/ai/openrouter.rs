//! OpenRouter Provider Implementation
//!
//! Cloud LLM provider using OpenRouter API for access to multiple AI models.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::provider::{AIError, LLMProvider, MusicIntent};

/// OpenRouter API provider
pub struct OpenRouterProvider {
    /// HTTP client
    client: Client,
    /// API key
    api_key: String,
    /// Model name (e.g., anthropic/claude-3.5-sonnet)
    model: String,
    /// Site URL for OpenRouter headers (optional)
    site_url: Option<String>,
    /// App name for OpenRouter headers (optional)
    app_name: Option<String>,
    /// Request timeout in seconds
    timeout_secs: u64,
}

/// OpenRouter chat request
#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// OpenRouter message
#[derive(Serialize, Deserialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
}

/// OpenRouter chat response
#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
}

/// OpenRouter choice
#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

/// OpenRouter token usage
#[derive(Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenRouter models list response
#[derive(Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

/// OpenRouter model info
#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
    name: String,
    #[serde(default)]
    pricing: Option<OpenRouterPricing>,
}

/// OpenRouter model pricing
#[derive(Deserialize)]
struct OpenRouterPricing {
    prompt: String,
    completion: String,
}

/// Recommended OpenRouter models
pub const RECOMMENDED_MODELS: &[(&str, &str)] = &[
    ("anthropic/claude-3.5-sonnet", "Best overall quality"),
    ("anthropic/claude-3-haiku", "Fast and cheap"),
    ("openai/gpt-4-turbo", "OpenAI's best"),
    ("openai/gpt-3.5-turbo", "Cheapest GPT"),
    ("google/gemini-pro", "Google's model"),
    ("meta-llama/llama-3.1-70b-instruct", "Open source, powerful"),
    ("mistralai/mistral-large", "Mistral's best"),
    (
        "perplexity/llama-3.1-sonar-huge-128k-online",
        "With web search",
    ),
];

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
            model,
            site_url: None,
            app_name: None,
            timeout_secs: 60,
        }
    }

    /// Add site URL for OpenRouter headers
    pub fn with_metadata(mut self, site_url: String, app_name: String) -> Self {
        self.site_url = Some(site_url);
        self.app_name = Some(app_name);
        self
    }

    /// Set custom timeout
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self.client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());
        self
    }

    /// Build the system prompt for intent parsing
    fn build_intent_system_prompt(&self) -> String {
        r#"You are a music assistant for a terminal music player called Symphony.
Parse the user's request into a structured JSON format.

Possible intents:
- play: Start playing music
- search: Search for music
- suggest: Get recommendations
- create_playlist: Create a new playlist
- skip: Skip current track
- info: Get information about current track
- set_state: Change playback settings

For "play" intent, extract these if mentioned:
- genre: rock, jazz, electronic, classical, hip-hop, pop, metal, etc.
- mood: happy, sad, energetic, calm, focus, chill, party, romantic, angry
- artist: specific artist name
- tempo: slow, medium, fast
- era: 60s, 70s, 80s, 90s, 2000s, 2010s, 2020s, recent
- duration_mins: duration in minutes if specified

Respond ONLY with valid JSON, no explanation:
{
  "intent": "play",
  "genre": "rock",
  "mood": "energetic",
  "artist": null,
  "tempo": null,
  "era": null,
  "duration_mins": null
}"#
        .to_string()
    }

    /// Build headers for OpenRouter requests
    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();

        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());

        // Optional metadata headers
        if let Some(ref url) = self.site_url {
            headers.insert("HTTP-Referer", url.parse().unwrap());
        }
        if let Some(ref name) = self.app_name {
            headers.insert("X-Title", name.parse().unwrap());
        }

        headers
    }

    /// Parse JSON response into MusicIntent
    fn parse_intent_json(&self, json_str: &str) -> Result<MusicIntent, AIError> {
        // Extract JSON from response (might have markdown code blocks)
        let json_str = json_str
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        #[derive(Deserialize)]
        struct IntentJson {
            intent: String,
            genre: Option<String>,
            mood: Option<String>,
            artist: Option<String>,
            tempo: Option<String>,
            era: Option<String>,
            duration_mins: Option<u32>,
            #[serde(default)]
            query: Option<String>,
            #[serde(default)]
            theme: Option<String>,
            #[serde(default)]
            count: Option<usize>,
        }

        let parsed: IntentJson = serde_json::from_str(json_str).map_err(|e| {
            AIError::ParseError(format!("Failed to parse JSON: {}. Input: {}", e, json_str))
        })?;

        match parsed.intent.to_lowercase().as_str() {
            "play" => Ok(MusicIntent::Play {
                genre: parsed.genre,
                mood: parsed.mood,
                artist: parsed.artist,
                tempo: parsed.tempo,
                era: parsed.era,
                duration_mins: parsed.duration_mins,
            }),
            "search" => Ok(MusicIntent::Search {
                query: parsed.query.unwrap_or_else(|| json_str.to_string()),
            }),
            "suggest" => Ok(MusicIntent::Suggest {
                based_on: None,
                count: parsed.count.unwrap_or(10),
            }),
            "create_playlist" => Ok(MusicIntent::CreatePlaylist {
                theme: parsed.theme.unwrap_or_else(|| "New Playlist".to_string()),
                duration_mins: parsed.duration_mins.unwrap_or(60),
            }),
            "skip" => Ok(MusicIntent::Skip { reason: None }),
            "info" => Ok(MusicIntent::Info { about: None }),
            "set_state" => Ok(MusicIntent::SetState {
                volume: None,
                shuffle: None,
                repeat: None,
            }),
            _ => Ok(MusicIntent::Unknown {
                query: json_str.to_string(),
            }),
        }
    }

    /// Get available models
    pub async fn list_models(&self) -> Result<Vec<(String, String)>, AIError> {
        let response = self
            .client
            .get("https://openrouter.ai/api/v1/models")
            .headers(self.build_headers())
            .send()
            .await?;

        let models: OpenRouterModelsResponse = response.json().await?;

        Ok(models.data.into_iter().map(|m| (m.id, m.name)).collect())
    }

    /// Calculate estimated cost for a request
    pub fn estimate_cost(&self, prompt_tokens: u32, completion_tokens: u32) -> Option<f64> {
        // Rough estimates for common models (USD per 1M tokens)
        let (prompt_cost, completion_cost) = match self.model.as_str() {
            "anthropic/claude-3.5-sonnet" => (3.0, 15.0),
            "anthropic/claude-3-haiku" => (0.25, 1.25),
            "openai/gpt-4-turbo" => (10.0, 30.0),
            "openai/gpt-3.5-turbo" => (0.5, 1.5),
            "google/gemini-pro" => (0.5, 1.5),
            _ => return None,
        };

        let cost = (prompt_tokens as f64 * prompt_cost / 1_000_000.0)
            + (completion_tokens as f64 * completion_cost / 1_000_000.0);

        Some(cost)
    }
}

#[async_trait]
impl LLMProvider for OpenRouterProvider {
    async fn parse_command(&self, query: &str, context: &str) -> Result<MusicIntent, AIError> {
        let system_prompt = self.build_intent_system_prompt();
        let user_prompt = format!(
            "Context:\n{}\n\nUser request: \"{}\"\n\nRespond with JSON only:",
            context, query
        );

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![
                OpenRouterMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                OpenRouterMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            max_tokens: Some(200),
            temperature: Some(0.3),
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .headers(self.build_headers())
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AIError::RateLimited);
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AIError::ApiError(format!(
                "OpenRouter error {}: {}",
                status, body
            )));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        let content = openrouter_response
            .choices
            .first()
            .map(|c| c.message.content.as_str())
            .ok_or_else(|| AIError::ParseError("No response from OpenRouter".to_string()))?;

        self.parse_intent_json(content)
    }

    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![OpenRouterMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: Some(500),
            temperature: Some(0.7),
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .headers(self.build_headers())
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AIError::RateLimited);
        }

        if !status.is_success() {
            return Err(AIError::ApiError(format!("OpenRouter error: {}", status)));
        }

        let openrouter_response: OpenRouterResponse = response.json().await?;

        Ok(openrouter_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    async fn generate_playlist(
        &self,
        description: &str,
        available_tracks: &[String],
        count: usize,
    ) -> Result<Vec<String>, AIError> {
        let prompt = format!(
            r#"You are a music DJ. Create a playlist based on this description: "{}"

Available tracks (select from these):
{}

Select exactly {} tracks. Respond with a JSON array of track names only:
["Track 1", "Track 2", ...]"#,
            description,
            available_tracks.join("\n"),
            count
        );

        let response = self.generate_response(&prompt).await?;

        // Parse the track list from response
        let tracks: Vec<String> = serde_json::from_str(&response).unwrap_or_else(|_| {
            response
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('[') && !line.starts_with(']'))
                .take(count)
                .map(|s| s.trim_matches('"').trim_matches(',').to_string())
                .collect()
        });

        Ok(tracks)
    }

    async fn health_check(&self) -> Result<bool, AIError> {
        // Try to list models as a health check
        let result = self
            .client
            .get("https://openrouter.ai/api/v1/models")
            .headers(self.build_headers())
            .send()
            .await;

        match result {
            Ok(response) => {
                if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(AIError::ConfigError("Invalid API key".to_string()));
                }
                Ok(response.status().is_success())
            }
            Err(e) => Err(AIError::NetworkError(e)),
        }
    }

    fn name(&self) -> &str {
        "OpenRouter (Cloud)"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenRouterProvider::new(
            "test-api-key".to_string(),
            "anthropic/claude-3.5-sonnet".to_string(),
        );

        assert_eq!(provider.name(), "OpenRouter (Cloud)");
        assert_eq!(provider.model(), "anthropic/claude-3.5-sonnet");
    }

    #[test]
    fn test_cost_estimation() {
        let provider = OpenRouterProvider::new(
            "test-key".to_string(),
            "anthropic/claude-3.5-sonnet".to_string(),
        );

        let cost = provider.estimate_cost(100, 100);
        assert!(cost.is_some());
        assert!(cost.unwrap() > 0.0);
    }

    #[test]
    fn test_parse_intent_json() {
        let provider = OpenRouterProvider::new("test-key".to_string(), "test-model".to_string());

        let json = r#"{"intent": "play", "genre": "electronic", "mood": "focus"}"#;
        let intent = provider.parse_intent_json(json).unwrap();
        match intent {
            MusicIntent::Play { genre, mood, .. } => {
                assert_eq!(genre, Some("electronic".to_string()));
                assert_eq!(mood, Some("focus".to_string()));
            }
            _ => panic!("Expected Play intent"),
        }
    }

    #[test]
    fn test_recommended_models() {
        assert!(!RECOMMENDED_MODELS.is_empty());
        assert!(RECOMMENDED_MODELS
            .iter()
            .any(|(id, _)| id.contains("claude")));
    }
}
