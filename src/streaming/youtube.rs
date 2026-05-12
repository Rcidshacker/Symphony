//! YouTube streaming provider
//!
//! Uses yt-dlp to search, stream, and download audio from YouTube.
//!
//! # Requirements
//! yt-dlp must be installed on the system:
//! ```bash
//! pip install yt-dlp
//! # or
//! brew install yt-dlp
//! ```

use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use super::provider::{
    DownloadProgress, DownloadStatus, SearchResult, StreamError, StreamProvider, StreamQuality,
    StreamSource, StreamTrack,
};

/// YouTube provider configuration
#[derive(Debug, Clone)]
pub struct YouTubeConfig {
    /// Path to yt-dlp executable (default: "yt-dlp")
    pub ytdlp_path: String,
    /// Default quality for streams
    pub default_quality: StreamQuality,
    /// Include thumbnails in results
    pub include_thumbnails: bool,
    /// Max search results
    pub max_results: usize,
    /// Use cookies for age-restricted content
    pub cookies_file: Option<PathBuf>,
    /// Proxy URL (optional)
    pub proxy: Option<String>,
    /// Rate limit in KB/s (0 = unlimited)
    pub rate_limit: u32,
}

impl Default for YouTubeConfig {
    fn default() -> Self {
        Self {
            ytdlp_path: "yt-dlp".to_string(),
            default_quality: StreamQuality::Medium,
            include_thumbnails: true,
            max_results: 20,
            cookies_file: None,
            proxy: None,
            rate_limit: 0,
        }
    }
}

/// YouTube provider implementation
pub struct YouTubeProvider {
    config: YouTubeConfig,
    available: bool,
}

impl YouTubeProvider {
    /// Create a new YouTube provider
    pub fn new(config: YouTubeConfig) -> Self {
        Self {
            config,
            available: false,
        }
    }

    /// Create a YouTube provider with default configuration
    pub fn with_defaults() -> Self {
        Self::new(YouTubeConfig::default())
    }

    /// Initialize the provider (check if yt-dlp is available)
    pub async fn initialize(&mut self) -> Result<(), StreamError> {
        // Check if yt-dlp is installed
        let output = Command::new(&self.config.ytdlp_path)
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    info!("yt-dlp version: {}", version.trim());
                    self.available = true;
                    Ok(())
                } else {
                    error!("yt-dlp check failed: {}", String::from_utf8_lossy(&output.stderr));
                    Err(StreamError::Unavailable(
                        "yt-dlp is installed but returned an error".to_string(),
                    ))
                }
            }
            Err(e) => {
                error!("yt-dlp not found: {}", e);
                Err(StreamError::Unavailable(
                    "yt-dlp is not installed. Install with: pip install yt-dlp".to_string(),
                ))
            }
        }
    }

    /// Build common yt-dlp arguments
    fn build_common_args(&self) -> Vec<String> {
        let mut args = vec![
            "--no-warnings".to_string(),
            "--no-playlist".to_string(),
            "--flat-playlist".to_string(),
            "-j".to_string(), // JSON output
        ];

        if let Some(ref proxy) = self.config.proxy {
            args.push("--proxy".to_string());
            args.push(proxy.clone());
        }

        if let Some(ref cookies) = self.config.cookies_file {
            args.push("--cookies".to_string());
            args.push(cookies.to_string_lossy().to_string());
        }

        if self.config.rate_limit > 0 {
            args.push("--limit-rate".to_string());
            args.push(format!("{}K", self.config.rate_limit));
        }

        args
    }

    /// Parse yt-dlp JSON output into StreamTrack
    fn parse_track(&self, json: &str) -> Result<StreamTrack, StreamError> {
        let value: serde_json::Value = serde_json::from_str(json)?;

        let id = value["id"]
            .as_str()
            .ok_or_else(|| StreamError::SearchFailed("Missing video ID".to_string()))?
            .to_string();

        let title = value["title"]
            .as_str()
            .unwrap_or("Unknown Title")
            .to_string();

        // Try to extract artist from title or channel
        let artist = value["channel"]
            .as_str()
            .or_else(|| value["uploader"].as_str())
            .unwrap_or("Unknown Artist")
            .to_string();

        let duration = value["duration"]
            .as_i64()
            .or_else(|| value["duration_string"].as_str().and_then(|s| parse_duration(s)))
            .unwrap_or(0) as u64;

        let thumbnail_url = if self.config.include_thumbnails {
            value["thumbnail"]
                .as_str()
                .or_else(|| {
                    value["thumbnails"]
                        .as_array()
                        .and_then(|arr| arr.last())
                        .and_then(|t| t["url"].as_str())
                })
                .map(|s| s.to_string())
        } else {
            None
        };

        let view_count = value["view_count"].as_i64().map(|v| v as u64);

        let upload_date = value["upload_date"].as_str().map(|s| s.to_string());

        let mut track = StreamTrack::new(id, title, artist, StreamSource::YouTube)
            .with_duration(duration);

        track.thumbnail_url = thumbnail_url;
        track.view_count = view_count;
        track.upload_date = upload_date;

        Ok(track)
    }

    /// Search YouTube for videos
    async fn search_youtube(&self, query: &str, limit: usize) -> Result<Vec<StreamTrack>, StreamError> {
        let search_query = format!("ytsearch{}:{}", limit, query);

        let mut args = self.build_common_args();
        args.push(search_query);

        let output = Command::new(&self.config.ytdlp_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp search failed: {}", stderr);
            return Err(StreamError::SearchFailed(stderr.to_string()));
        }

        // Parse each line as JSON
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut tracks = Vec::new();

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            match self.parse_track(line) {
                Ok(track) => tracks.push(track),
                Err(e) => {
                    warn!("Failed to parse track: {}", e);
                    debug!("Problematic JSON: {}", line);
                }
            }
        }

        Ok(tracks)
    }

    /// Get video info by ID or URL
    async fn get_video_info(&self, video_id: &str) -> Result<StreamTrack, StreamError> {
        let url = format!("https://www.youtube.com/watch?v={}", video_id);

        let mut args = self.build_common_args();
        args.push(url);

        let output = Command::new(&self.config.ytdlp_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // Check for specific errors
            if stderr.contains("Video unavailable") || stderr.contains("This video is unavailable") {
                return Err(StreamError::TrackNotFound(video_id.to_string()));
            }
            if stderr.contains("Sign in") || stderr.contains("age") {
                return Err(StreamError::AgeRestricted);
            }
            
            return Err(StreamError::SearchFailed(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_track(&stdout)
    }

    /// Get the best audio stream URL
    async fn extract_audio_url(&self, video_id: &str, quality: StreamQuality) -> Result<String, StreamError> {
        let url = format!("https://www.youtube.com/watch?v={}", video_id);
        let format = quality.yt_dlp_format();

        let args = vec![
            "--no-warnings",
            "--no-playlist",
            "-f", format,
            "--get-url",
            &url,
        ];

        let output = Command::new(&self.config.ytdlp_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StreamError::DownloadFailed(format!(
                "Failed to get stream URL: {}",
                stderr
            )));
        }

        let stream_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        
        if stream_url.is_empty() {
            return Err(StreamError::DownloadFailed("No stream URL returned".to_string()));
        }

        Ok(stream_url)
    }
}

#[async_trait]
impl StreamProvider for YouTubeProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<SearchResult, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "YouTube provider not initialized. Call initialize() first.".to_string(),
            ));
        }

        info!("Searching YouTube for: {}", query);
        let tracks = self.search_youtube(query, limit).await?;
        
        Ok(SearchResult::new(tracks, query, StreamSource::YouTube))
    }

    async fn get_stream_url(&self, track_id: &str, quality: StreamQuality) -> Result<String, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "YouTube provider not initialized.".to_string(),
            ));
        }

        debug!("Getting stream URL for: {}", track_id);
        self.extract_audio_url(track_id, quality).await
    }

    async fn get_track_info(&self, track_id: &str) -> Result<StreamTrack, StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "YouTube provider not initialized.".to_string(),
            ));
        }

        debug!("Getting track info for: {}", track_id);
        self.get_video_info(track_id).await
    }

    async fn download_track(
        &self,
        track_id: &str,
        quality: StreamQuality,
        output_path: &PathBuf,
    ) -> Result<(), StreamError> {
        if !self.available {
            return Err(StreamError::Unavailable(
                "YouTube provider not initialized.".to_string(),
            ));
        }

        let url = format!("https://www.youtube.com/watch?v={}", track_id);
        let format = quality.yt_dlp_format();

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        info!("Downloading {} to {:?}", track_id, output_path);

        let output_path_str = output_path.to_string_lossy();
        let args = vec![
            "--no-warnings",
            "--no-playlist",
            "-f",
            format,
            "-x", // Extract audio
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "-o",
            &output_path_str,
            &url,
        ];

        let output = Command::new(&self.config.ytdlp_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Download failed: {}", stderr);
            return Err(StreamError::DownloadFailed(stderr.to_string()));
        }

        info!("Successfully downloaded {}", track_id);
        Ok(())
    }

    async fn is_available(&self) -> bool {
        self.available
    }

    fn name(&self) -> &str {
        "YouTube"
    }

    fn source(&self) -> StreamSource {
        StreamSource::YouTube
    }
}

/// Parse a duration string (e.g., "3:45", "1:23:45")
fn parse_duration(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split(':').collect();
    
    match parts.len() {
        2 => {
            // MM:SS
            let mins: i64 = parts[0].parse().ok()?;
            let secs: i64 = parts[1].parse().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            // HH:MM:SS
            let hours: i64 = parts[0].parse().ok()?;
            let mins: i64 = parts[1].parse().ok()?;
            let secs: i64 = parts[2].parse().ok()?;
            Some(hours * 3600 + mins * 60 + secs)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("3:45"), Some(225));
        assert_eq!(parse_duration("1:23:45"), Some(5025));
        assert_eq!(parse_duration("0:30"), Some(30));
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_youtube_config_default() {
        let config = YouTubeConfig::default();
        assert_eq!(config.ytdlp_path, "yt-dlp");
        assert_eq!(config.default_quality, StreamQuality::Medium);
        assert!(config.include_thumbnails);
    }

    #[tokio::test]
    async fn test_youtube_provider_creation() {
        let provider = YouTubeProvider::with_defaults();
        assert_eq!(provider.name(), "YouTube");
        assert_eq!(provider.source(), StreamSource::YouTube);
    }
}
