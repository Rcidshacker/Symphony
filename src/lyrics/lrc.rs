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
    ///
    /// Supports `[mm:ss.xx]` lines (any fraction length, several timestamps per line for
    /// repeated choruses), header tags (`[ti:..]`, `[ar:..]`, `[offset:..]`, ...) and
    /// enhanced LRC word timing (`<mm:ss.xx>word`).
    pub fn parse(content: &str) -> Result<Lyrics, LyricsError> {
        let compile = |pattern: &str| {
            Regex::new(pattern)
                .map_err(|e| LyricsError::ParseError(format!("Failed to compile regex: {e}")))
        };

        // Leading timestamp tags: [mm:ss], [mm:ss.x], [mm:ss.xx], [mm:ss.xxx] ('.' or ':' before the fraction)
        let stamp_re = compile(r"^\s*\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]")?;
        // Header tags such as [ti:Title]. They must be the whole line so that a lyric
        // containing "[x: y]" is not mistaken for one.
        let metadata_re = compile(r"^\[([A-Za-z]+):(.*)\]\s*$")?;
        // Enhanced LRC word timing: <mm:ss.xx>
        let word_re = compile(r"<(\d+):(\d{1,2})(?:[.:](\d{1,3}))?>")?;

        let mut metadata = LyricsMetadata::default();
        let mut lines: Vec<LyricsLine> = Vec::new();

        for raw in content.lines() {
            let line = raw.trim().trim_start_matches('\u{feff}');
            if line.is_empty() {
                continue;
            }

            // One or more leading timestamps
            let mut rest = line;
            let mut stamps = Vec::new();
            while let Some(caps) = stamp_re.captures(rest) {
                stamps.push(to_ms(&caps[1], &caps[2], caps.get(3).map(|m| m.as_str())));
                rest = &rest[caps.get(0).map_or(0, |m| m.end())..];
            }

            if stamps.is_empty() {
                if let Some(caps) = metadata_re.captures(line) {
                    let tag = caps[1].to_lowercase();
                    let value = caps[2].trim().to_string();

                    match tag.as_str() {
                        "ti" | "title" => metadata.title = Some(value),
                        "ar" | "artist" => metadata.artist = Some(value),
                        "al" | "album" => metadata.album = Some(value),
                        "au" | "author" => metadata.author = Some(value),
                        "by" | "creator" => metadata.creator = Some(value),
                        "length" => metadata.length_ms = parse_length(&value),
                        "offset" => metadata.offset_ms = value.parse().unwrap_or(0),
                        "re" | "tool" | "ve" | "version" => {}
                        _ => debug!("Unknown metadata tag: {}", tag),
                    }
                }
                continue;
            }

            let body = rest.trim();
            let words = word_re
                .is_match(body)
                .then(|| parse_enhanced_words(body, &word_re));
            let clean_text = if words.is_some() {
                word_re
                    .replace_all(body, "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                body.to_string()
            };
            if clean_text.is_empty() {
                continue;
            }

            for timestamp_ms in stamps {
                lines.push(LyricsLine {
                    timestamp_ms,
                    text: clean_text.clone(),
                    translation: None,
                    word_timestamps: words.clone(),
                });
            }
        }

        // [offset:+500] makes the lyrics appear 500 ms earlier (the time is shifted down)
        if metadata.offset_ms != 0 {
            let shift = |ms: u64| (ms as i64 - metadata.offset_ms).max(0) as u64;
            for line in &mut lines {
                line.timestamp_ms = shift(line.timestamp_ms);
                for word in line.word_timestamps.iter_mut().flatten() {
                    word.start_ms = shift(word.start_ms);
                    word.end_ms = shift(word.end_ms);
                }
            }
        }

        lines.sort_by_key(|l| l.timestamp_ms);

        // The last word of a line ends no later than the next line starts
        let next_starts: Vec<Option<u64>> = (0..lines.len())
            .map(|i| lines.get(i + 1).map(|l| l.timestamp_ms))
            .collect();
        for (line, next_start) in lines.iter_mut().zip(next_starts) {
            if let (Some(words), Some(next)) = (line.word_timestamps.as_mut(), next_start) {
                if let Some(last) = words.last_mut() {
                    last.end_ms = last.end_ms.min(next).max(last.start_ms);
                }
            }
        }

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

/// Milliseconds from `mm`, `ss` and an optional fraction of a second ("5" = 500 ms, "50" = 500 ms, "500" = 500 ms)
fn to_ms(minutes: &str, seconds: &str, fraction: Option<&str>) -> u64 {
    let minutes: u64 = minutes.parse().unwrap_or(0);
    let seconds: u64 = seconds.parse().unwrap_or(0);
    let fraction_ms = fraction.map_or(0, |f| {
        let value: u64 = f.parse().unwrap_or(0);
        value * 10u64.pow(3 - f.len().min(3) as u32)
    });
    minutes * 60_000 + seconds * 1_000 + fraction_ms
}

/// Word timing from enhanced LRC: `<00:12.00>Hello <00:12.50>world`.
/// A word ends where the next one starts; the last word gets an estimate that
/// the caller clamps to the next line.
fn parse_enhanced_words(text: &str, re: &Regex) -> Vec<WordTimestamp> {
    let tags: Vec<_> = re.captures_iter(text).collect();
    let mut words = Vec::new();

    for (i, caps) in tags.iter().enumerate() {
        let start_ms = to_ms(&caps[1], &caps[2], caps.get(3).map(|m| m.as_str()));
        let text_start = caps.get(0).map_or(0, |m| m.end());
        let text_end = tags
            .get(i + 1)
            .and_then(|next| next.get(0))
            .map_or(text.len(), |m| m.start());

        let word = text[text_start..text_end].trim().to_string();
        if word.is_empty() {
            continue;
        }

        let end_ms = tags.get(i + 1).map_or(
            start_ms + 100 * word.chars().count() as u64,
            |next| to_ms(&next[1], &next[2], next.get(3).map(|m| m.as_str())),
        );
        words.push(WordTimestamp {
            word,
            start_ms,
            end_ms,
        });
    }

    words
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
    // (no [offset] tag: parsing already applied it to the timestamps)

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
    fn test_repeated_timestamps_make_one_line_each() {
        let lyrics = LrcParser::parse("[00:10.00][00:50.00]Chorus\n[00:20.00]Verse").unwrap();
        let got: Vec<_> = lyrics.lines.iter().map(|l| (l.timestamp_ms, l.text.as_str())).collect();
        assert_eq!(got, vec![(10_000, "Chorus"), (20_000, "Verse"), (50_000, "Chorus")]);
    }

    #[test]
    fn test_fraction_digits_are_scaled() {
        let lyrics = LrcParser::parse("[00:05.5]a\n[00:06.25]b\n[00:07.123]c\n[01:02]d").unwrap();
        let times: Vec<_> = lyrics.lines.iter().map(|l| l.timestamp_ms).collect();
        assert_eq!(times, vec![5_500, 6_250, 7_123, 62_000]);
    }

    #[test]
    fn test_offset_shifts_timestamps() {
        let early = LrcParser::parse("[offset:500]\n[00:01.00]a\n[00:00.20]b").unwrap();
        assert_eq!(early.lines[0].timestamp_ms, 0, "clamped at zero");
        assert_eq!(early.lines[1].timestamp_ms, 500);

        let late = LrcParser::parse("[offset:-500]\n[00:01.00]a").unwrap();
        assert_eq!(late.lines[0].timestamp_ms, 1_500);
    }

    #[test]
    fn test_brackets_inside_a_lyric_are_not_headers() {
        let lyrics = LrcParser::parse("[00:01.00]Hello [world: again]").unwrap();
        assert_eq!(lyrics.lines[0].text, "Hello [world: again]");
        assert!(lyrics.metadata.title.is_none());
    }

    #[test]
    fn test_unicode_and_bom() {
        let lyrics = LrcParser::parse("\u{feff}[ti:旧支配者のキャロル]\n[00:01.00]歌ってみた ☃️").unwrap();
        assert_eq!(lyrics.title.as_deref(), Some("旧支配者のキャロル"));
        assert_eq!(lyrics.lines[0].text, "歌ってみた ☃️");
    }

    #[test]
    fn test_enhanced_lrc_word_timing() {
        let lrc = "[00:12.00]<00:12.00>Hello <00:12.50>big <00:13.20>world\n[00:14.00]Next";
        let lyrics = LrcParser::parse(lrc).unwrap();

        let line = &lyrics.lines[0];
        assert_eq!(line.text, "Hello big world");
        let words = line.word_timestamps.as_ref().expect("word timing");
        let got: Vec<_> = words.iter().map(|w| (w.word.as_str(), w.start_ms, w.end_ms)).collect();
        // each word ends where the next starts; the last one is clamped to the next line
        assert_eq!(got, vec![("Hello", 12_000, 12_500), ("big", 12_500, 13_200), ("world", 13_200, 13_700)]);
        assert!(lyrics.lines[1].word_timestamps.is_none());
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
