//! Playing from YouTube end to end, with a fake yt-dlp (examples/fake_ytdlp.rs). Needs an audio
//! device for the playback tests (skipped without one).

mod common;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once, OnceLock};
use std::time::{Duration, Instant};

use symphony::app::{import_stream_spec, App};
use symphony::config::Config;
use symphony::db::Database;
use symphony::streaming::{StreamManager, StreamManagerConfig};

/// Path of the compiled `fake_ytdlp` example. `cargo test` builds examples next to the test
/// binary; runners that do not (cargo tarpaulin) get it built on demand.
fn fake_ytdlp() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let name = if cfg!(windows) {
            "fake_ytdlp.exe"
        } else {
            "fake_ytdlp"
        };
        let exe = std::env::current_exe().unwrap();
        let beside = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples")
            .join(name);
        if beside.exists() {
            return beside;
        }
        let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("fake_ytdlp");
        let status = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "--quiet",
                "--example",
                "fake_ytdlp",
                "--manifest-path",
            ])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target)
            .status()
            .expect("cargo runs");
        assert!(status.success(), "building the fake yt-dlp failed");
        target.join("debug").join("examples").join(name)
    })
    .clone()
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
    let app = App::new(config, common::audio_engine()?);
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
    app.playback_state
        .current_track
        .as_ref()
        .map(|t| t.id.clone())
}

#[test]
fn a_stream_query_skips_marathon_results_for_the_first_song() {
    let _serial = serial();
    let (dir, log) = (
        tempfile::tempdir().unwrap(),
        tempfile::NamedTempFile::new().unwrap(),
    );
    fake_env(log.path());
    let Some(mut app) = new_app(config(dir.path())) else {
        return;
    };

    // the fake finds a 12-hour loop first and a 200 s song second
    app.play_stream_query("marathon");
    assert!(
        wait_for(&mut app, |a| playing(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );
    assert_eq!(playing(&app).as_deref(), Some("youtube:marathon-200"));
}

#[test]
fn searching_a_stream_downloads_it_and_plays_the_file() {
    let _serial = serial();
    let (dir, log) = (
        tempfile::tempdir().unwrap(),
        tempfile::NamedTempFile::new().unwrap(),
    );
    fake_env(log.path());
    let Some(mut app) = new_app(config(dir.path())) else {
        return;
    };

    app.play_stream_query("lofi beats");
    assert!(
        wait_for(&mut app, |a| playing(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );

    // the first search result ("lofi beats #1", the 60 s one) is playing
    assert_eq!(playing(&app).as_deref(), Some("youtube:lofi_beats-60"));
    assert!(app.playback_state.is_playing);
    assert!(
        app.audio_engine.is_playing(),
        "the downloaded m4a is being decoded"
    );
    assert_eq!(downloads(log.path()), 1);
    // saved in the library, and the file is in the cache
    assert!(app
        .library
        .tracks
        .iter()
        .any(|t| t.id == "youtube:lofi_beats-60"));
    let cached: Vec<_> = walk(&dir.path().join("streams"))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "m4a"))
        .collect();
    assert_eq!(cached.len(), 1, "{cached:?}");
}

#[test]
fn a_cached_track_plays_offline_without_yt_dlp() {
    let _serial = serial();
    let (dir, log) = (
        tempfile::tempdir().unwrap(),
        tempfile::NamedTempFile::new().unwrap(),
    );
    fake_env(log.path());
    let db_path = dir.path().join("lib.db");

    // first session: online, downloads and saves the track
    let Some(mut app) = new_app(config(dir.path())) else {
        return;
    };
    app.database = Some(Database::new(db_path.to_str().unwrap()).unwrap());
    app.play_stream_query("lofi beats");
    assert!(
        wait_for(&mut app, |a| playing(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );
    drop(app);
    assert_eq!(downloads(log.path()), 1);

    // second session: yt-dlp is gone, the cached file still plays
    let mut offline = config(dir.path());
    offline.streaming.youtube.ytdlp_path = dir
        .path()
        .join("no-such-yt-dlp")
        .to_string_lossy()
        .to_string();
    let Some(mut app) = new_app(offline) else {
        return;
    };
    app.database = Some(Database::new(db_path.to_str().unwrap()).unwrap());
    app.load_library().unwrap();
    app.select_current_track();
    assert!(
        wait_for(&mut app, |a| playing(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );
    assert_eq!(playing(&app).as_deref(), Some("youtube:lofi_beats-60"));
    assert_eq!(
        downloads(log.path()),
        1,
        "nothing was downloaded the second time"
    );
}

#[test]
fn a_failed_download_is_reported_and_nothing_plays() {
    let _serial = serial();
    let (dir, log) = (
        tempfile::tempdir().unwrap(),
        tempfile::NamedTempFile::new().unwrap(),
    );
    fake_env(log.path());
    let Some(mut app) = new_app(config(dir.path())) else {
        return;
    };

    app.play_stream_query("broken");
    assert!(wait_for(&mut app, |a| a.ui_state.error_message.is_some()));

    let error = app.ui_state.error_message.clone().unwrap();
    assert!(
        error.contains("Stream failed") && error.contains("unavailable"),
        "{error}"
    );
    assert!(playing(&app).is_none());
}

#[test]
fn enter_on_a_saved_stream_track_fetches_and_plays_it() {
    let _serial = serial();
    let (dir, log) = (
        tempfile::tempdir().unwrap(),
        tempfile::NamedTempFile::new().unwrap(),
    );
    fake_env(log.path());
    let config = config(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = Database::new(dir.path().join("lib.db").to_str().unwrap()).unwrap();
    let added = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming))
            .await
            .unwrap();
        import_stream_spec(&manager, &db, "https://www.youtube.com/watch?v=abc123")
            .await
            .unwrap()
    });
    assert_eq!(added[0].id, "youtube:abc123");

    let Some(mut app) = new_app(config) else {
        return;
    };
    app.database = Some(db);
    app.load_library().unwrap();
    assert_eq!(
        app.library.tracks.len(),
        1,
        "the stream track survives a restart"
    );

    app.select_current_track();
    assert!(
        wait_for(&mut app, |a| playing(a).is_some()),
        "{:?}",
        app.ui_state.error_message
    );
    assert_eq!(playing(&app).as_deref(), Some("youtube:abc123"));
}

#[test]
fn unknown_sources_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let config = config(dir.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = Database::new(dir.path().join("lib.db").to_str().unwrap()).unwrap();
    let err = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming))
            .await
            .unwrap();
        import_stream_spec(&manager, &db, "https://example.com/song.mp3")
            .await
            .unwrap_err()
    });
    assert!(err.contains("unrecognized"), "{err}");

    // Spotify was removed: its specs are no longer recognized either
    let err = runtime.block_on(async {
        let manager = StreamManager::new(StreamManagerConfig::from_config(&config.streaming))
            .await
            .unwrap();
        import_stream_spec(&manager, &db, "spotify:playlist:PL1")
            .await
            .unwrap_err()
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
