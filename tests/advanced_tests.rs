//! Advanced features tests (Phase 5)
//!
//! Tests for plugin system, lyrics, and integrations.

// =============================================================================
// Plugin System Tests
// =============================================================================

#[cfg(test)]
mod plugin_tests {
    use symphony::plugins::{Permission, PlaybackState, PluginEvent, TrackInfo};

    #[test]
    fn test_plugin_event_serialization() {
        let event =
            PluginEvent::TrackChanged(TrackInfo::new("test123", "Test Song", "Test Artist"));

        let json = event.to_json().unwrap();
        assert!(json.contains("Test Song"));
        assert!(json.contains("Test Artist"));
    }

    #[test]
    fn test_plugin_event_types() {
        assert_eq!(PluginEvent::AppStarted.event_type(), "app_started");
        assert_eq!(
            PluginEvent::PlaybackStateChanged(PlaybackState::Playing).event_type(),
            "playback_state_changed"
        );
    }

    #[test]
    fn test_track_info_builder() {
        let track = TrackInfo::new("id", "Title", "Artist")
            .with_album("Album")
            .with_duration(180)
            .with_position(60);

        assert_eq!(track.title, "Title");
        assert_eq!(track.artist, "Artist");
        assert_eq!(track.album, Some("Album".to_string()));
        assert_eq!(track.duration, 180);
        assert_eq!(track.position, 60);
    }

    #[test]
    fn test_playback_state() {
        assert_eq!(PlaybackState::Playing.to_string(), "Playing");
        assert_eq!(PlaybackState::Paused.to_string(), "Paused");
        assert_eq!(PlaybackState::Stopped.to_string(), "Stopped");
    }

    #[test]
    fn test_permission_display() {
        assert_eq!(Permission::TrackInfo.to_string(), "track_info");
        assert_eq!(Permission::Discord.to_string(), "discord");
        assert_eq!(Permission::Lastfm.to_string(), "lastfm");
    }
}

// =============================================================================
// Lyrics System Tests
// =============================================================================

#[cfg(test)]
mod lyrics_tests {
    use symphony::lyrics::{LrcParser, Lyrics, LyricsLine};

    #[test]
    fn test_lrc_parser_basic() {
        let lrc = r#"
[ti:Test Song]
[ar:Test Artist]
[al:Test Album]
[00:00.00]First line
[00:05.50]Second line
[00:10.00]Third line
"#;

        let lyrics = LrcParser::parse(lrc).unwrap();

        assert_eq!(lyrics.title, Some("Test Song".to_string()));
        assert_eq!(lyrics.artist, Some("Test Artist".to_string()));
        assert_eq!(lyrics.album, Some("Test Album".to_string()));
        assert_eq!(lyrics.lines.len(), 3);
        assert_eq!(lyrics.lines[0].text, "First line");
        assert_eq!(lyrics.lines[0].timestamp_ms, 0);
        assert_eq!(lyrics.lines[1].timestamp_ms, 5500);
        assert_eq!(lyrics.lines[2].timestamp_ms, 10000);
    }

    #[test]
    fn test_lrc_parser_empty() {
        let lyrics = LrcParser::parse("").unwrap();
        assert!(lyrics.is_empty());
    }

    #[test]
    fn test_lyrics_find_line() {
        let lyrics = Lyrics::new(
            "test",
            vec![
                LyricsLine::new(0, "Line 1"),
                LyricsLine::new(5000, "Line 2"),
                LyricsLine::new(10000, "Line 3"),
            ],
        );

        assert_eq!(lyrics.find_line_index(0), Some(0));
        assert_eq!(lyrics.find_line_index(3000), Some(0));
        assert_eq!(lyrics.find_line_index(5000), Some(1));
        assert_eq!(lyrics.find_line_index(7000), Some(1));
        assert_eq!(lyrics.find_line_index(15000), Some(2));
    }

    #[test]
    fn test_lyrics_context() {
        let lyrics = Lyrics::new(
            "test",
            vec![
                LyricsLine::new(0, "Line 1"),
                LyricsLine::new(5000, "Line 2"),
                LyricsLine::new(10000, "Line 3"),
                LyricsLine::new(15000, "Line 4"),
                LyricsLine::new(20000, "Line 5"),
            ],
        );

        let (past, current, future) = lyrics.get_context_lines(10000, 1, 1);

        assert_eq!(past.len(), 1);
        assert_eq!(past[0].text, "Line 2");
        assert!(current.is_some());
        assert_eq!(current.unwrap().text, "Line 3");
        assert_eq!(future.len(), 1);
        assert_eq!(future[0].text, "Line 4");
    }

    #[test]
    fn test_lyrics_to_lrc() {
        let lyrics = Lyrics::new(
            "test",
            vec![LyricsLine::new(0, "First"), LyricsLine::new(5000, "Second")],
        );

        let lrc = symphony::lyrics::lrc::to_lrc(&lyrics);
        assert!(lrc.contains("[00:00.00]First"));
        assert!(lrc.contains("[00:05.00]Second"));
    }
}

// =============================================================================
// Git Integration Tests
// =============================================================================

#[cfg(test)]
mod git_tests {
    use symphony::integrations::git::{ActivityLevel, GitMonitor};

    #[test]
    fn test_activity_level_mood() {
        assert_eq!(ActivityLevel::Idle.recommended_mood(), "relaxing");
        assert_eq!(ActivityLevel::Light.recommended_mood(), "chill");
        assert_eq!(ActivityLevel::Focused.recommended_mood(), "focus");
        assert_eq!(ActivityLevel::Intense.recommended_mood(), "energetic");
    }

    #[test]
    fn test_activity_level_display() {
        assert_eq!(ActivityLevel::Focused.to_string(), "Focused");
    }

    #[test]
    fn test_git_monitor_creation() {
        let monitor = GitMonitor::new();
        // Monitor is created, may or may not be in a repo
        let stats = monitor.get_stats();
        // Just verify it doesn't crash
        let _ = stats.format_summary();
    }
}

// =============================================================================
// Pomodoro Timer Tests
// =============================================================================

#[cfg(test)]
mod pomodoro_tests {
    use symphony::integrations::pomodoro::{PomodoroEvent, PomodoroState, PomodoroTimer};

    #[test]
    fn test_pomodoro_creation() {
        let timer = PomodoroTimer::new();
        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_start_work() {
        let mut timer = PomodoroTimer::new();
        let event = timer.start_work();

        assert_eq!(event, PomodoroEvent::Started(PomodoroState::Working));
        assert_eq!(timer.get_state(), PomodoroState::Working);
        assert!(timer.get_remaining_time().is_some());
    }

    #[test]
    fn test_stop() {
        let mut timer = PomodoroTimer::new();
        timer.start_work();
        timer.stop();

        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_toggle() {
        let mut timer = PomodoroTimer::new();

        timer.toggle();
        assert_eq!(timer.get_state(), PomodoroState::Working);

        timer.toggle();
        assert_eq!(timer.get_state(), PomodoroState::Idle);
    }

    #[test]
    fn test_format_remaining() {
        let mut timer = PomodoroTimer::with_durations(25, 5);
        assert_eq!(timer.format_remaining(), "Ready");

        timer.start_work();
        let formatted = timer.format_remaining();
        // Should show approximately 25:00
        assert!(formatted.starts_with("25:") || formatted.starts_with("24:"));
    }

    #[test]
    fn test_progress_bar() {
        let mut timer = PomodoroTimer::new();
        timer.start_work();

        let bar = timer.format_progress_bar(10);
        assert!(bar.starts_with('['));
        assert!(bar.ends_with(']'));
    }

    #[test]
    fn test_pomodoro_state_display() {
        assert_eq!(PomodoroState::Working.to_string(), "Working");
        assert_eq!(PomodoroState::Break.to_string(), "Short Break");
        assert_eq!(PomodoroState::LongBreak.to_string(), "Long Break");
    }

    #[test]
    fn test_pomodoro_state_checks() {
        assert!(PomodoroState::Working.is_work());
        assert!(!PomodoroState::Working.is_break());

        assert!(!PomodoroState::Break.is_work());
        assert!(PomodoroState::Break.is_break());

        assert!(PomodoroState::Working.is_running());
        assert!(!PomodoroState::Idle.is_running());
    }
}

// =============================================================================
// Status Line Tests
// =============================================================================

#[cfg(test)]
mod statusline_tests {
    use symphony::integrations::statusline::StatusLine;

    #[test]
    fn test_tmux_format() {
        let status = StatusLine::tmux_format(Some(("Test Song", "Test Artist", 180, 90)), true);
        assert!(status.contains("Test Song"));
        assert!(status.contains("Test Artist"));
        assert!(status.contains("01:30/03:00"));
    }

    #[test]
    fn test_tmux_format_empty() {
        let status = StatusLine::tmux_format(None, false);
        assert_eq!(status, "🎵 Symphony");
    }

    #[test]
    fn test_zsh_format() {
        let status = StatusLine::zsh_format(Some(("Song", "Artist")), true);
        assert!(status.contains("▶"));
        assert!(status.contains("Song"));
    }

    #[test]
    fn test_json_format() {
        let json = StatusLine::json_format(Some(("Test", "Artist", 100, 50)), true, 0.75);

        assert!(json.contains("\"title\":\"Test\""));
        assert!(json.contains("\"playing\":true"));
        assert!(json.contains("\"volume\":0.75"));
    }

    #[test]
    fn test_minimal_format() {
        assert_eq!(StatusLine::minimal_format(true, true), "🎵▶");
        assert_eq!(StatusLine::minimal_format(false, true), "🎵⏸");
        assert_eq!(StatusLine::minimal_format(false, false), "");
    }

    #[test]
    fn test_i3blocks_format() {
        let status = StatusLine::i3blocks_format(Some(("Song", "Artist")), true);
        assert!(status.contains("▶"));
    }

    #[test]
    fn test_waybar_format() {
        let status = StatusLine::waybar_format(Some(("Song", "Artist", 100, 50)), true);

        assert!(status.contains("\"text\""));
        assert!(status.contains("\"class\":\"playing\""));
    }
}

// =============================================================================
// Configuration Tests
// =============================================================================

#[cfg(test)]
mod config_tests {
    use symphony::config::Config;

    #[test]
    fn test_default_config_has_plugins() {
        let config = Config::default();
        assert!(config.plugins.enabled);
    }

    #[test]
    fn test_default_config_has_lyrics() {
        let config = Config::default();
        assert!(config.lyrics.enabled);
        assert!(config.lyrics.karaoke_mode);
    }

    #[test]
    fn test_default_config_has_integrations() {
        let config = Config::default();
        assert!(config.integrations.git.enabled);
        assert!(!config.integrations.pomodoro.enabled); // Disabled by default
    }

    #[test]
    fn test_pomodoro_config_defaults() {
        let config = Config::default();
        assert_eq!(config.integrations.pomodoro.work_minutes, 25);
        assert_eq!(config.integrations.pomodoro.break_minutes, 5);
        assert_eq!(config.integrations.pomodoro.long_break_minutes, 15);
    }
}
