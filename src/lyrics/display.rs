//! Lyrics Display Engine
//!
//! Renders karaoke-style synced lyrics in the terminal.

use super::{Lyrics, LyricsLine};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::time::Instant;

/// Lyrics display configuration
#[derive(Debug, Clone)]
pub struct LyricsDisplayConfig {
    /// Enable karaoke-style highlighting
    pub karaoke_mode: bool,
    /// Show translations
    pub show_translation: bool,
    /// Number of lines to show before current
    pub context_before: usize,
    /// Number of lines to show after current
    pub context_after: usize,
    /// Center the current line
    pub center_current: bool,
    /// Fade past lines
    pub fade_past: bool,
    /// Highlight color for current line
    pub highlight_color: Color,
    /// Past line color
    pub past_color: Color,
    /// Future line color
    pub future_color: Color,
}

impl Default for LyricsDisplayConfig {
    fn default() -> Self {
        Self {
            karaoke_mode: true,
            show_translation: true,
            context_before: 2,
            context_after: 2,
            center_current: true,
            fade_past: true,
            highlight_color: Color::Yellow,
            past_color: Color::DarkGray,
            future_color: Color::Gray,
        }
    }
}

/// Lyrics display engine
pub struct LyricsDisplay {
    /// Current lyrics
    lyrics: Option<Lyrics>,
    /// Current playback position in milliseconds
    current_position_ms: u64,
    /// Display configuration
    config: LyricsDisplayConfig,
    /// Last update time (for animations)
    last_update: Instant,
    /// Karaoke animation progress (0.0 to 1.0)
    karaoke_progress: f32,
}

impl LyricsDisplay {
    /// Create a new lyrics display
    pub fn new() -> Self {
        Self {
            lyrics: None,
            current_position_ms: 0,
            config: LyricsDisplayConfig::default(),
            last_update: Instant::now(),
            karaoke_progress: 0.0,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: LyricsDisplayConfig) -> Self {
        Self {
            config,
            ..Self::new()
        }
    }

    /// Set the lyrics to display
    pub fn set_lyrics(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
        self.current_position_ms = 0;
        self.karaoke_progress = 0.0;
    }

    /// Clear the current lyrics
    pub fn clear(&mut self) {
        self.lyrics = None;
        self.current_position_ms = 0;
    }

    /// Update playback position
    pub fn update_position(&mut self, position_ms: u64) {
        // Calculate karaoke progress based on position within current line
        if let Some(lyrics) = &self.lyrics {
            if let Some(current_idx) = lyrics.find_line_index(position_ms) {
                if let Some(current_line) = lyrics.lines.get(current_idx) {
                    if let Some(next_line) = lyrics.lines.get(current_idx + 1) {
                        let line_start = current_line.timestamp_ms;
                        let line_end = next_line.timestamp_ms;
                        let line_duration = line_end - line_start;

                        if line_duration > 0 {
                            self.karaoke_progress =
                                (position_ms - line_start) as f32 / line_duration as f32;
                        }
                    } else {
                        // Last line
                        self.karaoke_progress = 1.0;
                    }
                }
            }
        }

        self.current_position_ms = position_ms;
        self.last_update = Instant::now();
    }

    /// Toggle karaoke mode
    pub fn toggle_karaoke_mode(&mut self) {
        self.config.karaoke_mode = !self.config.karaoke_mode;
    }

    /// Toggle translation display
    pub fn toggle_translation(&mut self) {
        self.config.show_translation = !self.config.show_translation;
    }

    /// Check if lyrics are loaded
    pub fn has_lyrics(&self) -> bool {
        self.lyrics.is_some() && !self.lyrics.as_ref().unwrap().is_empty()
    }

    /// Get current line text
    pub fn get_current_line_text(&self) -> Option<&str> {
        let lyrics = self.lyrics.as_ref()?;
        let idx = lyrics.find_line_index(self.current_position_ms)?;
        lyrics.lines.get(idx).map(|l| l.text.as_str())
    }

    /// Render the lyrics display
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let Some(lyrics) = &self.lyrics else {
            self.render_empty(frame, area);
            return;
        };

        if lyrics.is_empty() {
            self.render_empty(frame, area);
            return;
        }

        // Find current line
        let current_idx = lyrics.find_line_index(self.current_position_ms);

        // Build display lines
        let mut display_lines = Vec::new();

        // Calculate range to display
        let (start_idx, end_idx) = match current_idx {
            Some(idx) => {
                let start = idx.saturating_sub(self.config.context_before);
                let end = (idx + 1 + self.config.context_after).min(lyrics.lines.len());
                (start, end)
            }
            None => {
                // Before first line - show first few lines
                let end = (self.config.context_after + 1).min(lyrics.lines.len());
                (0, end)
            }
        };

        // Render each line
        for idx in start_idx..end_idx {
            let is_current = current_idx == Some(idx);
            let line = &lyrics.lines[idx];

            let text_line = self.render_line(line, is_current, idx, current_idx);

            // Add translation if enabled and available
            if self.config.show_translation {
                if let Some(translation) = &line.translation {
                    let trans_style = if is_current {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::ITALIC)
                    } else {
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC)
                    };

                    display_lines.push(Line::from(Span::styled(
                        translation.clone(),
                        trans_style,
                    )));
                }
            }

            display_lines.push(text_line);
        }

        // Create paragraph
        let title = if self.config.karaoke_mode {
            "🎤 Lyrics (Karaoke)"
        } else {
            "🎤 Lyrics"
        };

        let paragraph = Paragraph::new(display_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .title_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false })
            .style(Style::default());

        frame.render_widget(paragraph, area);
    }

    /// Render a single lyrics line
    fn render_line(
        &self,
        line: &LyricsLine,
        is_current: bool,
        idx: usize,
        current_idx: Option<usize>,
    ) -> Line {
        let style = if is_current {
            if self.config.karaoke_mode {
                Style::default()
                    .fg(self.config.highlight_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            }
        } else if let Some(current) = current_idx {
            if idx < current {
                // Past line
                if self.config.fade_past {
                    Style::default().fg(self.config.past_color)
                } else {
                    Style::default().fg(Color::Gray)
                }
            } else {
                // Future line
                Style::default().fg(self.config.future_color)
            }
        } else {
            Style::default().fg(Color::Gray)
        };

        // Karaoke-style partial highlighting
        if is_current && self.config.karaoke_mode && self.karaoke_progress > 0.0 {
            let text = &line.text;
            let highlight_len = ((text.len() as f32) * self.karaoke_progress) as usize;

            let highlighted = &text[..highlight_len.min(text.len())];
            let remaining = &text[highlight_len.min(text.len())..];

            Line::from(vec![
                Span::styled(
                    highlighted.to_string(),
                    Style::default()
                        .fg(self.config.highlight_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    remaining.to_string(),
                    Style::default()
                        .fg(self.config.future_color)
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(Span::styled(line.text.clone(), style))
        }
    }

    /// Render empty state
    fn render_empty(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new("No lyrics available\n\nPress 'L' to load lyrics")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("🎤 Lyrics")
                    .title_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(paragraph, area);
    }

    /// Get display configuration
    pub fn config(&self) -> &LyricsDisplayConfig {
        &self.config
    }

    /// Set display configuration
    pub fn set_config(&mut self, config: LyricsDisplayConfig) {
        self.config = config;
    }
}

impl Default for LyricsDisplay {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal lyrics display (for status bar)
pub struct MiniLyricsDisplay {
    lyrics: Option<Lyrics>,
    position_ms: u64,
}

impl MiniLyricsDisplay {
    /// Create a new mini display
    pub fn new() -> Self {
        Self {
            lyrics: None,
            position_ms: 0,
        }
    }

    /// Set lyrics
    pub fn set_lyrics(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
    }

    /// Update position
    pub fn update_position(&mut self, position_ms: u64) {
        self.position_ms = position_ms;
    }

    /// Get current line text (truncated for status bar)
    pub fn get_current_text(&self, max_len: usize) -> String {
        let lyrics = match &self.lyrics {
            Some(l) => l,
            None => return String::new(),
        };

        match lyrics.get_current_line(self.position_ms) {
            Some(line) => {
                let text = &line.text;
                if text.len() <= max_len {
                    text.clone()
                } else {
                    format!("{}...", &text[..max_len - 3])
                }
            }
            None => String::new(),
        }
    }

    /// Render as a single line
    pub fn render(&self) -> Line {
        let text = self.get_current_text(60);
        if text.is_empty() {
            Line::raw("")
        } else {
            Line::styled(
                format!("♪ {}", text),
                Style::default().fg(Color::Yellow),
            )
        }
    }
}

impl Default for MiniLyricsDisplay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lyrics_display_creation() {
        let display = LyricsDisplay::new();
        assert!(!display.has_lyrics());
    }

    #[test]
    fn test_lyrics_display_with_lyrics() {
        let mut display = LyricsDisplay::new();
        let lyrics = Lyrics::new(
            "test",
            vec![
                LyricsLine::new(0, "Line 1"),
                LyricsLine::new(5000, "Line 2"),
            ],
        );

        display.set_lyrics(lyrics);
        assert!(display.has_lyrics());

        display.update_position(3000);
        assert_eq!(display.get_current_line_text(), Some("Line 1"));

        display.update_position(6000);
        assert_eq!(display.get_current_line_text(), Some("Line 2"));
    }

    #[test]
    fn test_mini_lyrics_display() {
        let mut mini = MiniLyricsDisplay::new();
        let lyrics = Lyrics::new("test", vec![LyricsLine::new(0, "Test Line")]);

        mini.set_lyrics(lyrics);
        mini.update_position(0);

        assert_eq!(mini.get_current_text(10), "Test Line");
        assert_eq!(mini.get_current_text(5), "Te...");
    }

    #[test]
    fn test_toggle_modes() {
        let mut display = LyricsDisplay::new();

        assert!(display.config().karaoke_mode);
        display.toggle_karaoke_mode();
        assert!(!display.config().karaoke_mode);

        assert!(display.config().show_translation);
        display.toggle_translation();
        assert!(!display.config().show_translation);
    }
}
