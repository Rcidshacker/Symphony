//! AI Module
//!
//! AI-powered features for Symphony including:
//! - Natural language music control
//! - Smart playlist generation
//! - Music recommendations
//! - Context awareness
//! - Music embeddings

pub mod manager;
pub mod ollama;
pub mod openrouter;
pub mod provider;

// Feature modules
pub mod context;
pub mod embeddings;
pub mod mood;
pub mod playlists;
pub mod recommendations;
pub mod semantic;
pub mod suggest;

// Re-exports
pub use manager::{AIManager, AIManagerBuilder, AIState};
pub use ollama::OllamaProvider;
pub use openrouter::OpenRouterProvider;
pub use provider::{AIContext, AIError, AIProviderConfig, AIResponse, LLMProvider, MusicIntent};
