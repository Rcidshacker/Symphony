//! Ollama Provider Implementation
//!
//! Local LLM provider using Ollama for privacy-first AI features.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::provider::{AIError, AIResponse, LLMProvider, MusicIntent};

/// Ollama API provider
pub struct OllamaProvider {
    /// HTTP client
    client: Client,
    /// Base URL for Ollama API
    base_url: String,
    /// Model name
    model: String,
    /// Request timeout in seconds
    timeout_secs: u64,
}

/// Ollama generate request
#[derive(Serialize)]
struct OllamaGenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

/// Ollama options
#[derive(Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

/// Ollama generate response
#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Ollama chat request
#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

/// Ollama message
#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

/// Ollama chat response
#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessage,
    #[serde(default)]
    total_duration: Option<u64>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Ollama models list response
#[derive(Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

/// Ollama model info
#[derive(Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    size: Option<u64>,
}

impl OllamaProvider {
    /// Create a new Ollama provider
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url,
            model,
            timeout_secs: 60,
        }
    }

    /// Create provider with custom timeout
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
}"#.to_string()
    }

    /// Build intent parsing prompt
    fn build_intent_prompt(&self, query: &str, context: &str) -> String {
        format!(
            "Context:\n{}\n\nUser request: \"{}\"\n\nRespond with JSON only:",
            context, query
        )
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

        let parsed: IntentJson = serde_json::from_str(json_str)
            .map_err(|e| AIError::ParseError(format!("Failed to parse JSON: {}. Input: {}", e, json_str)))?;

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

    /// Check if a model is available locally
    pub async fn is_model_available(&self, model_name: &str) -> Result<bool, AIError> {
        let response = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;

        let models: OllamaModelsResponse = response.json().await?;

        Ok(models.models.iter().any(|m| m.name.starts_with(model_name)))
    }

    /// Pull a model (download if not present)
    pub async fn pull_model(&self, model_name: &str) -> Result<(), AIError> {
        #[derive(Serialize)]
        struct PullRequest {
            name: String,
        }

        let _response = self.client
            .post(format!("{}/api/pull", self.base_url))
            .json(&PullRequest {
                name: model_name.to_string(),
            })
            .send()
            .await?;

        Ok(())
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn parse_command(&self, query: &str, context: &str) -> Result<MusicIntent, AIError> {
        let system_prompt = self.build_intent_system_prompt();
        let user_prompt = self.build_intent_prompt(query, context);

        // Use chat API for better results
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: vec![
                OllamaMessage {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                OllamaMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
            stream: false,
        };

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AIError::ApiError(format!("Ollama error {}: {}", status, body)));
        }

        let ollama_response: OllamaChatResponse = response.json().await?;

        self.parse_intent_json(&ollama_response.message.content)
    }

    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        let request = OllamaGenerateRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            stream: false,
            options: Some(OllamaOptions {
                temperature: Some(0.7),
                num_predict: Some(500),
            }),
        };

        let response = self.client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AIError::ApiError(format!("Ollama error: {}", response.status())));
        }

        let ollama_response: OllamaGenerateResponse = response.json().await?;

        Ok(ollama_response.response)
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
        let tracks: Vec<String> = serde_json::from_str(&response)
            .unwrap_or_else(|_| {
                // Fallback: try to extract tracks from text
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
        let result = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await;

        match result {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => Err(AIError::NetworkError(e)),
        }
    }

    fn name(&self) -> &str {
        "Ollama (Local)"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_intent_json() {
        let provider = OllamaProvider::new(
            "http://localhost:11434".to_string(),
            "llama3.2:3b".to_string(),
        );

        // Test play intent
        let json = r#"{"intent": "play", "genre": "rock", "mood": "energetic"}"#;
        let intent = provider.parse_intent_json(json).unwrap();
        match intent {
            MusicIntent::Play { genre, mood, .. } => {
                assert_eq!(genre, Some("rock".to_string()));
                assert_eq!(mood, Some("energetic".to_string()));
            }
            _ => panic!("Expected Play intent"),
        }

        // Test with markdown code block
        let json_with_block = r#"```json
{"intent": "play", "genre": "jazz"}
```"#;
        let intent = provider.parse_intent_json(json_with_block).unwrap();
        match intent {
            MusicIntent::Play { genre, .. } => {
                assert_eq!(genre, Some("jazz".to_string()));
            }
            _ => panic!("Expected Play intent"),
        }
    }

    #[test]
    fn test_provider_creation() {
        let provider = OllamaProvider::new(
            "http://localhost:11434".to_string(),
            "llama3.2:3b".to_string(),
        );

        assert_eq!(provider.name(), "Ollama (Local)");
        assert_eq!(provider.model(), "llama3.2:3b");
    }
}
