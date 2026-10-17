//! Last.fm scrobbling and WASM plugins wired into the player, against a mock Last.fm.
//! Playback tests need an audio device (skipped without one); playback is muted.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{write_wav, MockServer};
use symphony::app::{App, Track, TrackSource};
use symphony::config::Config;
use symphony::integrations::lastfm::{LastFm, ScrobbleTrack};

fn mock_lastfm() -> MockServer {
    MockServer::start(vec![(
        "POST /2.0/",
        Box::new(|request| {
            let form = form(&request.body);
            match form.get("method").map(String::as_str) {
                Some("auth.getToken") => (200, r#"{"token":"tok123"}"#.to_string()),
                Some("auth.getSession") => (
                    200,
                    r#"{"session":{"name":"ruchit","key":"sess456"}}"#.to_string(),
                ),
                _ if form.get("sk").map(String::as_str) == Some("bad") => (
                    403,
                    r#"{"error":9,"message":"Invalid session key"}"#.to_string(),
                ),
                _ => (200, r#"{"scrobbles":{"@attr":{"accepted":1}}}"#.to_string()),
            }
        }),
    )])
}

fn form(body: &str) -> BTreeMap<String, String> {
    url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect()
}

/// Form bodies of the requests that called `method`
fn calls(server: &MockServer, method: &str) -> Vec<BTreeMap<String, String>> {
    server
        .requests()
        .iter()
        .map(|r| form(&r.body))
        .filter(|f| f.get("method").map(String::as_str) == Some(method))
        .collect()
}

fn api_url(server: &MockServer) -> String {
    format!("{}/2.0/", server.url)
}

fn track() -> ScrobbleTrack {
    ScrobbleTrack {
        artist: "Daft Punk".into(),
        title: "Around the World".into(),
        album: "Homework".into(),
        duration_secs: 429,
    }
}

#[tokio::test]
async fn scrobbles_are_signed_and_carry_the_track() {
    let server = mock_lastfm();
    let client = LastFm::new("key", "secret", "sess", api_url(&server));

    client.now_playing(&track()).await.unwrap();
    client.scrobble(&track(), 1_700_000_000).await.unwrap();

    let now = &calls(&server, "track.updateNowPlaying")[0];
    assert_eq!(
        (
            now["artist"].as_str(),
            now["track"].as_str(),
            now["duration"].as_str()
        ),
        ("Daft Punk", "Around the World", "429")
    );

    let scrobble = &calls(&server, "track.scrobble")[0];
    assert_eq!(scrobble["artist[0]"], "Daft Punk");
    assert_eq!(scrobble["track[0]"], "Around the World");
    assert_eq!(scrobble["album[0]"], "Homework");
    assert_eq!(scrobble["timestamp[0]"], "1700000000");
    assert_eq!(
        (scrobble["api_key"].as_str(), scrobble["sk"].as_str()),
        ("key", "sess")
    );

    // the signature covers every sent parameter except `format` and itself
    let mut signed: BTreeMap<String, String> = scrobble.clone();
    let signature = signed.remove("api_sig").unwrap();
    signed.remove("format");
    assert_eq!(client.sign(&signed), signature);
}

#[tokio::test]
async fn login_trades_a_token_for_a_session_and_errors_are_reported() {
    let server = mock_lastfm();
    let client = LastFm::new("key", "secret", "", api_url(&server));

    let token = client.request_token().await.unwrap();
    assert_eq!(token, "tok123");
    assert_eq!(
        client.auth_url(&token),
        "https://www.last.fm/api/auth/?api_key=key&token=tok123"
    );
    assert_eq!(
        client.request_session(&token).await.unwrap(),
        ("ruchit".to_string(), "sess456".to_string())
    );
    assert_eq!(calls(&server, "auth.getSession")[0]["token"], "tok123");

    let rejected = LastFm::new("key", "secret", "bad", api_url(&server));
    let error = rejected.scrobble(&track(), 1).await.unwrap_err();
    assert!(error.contains("Invalid session key"), "{error}");

    let offline = LastFm::new("key", "secret", "sess", "http://127.0.0.1:9/2.0/");
    assert!(offline.scrobble(&track(), 1).await.is_err());
}

fn local_track(dir: &Path, name: &str, artist: &str) -> Track {
    let path: PathBuf = dir.join(format!("{name}.wav"));
    write_wav(&path, &[], 5.0, 0.0);
    Track {
        id: name.into(),
        title: name.into(),
        artist: artist.into(),
        album: "Album".into(),
        duration: 31.0, // Last.fm ignores tracks under 30 s
        path,
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

fn app(config: Config) -> Option<App> {
    let app = App::new(config, common::audio_engine()?);
    app.audio_engine.set_volume(0.0);
    Some(app)
}

fn run_for(app: &mut App, duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration {
        app.update();
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
fn the_player_scrobbles_tracks_that_were_listened_to_and_skips_the_rest() {
    let (server, dir) = (mock_lastfm(), tempfile::tempdir().unwrap());
    let mut config = Config::default();
    config.general.cache_directory = dir.path().join("cache");
    config.lyrics.auto_fetch = false;
    config.plugins.enabled = false;
    config.plugins.lastfm.enabled = true;
    config.plugins.lastfm.api_key = "key".into();
    config.plugins.lastfm.api_secret = "secret".into();
    config.plugins.lastfm.session_key = "sess".into();
    config.plugins.lastfm.api_url = api_url(&server);
    config.plugins.lastfm.scrobble_threshold = 1; // 1% of 31 s: about a third of a second
    let Some(mut app) = app(config) else { return };
    app.library.tracks = vec![
        local_track(dir.path(), "Heard", "Band A"),
        local_track(dir.path(), "Skipped", "Band B"),
    ];
    app.start_integrations();

    app.select_current_track(); // plays "Heard"
    run_for(&mut app, Duration::from_millis(900));
    app.next_track(); // "Skipped" starts right away...
    app.stop_integrations(); // ...and is left immediately

    let now_playing: Vec<_> = calls(&server, "track.updateNowPlaying")
        .iter()
        .map(|c| c["track"].clone())
        .collect();
    assert_eq!(now_playing, ["Heard", "Skipped"]);
    let scrobbles = calls(&server, "track.scrobble");
    assert_eq!(scrobbles.len(), 1, "{scrobbles:?}");
    assert_eq!(
        (
            scrobbles[0]["artist[0]"].as_str(),
            scrobbles[0]["track[0]"].as_str()
        ),
        ("Band A", "Heard")
    );
    assert!(scrobbles[0]["timestamp[0]"].parse::<i64>().unwrap() > 1_700_000_000);
}

/// Reacts to the first track with a notification, from inside WASM
const NOTIFIER: &str = r#"
(module
  (import "symphony" "notify" (func $notify (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "Now playing!")
  (global $heap (mut i32) (i32.const 4096))
  (func (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (local.get $p) (local.get $n)))
    (local.get $p))
  (func (export "on_event") (param $p i32) (param $n i32)
    (drop (call $notify (i32.const 0) (i32.const 12) (i32.const 0) (i32.const 3)))))
"#;

#[test]
fn a_wasm_plugin_reacts_to_playback_and_reaches_the_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let plugin = dir.path().join("plugins/notifier");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(
        plugin.join("plugin.toml"),
        "name = \"notifier\"\nversion = \"1.0.0\"\nauthor = \"t\"\ndescription = \"t\"\npermissions = [\"track_info\", \"notifications\"]\n",
    )
    .unwrap();
    std::fs::write(
        plugin.join("plugin.wasm"),
        wat::parse_str(NOTIFIER).unwrap(),
    )
    .unwrap();

    let mut config = Config::default();
    config.general.cache_directory = dir.path().join("cache");
    config.lyrics.auto_fetch = false;
    config.plugins.plugin_dir = dir.path().join("plugins");
    let Some(mut app) = app(config) else { return };
    app.library.tracks = vec![local_track(dir.path(), "Song", "Band")];
    app.start_integrations();

    app.select_current_track();
    app.update();

    assert_eq!(
        app.ui_state.status_message.as_deref(),
        Some("Now playing!: Now")
    );
    app.stop_integrations();
}
