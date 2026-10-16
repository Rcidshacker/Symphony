//! UI module
//!
//! Terminal user interface using Ratatui.

pub mod artwork;
pub mod visualizer;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Text,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{App, PlaybackState, RepeatMode, Track, View};
use crate::config::theme::Theme;
use crate::ui::visualizer::{render_mini_spectrum, render_visualizer, VisualizerState};

/// Format seconds as `m:ss`
fn fmt_time(secs: f64) -> String {
    let secs = secs.max(0.0) as u64;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Main render function
pub fn render(f: &mut Frame, app: &App) {
    let theme = app.theme_manager.current();

    // Create main layout with visualizer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(12), // Visualizer + Now Playing
            Constraint::Min(10),    // Library/Queue
            Constraint::Length(5),  // Status bar + key hints
        ])
        .split(f.area());

    // Render header
    render_header(f, app, chunks[0], theme);

    // Render now playing section with visualizer
    render_now_playing_section(f, app, chunks[1], theme);

    // Render library/queue, or the lyrics panel when toggled with `l`
    if app.ui_state.show_lyrics {
        app.lyrics.render(f, chunks[2]);
    } else {
        render_library(f, app, chunks[2], theme);
    }

    // Render status bar
    render_status_bar(f, app, chunks[3], theme);

    // Render help overlay if shown
    if app.ui_state.show_help {
        render_help(f, theme);
    }

    // The AI's answer to a question
    if let Some(answer) = &app.ui_state.ai_answer {
        let area = centered_rect(70, 40, f.area());
        f.render_widget(Clear, area);
        f.render_widget(
            Paragraph::new(answer.as_str())
                .wrap(ratatui::widgets::Wrap { trim: true })
                .style(Style::default().fg(theme.foreground()))
                .block(
                    Block::default()
                        .title(" 🤖 AI (any key to close) ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.accent())),
                ),
            area,
        );
    }
}

/// Render header
fn render_header(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let theme_name = app.theme_manager.current_name();
    let title = format!(
        "🎵 SYMPHONY v2.0 | Theme: {} | AI-Powered Terminal Music Player",
        theme_name
    );

    let header = Paragraph::new(title)
        .style(
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent())),
        );

    f.render_widget(header, area);
}

/// Render now playing section with visualizer
fn render_now_playing_section(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    // Split into track info, visualizer, and album art
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(artwork::ART_COLS as u16 + 2), // Album art (+ border)
            Constraint::Min(30),                              // Track info + progress
            Constraint::Length(25),                           // Visualizer
        ])
        .split(area);

    // Render album art
    render_album_art_panel(f, app, chunks[0], theme);

    // Render track info
    render_track_info(f, app, chunks[1], theme);

    // Render visualizer
    render_visualizer_panel(f, app, chunks[2], theme);
}

/// Render album art panel
fn render_album_art_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let artwork = app.current_track_artwork.as_ref();
    let track_title = app.playback_state.current_track.as_ref().map(|t| &t.title);

    artwork::render_album_art(f, area, artwork, theme, track_title.map(|s| s.as_str()));
}

/// Render track info
fn render_track_info(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),    // Track details
            Constraint::Length(3), // Progress bar
            Constraint::Length(3), // Volume
        ])
        .split(area);

    // Track details
    let track_info = if let Some(ref track) = app.playback_state.current_track {
        let status = if app.playback_state.is_playing {
            "▶ Playing"
        } else {
            "⏸ Paused"
        };

        let source = match track.source {
            crate::app::TrackSource::Local => "LOCAL",
            crate::app::TrackSource::YouTube => "YOUTUBE",
            crate::app::TrackSource::Spotify => "SPOTIFY",
            crate::app::TrackSource::SoundCloud => "SOUNDCLOUD",
            crate::app::TrackSource::Cached => "CACHED",
        };

        // Add BPM if available from visualizer
        let bpm_info = app
            .visualizer_state
            .spectrum
            .as_ref()
            .and_then(|s| s.beat.estimated_bpm)
            .map(|bpm| format!(" | {} BPM", bpm as i32))
            .unwrap_or_default();

        format!(
            "{}  [{}]{}\n{}\n{} · {} / {}",
            status,
            source,
            bpm_info,
            track.title,
            track.artist,
            fmt_time(app.playback_state.position),
            fmt_time(app.playback_state.duration),
        )
    } else {
        "⏹ No track loaded\nPress Enter on a track to play".to_string()
    };

    let track_widget = Paragraph::new(track_info)
        .style(Style::default().fg(theme.foreground()))
        .block(
            Block::default()
                .title(" 🎵 NOW PLAYING ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border())),
        );

    f.render_widget(track_widget, chunks[0]);

    // Progress bar
    let progress_pct = if app.playback_state.duration > 0.0 {
        ((app.playback_state.position / app.playback_state.duration) * 100.0) as u16
    } else {
        0
    };

    let progress_label = format!(
        "{} / {}",
        fmt_time(app.playback_state.position),
        fmt_time(app.playback_state.duration)
    );

    let progress_bar = Gauge::default()
        .block(
            Block::default()
                .title(" Progress ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border())),
        )
        .gauge_style(Style::default().fg(theme.accent()))
        .percent(progress_pct.min(100))
        .label(progress_label);

    f.render_widget(progress_bar, chunks[1]);

    // Volume bar
    let volume_pct = (app.playback_state.volume * 100.0).round() as u16;
    let volume_label = if app.playback_state.is_muted {
        "MUTED".to_string()
    } else {
        format!("VOL: {}%", volume_pct)
    };

    let volume_bar = Gauge::default()
        .block(
            Block::default()
                .title(" 🔊 ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border())),
        )
        .gauge_style(Style::default().fg(theme.accent()))
        .percent(volume_pct)
        .label(volume_label);

    f.render_widget(volume_bar, chunks[2]);
}

/// Render visualizer panel
fn render_visualizer_panel(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    visualizer::render_visualizer(f, area, &app.visualizer_state, theme);
}

/// Render library/queue
fn render_library(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    // Get tracks to display
    let tracks = if app.ui_state.search_mode {
        &app.library.filtered_tracks
    } else {
        &app.library.tracks
    };

    // Build track list items
    let items: Vec<ListItem> = tracks
        .iter()
        .map(|track| {
            let is_current = app
                .playback_state
                .current_track
                .as_ref()
                .map(|t| t.id == track.id)
                .unwrap_or(false);

            let prefix = if is_current {
                if app.playback_state.is_playing {
                    "▶ "
                } else {
                    "⏸ "
                }
            } else {
                "  "
            };

            let source = match track.source {
                crate::app::TrackSource::Local => "",
                crate::app::TrackSource::YouTube => " [YT]",
                crate::app::TrackSource::Spotify => " [SP]",
                crate::app::TrackSource::SoundCloud => " [SC]",
                crate::app::TrackSource::Cached => " [CA]",
            };

            let text = format!(
                "{}{} - {} ({}){}",
                prefix,
                track.artist,
                track.title,
                fmt_time(track.duration),
                source
            );

            let style = if is_current {
                Style::default().fg(theme.success())
            } else {
                Style::default().fg(theme.foreground())
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let title = if app.ui_state.ai_mode {
        format!(
            " 🤖 AI: {}▌  (Enter to send, Esc to cancel) ",
            app.ui_state.ai_query
        )
    } else if app.ui_state.ai_pending {
        " 🤖 AI: thinking... ".to_string()
    } else if app.ui_state.search_mode {
        format!(" 📚 LIBRARY [Search: {}] ", app.ui_state.search_query)
    } else {
        " 📚 LIBRARY ".to_string()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border())),
        )
        .highlight_symbol("› ")
        .highlight_style(
            Style::default()
                .fg(theme.ui.selection_fg)
                .bg(theme.ui.selection_bg)
                .add_modifier(Modifier::BOLD),
        );

    // ListState keeps the selected row in view when the library is longer than the pane
    let mut state = ListState::default();
    if !tracks.is_empty() {
        state.select(Some(app.library.selected_index.min(tracks.len() - 1)));
    }
    f.render_stateful_widget(list, area, &mut state);
}

/// Key hints shown on the second status line
const KEY_HINTS: [&str; 2] = [
    "space play/pause · n/p next/prev · +/- volume · m mute · s shuffle · r repeat · q quit",
    "/ search · a ask AI · l lyrics · x translate · w pomodoro · v visualizer · t theme · ? help",
];

/// Render status bar (state + message line, key hint line)
fn render_status_bar(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let accent = Style::default().fg(theme.accent());
    let dim = Style::default().fg(theme.text_dim());
    let sep = || Span::styled(" │ ", dim);

    let mut parts: Vec<Span> = Vec::new();

    // Playback mode indicators
    if app.playback_state.shuffle {
        parts.push(Span::styled("🔀 SHUF", accent));
        parts.push(sep());
    }
    match app.playback_state.repeat {
        RepeatMode::All => {
            parts.push(Span::styled("🔁 ALL", accent));
            parts.push(sep());
        }
        RepeatMode::One => {
            parts.push(Span::styled("🔂 ONE", accent));
            parts.push(sep());
        }
        RepeatMode::Off => {}
    }

    // Visualization mode
    let viz_mode = match app.visualizer_state.mode {
        visualizer::VisualizationMode::Bars => "📊 Bars",
        visualizer::VisualizationMode::Waveform => "〰️ Wave",
        visualizer::VisualizationMode::Circular => "⭕ Circle",
        visualizer::VisualizationMode::Minimal => "▪ Minimal",
        visualizer::VisualizationMode::Off => "❌ Off",
    };
    parts.push(Span::styled(viz_mode, dim));
    parts.push(sep());

    // Track count
    parts.push(Span::styled(
        format!("📁 {} tracks", app.library.tracks.len()),
        dim,
    ));

    // Audio analysis running in the background (makes mood and tempo requests work)
    if app.ui_state.analysis_remaining > 0 {
        parts.push(sep());
        parts.push(Span::styled(
            format!("🔬 analyzing {}", app.ui_state.analysis_remaining),
            dim,
        ));
    }

    // Pomodoro timer
    let pomodoro_state = app.pomodoro.get_state();
    if pomodoro_state.is_running() {
        parts.push(sep());
        parts.push(Span::styled(
            format!(
                "🍅 {} {}",
                pomodoro_state.label(),
                app.pomodoro.format_remaining()
            ),
            accent,
        ));
    }

    // Git activity
    if let Some(git) = app.git_monitor.as_ref().filter(|g| g.in_repo()) {
        parts.push(sep());
        parts.push(Span::styled(
            format!(
                "⎇ {} {}",
                git.get_current_branch().unwrap_or("..."),
                git.get_activity_level().icon()
            ),
            dim,
        ));
    }

    // Latest message or error
    if let Some(ref err) = app.ui_state.error_message {
        parts.push(sep());
        parts.push(Span::styled(
            err.clone(),
            Style::default().fg(theme.error()),
        ));
    } else if let Some(ref msg) = app.ui_state.status_message {
        parts.push(sep());
        parts.push(Span::styled(
            msg.clone(),
            Style::default().fg(theme.success()),
        ));
    }

    let text = Text::from(vec![
        Line::from(parts),
        Line::from(Span::styled(KEY_HINTS[0], dim)),
        Line::from(Span::styled(KEY_HINTS[1], dim)),
    ]);

    let status_bar =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).border_style(dim));

    f.render_widget(status_bar, area);
}

/// Render help overlay
fn render_help(f: &mut Frame, theme: &Theme) {
    let help_text = r#"
╔═══════════════════════════════════════════╗
║           ⌨️  KEYBOARD SHORTCUTS           ║
╠═══════════════════════════════════════════╣
║  Space    Play/Pause                      ║
║  n / ]    Next track                      ║
║  p / [    Previous track                  ║
║  + / =    Volume up                       ║
║  -        Volume down                     ║
║  m        Mute toggle                     ║
║  s        Shuffle toggle                  ║
║  r        Cycle repeat modes              ║
║  a        Ask the AI (type, Enter)        ║
║  l        Show/hide lyrics                ║
║  x        Translate lyrics (AI)           ║
║  w        Pomodoro start/stop             ║
║  /        Search mode                     ║
║  Esc      Exit search/cancel              ║
║  j/k      Navigate up/down                ║
║  Enter    Play selected track             ║
║  v        Cycle visualizer modes          ║
║  t        Cycle themes                    ║
║  1-5      Pick visualizer (5 = off)       ║
║  h / ?    Toggle this help                ║
║  q / Ctrl+C  Quit                         ║
╚═══════════════════════════════════════════╝
"#;

    // Center the help overlay
    let area = centered_rect(60, 75, f.area());

    // Clear the area first
    f.render_widget(Clear, area);

    let help = Paragraph::new(help_text)
        .style(Style::default().fg(theme.foreground()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent())),
        );

    f.render_widget(help, area);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
