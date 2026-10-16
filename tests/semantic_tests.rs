//! Semantic search against a mock Ollama whose embeddings group words by meaning
//! (sad / happy / calm), so a query can match a title sharing none of its words.

mod common;

use std::path::PathBuf;

use common::{fixed, MockServer};
use symphony::ai::semantic::{embedding_cache_path, semantic_rank};
use symphony::ai::AIProviderConfig;
use symphony::app::{Track, TrackSource};

/// Embedding = how many words of each concept the text contains
fn concept_embedding(text: &str) -> Vec<f32> {
    const CONCEPTS: [&[&str]; 3] = [
        &["sad", "melancholy", "rain", "tears", "blue", "lonely"],
        &["happy", "sunny", "joy", "party", "dance", "celebration"],
        &["calm", "quiet", "sleep", "ambient", "lullaby", "peaceful"],
    ];
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .map(String::from)
        .collect();
    CONCEPTS
        .iter()
        .map(|concept| 0.01 + words.iter().filter(|w| concept.contains(&w.as_str())).count() as f32)
        .collect()
}

fn mock_embeddings() -> MockServer {
    MockServer::start(vec![(
        "POST /api/embeddings",
        Box::new(|request| {
            let body: serde_json::Value = serde_json::from_str(&request.body).unwrap_or_default();
            let vector = concept_embedding(body["prompt"].as_str().unwrap_or_default());
            (200, serde_json::json!({ "embedding": vector }).to_string())
        }),
    )])
}

fn track(title: &str) -> Track {
    Track {
        id: title.into(),
        title: title.into(),
        artist: "Unknown Artist".into(),
        album: "Unknown".into(),
        duration: 200.0,
        path: PathBuf::from(format!("{title}.mp3")),
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

fn library() -> Vec<Track> {
    vec![track("Sunny Day Party"), track("Melancholy Rain"), track("Lullaby for Sleep")]
}

fn ollama(url: &str) -> AIProviderConfig {
    AIProviderConfig::Ollama { base_url: url.into(), model: "llm".into() }
}

#[tokio::test]
async fn embeddings_match_meaning_not_words() {
    let server = mock_embeddings();
    let cache = tempfile::tempdir().unwrap();
    let tracks = library();

    // "feeling blue" shares no word with "Melancholy Rain"
    let result = semantic_rank(&ollama(&server.url), "test-model", cache.path(), "feeling blue", &tracks).await;
    assert_eq!(result.method, "embeddings (test-model)");
    assert!(result.note.is_none());
    assert_eq!(tracks[result.hits[0].0].title, "Melancholy Rain");
    assert_eq!(result.hits.len(), 1, "the other tracks are far below the best match");

    let result = semantic_rank(&ollama(&server.url), "test-model", cache.path(), "peaceful bedtime", &tracks).await;
    assert_eq!(tracks[result.hits[0].0].title, "Lullaby for Sleep");
}

#[tokio::test]
async fn track_vectors_are_cached_on_disk() {
    let server = mock_embeddings();
    let cache = tempfile::tempdir().unwrap();
    let mut tracks = library();

    semantic_rank(&ollama(&server.url), "test-model", cache.path(), "sad", &tracks).await;
    assert_eq!(server.count("POST /api/embeddings"), 4, "3 tracks + the query");
    assert!(embedding_cache_path(cache.path(), "test-model").exists());

    // same library: only the query is embedded again
    semantic_rank(&ollama(&server.url), "test-model", cache.path(), "happy", &tracks).await;
    assert_eq!(server.count("POST /api/embeddings"), 5);

    // one new track: one more request for it, plus the query
    tracks.push(track("Peaceful Ambient Night"));
    semantic_rank(&ollama(&server.url), "test-model", cache.path(), "calm", &tracks).await;
    assert_eq!(server.count("POST /api/embeddings"), 7);
}

#[tokio::test]
async fn falls_back_to_keywords_when_embeddings_are_unavailable() {
    let server = MockServer::start(vec![("POST /api/embeddings", fixed(404, r#"{"error":"model not found"}"#))]);
    let cache = tempfile::tempdir().unwrap();
    let tracks = library();

    let result = semantic_rank(&ollama(&server.url), "missing-model", cache.path(), "rain", &tracks).await;
    assert_eq!(result.method, "keywords");
    assert!(result.note.as_deref().is_some_and(|n| n.contains("embeddings unavailable")), "{:?}", result.note);
    assert_eq!(tracks[result.hits[0].0].title, "Melancholy Rain");
}

#[tokio::test]
async fn unreachable_server_also_falls_back() {
    let cache = tempfile::tempdir().unwrap();
    let tracks = library();
    let result = semantic_rank(&ollama("http://127.0.0.1:9"), "m", cache.path(), "sunny", &tracks).await;
    assert_eq!(result.method, "keywords");
    assert_eq!(tracks[result.hits[0].0].title, "Sunny Day Party");
}

#[tokio::test]
async fn openrouter_uses_keywords_without_any_request() {
    let cache = tempfile::tempdir().unwrap();
    let provider = AIProviderConfig::OpenRouter {
        api_key: "k".into(),
        model: "m".into(),
        site_url: None,
        app_name: None,
    };
    let tracks = library();
    let result = semantic_rank(&provider, "unused", cache.path(), "lullaby", &tracks).await;
    assert_eq!(result.method, "keywords");
    assert!(result.note.is_none());
    assert_eq!(tracks[result.hits[0].0].title, "Lullaby for Sleep");
}
