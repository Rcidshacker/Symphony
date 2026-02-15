//! Integration tests for Symphony
//!
//! These tests verify the core functionality works end-to-end.

#[cfg(test)]
mod tests {
    // Note: These tests require audio hardware/output device to work
    // In CI, we mock or skip audio-related tests

    #[test]
    fn test_database_operations() {
        use std::path::PathBuf;
        use symphony::app::{Track, TrackSource};
        use symphony::db::Database;

        let db = Database::in_memory().expect("Failed to create in-memory database");

        // Insert a track
        let track = Track {
            id: "test-track-1".to_string(),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            duration: 180.0,
            path: PathBuf::from("/test/song.mp3"),
            track_number: Some(1),
            genre: Some("Rock".to_string()),
            year: Some(2024),
            source: TrackSource::Local,
        };

        db.upsert_track(&track).expect("Failed to insert track");

        // Retrieve the track
        let retrieved = db.get_track("test-track-1").expect("Failed to get track");

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Song");
        assert_eq!(retrieved.artist, "Test Artist");
    }

    #[test]
    fn test_config_loading() {
        use symphony::config::Config;

        // Default config should work
        let config = Config::default();
        assert_eq!(config.audio.volume, 0.6);
        assert_eq!(config.ai.provider, "ollama");
    }

    #[test]
    fn test_scanner_file_parsing() {
        use symphony::scanner::LibraryScanner;

        let scanner = LibraryScanner::new();

        // Test file name parsing
        let (title, artist) = scanner.parse_file_name("Queen - Bohemian Rhapsody");
        assert_eq!(artist, "Queen");
        assert_eq!(title, "Bohemian Rhapsody");
    }
}
