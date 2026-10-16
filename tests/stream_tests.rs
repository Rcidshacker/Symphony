//! Playing from YouTube and Spotify end to end, with a fake yt-dlp (examples/fake_ytdlp.rs)
//! and a mock Spotify API. Needs an audio device for the playback tests (skipped without one).

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Once, OnceLock};
use std::time::{Duration, Instant};

use common::{fixed, MockServer};
use symphony::app::{import_stream_spec, App};
use symphony::audio::AudioEngine;
use symphony::config::Config;
use symphony::db::Database;
use symphony::streaming::{StreamManager, StreamManagerConfig};

/// Path of the compiled `fake_ytdlp` example (cargo builds examples for `cargo test`)
fn fake_ytdlp() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let examples = exe.parent().unwrap().parent().unwrap().join("examples");
    let name = if cfg!(windows) { "fake_ytdlp.exe" } else { "fake_ytdlp" };
    examples.join(name)
}

/// The fake yt-dlp is configured through process-wide env vars, so these tests run one at a time
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The fake downloads copy this file; every call is logged to `log`
fn fake_env(log: &Path) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let audio = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/tone.m4a");
        std::env::set_var("FAKE_YTDLP_AUDIO", audio);
    });
    std::env::set_var("FAKE_YTDLP_LOG", log);
}

/// Number of downloads the fake yt-dlp was asked for
fn downloads(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(" -o "))
        .count()
}

fn config(dir: &Path) -> Config {
    let mut config = Config::default();
    config.streaming.enabled = true;
    config.streaming.youtube.ytdlp_path = fake_ytdlp().to_string_lossy().to_string();
    config.streaming.cache.cache_dir = dir.join("streams");
    config.general.cache_directory = dir.join("cache");
    config.lyrics.auto_fetch = false;
    config
}

fn new_app(config: Config) -> Option<App> {
    let app = App::new(config, AudioEngine::new().ok()?);
    app.audio_engine.set_volume(0.0);
    Some(app)
}

fn wait_for(app: &mut App, mut done: impl FnMut(&App) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

fn playing(app: &App) -> Option<String> {
    app.playback_state.current_track.as_ref().map(|t| t.id.clone())
}

fn spotify_track(id: &str, name: &str, artist: &str, duration_ms: u64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "duration_ms": duration_ms,
        "preview_url": null,
        "artists": [{"id": "a1", "name": artist}],
        "album": {"id": "al1", "name": "Album", "images": []},
        "external_urls": {"spotify": format!("https://open.spotify.com/track/{id}")},
    })
}

fn mock_spotify() -> MockServer {
    let token = serde_json::json!({"access_token": "tok", "token_type": "Bearer", "expires_in": 3600});
    // the playlist has two tracks and one removed entry; its second page proves pagination
    let page_one = |base: &str| {
        serde_json::json!({
            "items": [{"track": spotify_track("s1", "Song One", "Band", 118_000)}, {"track": null}],
            "next": format!("{base}/playlists/PL1/tracks?offset=2"),
        })
        .to_string()
    };
    let page_two = serde_json::json!({
        "items": [{"track": spotify_track("s2", "Song Two", "Band", 60_000)}],
        "next": null,
    })
    .to_string();
    // the next-page link must point back at the server, whose port is only known once it runs
    let own_url: Arc<OnceLock<String>> = Arc::default();
    let url = own_url.clone();
    let server = MockServer::start(vec![
        ("POST /api/token", fixed(200, &token.to_string())),
        ("GET /playlists/PL1/tracks?offset=2", fixed(200, &page_two)),
        (
            "GET /playlists/PL1/tracks",
            Box::new(move |_| (200, page_one(url.get().map_or("", String::as_str)))),
        ),
    ]);
    let _ = own_url.set(server.url.clone());
    server
}

#[test]
fn searching_a_stream_downloads_it_and_plays_the_file() {
    let _serial = serial();
    let (dir, log) = (tempfile::tempdir().unwrap(), tempfile::NamedTempFile::new().unwrap());
    fake_env(log.path());
    let Some(mut app) = new_app(config(dir.path())) else { return };

    app.play_stream_query("lofi beats");
    assert!(wait_for(&mut app, |a| playing(a).is_some()), "{:?}", app.ui_state.error_message);

    // the first search result ("lofi beats #1", the 60 s one) is playing
    assert_eq!(playing(&app).as_deref(), Some("youtube:lofi_beats-60"));
    assert!(app.playback_state.is_playing);
    assert!(app.audio_engine.is_playing(), "the downloaded m4a is being decoded");
    assert_eq!(downloads(log.path()), 1);
    // saved in the library, and the file is in the cache
    assert!(app.library.tracks.iter().any(|t| t.id == "youtube:lofi_beats-60"));
    let cached: Vec<_> = walk(&dir.path().join("streams")).into_iter().filter(|p| p.extension().is_some_and(|e| e == "m4a")).collect();
    assert_eq!(cached.len(), 1, "{cached:?}");
}

#[test]
fn a_cached_track_plays_offline_without_yt_dlp() {
    let _serial = serial();
    let (dir, log) = (tempfile::tempdir().unwrap(), tempfile::NamedTempFile::new().unwrap());
    fake_env(log.path());
    let db_path = dir.path().join("lib.db");

    // first session: online, downloads and saves the track
    let Some(mut app) = new_app(config(dir.path())) else { return };
    app.database = Some(Database::new(db_path.to_str().unwrap()).unwrap());
    app.play_stream_query("lofi beats");
    assert!(wait_for(&mut app, |a| playing(a).is_some()), "{:?}", app.ui_state.error_message);
    drop(app);
    assert_eq!(downloads(log.path()), 1);

    // second session: yt-dlp is gone, the cached file still plays
    let mut offline = config(dir.path());
    offline.streaming.youtube.ytdlp_path = dir.path().join("no-such-yt-dlp").to_string_lossy().to_string();
    let Some(mut app) = new_app(offline) else { return };
    app.database = Some(Database::new(db_path.to_str().unwrap()).unwrap());
    app.load_library().unwrap();
    app.select_current_track();
    assert!(wait_for(&mut app, |a| playing(a).is_some()), "{:?}", app.ui_state.error_message);
    assert_eq!(playing(&app).as_deref(), Some("youtube:lofi_beats-60"));
    assert_eq!(downloads(log.path()), 1, "nothing was downloaded the second time");
}

#[test]
fn a_failed_download_is_reported_and_nothing_plays() {
    let _serial = serial();
    let (dir, log) = (tempfile::tempdir().unwrap(), tempfile::NamedTempFile::new().unwrap());
    fake_env(log.path());
    let Some(mut app) = new_app(config(dir.path())) else { return };

    app.play_stream_query("broken");
    assert!(wait_for(&mut app, |a| a.ui_state.error_message.is_some()));

    let error = app.ui_state.error_message.clone().unwrap();
    assert!(error.contains("Stream failed") && error.contains("unavailable"), "{error}");
    assert!(playing(&app).is_none());
}

#[test]
fn enter_on_a_saved_stream_track_fetches_and_plays_it() {
    let _serial = serial();
    let (dir, log) = (tempfile::tempdir().unwrap(), tempfile::NamedTempFile::new().unwrap());
    fake_env(log.path());
    let config = config(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = Database::new(dir.path().join("lib.db").to_str().unwrap()).unwrap();
    let added = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming)).await.unwrap();
        import_stream_spec(&manager, &db, "https://www.youtube.com/watch?v=abc123").await.unwrap()
    });
    assert_eq!(added[0].id, "youtube:abc123");

    let Some(mut app) = new_app(config) else { return };
    app.database = Some(db);
    app.load_library().unwrap();
    assert_eq!(app.library.tracks.len(), 1, "the stream track survives a restart");

    app.select_current_track();
    assert!(wait_for(&mut app, |a| playing(a).is_some()), "{:?}", app.ui_state.error_message);
    assert_eq!(playing(&app).as_deref(), Some("youtube:abc123"));
}

#[test]
fn spotify_playlists_import_page_by_page_and_play_through_the_closest_youtube_match() {
    let _serial = serial();
    let (dir, log) = (tempfile::tempdir().unwrap(), tempfile::NamedTempFile::new().unwrap());
    fake_env(log.path());
    let spotify = mock_spotify();

    let mut config = config(dir.path());
    config.streaming.spotify.enabled = true;
    config.streaming.spotify.client_id = "id".into();
    config.streaming.spotify.client_secret = "secret".into();
    config.streaming.spotify.api_url = spotify.url.clone();
    config.streaming.spotify.accounts_url = spotify.url.clone();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = Database::new(dir.path().join("lib.db").to_str().unwrap()).unwrap();
    let added = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming)).await.unwrap();
        import_stream_spec(&manager, &db, "spotify:playlist:PL1").await.unwrap()
    });
    let ids: Vec<_> = added.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["spotify:s1", "spotify:s2"], "removed entries are skipped, both pages read");
    assert_eq!(db.track_count().unwrap(), 2);
    assert!(spotify.count("GET /playlists/PL1/tracks?offset=2") == 1);

    let Some(mut app) = new_app(config) else { return };
    app.database = Some(db);
    app.load_library().unwrap();
    app.select_current_track();
    assert!(wait_for(&mut app, |a| playing(a).is_some()), "{:?}", app.ui_state.error_message);

    // Spotify itself only has 30 s previews: the 118 s track plays the 120 s YouTube result
    assert_eq!(playing(&app).as_deref(), Some("spotify:s1"));
    let log = std::fs::read_to_string(log.path()).unwrap();
    assert!(log.contains("ytsearch5:Band Song One"), "{log}");
    assert!(log.contains("watch?v=band_song_one-120"), "{log}");
}

#[test]
fn unknown_sources_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = Database::new(dir.path().join("lib.db").to_str().unwrap()).unwrap();
    let err = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming)).await.unwrap();
        import_stream_spec(&manager, &db, "https://example.com/song.mp3").await.unwrap_err()
    });
    assert!(err.contains("unrecognized"), "{err}");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}
