//! Semantic library search.
//!
//! With a local Ollama server, tracks and the query are turned into embeddings and ranked by
//! cosine similarity, so "something sad" finds "Melancholy Rain". Vectors are cached on disk
//! (one JSON file per model). Without embeddings (OpenRouter, Ollama not running) it falls
//! back to keyword search weighted by how rare each word is in the library.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;

use super::provider::AIProviderConfig;
use crate::app::Track;

/// Parallel embedding requests while indexing
const CONCURRENCY: usize = 4;

/// Results kept: at most this many...
const MAX_HITS: usize = 15;
/// ...and only those scoring at least this share of the best one
const RELATIVE_CUTOFF: f32 = 0.6;

/// Ranked hits from a semantic search
#[derive(Debug, Clone)]
pub struct SemanticHits {
    /// `(index into the searched tracks, score)`, best first
    pub hits: Vec<(usize, f32)>,
    /// How the ranking was made: "embeddings (model)" or "keywords"
    pub method: String,
    /// Why the fallback was used, if it was
    pub note: Option<String>,
}

/// Text describing a track for matching (unknown fields are left out)
pub fn track_text(track: &Track) -> String {
    let mut parts: Vec<String> = Vec::new();
    for field in [&track.title, &track.artist, &track.album] {
        if !field.is_empty() && !field.starts_with("Unknown") {
            parts.push(field.clone());
        }
    }
    if let Some(genre) = track.genre.as_ref().filter(|g| !g.is_empty()) {
        parts.push(genre.clone());
    }
    if let Some(year) = track.year {
        parts.push(year.to_string());
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Keyword fallback
// ---------------------------------------------------------------------------

const STOPWORDS: &[&str] = &[
    "a",
    "an",
    "the",
    "of",
    "and",
    "or",
    "to",
    "in",
    "on",
    "for",
    "with",
    "by",
    "is",
    "it",
    "my",
    "me",
    "you",
    "your",
    "do",
    "we",
    "i",
    "at",
    "from",
    "mv",
    "ft",
    "feat",
    "official",
    "full",
    "version",
    "audio",
    "video",
    "songs",
    "song",
    "music",
    "play",
    "find",
    "something",
    "some",
    "like",
    "that",
    "this",
];

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF   // hiragana, katakana
        | 0x3400..=0x4DBF // CJK extension A
        | 0x4E00..=0x9FFF // CJK unified
        | 0xAC00..=0xD7AF // hangul
    )
}

/// Lowercase words; runs of CJK characters become character pairs (no spaces to split on)
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut cjk: Vec<char> = Vec::new();

    let flush_word = |word: &mut String, tokens: &mut Vec<String>| {
        if word.chars().count() >= 2 && !STOPWORDS.contains(&word.as_str()) {
            tokens.push(std::mem::take(word));
        } else {
            word.clear();
        }
    };
    let flush_cjk = |cjk: &mut Vec<char>, tokens: &mut Vec<String>| {
        match cjk.len() {
            0 => {}
            1 => tokens.push(cjk[0].to_string()),
            _ => tokens.extend(cjk.windows(2).map(|w| w.iter().collect::<String>())),
        }
        cjk.clear();
    };

    for c in text.chars() {
        if is_cjk(c) {
            flush_word(&mut word, &mut tokens);
            cjk.push(c);
        } else if c.is_alphanumeric() {
            flush_cjk(&mut cjk, &mut tokens);
            word.extend(c.to_lowercase());
        } else {
            flush_word(&mut word, &mut tokens);
            flush_cjk(&mut cjk, &mut tokens);
        }
    }
    flush_word(&mut word, &mut tokens);
    flush_cjk(&mut cjk, &mut tokens);
    tokens
}

type SparseVector = HashMap<String, f32>;

fn tf_idf(tokens: &[String], idf: &HashMap<String, f32>) -> SparseVector {
    let mut vector = SparseVector::new();
    for token in tokens {
        if let Some(weight) = idf.get(token) {
            *vector.entry(token.clone()).or_default() += weight;
        }
    }
    vector
}

fn sparse_cosine(a: &SparseVector, b: &SparseVector) -> f32 {
    let dot: f32 = a.iter().filter_map(|(k, v)| b.get(k).map(|w| v * w)).sum();
    let norm = |v: &SparseVector| v.values().map(|x| x * x).sum::<f32>().sqrt();
    let denominator = norm(a) * norm(b);
    if denominator == 0.0 {
        0.0
    } else {
        dot / denominator
    }
}

/// Keyword ranking: TF-IDF with the vocabulary taken from the library itself
pub fn lexical_rank(query: &str, tracks: &[Track]) -> Vec<(usize, f32)> {
    let documents: Vec<Vec<String>> = tracks.iter().map(|t| tokenize(&track_text(t))).collect();

    let mut document_frequency: HashMap<&str, usize> = HashMap::new();
    for document in &documents {
        for token in document.iter().collect::<HashSet<_>>() {
            *document_frequency.entry(token.as_str()).or_default() += 1;
        }
    }
    let n = documents.len() as f32;
    let idf: HashMap<String, f32> = document_frequency
        .into_iter()
        .map(|(token, df)| {
            (
                token.to_string(),
                ((n + 1.0) / (df as f32 + 1.0)).ln() + 1.0,
            )
        })
        .collect();

    let query_vector = tf_idf(&tokenize(query), &idf);
    let mut ranked: Vec<(usize, f32)> = documents
        .iter()
        .enumerate()
        .map(|(i, document)| (i, sparse_cosine(&query_vector, &tf_idf(document, &idf))))
        .filter(|(_, score)| *score > 0.0)
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
}

// ---------------------------------------------------------------------------
// Embeddings (Ollama)
// ---------------------------------------------------------------------------

/// Client for Ollama's `/api/embeddings`
#[derive(Clone)]
pub struct OllamaEmbedder {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

impl OllamaEmbedder {
    /// Embedder for `model` on the server at `base_url`
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                // an unreachable server must fail fast so the keyword fallback can take over
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    /// The model name
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Embedding of `text`
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let response = self
            .client
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&serde_json::json!({ "model": self.model, "prompt": text }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "Ollama returned {status} (is '{}' pulled?)",
                self.model
            ));
        }
        let parsed: EmbeddingResponse = response.json().await.map_err(|e| e.to_string())?;
        if parsed.embedding.is_empty() {
            return Err("Ollama returned an empty embedding".to_string());
        }
        Ok(parsed.embedding)
    }
}

fn dense_cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denominator = norm(a) * norm(b);
    if denominator == 0.0 {
        0.0
    } else {
        dot / denominator
    }
}

fn load_cache(path: Option<&Path>) -> HashMap<String, Vec<f32>> {
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_cache(path: Option<&Path>, cache: &HashMap<String, Vec<f32>>) {
    let Some(path) = path else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

/// Rank `tracks` against `query` by embedding similarity. Track vectors come from (and go to)
/// `cache_file`, so only new or changed tracks cost a request.
pub async fn neural_rank(
    embedder: &OllamaEmbedder,
    cache_file: Option<&Path>,
    query: &str,
    tracks: &[Track],
) -> Result<Vec<(usize, f32)>, String> {
    // Ask for the query first: if the server is down this fails before any track is embedded
    let query_vector = embedder.embed(query).await?;

    let mut cache = load_cache(cache_file);
    let texts: Vec<String> = tracks.iter().map(track_text).collect();

    let mut wanted: Vec<&String> = texts.iter().filter(|t| !cache.contains_key(*t)).collect();
    wanted.sort();
    wanted.dedup();
    let wanted: Vec<String> = wanted.into_iter().cloned().collect();

    let mut embedded = stream::iter(wanted)
        .map(|text| async move {
            let vector = embedder.embed(&text).await;
            (text, vector)
        })
        .buffer_unordered(CONCURRENCY);

    let mut added = false;
    while let Some((text, vector)) = embedded.next().await {
        match vector {
            Ok(vector) => {
                cache.insert(text, vector);
                added = true;
            }
            Err(e) => {
                // keep what was embedded so far: a retry only needs the rest
                if added {
                    save_cache(cache_file, &cache);
                }
                return Err(e);
            }
        }
    }
    if added {
        save_cache(cache_file, &cache);
    }

    let mut ranked: Vec<(usize, f32)> = texts
        .iter()
        .enumerate()
        .filter_map(|(i, text)| cache.get(text).map(|v| (i, dense_cosine(&query_vector, v))))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(ranked)
}

/// Keep the best few: at most [`MAX_HITS`], all within [`RELATIVE_CUTOFF`] of the top score
fn keep_best(mut ranked: Vec<(usize, f32)>) -> Vec<(usize, f32)> {
    let Some(&(_, top)) = ranked.first() else {
        return ranked;
    };
    if top <= 0.0 {
        return Vec::new();
    }
    ranked.retain(|(_, score)| *score >= top * RELATIVE_CUTOFF);
    ranked.truncate(MAX_HITS);
    ranked
}

/// Search the library the best way the provider allows
pub async fn semantic_rank(
    provider: &AIProviderConfig,
    embedding_model: &str,
    cache_dir: &Path,
    query: &str,
    tracks: &[Track],
) -> SemanticHits {
    let keywords = |note: Option<String>| SemanticHits {
        hits: keep_best(lexical_rank(query, tracks)),
        method: "keywords".to_string(),
        note,
    };

    match provider {
        AIProviderConfig::Ollama { base_url, .. } => {
            let embedder = OllamaEmbedder::new(base_url, embedding_model);
            let cache_file = embedding_cache_path(cache_dir, embedding_model);
            match neural_rank(&embedder, Some(&cache_file), query, tracks).await {
                Ok(ranked) => SemanticHits {
                    hits: keep_best(ranked),
                    method: format!("embeddings ({embedding_model})"),
                    note: None,
                },
                Err(e) => keywords(Some(format!("embeddings unavailable ({e}); used keywords"))),
            }
        }
        AIProviderConfig::OpenRouter { .. } => keywords(None),
    }
}

/// Cache file for one embedding model
pub fn embedding_cache_path(cache_dir: &Path, model: &str) -> PathBuf {
    let safe: String = model
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cache_dir.join(format!("embeddings-{safe}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TrackSource;

    fn track(title: &str, artist: &str) -> Track {
        Track {
            id: title.into(),
            title: title.into(),
            artist: artist.into(),
            album: "Unknown".into(),
            duration: 200.0,
            path: PathBuf::from(format!("{title}.mp3")),
            track_number: None,
            genre: None,
            year: None,
            source: TrackSource::Local,
        }
    }

    #[test]
    fn tokenize_drops_noise_and_splits_cjk_into_pairs() {
        assert_eq!(
            tokenize("Do you want to build a Snowman?!"),
            vec!["want", "build", "snowman"]
        );
        assert_eq!(tokenize("🎶 Lo-Fi Beats ☃️"), vec!["lo", "fi", "beats"]);
        assert_eq!(
            tokenize("旧支配者のキャロル"),
            vec!["旧支", "支配", "配者", "者の", "のキ", "キャ", "ャロ", "ロル"]
        );
        assert_eq!(tokenize("歌 band"), vec!["歌", "band"]);
    }

    #[test]
    fn keyword_search_prefers_rare_words_and_ignores_unrelated_queries() {
        let library = vec![
            track("Build a Snowman", "Kristen"),
            track("Build a Fire", "Someone"),
            track("Winter Song", "Kristen"),
        ];
        // "build" is in two tracks, "snowman" in one: the rarer word wins
        let ranked = lexical_rank("build snowman", &library);
        assert_eq!(ranked[0].0, 0);
        assert!(ranked[0].1 > ranked[1].1);

        assert!(lexical_rank("quantum chromodynamics", &library).is_empty());
        assert!(
            lexical_rank("the of and", &library).is_empty(),
            "stopwords only"
        );
    }

    #[test]
    fn keyword_search_finds_japanese_titles() {
        let library = vec![track("旧支配者のキャロル", "ほんだ"), track("Other", "x")];
        let ranked = lexical_rank("キャロル", &library);
        assert_eq!(ranked[0].0, 0);
    }

    #[test]
    fn unknown_fields_are_not_part_of_the_text() {
        let t = track("Song", "Unknown Artist");
        assert_eq!(track_text(&t), "Song");
    }

    #[test]
    fn keep_best_applies_the_relative_cutoff_and_the_limit() {
        let ranked = vec![(0, 1.0), (1, 0.7), (2, 0.5), (3, 0.1)];
        assert_eq!(keep_best(ranked), vec![(0, 1.0), (1, 0.7)]);
        assert!(keep_best(vec![(0, 0.0)]).is_empty());
        let many: Vec<_> = (0..40).map(|i| (i, 1.0)).collect();
        assert_eq!(keep_best(many).len(), MAX_HITS);
    }

    #[test]
    fn cache_path_is_filesystem_safe() {
        let path = embedding_cache_path(Path::new("/c"), "nomic-embed-text:latest");
        assert_eq!(
            path.file_name().unwrap(),
            "embeddings-nomic-embed-text_latest.json"
        );
    }
}
