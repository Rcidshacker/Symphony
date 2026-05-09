//! UI module
//!
//! Terminal user interface using Ratatui.

pub mod artwork;
pub mod visualizer;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, PlaybackState, RepeatMode, Track, View};
use crate::config::theme::Theme;
use crate::ui::visualizer::{render_mini_spectrum, render_visualizer, VisualizerState};

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
            Constraint::Length(3),  // Status bar
        ])
        .split(f.area());

    // Render header
    render_header(f, app, chunks[0], theme);

    // Render now playing section with visualizer
    render_now_playing_section(f, app, chunks[1], theme);

    // Render library/queue
    render_library(f, app, chunks[2], theme);

    // Render status bar
    render_status_bar(f, app, chunks[3], theme);

    // Render help overlay if shown
    if app.ui_state.show_help {
        render_help(f, theme);
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
            Constraint::Length(20), // Album art
            Constraint::Min(30),    // Track info + progress
            Constraint::Length(25), // Visualizer
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
            "{}\n\n{}\n{}\n\n[{}]{}\n{:.0}:{:05.2} / {:.0}:{:05.2}",
            status,
            track.title,
            track.artist,
            source,
            bpm_info,
            app.playback_state.position.floor() / 60.0,
            app.playback_state.position % 60.0,
            app.playback_state.duration.floor() / 60.0,
            app.playback_state.duration % 60.0,
        )
    } else {
        "⏹ No track loaded\n\n\n\n\n".to_string()
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
        "{:.0}:{:02.0}",
        app.playback_state.position.floor() / 60.0,
        app.playback_state.position % 60.0
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
    let volume_pct = (app.playback_state.volume * 100.0) as u16;
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
        .enumerate()
        .map(|(i, track)| {
            let is_current = app
                .playback_state
                .current_track
                .as_ref()
                .map(|t| t.id == track.id)
                .unwrap_or(false);

            let is_selected = i == app.library.selected_index;

            let prefix = if is_current {
                if app.playback_state.is_playing {
                    "▶ "
                } else {
                    "⏸ "
                }
            } else {
                "  "
            };

            let duration_mins = (track.duration / 60.0).floor() as i32;
            let duration_secs = (track.duration % 60.0).round() as i32;

            let source = match track.source {
                crate::app::TrackSource::Local => "",
                crate::app::TrackSource::YouTube => " [YT]",
                crate::app::TrackSource::Spotify => " [SP]",
                crate::app::TrackSource::SoundCloud => " [SC]",
                crate::app::TrackSource::Cached => " [CA]",
            };

            let text = format!(
                "{}{} - {} ({:02}:{:02}){}",
                prefix, track.artist, track.title, duration_mins, duration_secs, source
            );

            let style = if is_selected {
                Style::default()
                    .fg(theme.accent())
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(theme.success())
            } else {
                Style::default().fg(theme.foreground())
            };

            ListItem::new(text).style(style)
        })
        .collect();

    let title = if app.ui_state.search_mode {
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
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(list, area);
}

/// Render status bar
fn render_status_bar(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mut status_parts = Vec::new();

    // Playback mode indicators
    if app.playback_state.shuffle {
        status_parts.push(Span::styled("🔀 SHUF", Style::default().fg(theme.accent())));
    }
    match app.playback_state.repeat {
        RepeatMode::All => {
            status_parts.push(Span::styled("🔁 ALL", Style::default().fg(theme.accent())));
        }
        RepeatMode::One => {
            status_parts.push(Span::styled("🔂 ONE", Style::default().fg(theme.accent())));
        }
        RepeatMode::Off => {}
    }

    // Visualization mode
    let viz_mode = match app.visualizer_state.mode {
        visualizer::VisualizationMode::Bars => "📊 Bars",
        visualizer::VisualizationMode::Waveform => "〰️ Wave",
        visualizer::VisualizationMode::Circular => "⭕ Circle",
        visualizer::VisualizationMode::Off => "❌ Off",
    };
    status_parts.push(Span::styled(
        format!("| {}", viz_mode),
        Style::default().fg(theme.text_dim()),
    ));

    // Track count
    status_parts.push(Span::styled(
        format!("| 📁 {} tracks", app.library.tracks.len()),
        Style::default().fg(theme.text_dim()),
    ));

    // Message or key hints
    let message = if let Some(ref msg) = app.ui_state.status_message {
        Span::styled(format!("| {}", msg), Style::default().fg(theme.success()))
    } else if let Some(ref err) = app.ui_state.error_message {
        Span::styled(format!("| {}", err), Style::default().fg(theme.error()))
    } else {
        Span::styled(
            "| Press ? for help | t: themes | v: visualizer",
            Style::default().fg(theme.text_dim()),
        )
    };
    status_parts.push(message);

    let status_line = Line::from(status_parts);

    let status_bar = Paragraph::new(status_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.text_dim())),
    );

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
║  /        Search mode                     ║
║  Esc      Exit search/cancel              ║
║  j/k      Navigate up/down                ║
║  Enter    Play selected track             ║
║  v        Cycle visualizer modes          ║
║  t        Cycle themes                    ║
║  1-4      Select visualizer mode          ║
║  h / ?    Toggle this help                ║
║  q / Ctrl+C  Quit                         ║
╠═══════════════════════════════════════════╣
║  Phase 2: Visual Excellence               ║
║  ✓ FFT Spectrum Analyzer                  ║
║  ✓ 5 Themes                               ║
║  ✓ Album Art                              ║
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
