//! Lyrics system (Phase 5)
//!
//! Provides synced lyrics display with karaoke-style highlighting.
//! Supports LRC format parsing and online lyrics fetching.
//!
//! # Features
//! - LRC file parsing (synced lyrics)
//! - Online lyrics fetching
//! - Karaoke-style display
//! - Translation support (via AI)
//!
//! # Usage
//! ```rust,no_run
//! use symphony::lyrics::{LyricsDisplay, LrcParser, LyricsFetcher};
//!
//! // Parse LRC file
//! let lyrics = LrcParser::parse(lrc_content)?;
//!
//! // Create display
//! let mut display = LyricsDisplay::new();
//! display.set_lyrics(lyrics);
//!
//! // Update position during playback
//! display.update_position(45000); // 45 seconds
//!
//! // Render to terminal
//! display.render(&mut frame, area);
//! ```

pub mod display;
pub mod fetcher;
pub mod lrc;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export main types
pub use display::LyricsDisplay;
pub use fetcher::LyricsFetcher;
pub use lrc::LrcParser;

/// Synced lyrics line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    /// Timestamp in milliseconds
    pub timestamp_ms: u64,
    /// Lyrics text
    pub text: String,
    /// Optional translation
    pub translation: Option<String>,
    /// Line index for karaoke timing (if word-level sync available)
    pub word_timestamps: Option<Vec<WordTimestamp>>,
}

impl LyricsLine {
    /// Create a new lyrics line
    pub fn new(timestamp_ms: u64, text: impl Into<String>) -> Self {
        Self {
            timestamp_ms,
            text: text.into(),
            translation: None,
            word_timestamps: None,
        }
    }

    /// Add a translation
    pub fn with_translation(mut self, translation: impl Into<String>) -> Self {
        self.translation = Some(translation.into());
        self
    }

    /// Check if this line should be displayed at a given position
    pub fn is_active_at(&self, position_ms: u64) -> bool {
        self.timestamp_ms <= position_ms
    }
}

/// Word-level timestamp for karaoke
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTimestamp {
    /// Word text
    pub word: String,
    /// Start time in milliseconds
    pub start_ms: u64,
    /// End time in milliseconds
    pub end_ms: u64,
}

/// Complete lyrics for a track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    /// Track identifier
    pub track_id: String,
    /// Track title
    pub title: Option<String>,
    /// Artist name
    pub artist: Option<String>,
    /// Album name
    pub album: Option<String>,
    /// Lyrics lines sorted by timestamp
    pub lines: Vec<LyricsLine>,
    /// Language code (ISO 639-1)
    pub language: Option<String>,
    /// Source of the lyrics
    pub source: LyricsSource,
    /// Author/creator of the lyrics
    pub author: Option<String>,
    /// LRC metadata
    pub metadata: LyricsMetadata,
}

impl Lyrics {
    /// Create empty lyrics
    pub fn empty(track_id: impl Into<String>) -> Self {
        Self {
            track_id: track_id.into(),
            title: None,
            artist: None,
            album: None,
            lines: Vec::new(),
            language: None,
            source: LyricsSource::Unknown,
            author: None,
            metadata: LyricsMetadata::default(),
        }
    }

    /// Create lyrics with lines
    pub fn new(track_id: impl Into<String>, lines: Vec<LyricsLine>) -> Self {
        Self {
            track_id: track_id.into(),
            lines,
            ..Self::empty("")
        }
    }

    /// Check if lyrics are empty
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get the number of lines
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Find the current line index for a position
    pub fn find_line_index(&self, position_ms: u64) -> Option<usize> {
        // Binary search for the current line
        let idx = self
            .lines
            .partition_point(|line| line.timestamp_ms <= position_ms);

        if idx == 0 {
            if self.lines.first()?.timestamp_ms > position_ms {
                return None;
            }
            return Some(0);
        }

        Some(idx - 1)
    }

    /// Get the current line at a position
    pub fn get_current_line(&self, position_ms: u64) -> Option<&LyricsLine> {
        let idx = self.find_line_index(position_ms)?;
        self.lines.get(idx)
    }

    /// Get lines within a range of the current position
    pub fn get_context_lines(
        &self,
        position_ms: u64,
        before: usize,
        after: usize,
    ) -> (Vec<&LyricsLine>, Option<&LyricsLine>, Vec<&LyricsLine>) {
        let current_idx = self.find_line_index(position_ms);

        let past = match current_idx {
            Some(idx) => {
                let start = idx.saturating_sub(before);
                self.lines[start..idx].iter().collect()
            }
            None => Vec::new(),
        };

        let current = current_idx.and_then(|idx| self.lines.get(idx));

        let future = match current_idx {
            Some(idx) => {
                let start = (idx + 1).min(self.lines.len());
                let end = (idx + 1 + after).min(self.lines.len());
                self.lines[start..end].iter().collect()
            }
            None => {
                let end = after.min(self.lines.len());
                self.lines[..end].iter().collect()
            }
        };

        (past, current, future)
    }

    /// Add translations to all lines
    pub fn with_translations(mut self, translations: Vec<Option<String>>) -> Self {
        for (line, translation) in self.lines.iter_mut().zip(translations.iter()) {
            if let Some(t) = translation {
                line.translation = Some(t.clone());
            }
        }
        self
    }

    /// Get total duration covered by lyrics
    pub fn total_duration_ms(&self) -> u64 {
        self.lines.last().map(|l| l.timestamp_ms).unwrap_or(0)
    }
}

/// Source of lyrics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LyricsSource {
    /// Local LRC file
    LocalFile,
    /// Embedded in audio file
    Embedded,
    /// Fetched from online service
    Online {
        service: String,
        url: Option<String>,
    },
    /// Generated by AI
    AIGenerated,
    /// User provided
    UserProvided,
    /// Unknown source
    Unknown,
}

impl std::fmt::Display for LyricsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LyricsSource::LocalFile => write!(f, "Local"),
            LyricsSource::Embedded => write!(f, "Embedded"),
            LyricsSource::Online { service, .. } => write!(f, "{}", service),
            LyricsSource::AIGenerated => write!(f, "AI"),
            LyricsSource::UserProvided => write!(f, "User"),
            LyricsSource::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Lyrics metadata from LRC file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricsMetadata {
    /// Song title
    pub title: Option<String>,
    /// Artist name
    pub artist: Option<String>,
    /// Album name
    pub album: Option<String>,
    /// Lyrics author
    pub author: Option<String>,
    /// LRC file creator
    pub creator: Option<String>,
    /// Length in milliseconds
    pub length_ms: Option<u64>,
    /// Karaoke mode
    pub karaoke: bool,
    /// Offset in milliseconds (for timing adjustment)
    pub offset_ms: i64,
}

/// Lyrics cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLyrics {
    /// Lyrics data
    pub lyrics: Lyrics,
    /// When cached
    pub cached_at: chrono::DateTime<chrono::Utc>,
    /// Cache file path
    pub path: PathBuf,
}

/// Lyrics error type
#[derive(Debug, thiserror::Error)]
pub enum LyricsError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Lyrics not found")]
    NotFound,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Cache error: {0}")]
    CacheError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lyrics_line() {
        let line = LyricsLine::new(5000, "Hello World");
        assert_eq!(line.timestamp_ms, 5000);
        assert_eq!(line.text, "Hello World");
        assert!(line.is_active_at(6000));
        assert!(!line.is_active_at(4000));
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
}
