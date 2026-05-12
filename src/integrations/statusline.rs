//! Status Line Integration
//!
//! Output formatted status for tmux, zsh, and other shell integrations.

use std::path::PathBuf;

/// Status line formatter
pub struct StatusLine;

impl StatusLine {
    /// Generate tmux status line format
    ///
    /// # Example
    /// ```
    /// let status = StatusLine::tmux_format(
    ///     Some(("Bohemian Rhapsody", "Queen", 180, 90)),
    ///     true,
    /// );
    /// // Returns: "🎵 Bohemian Rhapsody - Queen [01:30/03:00]"
    /// ```
    pub fn tmux_format(track: Option<(&str, &str, u64, u64)>, is_playing: bool) -> String {
        if let Some((title, artist, duration, position)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            let pos_str = format_duration(position);
            let dur_str = format_duration(duration);

            format!(
                "🎵 {} {} - {} [{}/{}]",
                icon,
                truncate(title, 20),
                truncate(artist, 15),
                pos_str,
                dur_str
            )
        } else {
            "🎵 Symphony".to_string()
        }
    }

    /// Generate zsh prompt format (compact)
    pub fn zsh_format(track: Option<(&str, &str)>, is_playing: bool) -> String {
        if let Some((title, artist)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            format!(
                "{} {} - {}",
                icon,
                truncate(title, 25),
                truncate(artist, 15)
            )
        } else {
            String::new()
        }
    }

    /// Generate minimal format (single icon)
    pub fn minimal_format(is_playing: bool, has_track: bool) -> String {
        if has_track {
            if is_playing {
                "🎵▶".to_string()
            } else {
                "🎵⏸".to_string()
            }
        } else {
            String::new()
        }
    }

    /// Generate JSON format (for scripts)
    pub fn json_format(
        track: Option<(&str, &str, u64, u64)>,
        is_playing: bool,
        volume: f32,
    ) -> String {
        let (title, artist, duration, position) = track
            .map(|(t, a, d, p)| (t.to_string(), a.to_string(), d, p))
            .unwrap_or_default();

        format!(
            r#"{{"title":"{}","artist":"{}","duration":{},"position":{},"playing":{},"volume":{}}}"#,
            escape_json(&title),
            escape_json(&artist),
            duration,
            position,
            is_playing,
            volume
        )
    }

    /// Generate i3blocks format
    pub fn i3blocks_format(track: Option<(&str, &str)>, is_playing: bool) -> String {
        if let Some((title, artist)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            format!("{} {} - {}", icon, truncate(title, 30), artist)
        } else {
            "".to_string()
        }
    }

    /// Generate waybar format (for Sway/Wayland)
    pub fn waybar_format(track: Option<(&str, &str, u64, u64)>, is_playing: bool) -> String {
        if let Some((title, artist, duration, position)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            let progress = if duration > 0 {
                (position as f64 / duration as f64 * 100.0) as u8
            } else {
                0
            };

            format!(
                r#"{{"text":"{} {} - {}","tooltip":"{}/{}","class":"{}","percentage":{}}}"#,
                icon,
                truncate(title, 20),
                truncate(artist, 15),
                format_duration(position),
                format_duration(duration),
                if is_playing { "playing" } else { "paused" },
                progress
            )
        } else {
            r#"{"text":"","class":"stopped"}"#.to_string()
        }
    }

    /// Write status to file (for tmux @plugin)
    pub fn write_status_file(path: &PathBuf, content: &str) -> std::io::Result<()> {
        std::fs::write(path, content)
    }

    /// Generate polybar format
    pub fn polybar_format(track: Option<(&str, &str, u64, u64)>, is_playing: bool) -> String {
        if let Some((title, artist, duration, position)) = track {
            let icon = if is_playing {
                "%{F#00ff00}▶%{F-}"
            } else {
                "⏸"
            };
            format!(
                "{} {} - {} %{{F#888}}{}%{{F-}}/%{{F#888}}{}%{{F-}}",
                icon,
                truncate(title, 25),
                truncate(artist, 15),
                format_duration(position),
                format_duration(duration)
            )
        } else {
            "".to_string()
        }
    }
}

/// Format duration as MM:SS
fn format_duration(secs: u64) -> String {
    let minutes = secs / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}", minutes, seconds)
}

/// Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut truncated = s[..max_len - 3].to_string();
        truncated.push_str("...");
        truncated
    }
}

/// Escape string for JSON
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Status line configuration
#[derive(Debug, Clone)]
pub struct StatusLineConfig {
    /// Enable tmux integration
    pub tmux_enabled: bool,
    /// Enable zsh integration
    pub zsh_enabled: bool,
    /// Status file path
    pub status_file: Option<PathBuf>,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
    /// Max title length
    pub max_title_len: usize,
    /// Max artist length
    pub max_artist_len: usize,
}

impl Default for StatusLineConfig {
    fn default() -> Self {
        Self {
            tmux_enabled: false,
            zsh_enabled: false,
            status_file: None,
            update_interval_ms: 1000,
            max_title_len: 25,
            max_artist_len: 15,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_truncate() {
        assert_eq!(truncate("Short", 10), "Short");
        assert_eq!(truncate("This is a very long string", 10), "This is...");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(59), "00:59");
        assert_eq!(format_duration(60), "01:00");
        assert_eq!(format_duration(125), "02:05");
        assert_eq!(format_duration(3661), "61:01");
    }
}
