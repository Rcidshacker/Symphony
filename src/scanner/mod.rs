//! Library scanner module
//!
//! Scans directories for music files and extracts metadata.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;
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

        // Tags and duration from the file; the file name is the fallback
        let (fallback_title, fallback_artist) = self.parse_file_name(&file_name);
        let meta = MetadataExtractor::extract(path);
        let meta = meta.as_ref();
        let tag = |pick: fn(&TrackMetadata) -> &Option<String>| {
            meta.and_then(|m| pick(m).clone())
                .filter(|s| !s.trim().is_empty())
        };

        Some(Track {
            id: Uuid::new_v4().to_string(),
            title: tag(|m| &m.title).unwrap_or(fallback_title),
            artist: tag(|m| &m.artist).unwrap_or(fallback_artist),
            album: tag(|m| &m.album).unwrap_or_else(|| "Unknown".to_string()),
            duration: meta.and_then(|m| m.duration).unwrap_or(0.0),
            path: path.to_path_buf(),
            track_number: meta.and_then(|m| m.track_number),
            genre: tag(|m| &m.genre),
            year: meta.and_then(|m| m.year),
            source: TrackSource::Local,
        })
    }

    /// Parse file name to extract artist and title
    /// Formats: "Artist - Title", "Artist-Title", "Title" (no artist)
    pub fn parse_file_name(&self, file_name: &str) -> (String, String) {
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
        (file_name.to_string(), "Unknown Artist".to_string())
    }

    /// Get list of supported extensions
    pub fn supported_extensions(&self) -> &HashSet<String> {
        &self.supported_extensions
    }
}

/// Metadata extractor (uses Symphonia for detailed metadata)
pub struct MetadataExtractor;

impl MetadataExtractor {
    /// Extract tags and duration from an audio file with Symphonia.
    /// Returns `None` if the file cannot be opened or probed.
    pub fn extract(path: &Path) -> Option<TrackMetadata> {
        let file = std::fs::File::open(path).ok()?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let mut probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .ok()?;

        let mut meta = TrackMetadata {
            title: None,
            artist: None,
            album: None,
            track_number: None,
            genre: None,
            year: None,
            duration: None,
        };

        // Tags found before the container (e.g. ID3v2 at the start of an MP3)
        if let Some(mut container_meta) = probed.metadata.get() {
            if let Some(rev) = container_meta.skip_to_latest() {
                meta.apply_tags(rev);
            }
        }
        // Tags stored inside the container (Vorbis comments, ID3v1, ...)
        if let Some(rev) = probed.format.metadata().skip_to_latest() {
            meta.apply_tags(rev);
        }

        meta.duration = Self::duration_secs(probed.format.as_mut());
        Some(meta)
    }

    /// Duration in seconds: from the header when present, else by summing packet lengths
    /// (no decoding; e.g. MP3 without a Xing/VBRI header).
    fn duration_secs(format: &mut dyn FormatReader) -> Option<f64> {
        let (track_id, time_base, n_frames) = {
            let track = format.default_track()?;
            (
                track.id,
                track.codec_params.time_base?,
                track.codec_params.n_frames,
            )
        };

        let frames = match n_frames {
            Some(n) => n,
            None => {
                let mut total = 0u64;
                while let Ok(packet) = format.next_packet() {
                    if packet.track_id() == track_id {
                        total += packet.dur;
                    }
                }
                total
            }
        };

        let time = time_base.calc_time(frames);
        let secs = time.seconds as f64 + time.frac;
        (secs > 0.0).then_some(secs)
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

impl TrackMetadata {
    /// Fill unset fields from a Symphonia tag revision.
    fn apply_tags(&mut self, rev: &MetadataRevision) {
        for tag in rev.tags() {
            let value = tag.value.to_string();
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => self.title.get_or_insert(value),
                Some(StandardTagKey::Artist) => self.artist.get_or_insert(value),
                Some(StandardTagKey::Album) => self.album.get_or_insert(value),
                Some(StandardTagKey::Genre) => self.genre.get_or_insert(value),
                Some(StandardTagKey::TrackNumber) => {
                    // "3" or "3/12"
                    if self.track_number.is_none() {
                        self.track_number =
                            value.split('/').next().and_then(|n| n.trim().parse().ok());
                    }
                    continue;
                }
                Some(StandardTagKey::Date | StandardTagKey::OriginalDate) => {
                    // "2004" or "2004-03-01"
                    if self.year.is_none() {
                        self.year = value.get(..4).and_then(|y| y.parse().ok());
                    }
                    continue;
                }
                _ => continue,
            };
        }
    }
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
        assert_eq!(title, "Just A Song Name");
        assert_eq!(artist, "Unknown Artist");
    }

    /// Write a silent 8 kHz mono 16-bit WAV of `samples` samples.
    fn write_silent_wav(path: &Path, samples: u32) {
        let data_len = samples * 2;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&8000u32.to_le_bytes()); // sample rate
        wav.extend_from_slice(&16000u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend(std::iter::repeat(0u8).take(data_len as usize));
        std::fs::write(path, wav).unwrap();
    }

    #[test]
    fn test_extract_reads_wav_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Some Artist - Some Song.wav");
        write_silent_wav(&path, 12_000); // 1.5 s at 8 kHz

        let meta = MetadataExtractor::extract(&path).expect("wav should probe");
        let duration = meta.duration.expect("wav has a duration");
        assert!((duration - 1.5).abs() < 0.01, "duration was {duration}");
    }

    #[test]
    fn test_scan_sets_duration_and_falls_back_to_file_name() {
        let dir = tempfile::tempdir().unwrap();
        write_silent_wav(&dir.path().join("Some Artist - Some Song.wav"), 16_000);

        let tracks = LibraryScanner::new().scan_directory(dir.path()).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].artist, "Some Artist");
        assert_eq!(tracks[0].title, "Some Song");
        assert!((tracks[0].duration - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_extract_returns_none_for_non_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.mp3");
        std::fs::write(&path, b"this is not audio").unwrap();
        assert!(MetadataExtractor::extract(&path).is_none());
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
