//! Headless checks of the player: renders the TUI into an in-memory terminal
//! and exercises the queue/shuffle/progress logic. Skipped when there is no audio device.

use std::path::PathBuf;
use std::time::Duration;

use ratatui::{backend::TestBackend, Terminal};
use symphony::app::{App, Track, TrackSource};
use symphony::audio::AudioEngine;
use symphony::config::Config;

fn track(n: usize, duration: f64) -> Track {
    Track {
        id: format!("id{n}"),
        title: format!("Song {n}"),
        artist: format!("Artist {n}"),
        album: "Album".into(),
        duration,
        path: PathBuf::from(format!("song{n}.mp3")),
        track_number: None,
        genre: None,
        year: None,
        source: TrackSource::Local,
    }
}

/// Build an app with `count` tracks, or `None` when no audio device exists (CI).
fn app_with_tracks(count: usize) -> Option<App> {
    let engine = AudioEngine::new().ok()?;
    let mut app = App::new(Config::default(), engine);
    app.library.tracks = (0..count).map(|n| track(n, 638.0)).collect();
    Some(app)
}

fn render_to_string(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| symphony::ui::render(f, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn ui_shows_cursor_time_and_key_hints() {
    let Some(mut app) = app_with_tracks(3) else { return };
    app.library.selected_index = 1;
    app.playback_state.current_track = Some(app.library.tracks[0].clone());
    app.playback_state.is_playing = true;
    app.playback_state.position = 65.0;
    app.playback_state.duration = 638.0;

    let screen = render_to_string(&app, 120, 32);
    println!("{screen}");

    assert!(screen.contains("1:05 / 10:38"), "elapsed / total time missing");
    assert!(screen.contains("›   Artist 1 - Song 1"), "selection cursor missing");
    assert!(screen.contains("▶ Artist 0 - Song 0"), "now-playing marker missing");
    assert!(screen.contains("space play/pause"), "key hints missing");
    assert!(screen.contains("+/- volume"), "volume hint missing");
    assert!(screen.contains("VOL: 60%"), "volume label missing");
}

#[test]
fn shuffle_keeps_tracks_and_restores_order() {
    let Some(mut app) = app_with_tracks(8) else { return };
    app.library.queue = app.library.tracks.clone();
    app.library.queue_index = 3;
    let current_id = app.library.queue[3].id.clone();

    app.toggle_shuffle();
    assert!(app.playback_state.shuffle);
    assert_eq!(app.library.queue.len(), 8);
    assert_eq!(app.library.queue[app.library.queue_index].id, current_id);

    app.toggle_shuffle();
    assert!(!app.playback_state.shuffle);
    let ids: Vec<_> = app.library.queue.iter().map(|t| t.id.as_str()).collect();
    let expected: Vec<_> = (0..8).map(|n| format!("id{n}")).collect();
    assert_eq!(ids, expected.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(app.library.queue[app.library.queue_index].id, current_id);
}

#[test]
fn volume_steps_are_whole_percents_and_clamped() {
    let Some(mut app) = app_with_tracks(1) else { return };
    app.playback_state.volume = 0.6;
    app.volume_up();
    assert_eq!(app.playback_state.volume, 0.65);
    for _ in 0..20 {
        app.volume_up();
    }
    assert_eq!(app.playback_state.volume, 1.0);
    for _ in 0..30 {
        app.volume_down();
    }
    assert_eq!(app.playback_state.volume, 0.0);
}

#[test]
fn position_advances_while_playing_and_freezes_when_paused() {
    let Some(mut app) = app_with_tracks(1) else { return };
    app.playback_state.current_track = Some(app.library.tracks[0].clone());
    app.playback_state.duration = 638.0;
    app.playback_state.is_playing = true;

    std::thread::sleep(Duration::from_millis(60));
    app.update();
    assert!(app.playback_state.position >= 0.05, "position did not advance");

    app.playback_state.is_playing = false;
    let frozen = app.playback_state.position;
    std::thread::sleep(Duration::from_millis(60));
    app.update();
    assert_eq!(app.playback_state.position, frozen);
}

// ---------------------------------------------------------------------------
// Keyboard input, AI prompt, lyrics, pomodoro and git status
// ---------------------------------------------------------------------------

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use symphony::ai::MusicIntent;
use symphony::input::handle_key_event;

fn key(app: &mut App, code: KeyCode) -> bool {
    handle_key_event(app, KeyEvent::new(code, KeyModifiers::NONE)).unwrap()
}

fn press(app: &mut App, c: char) -> bool {
    key(app, KeyCode::Char(c))
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, c);
    }
}

fn app_with_config(count: usize, config: Config) -> Option<App> {
    let engine = AudioEngine::new().ok()?;
    let mut app = App::new(config, engine);
    app.library.tracks = (0..count).map(|n| track(n, 200.0)).collect();
    Some(app)
}

fn play_intent(artist: Option<&str>, genre: Option<&str>) -> MusicIntent {
    MusicIntent::Play {
        genre: genre.map(String::from),
        mood: None,
        artist: artist.map(String::from),
        tempo: None,
        era: None,
        duration_mins: None,
    }
}

#[test]
fn key_release_events_are_ignored() {
    let Some(mut app) = app_with_tracks(1) else { return };

    press(&mut app, 's');
    assert!(app.playback_state.shuffle);

    // Windows sends a release after every press; it must not toggle again
    let release = KeyEvent::new_with_kind(KeyCode::Char('s'), KeyModifiers::NONE, KeyEventKind::Release);
    assert!(handle_key_event(&mut app, release).unwrap());
    assert!(app.playback_state.shuffle, "release toggled shuffle back off");

    press(&mut app, 's');
    assert!(!app.playback_state.shuffle);
}

#[test]
fn typing_in_search_does_not_trigger_commands() {
    let Some(mut app) = app_with_tracks(3) else { return };

    press(&mut app, '/');
    assert!(app.is_in_text_input());

    // q, s and n are all commands outside the prompt
    for c in ['q', 's', 'n'] {
        assert!(press(&mut app, c), "{c} quit the app while typing");
    }
    assert_eq!(app.ui_state.search_query, "qsn");
    assert!(!app.playback_state.shuffle);

    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.ui_state.search_query, "qs");

    key(&mut app, KeyCode::Esc);
    assert!(!app.is_in_text_input());
    assert!(!press(&mut app, 'q'), "q should quit outside the prompt");
}

#[test]
fn shifted_symbols_and_ctrl_c_work() {
    let Some(mut app) = app_with_tracks(1) else { return };
    app.playback_state.volume = 0.6;

    // `+` and `?` arrive with SHIFT set on most keyboards
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('+'), KeyModifiers::SHIFT)).unwrap();
    assert_eq!(app.playback_state.volume, 0.65);
    handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT)).unwrap();
    assert!(app.ui_state.show_help);

    let quit = handle_key_event(&mut app, KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!quit.unwrap());
}

#[test]
fn ai_intents_drive_the_player() {
    let Some(mut app) = app_with_tracks(5) else { return };

    // artist filter narrows the queue (playback itself fails: the files do not exist)
    app.apply_ai_intent(play_intent(Some("artist 2"), None));
    assert_eq!(app.library.queue.len(), 1);
    assert_eq!(app.library.queue[0].id, "id2");

    // no filter: the whole library, shuffled
    app.apply_ai_intent(play_intent(None, None));
    assert_eq!(app.library.queue.len(), 5);

    // nothing matches: it falls back to a search by meaning, which finds nothing either
    app.ui_state.error_message = None;
    app.apply_ai_intent(play_intent(Some("nobody"), None));
    assert!(wait_for(&mut app, |a| a
        .ui_state
        .error_message
        .as_deref()
        .is_some_and(|e| e.contains("nothing in your library matches"))));

    // search ranks by meaning (keywords here: no embedding server) and shows the results
    app.library.tracks[3].title = "Zebra Crossing".into();
    app.apply_ai_intent(MusicIntent::Search { query: "zebra".into() });
    assert!(wait_for(&mut app, |a| a.ui_state.pinned_results));
    assert!(app.ui_state.search_mode);
    assert_eq!(app.library.filtered_tracks.len(), 1);
    assert_eq!(app.library.filtered_tracks[0].title, "Zebra Crossing");
    app.exit_search_mode();
    assert!(!app.ui_state.pinned_results);

    // player state
    app.apply_ai_intent(MusicIntent::SetState {
        volume: Some(30),
        shuffle: Some(true),
        repeat: Some("all".into()),
    });
    assert_eq!(app.playback_state.volume, 0.3);
    assert!(app.playback_state.shuffle);
    assert_eq!(app.playback_state.repeat, symphony::app::RepeatMode::All);

    // a question goes to the AI; with no server the failure is reported, not dropped
    app.ui_state.error_message = None;
    app.apply_ai_intent(MusicIntent::Unknown { query: "what is jazz".into() });
    assert!(wait_for(&mut app, |a| a
        .ui_state
        .error_message
        .as_deref()
        .is_some_and(|e| e.starts_with("AI:"))));
}

/// Pump the app until `done`, up to 20 s
fn wait_for(app: &mut App, mut done: impl FnMut(&App) -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(20) {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

#[test]
fn ai_prompt_reports_an_unreachable_provider() {
    let mut config = Config::default();
    config.ai.enabled = true;
    config.ai.provider = "ollama".into();
    config.ai.ollama.base_url = "http://127.0.0.1:9".into(); // nothing listens here
    let Some(mut app) = app_with_config(2, config) else { return };

    press(&mut app, 'a');
    assert!(app.ui_state.ai_mode);
    type_text(&mut app, "play something chill");
    assert_eq!(app.ui_state.ai_query, "play something chill");
    key(&mut app, KeyCode::Enter);
    assert!(app.ui_state.ai_pending, "request was not started");
    assert!(!app.ui_state.ai_mode);

    let start = std::time::Instant::now();
    while app.ui_state.ai_pending && start.elapsed() < Duration::from_secs(30) {
        app.update();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!app.ui_state.ai_pending, "AI request never finished");
    assert!(app
        .ui_state
        .error_message
        .as_deref()
        .is_some_and(|e| e.starts_with("AI:")));
}

#[test]
fn ai_prompt_respects_the_config_switch() {
    let mut config = Config::default();
    config.ai.enabled = false;
    let Some(mut app) = app_with_config(1, config) else { return };

    press(&mut app, 'a');
    assert!(!app.ui_state.ai_mode);
    assert!(app
        .ui_state
        .status_message
        .as_deref()
        .is_some_and(|s| s.contains("disabled")));
}

#[test]
fn lyrics_load_from_lrc_and_render_in_the_panel() {
    let Some(mut app) = app_with_tracks(1) else { return };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("song.lrc"),
        "[00:00.00]First line\n[00:05.50]Second line\n[00:10.00]Third line\n",
    )
    .unwrap();

    app.load_lyrics_for(&dir.path().join("song.mp3"));
    assert!(app.lyrics.has_lyrics());
    app.lyrics.update_position(6_000);
    assert_eq!(app.lyrics.get_current_line_text(), Some("Second line"));

    press(&mut app, 'l');
    assert!(app.ui_state.show_lyrics);
    let screen = render_to_string(&app, 120, 32);
    assert!(screen.contains("Second line"), "lyrics panel did not show the line");

    // a track without an .lrc clears the lyrics
    app.load_lyrics_for(&dir.path().join("other.mp3"));
    assert!(!app.lyrics.has_lyrics());

    press(&mut app, 'l');
    assert!(!app.ui_state.show_lyrics);
}

#[test]
fn pomodoro_toggles_and_shows_in_the_status_bar() {
    let Some(mut app) = app_with_tracks(1) else { return };

    press(&mut app, 'w');
    assert!(app.pomodoro.get_state().is_running());
    let screen = render_to_string(&app, 120, 32);
    assert!(screen.contains("🍅"), "pomodoro missing from status bar");

    press(&mut app, 'w');
    assert!(!app.pomodoro.get_state().is_running());
}

#[test]
fn git_branch_shows_in_the_status_bar_inside_a_repo() {
    let Some(mut app) = app_with_tracks(1) else { return };
    let Some(git) = app.git_monitor.as_ref().filter(|g| g.in_repo()) else {
        return; // not running inside a git checkout
    };
    let _ = git;

    app.update(); // first update queries git immediately
    let screen = render_to_string(&app, 120, 32);
    assert!(screen.contains("⎇"), "git segment missing from status bar");
}
