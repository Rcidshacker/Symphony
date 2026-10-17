//! Lyrics inside the player: online lookup when a track starts, `.lrc` priority, and AI
//! translation. LRCLIB and Ollama are replaced by a local mock server. Needs an audio device
//! (tests are skipped without one); playback is muted.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{fixed, MockServer};
use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::config::Config;

const SYNCED: &str = "[00:00.50] Is this the real life?\\n[00:07.10] Is this just fantasy?";

fn lrclib_entry() -> String {
    format!(
        r#"{{"trackName":"Bohemian Rhapsody","artistName":"Queen","duration":3.0,"instrumental":false,"syncedLyrics":"{SYNCED}"}}"#
    )
}

/// A 3 s silent 8 kHz mono WAV
fn write_wav(path: &Path) {
    let samples = 24_000u32;
    let data_len = samples * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend(std::iter::repeat_n(0u8, data_len as usize));
    std::fs::write(path, wav).unwrap();
}

fn track(path: PathBuf, artist: &str) -> Track {
    Track {
        id: path.display().to_string(),
        title: "Bohemian Rhapsody".into(),
        artist: artist.into(),
        album: "Unknown".into(),
        duration: 3.0,
        path,
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

/// App with a muted track playing; `None` without an audio device
fn playing(dir: &Path, config: Config, artist: &str) -> Option<App> {
    let engine = common::audio_engine()?;
    let mut app = App::new(config, engine);
    let wav = dir.join("Queen - Bohemian Rhapsody.wav");
    write_wav(&wav);
    app.audio_engine.set_volume(0.0);
    app.library.tracks = vec![track(wav, artist)];
    app.select_current_track();
    assert!(
        app.playback_state.is_playing,
        "{:?}",
        app.ui_state.error_message
    );
    Some(app)
}

fn lyrics_config(server: &MockServer, cache: &Path) -> Config {
    let mut config = Config::default();
    config.lyrics.lrclib_url = server.url.clone();
    config.lyrics.cache_dir = cache.to_path_buf();
    config
}

fn run_until(app: &mut App, secs: f32, mut done: impl FnMut(&App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed().as_secs_f32() < secs {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
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
fn lyrics_are_fetched_online_when_a_track_starts() {
    let server = MockServer::start(vec![("GET /api/get", fixed(200, &lrclib_entry()))]);
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let Some(mut app) = playing(dir.path(), lyrics_config(&server, cache.path()), "Queen") else {
        return;
    };

    assert!(
        run_until(&mut app, 5.0, |a| a.lyrics.has_lyrics()),
        "no lyrics arrived"
    );
    assert_eq!(
        app.lyrics.lyrics().unwrap().lines[0].text,
        "Is this the real life?"
    );
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("Lyrics found online")));

    let request = &server.requests()[0].target;
    assert!(request.contains("artist_name=Queen"), "{request}");
    assert!(request.contains("duration=3"), "{request}");

    // and they show up in the lyrics panel
    app.toggle_lyrics();
    assert!(screen(&app).contains("Is this the real life?"));
}

#[test]
fn a_local_lrc_file_wins_and_no_request_is_made() {
    let server = MockServer::start(vec![("GET /", fixed(200, &lrclib_entry()))]);
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    std::fs::write(
        dir.path().join("Queen - Bohemian Rhapsody.lrc"),
        "[00:01.00]From my own file",
    )
    .unwrap();
    let Some(mut app) = playing(dir.path(), lyrics_config(&server, cache.path()), "Queen") else {
        return;
    };

    run_until(&mut app, 0.5, |_| false);
    assert_eq!(
        app.lyrics.lyrics().unwrap().lines[0].text,
        "From my own file"
    );
    assert!(
        server.requests().is_empty(),
        "looked online although a .lrc exists"
    );
}

#[test]
fn no_lookup_for_unknown_artists_or_when_auto_fetch_is_off() {
    let server = MockServer::start(vec![("GET /", fixed(200, &lrclib_entry()))]);
    let cache = tempfile::tempdir().unwrap();

    let dir = tempfile::tempdir().unwrap();
    if let Some(mut app) = playing(
        dir.path(),
        lyrics_config(&server, cache.path()),
        "Unknown Artist",
    ) {
        run_until(&mut app, 0.4, |_| false);
        assert!(!app.lyrics.has_lyrics());
    }

    let dir = tempfile::tempdir().unwrap();
    let mut config = lyrics_config(&server, cache.path());
    config.lyrics.auto_fetch = false;
    if let Some(mut app) = playing(dir.path(), config, "Queen") {
        run_until(&mut app, 0.4, |_| false);
        assert!(!app.lyrics.has_lyrics());
    }

    assert!(server.requests().is_empty());
}

#[test]
fn a_failing_lookup_is_reported_in_the_status_line() {
    let server = MockServer::start(vec![("GET /", fixed(503, ""))]);
    let (dir, cache) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let Some(mut app) = playing(dir.path(), lyrics_config(&server, cache.path()), "Queen") else {
        return;
    };

    assert!(run_until(&mut app, 5.0, |a| {
        a.ui_state
            .status_message
            .as_deref()
            .is_some_and(|s| s.contains("online lookup failed"))
    }));
    assert!(!app.lyrics.has_lyrics());
}

/// Ollama stand-in: health check plus a `/api/generate` that "translates" each numbered line
fn mock_ollama() -> MockServer {
    MockServer::start(vec![
        ("GET /api/tags", fixed(200, "{}")),
        (
            "POST /api/generate",
            Box::new(|request| {
                let body: serde_json::Value =
                    serde_json::from_str(&request.body).unwrap_or_default();
                let prompt = body["prompt"].as_str().unwrap_or_default();
                let reply: String = prompt
                    .lines()
                    .filter_map(|l| l.split_once('|'))
                    .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
                    .map(|(n, text)| format!("{n}|[fr] {text}\n"))
                    .collect();
                (
                    200,
                    serde_json::json!({ "response": reply, "done": true }).to_string(),
                )
            }),
        ),
    ])
}

fn app_with_lrc(dir: &Path, lines: usize, ollama_url: &str) -> Option<App> {
    let mut config = Config::default();
    config.lyrics.auto_fetch = false;
    config.ai.enabled = true;
    config.ai.provider = "ollama".into();
    config.ai.ollama.base_url = ollama_url.to_string();
    let lrc: String = (0..lines)
        .map(|i| format!("[00:{:02}.00]Line number {i}\n", i % 60))
        .collect();
    std::fs::write(dir.join("Queen - Bohemian Rhapsody.lrc"), lrc).unwrap();
    playing(dir, config, "Queen")
}

#[test]
fn lyrics_are_translated_through_the_ai_in_chunks() {
    let ollama = mock_ollama();
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = app_with_lrc(dir.path(), 25, &ollama.url) else {
        return;
    };
    assert_eq!(app.lyrics.lyrics().unwrap().lines.len(), 25);

    app.translate_lyrics();
    assert!(
        run_until(&mut app, 10.0, |a| a.lyrics.has_translations()),
        "no translation arrived: {:?} {:?}",
        app.ui_state.status_message,
        app.ui_state.error_message
    );

    let lines = &app.lyrics.lyrics().unwrap().lines;
    assert_eq!(lines[0].translation.as_deref(), Some("[fr] Line number 0"));
    assert_eq!(
        lines[24].translation.as_deref(),
        Some("[fr] Line number 24"),
        "second chunk"
    );
    assert_eq!(
        ollama.count("POST /api/generate"),
        2,
        "25 lines go out as 20 + 5"
    );
    assert!(
        app.ui_state.show_lyrics,
        "the panel opens to show the translation"
    );

    // the translation is drawn under the original line
    let shown = screen(&app);
    assert!(shown.contains("Line number 0") && shown.contains("[fr] Line number 0"));

    // x again hides it without another request
    app.translate_lyrics();
    assert!(!app.lyrics.config().show_translation);
    assert_eq!(ollama.count("POST /api/generate"), 2);
}

#[test]
fn translation_explains_what_is_missing() {
    let dir = tempfile::tempdir().unwrap();

    // no lyrics loaded
    let Some(mut app) = playing(dir.path(), Config::default(), "Unknown Artist") else {
        return;
    };
    app.translate_lyrics();
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("No lyrics")));

    // lyrics but the AI is switched off
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = app_with_lrc(dir.path(), 2, "http://127.0.0.1:9") else {
        return;
    };
    app.config.ai.enabled = false;
    app.translate_lyrics();
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("needs the AI")));
}
