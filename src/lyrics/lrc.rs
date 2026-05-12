//! LRC Parser
//!
//! Parses LRC format lyrics files with support for:
//! - Standard LRC format
//! - Enhanced LRC (word-level timing)
//! - Metadata tags

use super::{Lyrics, LyricsError, LyricsLine, LyricsMetadata, LyricsSource, WordTimestamp};
use regex::Regex;
use std::path::Path;
use tracing::{debug, warn};

/// LRC file parser
pub struct LrcParser;

impl LrcParser {
    /// Parse LRC content string
    pub fn parse(content: &str) -> Result<Lyrics, LyricsError> {
        let mut lines = Vec::new();
        let mut metadata = LyricsMetadata::default();

        // Regex for timestamp: [mm:ss.xx] or [mm:ss:xx]
        let timestamp_re = Regex::new(r"\[(\d+):(\d+)(?:[.:](\d+))?\](.*)").map_err(|e| {
            LyricsError::ParseError(format!("Failed to compile regex: {}", e))
        })?;

        // Regex for metadata tags: [tag:value]
        let metadata_re = Regex::new(r"\[([a-z]+):(.+)\]").map_err(|e| {
            LyricsError::ParseError(format!("Failed to compile regex: {}", e))
        })?;

        // Regex for enhanced LRC word timing: <mm:ss.xx>
        let word_re = Regex::new(r"<(\d+):(\d+)(?:[.:](\d+))?([^>]*)>").map_err(|e| {
            LyricsError::ParseError(format!("Failed to compile regex: {}", e))
        })?;

        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            // Try to parse as metadata tag first
            if parse_metadata_tag(line, &metadata_re, &mut metadata) {
                continue;
            }

            // Try to parse as lyrics line with timestamp
            if let Some(lyrics_line) = parse_time_tag(line, &timestamp_re, &word_re)? {
                lines.push(lyrics_line);
            }
        }

        // Sort lines by timestamp
        lines.sort_by_key(|l| l.timestamp_ms);

        Ok(Lyrics {
            track_id: String::new(),
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            album: metadata.album.clone(),
            lines,
            language: None,
            source: LyricsSource::LocalFile,
            author: metadata.author.clone(),
            metadata,
        })
    }

    /// Parse LRC file from path
    pub fn parse_file(path: &Path) -> Result<Lyrics, LyricsError> {
        let content = std::fs::read_to_string(path)?;
        let mut lyrics = Self::parse(&content)?;
        lyrics.source = LyricsSource::LocalFile;
        Ok(lyrics)
    }

    /// Parse LRC from bytes (for embedded lyrics)
    pub fn parse_bytes(bytes: &[u8]) -> Result<Lyrics, LyricsError> {
        // Try UTF-8 first, then fall back to other encodings
        let content = String::from_utf8_lossy(bytes);
        let mut lyrics = Self::parse(&content)?;
        lyrics.source = LyricsSource::Embedded;
        Ok(lyrics)
    }
}

/// Parse time tag and return a LyricsLine if successful
fn parse_time_tag(
    line: &str,
    timestamp_re: &Regex,
    word_re: &Regex,
) -> Result<Option<LyricsLine>, LyricsError> {
    if let Some(captures) = timestamp_re.captures(line) {
        let minutes: u64 = captures[1].parse().unwrap_or(0);
        let seconds: u64 = captures[2].parse().unwrap_or(0);
        let centiseconds: u64 = captures
            .get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);

        // Convert to milliseconds
        let timestamp_ms = minutes * 60 * 1000 + seconds * 1000 + centiseconds * 10;

        let text = captures[4].trim().to_string();

        // Parse word-level timestamps if present (enhanced LRC)
        let word_timestamps = if word_re.is_match(&text) {
            Some(parse_word_timestamps(&text, word_re)?)
        } else {
            None
        };

        // Clean up text by removing word timing tags
        let clean_text = word_re.replace_all(&text, "$4").to_string();

        if !clean_text.is_empty() {
            return Ok(Some(LyricsLine {
                timestamp_ms,
                text: clean_text,
                translation: None,
                word_timestamps,
            }));
        }
    }
    Ok(None)
}

/// Parse metadata tag and update metadata struct
fn parse_metadata_tag(line: &str, metadata_re: &Regex, metadata: &mut LyricsMetadata) -> bool {
    if let Some(captures) = metadata_re.captures(line) {
        let tag = captures[1].to_lowercase();
        let value = captures[2].trim().to_string();

        match tag.as_str() {
            "ti" | "title" => metadata.title = Some(value),
            "ar" | "artist" => metadata.artist = Some(value),
            "al" | "album" => metadata.album = Some(value),
            "au" | "author" => metadata.author = Some(value),
            "by" | "creator" => metadata.creator = Some(value),
            "length" => {
                // Parse length like "3:45" or "225"
                metadata.length_ms = parse_length(&value);
            }
            "offset" => {
                metadata.offset_ms = value.parse().unwrap_or(0);
            }
            "re" | "tool" => {} // Tool used to create
            "ve" | "version" => {} // LRC version
            _ => debug!("Unknown metadata tag: {}", tag),
        }
        return true;
    }
    false
}

/// Parse length string to milliseconds
fn parse_length(s: &str) -> Option<u64> {
    // Try "mm:ss" format
    if let Some(idx) = s.find(':') {
        let minutes: u64 = s[..idx].parse().ok()?;
        let seconds: u64 = s[idx + 1..].parse().ok()?;
        return Some(minutes * 60 * 1000 + seconds * 1000);
    }

    // Try raw seconds
    if let Ok(secs) = s.parse::<u64>() {
        return Some(secs * 1000);
    }

    None
}

/// Parse word-level timestamps from enhanced LRC
fn parse_word_timestamps(text: &str, re: &Regex) -> Result<Vec<WordTimestamp>, LyricsError> {
    let mut words = Vec::new();
    let mut last_end_ms = 0u64;

    for captures in re.captures_iter(text) {
        let minutes: u64 = captures[1].parse().unwrap_or(0);
        let seconds: u64 = captures[2].parse().unwrap_or(0);
        let centiseconds: u64 = captures.get(3)
            .map(|m| m.as_str().parse().unwrap_or(0))
            .unwrap_or(0);

        let start_ms = minutes * 60 * 1000 + seconds * 1000 + centiseconds * 10;
        let word = captures[4].trim().to_string();

        // Calculate end time (either from next word or estimate)
        let end_ms = start_ms + (word.len() as u64 * 100); // Rough estimate

        if !word.is_empty() {
            words.push(WordTimestamp {
                word,
                start_ms,
                end_ms,
            });
            last_end_ms = end_ms;
        }
    }

    Ok(words)
}

/// Convert lyrics to LRC format
pub fn to_lrc(lyrics: &Lyrics) -> String {
    let mut output = String::new();

    // Write metadata
    if let Some(title) = &lyrics.metadata.title {
        output.push_str(&format!("[ti:{}]\n", title));
    }
    if let Some(artist) = &lyrics.metadata.artist {
        output.push_str(&format!("[ar:{}]\n", artist));
    }
    if let Some(album) = &lyrics.metadata.album {
        output.push_str(&format!("[al:{}]\n", album));
    }
    if let Some(author) = &lyrics.metadata.author {
        output.push_str(&format!("[au:{}]\n", author));
    }
    if lyrics.metadata.offset_ms != 0 {
        output.push_str(&format!("[offset:{}]\n", lyrics.metadata.offset_ms));
    }

    // Add blank line
    if !output.is_empty() {
        output.push('\n');
    }

    // Write lyrics lines
    for line in &lyrics.lines {
        let minutes = line.timestamp_ms / 60000;
        let seconds = (line.timestamp_ms % 60000) / 1000;
        let centiseconds = (line.timestamp_ms % 1000) / 10;

        output.push_str(&format!(
            "[{:02}:{:02}.{:02}]{}\n",
            minutes, seconds, centiseconds, line.text
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_lrc() {
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
    fn test_parse_with_offset() {
        let lrc = r#"
[offset:500]
[00:00.00]Line one
"#;

        let lyrics = LrcParser::parse(lrc).unwrap();
        assert_eq!(lyrics.metadata.offset_ms, 500);
    }

    #[test]
    fn test_to_lrc() {
        let lyrics = Lyrics::new(
            "test",
            vec![
                LyricsLine::new(0, "First"),
                LyricsLine::new(5000, "Second"),
            ],
        );

        let lrc = to_lrc(&lyrics);
        assert!(lrc.contains("[00:00.00]First"));
        assert!(lrc.contains("[00:05.00]Second"));
    }

    #[test]
    fn test_parse_empty() {
        let lyrics = LrcParser::parse("").unwrap();
        assert!(lyrics.is_empty());
    }

    #[test]
    fn test_parse_length() {
        assert_eq!(parse_length("3:45"), Some(225000));
        assert_eq!(parse_length("225"), Some(225000));
    }
}
