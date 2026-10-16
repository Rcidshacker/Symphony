//! The assistant side of the app: acting on what the AI understood, semantic search,
//! suggestions, conversational answers, and the background audio analysis that makes moods
//! and tempos meaningful for libraries without tags.

use std::collections::HashMap;
use std::path::Path;

use super::{App, BackgroundEvent, RepeatMode, Track};
use crate::ai::context::TimeOfDay;
use crate::ai::mood::mood_profile;
use crate::ai::playlists::{PlaylistCriteria, SmartPlaylistGenerator};
use crate::ai::recommendations::RecommendationEngine;
use crate::ai::semantic::{lexical_rank, semantic_rank};
use crate::ai::suggest::suggest;
use crate::ai::{AIContext, AIManager, MusicIntent};
use crate::audio::features::{analyze_file, AudioFeatures};
use crate::integrations::PomodoroState;

/// What to do with the tracks a semantic search found
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchKind {
    /// Show them as a list to pick from
    Browse,
    /// Queue them and start playing
    PlayBest,
}

/// Suggestions returned when the AI gives no count
const DEFAULT_SUGGESTIONS: usize = 10;

impl App {
    /// Act on what the AI understood
    pub fn apply_ai_intent(&mut self, intent: MusicIntent) {
        let description = intent.description();
        match &intent {
            MusicIntent::Play { .. } => self.ai_play(&intent, &description),
            MusicIntent::CreatePlaylist {
                theme,
                duration_mins,
            } => self.ai_create_playlist(theme, *duration_mins),
            MusicIntent::Search { query } => self.ai_semantic_search(query.clone(), SearchKind::Browse),
            MusicIntent::Suggest { based_on, count } => self.ai_suggest(based_on.as_deref(), *count),
            MusicIntent::Skip { .. } => self.next_track(),
            MusicIntent::SetState {
                volume,
                shuffle,
                repeat,
            } => {
                if let Some(volume) = volume {
                    self.set_volume(f32::from((*volume).min(100)) / 100.0);
                }
                if let Some(shuffle) = shuffle {
                    if *shuffle != self.playback_state.shuffle {
                        self.toggle_shuffle();
                    }
                }
                if let Some(repeat) = repeat {
                    self.playback_state.repeat = match repeat.to_lowercase().as_str() {
                        "all" => RepeatMode::All,
                        "one" | "track" | "song" => RepeatMode::One,
                        _ => RepeatMode::Off,
                    };
                }
            }
            MusicIntent::Info { about } => {
                let question = match (about, self.playback_state.current_track.as_ref()) {
                    (Some(about), Some(track)) => {
                        format!("Tell me about the {about} of \"{}\" by {}.", track.title, track.artist)
                    }
                    (Some(about), None) => format!("Tell me about {about}."),
                    (None, Some(track)) => format!("Tell me about \"{}\" by {}.", track.title, track.artist),
                    (None, None) => "What can you tell me about music?".to_string(),
                };
                self.ai_ask(question);
            }
            MusicIntent::Unknown { query } => self.ai_ask(query.clone()),
        }
    }

    /// Queue `tracks` and start the first one. The order is kept unless `shuffle` or shuffle mode is on.
    fn start_queue(&mut self, tracks: Vec<Track>, shuffle: bool) {
        self.library.queue = tracks;
        self.library.queue_index = 0;
        if shuffle || self.playback_state.shuffle {
            self.shuffle_queue();
        }
        self.play_track_at_index(0);
    }

    fn ai_play(&mut self, intent: &MusicIntent, description: &str) {
        let mut criteria = PlaylistCriteria::from_intent(intent);
        let any_criteria = criteria.genre.is_some()
            || criteria.mood.is_some()
            || !criteria.artists.is_empty()
            || criteria.tempo.is_some()
            || criteria.era.is_some();

        // A focus session in progress means focus music, unless something else was asked for
        let focus_session = !any_criteria && self.pomodoro.get_state() == PomodoroState::Working;
        if focus_session {
            criteria.mood = Some("focus".to_string());
        }

        let generator = SmartPlaylistGenerator::new().with_features(self.features.clone());
        match generator.generate(&self.library.tracks, &criteria) {
            Ok(playlist) => {
                let count = playlist.tracks.len();
                let mut notes = playlist.notes;
                if focus_session {
                    notes.push("focus session: calm music".to_string());
                }
                self.start_queue(playlist.tracks, !any_criteria && !focus_session);
                if self.ui_state.error_message.is_none() {
                    let notes = if notes.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", notes.join("; "))
                    };
                    self.ui_state.status_message =
                        Some(format!("AI: {description} - {count} tracks{notes}"));
                }
            }
            // an artist or era nothing matches: look for the closest thing by meaning, using only
            // the words that were asked for ("artist:" and "music" would match everything)
            Err(_) => {
                let asked = match intent {
                    MusicIntent::Play { genre, mood, artist, era, .. } => [artist, genre, mood, era]
                        .into_iter()
                        .flatten()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" "),
                    _ => description.to_string(),
                };
                self.ai_semantic_search(asked, SearchKind::PlayBest);
            }
        }
    }

    fn ai_create_playlist(&mut self, theme: &str, duration_mins: u32) {
        let mut criteria = PlaylistCriteria::from_intent(&MusicIntent::CreatePlaylist {
            theme: theme.to_string(),
            duration_mins,
        });
        // "2 hour focus session": the mood is the word in the theme that names one
        criteria.mood = theme
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .find(|word| mood_profile(word).is_some())
            .map(str::to_string);

        let generator = SmartPlaylistGenerator::new().with_features(self.features.clone());
        let playlist = match generator.generate(&self.library.tracks, &criteria) {
            Ok(playlist) => playlist,
            Err(e) => {
                self.ui_state.error_message = Some(format!("AI: could not build '{theme}': {e}"));
                return;
            }
        };

        let saved = self.save_playlist(&playlist.name, &playlist.tracks);
        let minutes = (playlist.total_duration / 60.0).round() as u32;
        let mut message = format!(
            "AI: playlist '{}': {} tracks, {} min{}",
            playlist.name,
            playlist.tracks.len(),
            minutes,
            if saved { ", saved" } else { "" }
        );
        if minutes + 1 < duration_mins {
            message.push_str(&format!(" (your library only has {minutes} of the {duration_mins} min asked for)"));
        }
        for note in &playlist.notes {
            message.push_str(&format!(" ({note})"));
        }

        self.start_queue(playlist.tracks, false);
        if self.ui_state.error_message.is_none() {
            self.ui_state.status_message = Some(message);
        }
    }

    /// Store a generated playlist in the library database; `false` without a database
    fn save_playlist(&self, name: &str, tracks: &[Track]) -> bool {
        let Some(db) = &self.database else {
            return false;
        };
        let Ok(id) = db.create_playlist(name) else {
            return false;
        };
        for (position, track) in tracks.iter().enumerate() {
            let _ = db.add_to_playlist(&id, &track.id, position as i32);
        }
        true
    }

    // ---- semantic search ----

    /// Find tracks by meaning in the background (embeddings when available, else keywords)
    fn ai_semantic_search(&mut self, query: String, kind: SearchKind) {
        let provider = self.config.ai.get_provider_config().ok();
        let (Some(provider), Some(runtime)) = (provider, &self.runtime) else {
            // no provider configured: keyword search needs nothing else
            let ids = self.keyword_ids(&query);
            self.apply_semantic(&query, kind, ids, "keywords", None);
            return;
        };

        let tracks = self.library.tracks.clone();
        let model = self.config.ai.ollama.embedding_model.clone();
        let cache_dir = self.config.general.cache_directory.join("embeddings");
        let tx = self.ai_tx.clone();
        self.ui_state.ai_pending = true;
        self.ui_state.status_message = Some(format!("AI: searching your library for \"{query}\"..."));

        runtime.spawn(async move {
            let result = semantic_rank(&provider, &model, &cache_dir, &query, &tracks).await;
            let ids = result.hits.iter().map(|(i, _)| tracks[*i].id.clone()).collect();
            let _ = tx.send(BackgroundEvent::Semantic {
                query,
                kind,
                ids,
                method: result.method,
                note: result.note,
            });
        });
    }

    fn keyword_ids(&self, query: &str) -> Vec<String> {
        lexical_rank(query, &self.library.tracks)
            .into_iter()
            .take(15)
            .map(|(i, _)| self.library.tracks[i].id.clone())
            .collect()
    }

    /// A semantic search finished
    pub(super) fn apply_semantic(
        &mut self,
        query: &str,
        kind: SearchKind,
        ids: Vec<String>,
        method: &str,
        note: Option<String>,
    ) {
        self.ui_state.ai_pending = false;
        let tracks: Vec<Track> = ids
            .iter()
            .filter_map(|id| self.library.tracks.iter().find(|t| &t.id == id).cloned())
            .collect();
        if tracks.is_empty() {
            self.ui_state.error_message = Some(format!("AI: nothing in your library matches \"{query}\""));
            return;
        }

        let count = tracks.len();
        let note = note.map(|n| format!(" - {n}")).unwrap_or_default();
        match kind {
            SearchKind::Browse => {
                self.show_results(&format!("≈ {query}"), tracks);
                self.ui_state.status_message =
                    Some(format!("AI: {count} matches by {method}, Enter plays{note}"));
            }
            SearchKind::PlayBest => {
                self.start_queue(tracks, false);
                if self.ui_state.error_message.is_none() {
                    self.ui_state.status_message =
                        Some(format!("AI: {count} closest tracks by {method} for \"{query}\"{note}"));
                }
            }
        }
    }

    /// Show a list of tracks in the search view (typing starts a normal search again)
    fn show_results(&mut self, label: &str, tracks: Vec<Track>) {
        self.ui_state.ai_mode = false;
        self.ui_state.search_mode = true;
        self.ui_state.search_query = label.to_string();
        self.ui_state.pinned_results = true;
        self.library.filtered_tracks = tracks;
        self.library.selected_index = 0;
    }

    // ---- suggestions ----

    fn ai_suggest(&mut self, based_on: Option<&str>, count: usize) {
        let count = if count == 0 { DEFAULT_SUGGESTIONS } else { count };
        let library = &self.library.tracks;

        let named_seed = based_on
            .and_then(|text| lexical_rank(text, library).first().map(|(i, _)| library[*i].clone()));
        let seed = named_seed.or_else(|| self.playback_state.current_track.clone());

        let (label, suggestions): (String, Vec<Track>) = match &seed {
            Some(seed) => (
                format!("similar to {}", seed.title),
                suggest(seed, library, &self.features, count)
                    .into_iter()
                    .map(|s| s.track)
                    .collect(),
            ),
            None => (
                "your daily mix".to_string(),
                RecommendationEngine::daily_mix(&self.history, library, count)
                    .into_iter()
                    .map(|r| r.track)
                    .collect(),
            ),
        };

        if suggestions.is_empty() {
            self.ui_state.error_message = Some(
                "AI: nothing similar found (the library may still be analyzing, or has too few tracks)".to_string(),
            );
            return;
        }
        let n = suggestions.len();
        self.show_results(&format!("✨ {label}"), suggestions);
        self.ui_state.status_message = Some(format!("AI: {n} suggestions, Enter plays"));
    }

    // ---- conversation ----

    /// Ask the AI a free-form question; the answer opens in a popup
    fn ai_ask(&mut self, question: String) {
        let provider = match self.config.ai.get_provider_config() {
            Ok(provider) => provider,
            Err(e) => {
                self.ui_state.error_message = Some(format!("AI: {e}"));
                return;
            }
        };
        let Some(runtime) = &self.runtime else {
            self.ui_state.error_message = Some("AI: could not start the async runtime".to_string());
            return;
        };

        let context = self.build_ai_context();
        let prompt = format!(
            "You are the assistant inside a terminal music player. {}\n\
             Answer in at most three short sentences.\nUser: {question}",
            context.to_context_string()
        );
        let tx = self.ai_tx.clone();
        self.ui_state.ai_pending = true;
        self.ui_state.status_message = Some("AI: thinking...".to_string());

        runtime.spawn(async move {
            let result = async {
                let manager = AIManager::new(provider).await?.with_context(context);
                manager.respond(&prompt).await
            }
            .await;
            let event = match result {
                Ok(answer) => BackgroundEvent::Answer(answer),
                Err(e) => BackgroundEvent::AiError(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    /// What the AI should know about the moment: time of day, focus session, what is playing
    pub(super) fn build_ai_context(&self) -> AIContext {
        let mut context = crate::ai::context::UserContext::new().to_ai_context();
        context.time_of_day = format!("{} ({})", TimeOfDay::current().display_name(), context.time_of_day);
        context.current_track = self.playback_state.current_track.as_ref().map(|t| t.title.clone());
        context.current_artist = self.playback_state.current_track.as_ref().map(|t| t.artist.clone());
        context.recent_tracks = self
            .history
            .iter()
            .rev()
            .take(5)
            .map(|t| format!("{} - {}", t.artist, t.title))
            .collect();
        context.favorite_genres = most_common_genres(&self.history, 3);
        context.library_size = self.library.tracks.len();
        context.focus_mode = self.pomodoro.get_state() == PomodoroState::Working;
        context
    }

    // ---- audio analysis ----

    /// Analyze tracks that have no stored features, one at a time in the background
    pub(super) fn start_feature_analysis(&mut self) {
        let Some(runtime) = &self.runtime else { return };
        let missing: Vec<String> = self
            .library
            .tracks
            .iter()
            .map(|t| t.path.to_string_lossy().to_string())
            .filter(|path| !self.features.contains_key(path) && Path::new(path).is_file())
            .collect();
        if missing.is_empty() {
            return;
        }

        self.ui_state.analysis_remaining = missing.len();
        let tx = self.ai_tx.clone();
        runtime.spawn(async move {
            for path in missing {
                let file = path.clone();
                let features = tokio::task::spawn_blocking(move || analyze_file(Path::new(&file)))
                    .await
                    .ok()
                    .flatten();
                let _ = tx.send(BackgroundEvent::Features { path, features });
            }
        });
    }

    /// One track was analyzed (or could not be)
    pub(super) fn apply_features(&mut self, path: String, features: Option<AudioFeatures>) {
        self.ui_state.analysis_remaining = self.ui_state.analysis_remaining.saturating_sub(1);
        if let Some(features) = features {
            if let Some(db) = &self.database {
                let _ = db.upsert_features(&path, &features);
            }
            self.features.insert(path, features);
        }
    }
}

/// The `limit` genres that occur most often in `tracks` (tags only)
fn most_common_genres(tracks: &[Track], limit: usize) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for genre in tracks.iter().filter_map(|t| t.genre.as_deref()) {
        *counts.entry(genre).or_default() += 1;
    }
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    ranked.into_iter().take(limit).map(|(g, _)| g.to_string()).collect()
}
