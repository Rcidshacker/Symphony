//! Status Line Integration
//!
//! Output formatted status for tmux, zsh, and other shell integrations.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A snapshot older than this means the player is not running any more
const STALE_AFTER_SECS: u64 = 4;

/// What the running player publishes for `symphony status` (the player rewrites it every second)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StatusSnapshot {
    /// Track title (empty when nothing is loaded)
    pub title: String,
    /// Track artist
    pub artist: String,
    /// Track length in seconds
    pub duration: u64,
    /// Position in seconds
    pub position: u64,
    /// Whether audio is playing
    pub playing: bool,
    /// Volume 0.0-1.0
    pub volume: f32,
    /// Unix time the snapshot was written
    pub updated_unix: u64,
}

/// Output formats of `symphony status`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFormat {
    Tmux,
    Zsh,
    Minimal,
    Json,
    I3blocks,
    Waybar,
    Polybar,
}

impl StatusFormat {
    /// Parse a format name; `None` for unknown names
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_lowercase().as_str() {
            "tmux" => Self::Tmux,
            "zsh" => Self::Zsh,
            "minimal" => Self::Minimal,
            "json" => Self::Json,
            "i3blocks" => Self::I3blocks,
            "waybar" => Self::Waybar,
            "polybar" => Self::Polybar,
            _ => return None,
        })
    }

    /// Names accepted by [`StatusFormat::parse`]
    pub const NAMES: &'static str = "tmux, zsh, minimal, json, i3blocks, waybar, polybar";
}

impl StatusSnapshot {
    /// Write to `path` (to a temporary file first, so readers never see half a file)
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec(self)?)?;
        std::fs::rename(temp, path)
    }

    /// Read the snapshot at `path`; `None` when missing or unreadable
    pub fn read(path: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    /// Stamp with the current time
    pub fn stamped(mut self) -> Self {
        self.updated_unix = unix_now();
        self
    }

    /// Whether the player published this recently enough to be running
    pub fn is_live(&self, now_unix: u64) -> bool {
        now_unix.saturating_sub(self.updated_unix) <= STALE_AFTER_SECS
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Status line formatter
pub struct StatusLine;

impl StatusLine {
    /// Generate tmux status line format
    ///
    /// # Example
    /// ```
    /// use symphony::integrations::StatusLine;
    ///
    /// let status = StatusLine::tmux_format(
    ///     Some(("Bohemian Rhapsody", "Queen", 180, 90)),
    ///     true,
    /// );
    /// assert_eq!(status, "🎵 ▶ Bohemian Rhapsody - Queen [01:30/03:00]");
    /// ```
    pub fn tmux_format(
        track: Option<(&str, &str, u64, u64)>,
        is_playing: bool,
    ) -> String {
        if let Some((title, artist, duration, position)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            let pos_str = format_clock(position);
            let dur_str = format_clock(duration);

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
    pub fn zsh_format(
        track: Option<(&str, &str)>,
        is_playing: bool,
    ) -> String {
        if let Some((title, artist)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            format!("{} {} - {}", icon, truncate(title, 25), truncate(artist, 15))
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
    pub fn i3blocks_format(
        track: Option<(&str, &str)>,
        is_playing: bool,
    ) -> String {
        if let Some((title, artist)) = track {
            let icon = if is_playing { "▶" } else { "⏸" };
            format!("{} {} - {}", icon, truncate(title, 30), artist)
        } else {
            "".to_string()
        }
    }

    /// Generate waybar format (for Sway/Wayland)
    pub fn waybar_format(
        track: Option<(&str, &str, u64, u64)>,
        is_playing: bool,
    ) -> String {
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

    /// Format a published snapshot; a missing or stale one means nothing is playing
    pub fn render(snapshot: Option<&StatusSnapshot>, format: StatusFormat, now_unix: u64) -> String {
        let live = snapshot.filter(|s| s.is_live(now_unix) && !s.title.is_empty());
        let track = live.map(|s| (s.title.as_str(), s.artist.as_str(), s.duration, s.position));
        let playing = live.is_some_and(|s| s.playing);
        match format {
            StatusFormat::Tmux => Self::tmux_format(track, playing),
            StatusFormat::Zsh => Self::zsh_format(track.map(|(t, a, ..)| (t, a)), playing),
            StatusFormat::Minimal => Self::minimal_format(playing, track.is_some()),
            StatusFormat::Json => Self::json_format(track, playing, live.map_or(0.0, |s| s.volume)),
            StatusFormat::I3blocks => Self::i3blocks_format(track.map(|(t, a, ..)| (t, a)), playing),
            StatusFormat::Waybar => Self::waybar_format(track, playing),
            StatusFormat::Polybar => Self::polybar_format(track, playing),
        }
    }

    /// Generate polybar format
    pub fn polybar_format(
        track: Option<(&str, &str, u64, u64)>,
        is_playing: bool,
    ) -> String {
        if let Some((title, artist, duration, position)) = track {
            let icon = if is_playing { "%{F#00ff00}▶%{F-}" } else { "⏸" };
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
/// Zero-padded `mm:ss`, used by the tmux format
fn format_clock(secs: u64) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn format_duration(secs: u64) -> String {
    let minutes = secs / 60;
    let seconds = secs % 60;
    format!("{}:{:02}", minutes, seconds)
}

/// Truncate string to max length
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let mut truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_format() {
        let status = StatusLine::tmux_format(
            Some(("Test Song", "Test Artist", 180, 90)),
            true,
        );
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
        let json = StatusLine::json_format(
            Some(("Test", "Artist", 100, 50)),
            true,
            0.75,
        );

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
    fn truncate_counts_characters_not_bytes() {
        // byte slicing used to panic in the middle of a multi-byte character
        assert_eq!(truncate("あいうえおかきくけこさしすせそ", 8), "あいうえお...");
        assert_eq!(truncate("Café Tacvba - Eres", 10), "Café Ta...");
        assert_eq!(truncate("abcdef", 2), "...");
    }

    fn snapshot(updated_unix: u64) -> StatusSnapshot {
        StatusSnapshot {
            title: "Song".into(),
            artist: "Band".into(),
            duration: 180,
            position: 90,
            playing: true,
            volume: 0.5,
            updated_unix,
        }
    }

    #[test]
    fn snapshots_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/status.json");
        assert_eq!(StatusSnapshot::read(&path), None);

        snapshot(1000).write(&path).unwrap();
        assert_eq!(StatusSnapshot::read(&path), Some(snapshot(1000)));
        assert!(!path.with_extension("tmp").exists(), "temporary file is gone");

        std::fs::write(&path, "not json").unwrap();
        assert_eq!(StatusSnapshot::read(&path), None);
    }

    #[test]
    fn render_shows_live_snapshots_and_ignores_stale_or_missing_ones() {
        let live = snapshot(1000);
        assert_eq!(
            StatusLine::render(Some(&live), StatusFormat::Tmux, 1002),
            "🎵 ▶ Song - Band [01:30/03:00]"
        );
        assert_eq!(StatusLine::render(Some(&live), StatusFormat::Zsh, 1002), "▶ Song - Band");
        assert!(StatusLine::render(Some(&live), StatusFormat::Json, 1002).contains("\"playing\":true"));
        assert!(StatusLine::render(Some(&live), StatusFormat::Waybar, 1002).contains("\"class\":\"playing\""));

        // the player stopped writing: not running
        assert_eq!(StatusLine::render(Some(&live), StatusFormat::Tmux, 1100), "🎵 Symphony");
        assert_eq!(StatusLine::render(Some(&live), StatusFormat::Zsh, 1100), "");
        assert_eq!(StatusLine::render(None, StatusFormat::Tmux, 1100), "🎵 Symphony");

        let mut paused = snapshot(1000);
        paused.playing = false;
        assert_eq!(StatusLine::render(Some(&paused), StatusFormat::Minimal, 1001), "🎵⏸");
        let mut idle = snapshot(1000);
        idle.title.clear();
        assert_eq!(StatusLine::render(Some(&idle), StatusFormat::Minimal, 1001), "");
    }

    #[test]
    fn format_names_parse() {
        assert_eq!(StatusFormat::parse("TMUX"), Some(StatusFormat::Tmux));
        assert_eq!(StatusFormat::parse("waybar"), Some(StatusFormat::Waybar));
        assert_eq!(StatusFormat::parse("fish"), None);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(59), "0:59");
        assert_eq!(format_duration(60), "1:00");
        assert_eq!(format_duration(125), "2:05");
        assert_eq!(format_duration(3661), "61:01");
    }
}
