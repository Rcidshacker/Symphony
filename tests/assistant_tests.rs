//! The AI assistant end to end: a prompt goes to a mock Ollama, the intent that comes back
//! drives playlists, suggestions, search, answers and context. Needs an audio device (tests
//! are skipped without one); playback is muted.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{fixed, write_wav, MockServer};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::audio::features::AudioFeatures;
use symphony::audio::AudioEngine;
use symphony::config::Config;
use symphony::db::Database;
use symphony::input::handle_key_event;
use symphony::scanner::LibraryScanner;

/// (title, energy, bpm): a loud fast track, a quiet slow one and two in between
const LIBRARY: [(&str, f32, f32); 4] = [
    ("Gym Anthem", 0.9, 150.0),
    ("Melancholy Rain", 0.15, 70.0),
    ("Sunday Stroll", 0.4, 95.0),
    ("Night Drive", 0.7, 120.0),
];

fn chat_reply(intent_json: &str) -> (u16, String) {
    let body = serde_json::json!({ "message": { "role": "assistant", "content": intent_json } });
    (200, body.to_string())
}

/// Ollama stand-in: understands a handful of prompts, answers questions, embeds by concept
fn mock_ollama() -> MockServer {
    MockServer::start(vec![
        ("GET /api/tags", fixed(200, "{}")),
        (
            "POST /api/chat",
            Box::new(|request| {
                // only the user's message: the system prompt contains examples of every intent
                let body: serde_json::Value =
                    serde_json::from_str(&request.body).unwrap_or_default();
                let prompt = body["messages"]
                    .as_array()
                    .and_then(|m| m.last())
                    .and_then(|m| m["content"].as_str())
                    .unwrap_or_default()
                    .to_lowercase();
                chat_reply(if prompt.contains("something energetic") {
                    r#"{"intent":"play","mood":"energetic"}"#
                } else if prompt.contains("something calm") {
                    r#"{"intent":"play","mood":"calm"}"#
                } else if prompt.contains("focus session") {
                    r#"{"intent":"create_playlist","theme":"focus session","duration_mins":120}"#
                } else if prompt.contains("more like gym") {
                    r#"{"intent":"suggest","based_on":"Gym Anthem","count":5}"#
                } else if prompt.contains("something sad") {
                    r#"{"intent":"search","query":"something sad"}"#
                } else if prompt.contains("what should i listen to") {
                    r#"{"intent":"unknown","query":"what should I listen to while debugging?"}"#
                } else {
                    r#"{"intent":"unknown","query":"?"}"#
                })
            }),
        ),
        (
            "POST /api/generate",
            fixed(
                200,
                r#"{"response":"Try some lo-fi beats: steady and unobtrusive.","done":true}"#,
            ),
        ),
        (
            "POST /api/embeddings",
            Box::new(|request| {
                let body: serde_json::Value =
                    serde_json::from_str(&request.body).unwrap_or_default();
                let text = body["prompt"].as_str().unwrap_or_default().to_lowercase();
                let sad = ["sad", "melancholy", "rain", "tears"];
                let vector: Vec<f32> = [
                    sad.iter().filter(|w| text.contains(*w)).count() as f32,
                    ["gym", "anthem", "energetic"]
                        .iter()
                        .filter(|w| text.contains(*w))
                        .count() as f32,
                    ["stroll", "drive", "night", "sunday"]
                        .iter()
                        .filter(|w| text.contains(*w))
                        .count() as f32,
                ]
                .iter()
                .map(|v| v + 0.01)
                .collect();
                (200, serde_json::json!({ "embedding": vector }).to_string())
            }),
        ),
    ])
}

fn track_for(title: &str, path: PathBuf) -> Track {
    Track {
        id: title.into(),
        title: title.into(),
        artist: "Unknown Artist".into(),
        album: "Unknown".into(),
        duration: 180.0,
        path,
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

/// App with the four-track library (real silent files, features measured as in LIBRARY)
fn app_with_library(dir: &Path, ollama_url: &str) -> Option<App> {
    let mut config = Config::default();
    config.ai.enabled = true;
    config.ai.provider = "ollama".into();
    config.ai.ollama.base_url = ollama_url.to_string();
    config.general.cache_directory = dir.join("cache");
    config.lyrics.auto_fetch = false;

    let engine = AudioEngine::new().ok()?;
    let mut app = App::new(config, engine);
    app.audio_engine.set_volume(0.0);
    for (title, energy, bpm) in LIBRARY {
        let path = dir.join(format!("{title}.wav"));
        write_wav(&path, &[], 3.0, 0.0);
        app.features.insert(
            path.to_string_lossy().to_string(),
            AudioFeatures {
                energy,
                bpm,
                beat_strength: 0.4,
            },
        );
        app.library.tracks.push(track_for(title, path));
    }
    Some(app)
}

fn wait_for(app: &mut App, mut done: impl FnMut(&App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

fn playing_title(app: &App) -> Option<String> {
    app.playback_state
        .current_track
        .as_ref()
        .map(|t| t.title.clone())
}

fn queue_titles(app: &App) -> Vec<String> {
    app.library.queue.iter().map(|t| t.title.clone()).collect()
}

fn screen(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
    terminal.draw(|f| symphony::ui::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..32u16)
        .map(|y| {
            (0..120u16)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_energetic_prompt_plays_the_loudest_fastest_track_first() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("play something energetic");
    assert!(
        wait_for(&mut app, |a| playing_title(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );

    assert_eq!(playing_title(&app).as_deref(), Some("Gym Anthem"));
    assert_eq!(
        queue_titles(&app)[..2],
        ["Gym Anthem", "Night Drive"],
        "best matches first"
    );
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.starts_with("AI:")));
}

#[test]
fn a_calm_prompt_plays_the_quietest_slowest_track_first() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("play something calm");
    assert!(wait_for(&mut app, |a| playing_title(a).is_some()));
    assert_eq!(playing_title(&app).as_deref(), Some("Melancholy Rain"));
    assert_eq!(
        queue_titles(&app).last().map(String::as_str),
        Some("Gym Anthem")
    );
}

#[test]
fn a_focus_session_playlist_is_built_calm_first_and_reports_the_shortfall() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("create a 2 hour focus session");
    assert!(wait_for(&mut app, |a| playing_title(a).is_some()));

    assert_eq!(playing_title(&app).as_deref(), Some("Melancholy Rain"));
    assert_eq!(app.library.queue.len(), 4, "every track fits in two hours");
    let status = app.ui_state.status_message.clone().unwrap_or_default();
    assert!(status.contains("playlist 'focus session'"), "{status}");
    assert!(status.contains("only has 12 of the 120 min"), "{status}");
}

#[test]
fn suggestions_are_the_tracks_that_sound_most_alike() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("more like gym");
    assert!(
        wait_for(&mut app, |a| a.ui_state.pinned_results),
        "{:?}",
        app.ui_state.error_message
    );

    let titles: Vec<_> = app
        .library
        .filtered_tracks
        .iter()
        .map(|t| t.title.as_str())
        .collect();
    assert_eq!(
        titles,
        vec!["Night Drive", "Sunday Stroll", "Melancholy Rain"],
        "closest sound first, seed excluded"
    );
    assert!(app.ui_state.search_query.contains("similar to Gym Anthem"));
}

#[test]
fn a_search_prompt_finds_tracks_by_meaning() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("find something sad");
    assert!(
        wait_for(&mut app, |a| a.ui_state.pinned_results),
        "{:?}",
        app.ui_state.error_message
    );
    assert_eq!(app.library.filtered_tracks[0].title, "Melancholy Rain");
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("embeddings")));
}

#[test]
fn questions_are_answered_in_a_popup_that_any_key_closes() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.submit_prompt("what should I listen to while debugging?");
    assert!(
        wait_for(&mut app, |a| a.ui_state.ai_answer.is_some()),
        "{:?}",
        app.ui_state.error_message
    );
    assert!(screen(&app).contains("Try some lo-fi beats"));

    // the key that closes the popup does nothing else (q would quit)
    let keep_running = handle_key_event(
        &mut app,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )
    .unwrap();
    assert!(keep_running);
    assert!(app.ui_state.ai_answer.is_none());
}

#[test]
fn the_model_is_told_about_the_moment() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };

    app.toggle_pomodoro(); // a focus session is running
    app.submit_prompt("play something calm");
    assert!(wait_for(&mut app, |a| playing_title(a).is_some()));

    let sent = ollama
        .requests()
        .into_iter()
        .find(|r| r.target.starts_with("/api/chat"))
        .expect("the prompt reached the model")
        .body;
    assert!(sent.contains("Library size: 4 tracks"), "{sent}");
    assert!(sent.contains("User is in focus mode"), "{sent}");
    assert!(sent.contains("Current time:"), "{sent}");

    // and what is playing is part of the next prompt
    app.submit_prompt("play something energetic");
    assert!(wait_for(&mut app, |a| playing_title(a).as_deref()
        == Some("Gym Anthem")));
    app.submit_prompt("play something calm");
    assert!(wait_for(&mut app, |a| playing_title(a).as_deref()
        == Some("Melancholy Rain")));
    let last = ollama
        .requests()
        .into_iter()
        .rfind(|r| r.target.starts_with("/api/chat"))
        .unwrap()
        .body;
    assert!(last.contains("Currently playing: Gym Anthem"), "{last}");
}

#[test]
fn the_library_is_analyzed_in_the_background_and_remembered() {
    let dir = tempfile::tempdir().unwrap();
    write_wav(&dir.path().join("loud.wav"), &[440.0], 4.0, 0.9);
    write_wav(&dir.path().join("quiet.wav"), &[440.0], 4.0, 0.03);
    write_wav(&dir.path().join("silence.wav"), &[], 4.0, 0.0);

    let db = Database::in_memory().unwrap();
    for track in LibraryScanner::new().scan_directory(dir.path()).unwrap() {
        db.upsert_track(&track).unwrap();
    }
    let Ok(engine) = AudioEngine::new() else {
        return;
    };
    let mut app = App::new(Config::default(), engine);
    app.database = Some(db);
    app.load_library().unwrap();
    assert_eq!(app.library.tracks.len(), 3);
    assert_eq!(app.ui_state.analysis_remaining, 3);

    assert!(
        wait_for(&mut app, |a| a.ui_state.analysis_remaining == 0),
        "analysis never finished"
    );
    assert_eq!(app.features.len(), 3);

    let energy = |name: &str| -> f32 {
        let path = dir.path().join(name).to_string_lossy().to_string();
        app.features[&path].energy
    };
    assert!(
        energy("loud.wav") > energy("quiet.wav") && energy("quiet.wav") > energy("silence.wav")
    );

    // stored in the database: a restart does not analyze again
    let stored = app.database.as_ref().unwrap().all_features().unwrap();
    assert_eq!(stored.len(), 3);
}

#[test]
fn a_prompt_from_the_command_line_respects_the_ai_switch() {
    let (ollama, dir) = (mock_ollama(), tempfile::tempdir().unwrap());
    let Some(mut app) = app_with_library(dir.path(), &ollama.url) else {
        return;
    };
    app.config.ai.enabled = false;

    app.submit_prompt("play something calm");
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("disabled")));
    assert!(ollama.requests().is_empty());
}
