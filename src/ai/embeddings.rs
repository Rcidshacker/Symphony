//! Music Embeddings
//!
//! Vector embeddings for semantic music search and recommendations.
//! Uses local embedding models for privacy.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::app::Track;

/// Embedding errors
#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("Failed to generate embedding: {0}")]
    GenerationError(String),

    #[error("Failed to serialize embedding: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Vector dimension mismatch")]
    DimensionMismatch,

    #[error("Model not loaded")]
    ModelNotLoaded,
}

/// Track embedding vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackEmbedding {
    /// Track ID
    pub track_id: String,
    /// Embedding vector
    pub vector: Vec<f32>,
    /// Metadata used for embedding
    pub text: String,
    /// When the embedding was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl TrackEmbedding {
    /// Create a new embedding
    pub fn new(track_id: String, vector: Vec<f32>, text: String) -> Self {
        Self {
            track_id,
            vector,
            text,
            created_at: chrono::Utc::now(),
        }
    }

    /// Calculate cosine similarity with another embedding
    pub fn cosine_similarity(&self, other: &TrackEmbedding) -> f32 {
        cosine_similarity(&self.vector, &other.vector)
    }
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// Embedding generator trait
pub trait EmbeddingGenerator: Send + Sync {
    /// Generate embedding for text
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;

    /// Get model name
    fn model_name(&self) -> &str;
}

/// Simple TF-IDF based embedding generator (no external dependencies)
/// For production, use a proper embedding model like sentence-transformers
pub struct SimpleEmbeddingGenerator {
    /// Word to index mapping
    vocabulary: HashMap<String, usize>,
    /// IDF values for each word
    idf_values: Vec<f32>,
    /// Embedding dimension
    dimension: usize,
}

impl SimpleEmbeddingGenerator {
    /// Create a new simple embedding generator
    pub fn new() -> Self {
        // Pre-built vocabulary of music-related terms
        let music_terms = Self::music_vocabulary();
        let vocab_size = music_terms.len();

        let vocabulary: HashMap<String, usize> = music_terms
            .into_iter()
            .enumerate()
            .map(|(i, term)| (term, i))
            .collect();

        Self {
            vocabulary,
            idf_values: vec![1.0; vocab_size],
            dimension: vocab_size,
        }
    }

    /// Get music-related vocabulary
    fn music_vocabulary() -> Vec<String> {
        // Genres
        let genres = vec![
            "rock", "pop", "jazz", "classical", "electronic", "hip-hop", "rap",
            "metal", "folk", "country", "blues", "r&b", "soul", "punk", "indie",
            "alternative", "techno", "house", "ambient", "chill", "lo-fi", "lofi",
            "reggae", "latin", "k-pop", "kpop", "disco", "funk", "gospel",
        ];

        // Moods
        let moods = vec![
            "happy", "sad", "energetic", "calm", "relaxing", "upbeat", "melancholic",
            "romantic", "angry", "peaceful", "focus", "study", "party", "chill",
            "epic", "dark", "bright", "dreamy", "intense", "gentle", "powerful",
        ];

        // Tempo/energy
        let tempo = vec![
            "fast", "slow", "medium", "upbeat", "downtempo", "high-energy",
            "low-energy", "driving", "ballad", "anthem",
        ];

        // Eras
        let eras = vec![
            "60s", "70s", "80s", "90s", "2000s", "2010s", "2020s", "classic",
            "modern", "contemporary", "vintage", "retro",
        ];

        // Instruments
        let instruments = vec![
            "guitar", "piano", "drums", "bass", "violin", "synth", "synthesizer",
            "acoustic", "electric", "orchestral", "electronic",
        ];

        // Common descriptors
        let descriptors = vec![
            "love", "heart", "night", "day", "summer", "winter", "rain", "sun",
            "moon", "star", "dance", "cry", "smile", "memories", "dream",
            "journey", "road", "home", "freedom", "time", "life", "world",
        ];

        let mut vocab = Vec::new();
        vocab.extend(genres);
        vocab.extend(moods);
        vocab.extend(tempo);
        vocab.extend(eras);
        vocab.extend(instruments);
        vocab.extend(descriptors);

        vocab.into_iter().map(|s| s.to_lowercase()).collect()
    }

    /// Tokenize text into words
    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Calculate TF (term frequency)
    fn term_frequencies(&self, tokens: &[String]) -> Vec<f32> {
        let mut tf = vec![0.0; self.dimension];
        if tokens.is_empty() {
            return tf;
        }
        let total = tokens.len() as f32;

        for token in tokens {
            if let Some(&idx) = self.vocabulary.get(token) {
                tf[idx] += 1.0;
            }
        }

        // Normalize by total tokens
        for count in &mut tf {
            *count /= total;
        }

        tf
    }
}

impl EmbeddingGenerator for SimpleEmbeddingGenerator {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let tokens = Self::tokenize(text);
        let tf = self.term_frequencies(&tokens);

        // Create embedding vector
        let mut embedding = vec![0.0; self.dimension];

        for idx in 0..self.dimension {
            embedding[idx] = tf[idx] * self.idf_values[idx];
        }

        // Normalize the vector
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        "simple-tfidf"
    }
}

impl Default for SimpleEmbeddingGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Embedding database for storing and searching embeddings
pub struct EmbeddingDatabase {
    /// Stored embeddings
    embeddings: HashMap<String, TrackEmbedding>,
    /// Embedding generator
    generator: Box<dyn EmbeddingGenerator>,
}

impl EmbeddingDatabase {
    /// Create a new embedding database
    pub fn new(generator: Box<dyn EmbeddingGenerator>) -> Self {
        Self {
            embeddings: HashMap::new(),
            generator,
        }
    }

    /// Create with default simple generator
    pub fn with_simple_generator() -> Self {
        Self::new(Box::new(SimpleEmbeddingGenerator::new()))
    }

    /// Add a track embedding
    pub fn add_track(&mut self, track: &Track) -> Result<(), EmbeddingError> {
        let text = format!(
            "{} {} {} {} {}",
            track.title,
            track.artist,
            track.album,
            track.genre.as_deref().unwrap_or(""),
            track.year.map(|y| y.to_string()).unwrap_or_default()
        );

        let vector = self.generator.embed(&text)?;
        let embedding = TrackEmbedding::new(track.id.clone(), vector, text);

        self.embeddings.insert(track.id.clone(), embedding);

        Ok(())
    }

    /// Remove a track embedding
    pub fn remove_track(&mut self, track_id: &str) {
        self.embeddings.remove(track_id);
    }

    /// Search for similar tracks
    pub fn search(&self, query: &str, limit: usize) -> Vec<(String, f32)> {
        let query_embedding = match self.generator.embed(query) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut results: Vec<(String, f32)> = self.embeddings
            .values()
            .map(|e| {
                let sim = cosine_similarity(&query_embedding, &e.vector);
                (e.track_id.clone(), sim)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        results
    }

    /// Find similar tracks to a given track
    pub fn find_similar(&self, track_id: &str, limit: usize) -> Vec<(String, f32)> {
        let embedding = match self.embeddings.get(track_id) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let mut results: Vec<(String, f32)> = self.embeddings
            .values()
            .filter(|e| e.track_id != track_id)
            .map(|e| {
                let sim = embedding.cosine_similarity(e);
                (e.track_id.clone(), sim)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        results
    }

    /// Get embedding count
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Clear all embeddings
    pub fn clear(&mut self) {
        self.embeddings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];
        let d = vec![0.5, 0.5, 0.0];

        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
        assert!((cosine_similarity(&a, &d) - 0.707).abs() < 0.01);
    }

    #[test]
    fn test_simple_embedding_generator() {
        let gen = SimpleEmbeddingGenerator::new();

        let emb1 = gen.embed("rock music energetic electric guitar").unwrap();
        let emb2 = gen.embed("jazz piano calm relaxing").unwrap();
        let emb3 = gen.embed("rock guitar high energy").unwrap();

        // Similar descriptions should have higher similarity
        let sim_13 = cosine_similarity(&emb1, &emb3);
        let sim_12 = cosine_similarity(&emb1, &emb2);

        assert!(sim_13 > sim_12, "Rock descriptions should be more similar");
    }

    #[test]
    fn test_embedding_database() {
        let mut db = EmbeddingDatabase::with_simple_generator();

        let track = Track {
            id: "test-1".to_string(),
            title: "Bohemian Rhapsody".to_string(),
            artist: "Queen".to_string(),
            album: "A Night at the Opera".to_string(),
            duration: 354.0,
            path: std::path::PathBuf::from("/test.mp3"),
            track_number: Some(11),
            genre: Some("Rock".to_string()),
            year: Some(1975),
            source: crate::app::TrackSource::Local,
        };

        db.add_track(&track).unwrap();
        assert_eq!(db.len(), 1);

        let results = db.search("rock queen", 5);
        assert!(!results.is_empty());
    }
}
