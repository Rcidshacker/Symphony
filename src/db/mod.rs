//! Database module
//!
//! SQLite database for music library storage.

use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::app::{Track, TrackSource};

/// Database errors
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Track not found: {0}")]
    NotFound(String),
}

/// Database wrapper
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Create or open database at path
    pub fn new(path: &str) -> Result<Self, DatabaseError> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Create in-memory database (for testing)
    pub fn in_memory() -> Result<Self, DatabaseError> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.run_migrations()?;
        Ok(db)
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<(), DatabaseError> {
        self.conn.execute_batch(
            r#"
            -- Tracks table
            CREATE TABLE IF NOT EXISTS tracks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                artist TEXT NOT NULL,
                album TEXT DEFAULT '',
                path TEXT UNIQUE NOT NULL,
                duration REAL DEFAULT 0,
                track_number INTEGER,
                genre TEXT,
                year INTEGER,
                source TEXT DEFAULT 'local',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            -- Playlists table
            CREATE TABLE IF NOT EXISTS playlists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            -- Playlist tracks junction table
            CREATE TABLE IF NOT EXISTS playlist_tracks (
                playlist_id TEXT NOT NULL,
                track_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (playlist_id, track_id, position),
                FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
                FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            -- Play history
            CREATE TABLE IF NOT EXISTS play_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id TEXT NOT NULL,
                played_at TEXT DEFAULT CURRENT_TIMESTAMP,
                duration_played REAL DEFAULT 0,
                FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            -- Stream metadata (Phase 4)
            -- Stores streaming-specific data for non-local tracks
            CREATE TABLE IF NOT EXISTS stream_metadata (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id TEXT UNIQUE NOT NULL,
                stream_id TEXT NOT NULL,
                stream_url TEXT,
                thumbnail_url TEXT,
                cached_path TEXT,
                last_fetched TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
            );

            -- Create indexes
            CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
            CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
            CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
            CREATE INDEX IF NOT EXISTS idx_tracks_source ON tracks(source);
            CREATE INDEX IF NOT EXISTS idx_play_history_track ON play_history(track_id);
            CREATE INDEX IF NOT EXISTS idx_play_history_played_at ON play_history(played_at);
            CREATE INDEX IF NOT EXISTS idx_stream_metadata_stream_id ON stream_metadata(stream_id);
            "#,
        )?;

        // Run migration to add new columns if they don't exist
        self.run_streaming_migration()?;

        Ok(())
    }

    /// Run streaming-specific migrations
    fn run_streaming_migration(&self) -> Result<(), DatabaseError> {
        // Add stream_id column to tracks if it doesn't exist
        let result = self.conn.execute(
            "ALTER TABLE tracks ADD COLUMN stream_id TEXT",
            [],
        );
        
        // Ignore error if column already exists
        if let Err(e) = result {
            if !e.to_string().contains("duplicate column name") {
                return Err(DatabaseError::Migration(e.to_string()));
            }
        }

        Ok(())
    }

    /// Insert or update a track
    pub fn upsert_track(&self, track: &Track) -> Result<(), DatabaseError> {
        self.conn.execute(
            r#"
            INSERT INTO tracks (id, title, artist, album, path, duration, track_number, genre, year, source)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                duration = excluded.duration,
                track_number = excluded.track_number,
                genre = excluded.genre,
                year = excluded.year,
                source = excluded.source,
                updated_at = CURRENT_TIMESTAMP
            "#,
            (
                &track.id,
                &track.title,
                &track.artist,
                &track.album,
                track.path.to_string_lossy().to_string(),
                track.duration,
                track.track_number,
                &track.genre,
                track.year,
                track_source_to_string(track.source),
            ),
        )?;
        Ok(())
    }

    /// Get track by ID
    pub fn get_track(&self, id: &str) -> Result<Option<Track>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, artist, album, path, duration, track_number, genre, year, source
            FROM tracks WHERE id = ?1
            "#,
        )?;

        let track = stmt
            .query_row([id], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    duration: row.get(5)?,
                    track_number: row.get(6)?,
                    genre: row.get(7)?,
                    year: row.get(8)?,
                    source: parse_track_source(&row.get::<_, String>(9)?),
                })
            })
            .optional()?;

        Ok(track)
    }

    /// Get all tracks
    pub fn get_all_tracks(&self) -> Result<Vec<Track>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, artist, album, path, duration, track_number, genre, year, source
            FROM tracks ORDER BY artist, album, track_number, title
            "#,
        )?;

        let tracks = stmt
            .query_map([], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    duration: row.get(5)?,
                    track_number: row.get(6)?,
                    genre: row.get(7)?,
                    year: row.get(8)?,
                    source: parse_track_source(&row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    /// Search tracks
    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>, DatabaseError> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, artist, album, path, duration, track_number, genre, year, source
            FROM tracks
            WHERE title LIKE ?1 OR artist LIKE ?1 OR album LIKE ?1
            ORDER BY artist, album, track_number, title
            "#,
        )?;

        let tracks = stmt
            .query_map([&pattern], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    duration: row.get(5)?,
                    track_number: row.get(6)?,
                    genre: row.get(7)?,
                    year: row.get(8)?,
                    source: parse_track_source(&row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    /// Delete a track
    pub fn delete_track(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn.execute("DELETE FROM tracks WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Get track count
    pub fn track_count(&self) -> Result<usize, DatabaseError> {
        let count: usize = self.conn.query_row("SELECT COUNT(*) FROM tracks", [], |row| {
            row.get(0)
        })?;
        Ok(count)
    }

    /// Record a play event
    pub fn record_play(&self, track_id: &str, duration_played: f64) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO play_history (track_id, duration_played) VALUES (?1, ?2)",
            (track_id, duration_played),
        )?;
        Ok(())
    }

    /// Get play count for a track
    pub fn get_play_count(&self, track_id: &str) -> Result<usize, DatabaseError> {
        let count: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM play_history WHERE track_id = ?1",
            [track_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get most played tracks
    pub fn get_most_played(&self, limit: usize) -> Result<Vec<(Track, usize)>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.title, t.artist, t.album, t.path, t.duration, t.track_number, t.genre, t.year, t.source, COUNT(ph.id) as play_count
            FROM tracks t
            JOIN play_history ph ON t.id = ph.track_id
            GROUP BY t.id
            ORDER BY play_count DESC
            LIMIT ?1
            "#,
        )?;

        let results = stmt
            .query_map([limit as i64], |row| {
                let track = Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    duration: row.get(5)?,
                    track_number: row.get(6)?,
                    genre: row.get(7)?,
                    year: row.get(8)?,
                    source: parse_track_source(&row.get::<_, String>(9)?),
                };
                let play_count: usize = row.get(10)?;
                Ok((track, play_count))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Create a playlist
    pub fn create_playlist(&self, name: &str) -> Result<String, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO playlists (id, name) VALUES (?1, ?2)",
            (&id, name),
        )?;
        Ok(id)
    }

    /// Add track to playlist
    pub fn add_to_playlist(&self, playlist_id: &str, track_id: &str, position: i32) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
            (playlist_id, track_id, position),
        )?;
        Ok(())
    }

    /// Get playlist tracks
    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.id, t.title, t.artist, t.album, t.path, t.duration, t.track_number, t.genre, t.year, t.source
            FROM tracks t
            JOIN playlist_tracks pt ON t.id = pt.track_id
            WHERE pt.playlist_id = ?1
            ORDER BY pt.position
            "#,
        )?;

        let tracks = stmt
            .query_map([playlist_id], |row| {
                Ok(Track {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    artist: row.get(2)?,
                    album: row.get(3)?,
                    path: row.get::<_, String>(4)?.into(),
                    duration: row.get(5)?,
                    track_number: row.get(6)?,
                    genre: row.get(7)?,
                    year: row.get(8)?,
                    source: parse_track_source(&row.get::<_, String>(9)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(tracks)
    }

    /// Clear all tracks (for rescanning)
    pub fn clear_all_tracks(&self) -> Result<(), DatabaseError> {
        self.conn.execute("DELETE FROM tracks", [])?;
        Ok(())
    }
}

/// Convert TrackSource to string for database storage
fn track_source_to_string(source: TrackSource) -> String {
    match source {
        TrackSource::Local => "local".to_string(),
        TrackSource::YouTube => "youtube".to_string(),
        TrackSource::Spotify => "spotify".to_string(),
        TrackSource::SoundCloud => "soundcloud".to_string(),
        TrackSource::Cached => "cached".to_string(),
    }
}

/// Parse TrackSource from string
fn parse_track_source(s: &str) -> TrackSource {
    match s {
        "youtube" => TrackSource::YouTube,
        "spotify" => TrackSource::Spotify,
        "soundcloud" => TrackSource::SoundCloud,
        "cached" => TrackSource::Cached,
        _ => TrackSource::Local,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() {
        let db = Database::in_memory().unwrap();
        assert_eq!(db.track_count().unwrap(), 0);
    }

    #[test]
    fn test_insert_and_get_track() {
        let db = Database::in_memory().unwrap();

        let track = Track {
            id: "test-1".to_string(),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            path: "/music/test.mp3".into(),
            duration: 180.0,
            track_number: Some(1),
            genre: Some("Rock".to_string()),
            year: Some(2024),
            source: TrackSource::Local,
        };

        db.upsert_track(&track).unwrap();
        let retrieved = db.get_track("test-1").unwrap();

        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Song");
        assert_eq!(retrieved.artist, "Test Artist");
    }

    #[test]
    fn test_search_tracks() {
        let db = Database::in_memory().unwrap();

        let track = Track {
            id: "test-1".to_string(),
            title: "Bohemian Rhapsody".to_string(),
            artist: "Queen".to_string(),
            album: "A Night at the Opera".to_string(),
            path: "/music/queen.mp3".into(),
            duration: 354.0,
            track_number: Some(11),
            genre: Some("Rock".to_string()),
            year: Some(1975),
            source: TrackSource::Local,
        };

        db.upsert_track(&track).unwrap();

        let results = db.search_tracks("Queen").unwrap();
        assert_eq!(results.len(), 1);

        let results = db.search_tracks("Bohemian").unwrap();
        assert_eq!(results.len(), 1);

        let results = db.search_tracks("NotFound").unwrap();
        assert_eq!(results.len(), 0);
    }
}
