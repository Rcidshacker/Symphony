//! AI Manager
//!
//! Facade for AI operations with automatic provider selection and fallback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::ollama::OllamaProvider;
use super::openrouter::OpenRouterProvider;
use super::provider::{AIContext, AIError, AIProviderConfig, AIResponse, LLMProvider, MusicIntent};

/// AI Manager state
#[derive(Debug, Clone)]
pub struct AIState {
    /// Whether AI is enabled and working
    pub enabled: bool,
    /// Provider name
    pub provider_name: String,
    /// Model name
    pub model_name: String,
    /// Last health check time
    pub last_health_check: Option<Instant>,
    /// Number of successful requests
    pub successful_requests: u64,
    /// Number of failed requests
    pub failed_requests: u64,
    /// Last error message
    pub last_error: Option<String>,
}

/// AI Manager - facade for all AI operations
pub struct AIManager {
    /// The active LLM provider
    provider: Arc<dyn LLMProvider>,
    /// Current configuration
    config: AIProviderConfig,
    /// Manager state
    state: Arc<RwLock<AIState>>,
    /// Request cache for common queries
    cache: Arc<RwLock<lru::LruCache<String, CachedResponse>>>,
    /// Context builder
    context: AIContext,
}

/// Cached response
#[derive(Clone)]
struct CachedResponse {
    intent: MusicIntent,
    timestamp: Instant,
}

/// LRU cache for AI responses
mod lru {
    use std::collections::HashMap;

    pub struct LruCache<K, V> {
        entries: HashMap<K, V>,
        order: Vec<K>,
        max_size: usize,
    }

    impl<K: Clone + Eq + std::hash::Hash, V: Clone> LruCache<K, V> {
        pub fn new(max_size: usize) -> Self {
            Self {
                entries: HashMap::new(),
                order: Vec::new(),
                max_size,
            }
        }

        pub fn get(&self, key: &K) -> Option<&V> {
            self.entries.get(key)
        }

        pub fn put(&mut self, key: K, value: V) {
            if self.entries.len() >= self.max_size && !self.entries.contains_key(&key) {
                if let Some(old_key) = self.order.first().cloned() {
                    self.entries.remove(&old_key);
                    self.order.remove(0);
                }
            }

            if !self.entries.contains_key(&key) {
                self.order.push(key.clone());
            }
            self.entries.insert(key, value);
        }

        pub fn clear(&mut self) {
            self.entries.clear();
            self.order.clear();
        }
    }
}

impl AIManager {
    /// Create a new AI manager from configuration
    pub async fn new(config: AIProviderConfig) -> Result<Self, AIError> {
        let provider: Arc<dyn LLMProvider> = match &config {
            AIProviderConfig::Ollama { base_url, model } => {
                Arc::new(OllamaProvider::new(base_url.clone(), model.clone()))
            }
            AIProviderConfig::OpenRouter { api_key, model, site_url, app_name } => {
                let mut provider = OpenRouterProvider::new(api_key.clone(), model.clone());
                if let (Some(url), Some(name)) = (site_url, app_name) {
                    provider = provider.with_metadata(url.clone(), name.clone());
                }
                Arc::new(provider)
            }
        };

        // Check if provider is available
        let enabled = provider.health_check().await.unwrap_or(false);

        let state = AIState {
            enabled,
            provider_name: provider.name().to_string(),
            model_name: provider.model().to_string(),
            last_health_check: Some(Instant::now()),
            successful_requests: 0,
            failed_requests: 0,
            last_error: None,
        };

        Ok(Self {
            provider,
            config,
            state: Arc::new(RwLock::new(state)),
            cache: Arc::new(RwLock::new(lru::LruCache::new(100))),
            context: AIContext::default(),
        })
    }

    /// Create AI manager with context
    pub fn with_context(mut self, context: AIContext) -> Self {
        self.context = context;
        self
    }

    /// Parse natural language command into structured intent
    pub async fn parse_command(&self, query: &str) -> Result<MusicIntent, AIError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&query.to_lowercase()) {
                // Cache valid for 5 minutes
                if cached.timestamp.elapsed() < Duration::from_secs(300) {
                    self.increment_success().await;
                    return Ok(cached.intent.clone());
                }
            }
        }

        // Check if enabled
        if !self.is_enabled().await {
            return Err(AIError::Unavailable(
                "AI provider is not available. Check your configuration.".to_string(),
            ));
        }

        let context = self.build_context_string();
        let start = Instant::now();

        match self.provider.parse_command(query, &context).await {
            Ok(intent) => {
                // Cache the result
                {
                    let mut cache = self.cache.write().await;
                    cache.put(
                        query.to_lowercase(),
                        CachedResponse {
                            intent: intent.clone(),
                            timestamp: Instant::now(),
                        },
                    );
                }

                self.increment_success().await;

                tracing::info!(
                    "AI parsed '{}' -> {:?} in {:?}",
                    query,
                    intent,
                    start.elapsed()
                );

                Ok(intent)
            }
            Err(e) => {
                self.increment_failure(&e.to_string()).await;
                Err(e)
            }
        }
    }

    /// Generate conversational response
    pub async fn respond(&self, message: &str) -> Result<String, AIError> {
        if !self.is_enabled().await {
            return Err(AIError::Unavailable(
                "AI provider is not available".to_string(),
            ));
        }

        let start = Instant::now();

        match self.provider.generate_response(message).await {
            Ok(response) => {
                self.increment_success().await;
                tracing::info!("AI responded in {:?}", start.elapsed());
                Ok(response)
            }
            Err(e) => {
                self.increment_failure(&e.to_string()).await;
                Err(e)
            }
        }
    }

    /// Generate a playlist based on description
    pub async fn generate_playlist(
        &self,
        description: &str,
        available_tracks: &[String],
        count: usize,
    ) -> Result<Vec<String>, AIError> {
        if !self.is_enabled().await {
            return Err(AIError::Unavailable(
                "AI provider is not available".to_string(),
            ));
        }

        let start = Instant::now();

        match self.provider
            .generate_playlist(description, available_tracks, count)
            .await
        {
            Ok(tracks) => {
                self.increment_success().await;
                tracing::info!(
                    "AI generated {} track playlist in {:?}",
                    tracks.len(),
                    start.elapsed()
                );
                Ok(tracks)
            }
            Err(e) => {
                self.increment_failure(&e.to_string()).await;
                Err(e)
            }
        }
    }

    /// Check provider health
    pub async fn check_health(&self) -> bool {
        match self.provider.health_check().await {
            Ok(healthy) => {
                let mut state = self.state.write().await;
                state.enabled = healthy;
                state.last_health_check = Some(Instant::now());
                if healthy {
                    state.last_error = None;
                }
                healthy
            }
            Err(e) => {
                let mut state = self.state.write().await;
                state.enabled = false;
                state.last_health_check = Some(Instant::now());
                state.last_error = Some(e.to_string());
                false
            }
        }
    }

    /// Get provider name
    pub fn provider_name(&self) -> &str {
        self.provider.name()
    }

    /// Get model name
    pub fn model_name(&self) -> &str {
        self.provider.model()
    }

    /// Check if AI is enabled
    pub async fn is_enabled(&self) -> bool {
        self.state.read().await.enabled
    }

    /// Get current state
    pub async fn get_state(&self) -> AIState {
        self.state.read().await.clone()
    }

    /// Update context
    pub fn update_context(&mut self, context: AIContext) {
        self.context = context;
    }

    /// Clear the response cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Switch to a different provider (requires restart for now)
    pub async fn switch_provider(&mut self, config: AIProviderConfig) -> Result<(), AIError> {
        let provider: Arc<dyn LLMProvider> = match &config {
            AIProviderConfig::Ollama { base_url, model } => {
                Arc::new(OllamaProvider::new(base_url.clone(), model.clone()))
            }
            AIProviderConfig::OpenRouter { api_key, model, site_url, app_name } => {
                let mut provider = OpenRouterProvider::new(api_key.clone(), model.clone());
                if let (Some(url), Some(name)) = (site_url, app_name) {
                    provider = provider.with_metadata(url.clone(), name.clone());
                }
                Arc::new(provider)
            }
        };

        let enabled = provider.health_check().await.unwrap_or(false);

        {
            let mut state = self.state.write().await;
            state.enabled = enabled;
            state.provider_name = provider.name().to_string();
            state.model_name = provider.model().to_string();
            state.last_health_check = Some(Instant::now());
            state.last_error = None;
        }

        self.provider = provider;
        self.config = config;

        // Clear cache when switching providers
        self.clear_cache().await;

        Ok(())
    }

    /// Build context string for AI prompts
    fn build_context_string(&self) -> String {
        self.context.to_context_string()
    }

    async fn increment_success(&self) {
        let mut state = self.state.write().await;
        state.successful_requests += 1;
    }

    async fn increment_failure(&self, error: &str) {
        let mut state = self.state.write().await;
        state.failed_requests += 1;
        state.last_error = Some(error.to_string());
    }
}

impl Clone for AIManager {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            config: self.config.clone(),
            state: self.state.clone(),
            cache: self.cache.clone(),
            context: self.context.clone(),
        }
    }
}

/// Builder for AIManager
pub struct AIManagerBuilder {
    config: Option<AIProviderConfig>,
    context: Option<AIContext>,
}

impl AIManagerBuilder {
    pub fn new() -> Self {
        Self {
            config: None,
            context: None,
        }
    }

    pub fn config(mut self, config: AIProviderConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn context(mut self, context: AIContext) -> Self {
        self.context = Some(context);
        self
    }

    pub async fn build(self) -> Result<AIManager, AIError> {
        let config = self.config.unwrap_or_default();
        let manager = AIManager::new(config).await?;

        if let Some(context) = self.context {
            Ok(manager.with_context(context))
        } else {
            Ok(manager)
        }
    }
}

impl Default for AIManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ai_manager_builder() {
        let config = AIProviderConfig::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model: "llama3.2:3b".to_string(),
        };

        let manager = AIManagerBuilder::new()
            .config(config)
            .build()
            .await;

        // Manager should be created (even if Ollama isn't running)
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_ai_manager_state() {
        let config = AIProviderConfig::default();
        let manager = AIManager::new(config).await.unwrap();

        let state = manager.get_state().await;
        assert_eq!(state.provider_name, "Ollama (Local)");
    }
}
