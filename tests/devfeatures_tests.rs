//! Developer features: the status file behind `symphony status`, commit velocity from a real
//! git repository, and the queue following coding activity. Playback tests need an audio
//! device (skipped without one); playback is muted.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use common::write_wav;
use symphony::app::{App, Track, TrackSource};
use symphony::audio::features::AudioFeatures;
use symphony::config::Config;
use symphony::integrations::git::{ActivityLevel, GitMonitor, GitMonitorConfig};
use symphony::integrations::statusline::StatusSnapshot;

/// (title, energy, bpm): loud and fast ... quiet and slow
const LIBRARY: [(&str, f32, f32); 4] = [
    ("Gym Anthem", 0.9, 150.0),
    ("Melancholy Rain", 0.15, 70.0),
    ("Sunday Stroll", 0.4, 95.0),
    ("Night Drive", 0.7, 120.0),
];

fn app(dir: &Path) -> Option<App> {
    let mut config = Config::default();
    config.general.cache_directory = dir.join("cache");
    config.lyrics.auto_fetch = false;
    config.plugins.enabled = false;
    config.integrations.git.enabled = false;
    let mut app = App::new(config, common::audio_engine()?);
    app.audio_engine.set_volume(0.0);
    for (title, energy, bpm) in LIBRARY {
        let path: PathBuf = dir.join(format!("{title}.wav"));
        write_wav(&path, &[], 3.0, 0.0);
        app.features.insert(
            path.to_string_lossy().to_string(),
            AudioFeatures {
                energy,
                bpm,
                beat_strength: 0.4,
            },
        );
        app.library.tracks.push(Track {
            id: title.into(),
            title: title.into(),
            artist: "Band".into(),
            album: "Album".into(),
            duration: 180.0,
            path,
            track_number: None,
            genre: None,
            year: None,
            source: TrackSource::Local,
        });
    }
    Some(app)
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git is installed");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn repo_with_commits(dir: &Path, commits: usize) -> PathBuf {
    let repo = dir.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    for i in 0..commits {
        git(
            &repo,
            &["commit", "-q", "--allow-empty", "-m", &format!("work {i}")],
        );
    }
    repo
}

fn eager() -> GitMonitorConfig {
    GitMonitorConfig {
        check_interval: Duration::ZERO,
        ..Default::default()
    }
}

fn queue_titles(app: &App) -> Vec<String> {
    app.library.queue.iter().map(|t| t.title.clone()).collect()
}

#[test]
fn commit_velocity_comes_from_the_real_git_log() {
    let dir = tempfile::tempdir().unwrap();
    for (commits, level, mood) in [
        (0, ActivityLevel::Idle, "calm"),
        (2, ActivityLevel::Light, "steady"),
        (4, ActivityLevel::Focused, "driven"),
        (7, ActivityLevel::Intense, "energetic"),
    ] {
        let repo = repo_with_commits(&dir.path().join(commits.to_string()), commits);
        let mut monitor = GitMonitor::for_repo(repo, eager());
        monitor.update();
        assert_eq!(
            monitor.get_commit_velocity(),
            commits as f32,
            "{commits} commits"
        );
        assert_eq!(monitor.get_activity_level(), level, "{commits} commits");
        assert_eq!(level.recommended_mood(), mood);
    }
}

#[test]
fn the_queue_follows_coding_activity() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = app(dir.path()) else {
        return;
    };

    assert!(app.adapt_to_activity(ActivityLevel::Intense));
    let loud = queue_titles(&app);
    assert_eq!(
        loud.first().map(String::as_str),
        Some("Gym Anthem"),
        "{loud:?}"
    );
    assert_eq!(
        loud.last().map(String::as_str),
        Some("Melancholy Rain"),
        "mood ranks, it does not filter: {loud:?}"
    );

    assert!(app.adapt_to_activity(ActivityLevel::Idle));
    let quiet = queue_titles(&app);
    assert_eq!(
        quiet.first().map(String::as_str),
        Some("Melancholy Rain"),
        "{quiet:?}"
    );
    assert_eq!(
        quiet.last().map(String::as_str),
        Some("Gym Anthem"),
        "{quiet:?}"
    );
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .unwrap_or("")
        .starts_with("Git:"));
}

#[test]
fn a_busier_hour_swaps_the_queue_at_the_next_track_change() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = app(dir.path()) else {
        return;
    };
    let repo = repo_with_commits(dir.path(), 0);
    app.config.integrations.git.adapt_music = true;
    app.git_monitor = Some(GitMonitor::for_repo(repo.clone(), eager()));

    // quiet hour: playback starts on the user's own queue and the first skip keeps it
    app.update();
    app.select_current_track();
    let own_queue = queue_titles(&app);
    app.next_track();
    assert_eq!(
        queue_titles(&app),
        own_queue,
        "no change in activity, no change in music"
    );

    // a burst of commits: the next skip brings in the energetic music
    for i in 0..7 {
        git(
            &repo,
            &["commit", "-q", "--allow-empty", "-m", &format!("burst {i}")],
        );
    }
    app.update();
    app.next_track();
    let queue = queue_titles(&app);
    assert_eq!(
        queue.first().map(String::as_str),
        Some("Gym Anthem"),
        "{queue:?}"
    );
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .unwrap_or("")
        .contains("energetic"));

    // adaptation off: the queue stays as it is however busy it gets
    app.config.integrations.git.adapt_music = false;
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "one more"]);
    let before = queue_titles(&app);
    app.next_track();
    assert_eq!(queue_titles(&app), before);
}

fn symphony_status(file: &Path, format: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_symphony"))
        .args(["status", format])
        .env("SYMPHONY_STATUS_FILE", file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn the_running_player_publishes_what_symphony_status_prints() {
    let dir = tempfile::tempdir().unwrap();
    let Some(mut app) = app(dir.path()) else {
        return;
    };
    let file = app.config.status_path();
    assert_eq!(file, dir.path().join("cache").join("status.json"));

    app.start_integrations();
    app.select_current_track(); // plays "Gym Anthem"
    app.update();

    let snapshot = StatusSnapshot::read(&file).expect("the player wrote its status");
    assert_eq!(
        (snapshot.title.as_str(), snapshot.artist.as_str()),
        ("Gym Anthem", "Band")
    );
    assert!(snapshot.playing && snapshot.duration == 180);
    assert_eq!(symphony_status(&file, "zsh"), "▶ Gym Anthem - Band");
    assert!(symphony_status(&file, "tmux").contains("Gym Anthem - Band"));
    assert!(symphony_status(&file, "json").contains("\"playing\":true"));

    // the file is rewritten once a second
    app.toggle_playback();
    std::thread::sleep(Duration::from_millis(1100));
    app.update();
    assert_eq!(symphony_status(&file, "minimal"), "🎵⏸");

    // quitting removes the file: nothing is playing
    app.stop_integrations();
    assert!(!file.exists());
    assert_eq!(symphony_status(&file, "zsh"), "");
    assert_eq!(symphony_status(&file, "tmux"), "🎵 Symphony");
}

#[test]
fn a_status_file_nobody_updates_means_the_player_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("status.json");
    StatusSnapshot {
        title: "Ghost".into(),
        artist: "Band".into(),
        duration: 10,
        position: 1,
        playing: true,
        volume: 0.5,
        updated_unix: 1, // the 1970s
    }
    .write(&file)
    .unwrap();
    assert_eq!(symphony_status(&file, "zsh"), "");
}

#[test]
fn unknown_status_formats_are_rejected() {
    let out = Command::new(env!("CARGO_BIN_EXE_symphony"))
        .args(["status", "fish"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown format"));
}
