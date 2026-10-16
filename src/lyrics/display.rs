//! Lyrics Display Engine
//!
//! Renders karaoke-style synced lyrics in the terminal.

use super::{Lyrics, LyricsLine, WordTimestamp};
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

    /// The lyrics being displayed
    pub fn lyrics(&self) -> Option<&Lyrics> {
        self.lyrics.as_ref()
    }

    /// Attach translations (one per line, `None` where missing) and show them
    pub fn set_translations(&mut self, translations: Vec<Option<String>>) {
        if let Some(lyrics) = &mut self.lyrics {
            for (line, translation) in lyrics.lines.iter_mut().zip(translations) {
                line.translation = translation;
            }
            self.config.show_translation = true;
        }
    }

    /// Whether any line has a translation
    pub fn has_translations(&self) -> bool {
        self.lyrics
            .as_ref()
            .is_some_and(|l| l.lines.iter().any(|line| line.translation.is_some()))
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
            display_lines.push(text_line);

            // Translation goes under the original line
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

                    display_lines.push(Line::from(Span::styled(translation.clone(), trans_style)));
                }
            }
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

        if is_current && self.config.karaoke_mode {
            // Real word timing (enhanced LRC): sung words lit, the current word underlined
            if let Some(words) = line.word_timestamps.as_ref().filter(|w| !w.is_empty()) {
                return self.word_level_line(words);
            }

            // Otherwise light up a share of the characters proportional to the time in the line
            if self.karaoke_progress > 0.0 {
                let chars: Vec<char> = line.text.chars().collect();
                let lit = ((chars.len() as f32 * self.karaoke_progress) as usize).min(chars.len());
                let (done, rest) = chars.split_at(lit);

                return Line::from(vec![
                    Span::styled(
                        done.iter().collect::<String>(),
                        Style::default()
                            .fg(self.config.highlight_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        rest.iter().collect::<String>(),
                        Style::default()
                            .fg(self.config.future_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);
            }
        }

        Line::from(Span::styled(line.text.clone(), style))
    }

    /// Current line with per-word highlighting from word timestamps
    fn word_level_line(&self, words: &[WordTimestamp]) -> Line<'static> {
        let position = self.current_position_ms;
        let highlight = Style::default()
            .fg(self.config.highlight_color)
            .add_modifier(Modifier::BOLD);

        let mut spans = Vec::with_capacity(words.len() * 2);
        for (i, word) in words.iter().enumerate() {
            let style = if position >= word.end_ms {
                highlight
            } else if position >= word.start_ms {
                highlight.add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(self.config.future_color)
                    .add_modifier(Modifier::BOLD)
            };
            spans.push(Span::styled(word.word.clone(), style));
            if i + 1 < words.len() {
                spans.push(Span::raw(" "));
            }
        }
        Line::from(spans)
    }

    /// Render empty state
    fn render_empty(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new("No lyrics for this track\n\nPut a .lrc file next to the song,\nor enable auto_fetch under [lyrics]")
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
            Line::styled(format!("♪ {}", text), Style::default().fg(Color::Yellow))
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

    fn render_to_buffer(
        display: &LyricsDisplay,
        width: u16,
        height: u16,
    ) -> ratatui::buffer::Buffer {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| display.render(f, f.area())).unwrap();
        terminal.backend().buffer().clone()
    }

    /// First cell of `needle` on the screen (row text must be ASCII-only before it)
    fn find(buffer: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
        for y in 0..buffer.area.height {
            let row: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect();
            if let Some(byte) = row.find(needle) {
                return Some((row[..byte].chars().count() as u16, y));
            }
        }
        None
    }

    #[test]
    fn test_word_level_highlighting_follows_the_timestamps() {
        let lyrics = crate::lyrics::LrcParser::parse(
            "[00:12.00]<00:12.00>Hello <00:12.50>big <00:13.20>world\n[00:20.00]Next line",
        )
        .unwrap();
        let mut display = LyricsDisplay::new();
        display.set_lyrics(lyrics);
        display.update_position(12_800); // "Hello" sung, "big" being sung, "world" ahead

        let buffer = render_to_buffer(&display, 40, 8);
        let (x, y) = find(&buffer, "Hello").unwrap();
        let hello = &buffer[(x, y)];
        assert_eq!(hello.fg, Color::Yellow);
        assert!(!hello.modifier.contains(Modifier::UNDERLINED));

        let (x, y) = find(&buffer, "big").unwrap();
        assert_eq!(buffer[(x, y)].fg, Color::Yellow);
        assert!(
            buffer[(x, y)].modifier.contains(Modifier::UNDERLINED),
            "current word underlined"
        );

        let (x, y) = find(&buffer, "world").unwrap();
        assert_eq!(
            buffer[(x, y)].fg,
            Color::Gray,
            "unsung word keeps the future color"
        );
    }

    #[test]
    fn test_karaoke_does_not_split_multibyte_characters() {
        // 3-byte characters: byte slicing at a proportional offset would panic
        let lyrics = Lyrics::new(
            "t",
            vec![
                LyricsLine::new(0, "旧支配者のキャロル"),
                LyricsLine::new(10_000, "次"),
            ],
        );
        let mut display = LyricsDisplay::new();
        display.set_lyrics(lyrics);
        for position in (0..10_000).step_by(317) {
            display.update_position(position);
            render_to_buffer(&display, 30, 6); // must not panic
        }
    }

    #[test]
    fn test_translation_is_shown_under_the_original() {
        let lyrics = Lyrics::new(
            "t",
            vec![LyricsLine::new(0, "Bonjour").with_translation("Hello")],
        );
        let mut display = LyricsDisplay::with_config(LyricsDisplayConfig {
            show_translation: true,
            ..Default::default()
        });
        display.set_lyrics(lyrics);
        display.update_position(100);

        let buffer = render_to_buffer(&display, 30, 6);
        let (_, original_row) = find(&buffer, "Bonjour").unwrap();
        let (_, translation_row) = find(&buffer, "Hello").unwrap();
        assert_eq!(translation_row, original_row + 1);
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
