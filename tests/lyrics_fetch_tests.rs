//! Online lyrics (LRCLIB) against a local mock of its API, plus one ignored live check.

mod common;

use common::{fixed, MockServer};
use symphony::lyrics::fetcher::{LyricsFetcher, LyricsFetcherConfig};
use symphony::lyrics::{LyricsError, LyricsSource};

const SYNCED: &str = "[00:00.50] Is this the real life?\\n[00:07.10] Is this just fantasy?";

/// Shape of a real LRCLIB record (camelCase keys), checked against lrclib.net
fn entry(name: &str, duration: f64, synced: Option<&str>, instrumental: bool) -> String {
    let synced = synced.map_or("null".to_string(), |s| format!("\"{s}\""));
    format!(
        r#"{{"id":1,"trackName":"{name}","artistName":"Queen","albumName":"A Night at the Opera","duration":{duration},"instrumental":{instrumental},"plainLyrics":"x","syncedLyrics":{synced}}}"#
    )
}

fn fetcher(server: &MockServer, cache: &std::path::Path, expiration_days: u32) -> LyricsFetcher {
    LyricsFetcher::new(LyricsFetcherConfig {
        cache_dir: cache.to_path_buf(),
        lrclib_url: server.url.clone(),
        cache_expiration_days: expiration_days,
        ..LyricsFetcherConfig::default()
    })
}

#[tokio::test]
async fn exact_lookup_returns_synced_lyrics_and_caches_them() {
    let server = MockServer::start(vec![(
        "GET /api/get",
        fixed(200, &entry("Bohemian Rhapsody", 355.0, Some(SYNCED), false)),
    )]);
    let cache = tempfile::tempdir().unwrap();
    let fetcher = fetcher(&server, cache.path(), 30);

    let lyrics = fetcher
        .fetch_track(
            "Bohemian Rhapsody",
            "Queen",
            Some("A Night at the Opera"),
            Some(354.6),
        )
        .await
        .unwrap();
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.lines[0].timestamp_ms, 500);
    assert_eq!(lyrics.lines[1].text, "Is this just fantasy?");
    assert!(
        matches!(lyrics.source, LyricsSource::Online { ref service, .. } if service == "LRCLIB")
    );

    // the request carries what LRCLIB needs to pick the right recording
    let target = server.requests()[0].target.clone();
    assert!(target.contains("artist_name=Queen"), "{target}");
    assert!(
        target.contains("track_name=Bohemian+Rhapsody")
            || target.contains("track_name=Bohemian%20Rhapsody"),
        "{target}"
    );
    assert!(
        target.contains("album_name=A+Night+at+the+Opera")
            || target.contains("album_name=A%20Night%20at%20the%20Opera"),
        "{target}"
    );
    assert!(target.contains("duration=355"), "{target}");

    // the second call is served from the cache: no new request
    let again = fetcher
        .fetch_track("Bohemian Rhapsody", "Queen", None, None)
        .await
        .unwrap();
    assert_eq!(again.lines.len(), 2);
    assert_eq!(server.count("GET /api/get"), 1);
}

#[tokio::test]
async fn unknown_track_falls_back_to_search_and_prefers_the_closest_duration() {
    let results = format!(
        "[{},{},{}]",
        entry("Live version", 420.0, Some("[00:01.00] live"), false),
        entry("Studio version", 355.0, Some(SYNCED), false),
        entry("No sync", 355.0, None, false),
    );
    let server = MockServer::start(vec![
        (
            "GET /api/get",
            fixed(404, r#"{"statusCode":404,"name":"TrackNotFound"}"#),
        ),
        ("GET /api/search", fixed(200, &results)),
    ]);
    let cache = tempfile::tempdir().unwrap();

    let lyrics = fetcher(&server, cache.path(), 30)
        .fetch_track("Bohemian Rhapsody", "Queen", None, Some(356.0))
        .await
        .unwrap();
    assert_eq!(
        lyrics.lines.len(),
        2,
        "picked the studio version, not the live one"
    );
    assert_eq!(server.count("GET /api/search"), 1);
}

#[tokio::test]
async fn instrumentals_and_unsynced_lyrics_are_not_found() {
    let cache = tempfile::tempdir().unwrap();

    let instrumental = MockServer::start(vec![
        (
            "GET /api/get",
            fixed(200, &entry("Theme", 100.0, Some(SYNCED), true)),
        ),
        ("GET /api/search", fixed(200, "[]")),
    ]);
    let result = fetcher(&instrumental, cache.path(), 30)
        .fetch("Theme", "Queen")
        .await;
    assert!(matches!(result, Err(LyricsError::NotFound)));

    let plain_only = MockServer::start(vec![
        (
            "GET /api/get",
            fixed(200, &entry("Song", 100.0, None, false)),
        ),
        ("GET /api/search", fixed(200, "[]")),
    ]);
    let result = fetcher(&plain_only, cache.path(), 30)
        .fetch("Song", "Queen")
        .await;
    assert!(matches!(result, Err(LyricsError::NotFound)));
}

#[tokio::test]
async fn a_failing_service_is_a_network_error_not_a_missing_track() {
    let server = MockServer::start(vec![
        ("GET /api/get", fixed(503, "")),
        ("GET /api/search", fixed(503, "")),
    ]);
    let cache = tempfile::tempdir().unwrap();

    let result = fetcher(&server, cache.path(), 30)
        .fetch("Song", "Queen")
        .await;
    assert!(
        matches!(result, Err(LyricsError::NetworkError(_))),
        "got {result:?}"
    );
}

#[tokio::test]
async fn expired_cache_entries_are_fetched_again() {
    let server = MockServer::start(vec![(
        "GET /api/get",
        fixed(200, &entry("Song", 100.0, Some(SYNCED), false)),
    )]);
    let cache = tempfile::tempdir().unwrap();

    // 0 days: anything already on disk counts as expired
    let fetcher = fetcher(&server, cache.path(), 0);
    fetcher.fetch("Song", "Queen").await.unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fetcher.fetch("Song", "Queen").await.unwrap();
    assert_eq!(server.count("GET /api/get"), 2);
}

#[tokio::test]
async fn offline_mode_never_touches_the_network() {
    let server = MockServer::start(vec![("GET /", fixed(200, "{}"))]);
    let cache = tempfile::tempdir().unwrap();
    let fetcher = LyricsFetcher::new(LyricsFetcherConfig {
        cache_dir: cache.path().to_path_buf(),
        lrclib_url: server.url.clone(),
        online_enabled: false,
        ..LyricsFetcherConfig::default()
    });

    assert!(matches!(
        fetcher.fetch("Song", "Queen").await,
        Err(LyricsError::NotFound)
    ));
    assert!(server.requests().is_empty());
}

/// Hits the real lrclib.net (flaky, needs network): `cargo test -- --ignored lrclib_live`
#[tokio::test]
#[ignore]
async fn lrclib_live() {
    let cache = tempfile::tempdir().unwrap();
    let fetcher = LyricsFetcher::new(LyricsFetcherConfig {
        cache_dir: cache.path().to_path_buf(),
        ..LyricsFetcherConfig::default()
    });
    let lyrics = fetcher
        .fetch_track(
            "Bohemian Rhapsody",
            "Queen",
            Some("A Night at the Opera"),
            Some(355.0),
        )
        .await
        .expect("lrclib.net should know Bohemian Rhapsody");
    assert!(lyrics.lines.len() > 20);
    assert!(lyrics.lines[0].text.to_lowercase().contains("real life"));
}
