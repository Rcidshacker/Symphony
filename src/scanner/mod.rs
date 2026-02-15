//! Library scanner module
//!
//! Scans directories for music files and extracts metadata.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use uuid::Uuid;
use walkdir::WalkDir;

use crate::app::{Track, TrackSource};
use crate::audio::AudioEngine;

/// Scanner errors
#[derive(Debug, thiserror::Error)]
pub enum ScannerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to scan directory: {0}")]
    ScanError(String),
}

/// Scanner progress callback
pub type ProgressCallback = Box<dyn Fn(ScanProgress) + Send + Sync>;

/// Scan progress information
#[derive(Debug, Clone)]
pub struct ScanProgress {
    /// Files scanned so far
    pub files_scanned: usize,

    /// Total files found
    pub total_files: usize,

    /// Current file being processed
    pub current_file: Option<String>,

    /// Tracks added
    pub tracks_added: usize,

    /// Elapsed time
    pub elapsed: std::time::Duration,
}

/// Library scanner
pub struct LibraryScanner {
    /// Supported file extensions
    supported_extensions: HashSet<String>,

    /// Progress callback
    progress_callback: Option<ProgressCallback>,
}

impl Default for LibraryScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryScanner {
    /// Create a new scanner
    pub fn new() -> Self {
        let supported_extensions = AudioEngine::supported_extensions()
            .iter()
            .map(|s| s.to_lowercase())
            .collect();

        Self {
            supported_extensions,
            progress_callback: None,
        }
    }

    /// Set progress callback
    pub fn set_progress_callback<F>(&mut self, callback: F)
    where
        F: Fn(ScanProgress) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
    }

    /// Scan a directory for music files
    pub fn scan_directory(&self, directory: &Path) -> Result<Vec<Track>, ScannerError> {
        let start_time = Instant::now();
        let mut tracks = Vec::new();
        let mut files_scanned = 0;

        // First pass: count total files
        let total_files = WalkDir::new(directory)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| self.is_music_file(e.path()))
            .count();

        // Second pass: process files
        for entry in WalkDir::new(directory)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            if !self.is_music_file(path) {
                continue;
            }

            files_scanned += 1;

            // Report progress
            if let Some(ref callback) = self.progress_callback {
                callback(ScanProgress {
                    files_scanned,
                    total_files,
                    current_file: Some(path.display().to_string()),
                    tracks_added: tracks.len(),
                    elapsed: start_time.elapsed(),
                });
            }

            // Extract metadata and create track
            if let Some(track) = self.create_track_from_file(path) {
                tracks.push(track);
            }
        }

        Ok(tracks)
    }

    /// Check if a file is a supported music file
    fn is_music_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| self.supported_extensions.contains(&ext.to_lowercase()))
            .unwrap_or(false)
    }

    /// Create a track from a file path
    fn create_track_from_file(&self, path: &Path) -> Option<Track> {
        // Get basic file info
        let file_name = path.file_stem()?.to_string_lossy().to_string();

        // Try to parse metadata (simplified - use file name as fallback)
        let (title, artist) = self.parse_file_name(&file_name);

        // Get duration (would require actual decoding - skip for now)
        // In production, use symphonia or rodio to get actual duration
        let duration = 0.0;

        Some(Track {
            id: Uuid::new_v4().to_string(),
            title,
            artist,
            album: "Unknown".to_string(),
            duration,
            path: path.to_path_buf(),
            track_number: None,
            genre: None,
            year: None,
            source: TrackSource::Local,
        })
    }

    /// Parse file name to extract artist and title
    /// Formats: "Artist - Title", "Artist-Title", "Title" (no artist)
    fn parse_file_name(&self, file_name: &str) -> (String, String) {
        // Common patterns
        let separators = [" - ", "-", " – ", " – ", " by "];

        for sep in separators {
            if let Some(pos) = file_name.find(sep) {
                let artist = file_name[..pos].trim().to_string();
                let title = file_name[pos + sep.len()..].trim().to_string();
                if !artist.is_empty() && !title.is_empty() {
                    return (title, artist);
                }
            }
        }

        // Fallback: use file name as title
        ("Unknown Artist".to_string(), file_name.to_string())
    }

    /// Get list of supported extensions
    pub fn supported_extensions(&self) -> &HashSet<String> {
        &self.supported_extensions
    }
}

/// Metadata extractor (uses Symphonia for detailed metadata)
pub struct MetadataExtractor;

impl MetadataExtractor {
    /// Extract metadata from an audio file
    pub fn extract(path: &Path) -> Option<TrackMetadata> {
        // This would use Symphonia to read actual metadata
        // For now, return None to indicate metadata extraction is not yet implemented
        let _ = path;
        None
    }
}

/// Track metadata from audio file
#[derive(Debug, Clone)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub duration: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_name() {
        let scanner = LibraryScanner::new();

        // Standard format
        let (title, artist) = scanner.parse_file_name("Queen - Bohemian Rhapsody");
        assert_eq!(artist, "Queen");
        assert_eq!(title, "Bohemian Rhapsody");

        // Single hyphen
        let (title, artist) = scanner.parse_file_name("Artist-Song");
        assert_eq!(artist, "Artist");
        assert_eq!(title, "Song");

        // No separator
        let (title, artist) = scanner.parse_file_name("Just A Song Name");
        assert_eq!(artist, "Unknown Artist");
        assert_eq!(title, "Just A Song Name");
    }

    #[test]
    fn test_is_music_file() {
        let scanner = LibraryScanner::new();

        assert!(scanner.is_music_file(Path::new("test.mp3")));
        assert!(scanner.is_music_file(Path::new("test.FLAC")));
        assert!(scanner.is_music_file(Path::new("test.Ogg")));
        assert!(!scanner.is_music_file(Path::new("test.txt")));
        assert!(!scanner.is_music_file(Path::new("test.pdf")));
    }
}
