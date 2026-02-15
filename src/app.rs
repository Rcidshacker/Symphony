//! Application state management
//!
//! This module contains the core application state and logic for Symphony.

use std::path::PathBuf;
use std::time::Instant;

use crate::audio::AudioEngine;
use crate::config::theme::ThemeManager;
use crate::config::Config;
use crate::db::Database;
use crate::ui::artwork::{ArtworkCache, AsciiArt};
use crate::ui::visualizer::VisualizerState;

/// Main application state
pub struct App {
    /// Application configuration
    pub config: Config,

    /// Audio playback engine
    pub audio_engine: AudioEngine,

    /// Database for music library
    pub database: Option<Database>,

    /// Theme manager
    pub theme_manager: ThemeManager,

    /// Current playback state
    pub playback_state: PlaybackState,

    /// Music library
    pub library: Library,

    /// User interface state
    pub ui_state: UiState,

    /// Visualizer state
    pub visualizer_state: VisualizerState,

    /// Album art cache
    pub artwork_cache: ArtworkCache,

    /// Current track artwork
    pub current_track_artwork: Option<AsciiArt>,

    /// Application start time
    pub start_time: Instant,
}

/// Playback state
#[derive(Debug, Clone)]
pub struct PlaybackState {
    /// Whether audio is currently playing
    pub is_playing: bool,

    /// Current volume (0.0 to 1.0)
    pub volume: f32,

    /// Whether muted
    pub is_muted: bool,

    /// Shuffle mode enabled
    pub shuffle: bool,

    /// Repeat mode
    pub repeat: RepeatMode,

    /// Currently playing track
    pub current_track: Option<Track>,

    /// Position in current track (seconds)
    pub position: f64,

    /// Duration of current track (seconds)
    pub duration: f64,
}

/// Repeat mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatMode {
    /// No repeat
    Off,
    /// Repeat all tracks in queue
    All,
    /// Repeat current track
    One,
}

/// A music track
#[derive(Debug, Clone)]
pub struct Track {
    /// Unique identifier
    pub id: String,

    /// Track title
    pub title: String,

    /// Artist name
    pub artist: String,

    /// Album name
    pub album: String,

    /// Track duration in seconds
    pub duration: f64,

    /// File path
    pub path: PathBuf,

    /// Track number
    pub track_number: Option<u32>,

    /// Genre
    pub genre: Option<String>,

    /// Year
    pub year: Option<u32>,

    /// Source (local, youtube, spotify)
    pub source: TrackSource,
}

/// Source of the track
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSource {
    /// Local file
    Local,
    /// YouTube stream
    YouTube,
    /// Spotify stream
    Spotify,
    /// SoundCloud stream
    SoundCloud,
    /// Cached stream
    Cached,
}

/// Music library state
#[derive(Debug, Clone)]
pub struct Library {
    /// All tracks in the library
    pub tracks: Vec<Track>,

    /// Current queue
    pub queue: Vec<Track>,

    /// Queue index
    pub queue_index: usize,

    /// Playlists
    pub playlists: Vec<Playlist>,

    /// Currently selected index in UI
    pub selected_index: usize,

    /// Filter/search query
    pub filter: String,

    /// Filtered tracks (based on search)
    pub filtered_tracks: Vec<Track>,
}

/// A playlist
#[derive(Debug, Clone)]
pub struct Playlist {
    /// Playlist ID
    pub id: String,

    /// Playlist name
    pub name: String,

    /// Tracks in playlist
    pub tracks: Vec<String>, // Track IDs
}

/// UI state
#[derive(Debug, Clone)]
pub struct UiState {
    /// Current view/mode
    pub view: View,

    /// Whether help is shown
    pub show_help: bool,

    /// Search mode active
    pub search_mode: bool,

    /// Search query
    pub search_query: String,

    /// Status message
    pub status_message: Option<String>,

    /// Error message
    pub error_message: Option<String>,
}

/// Current view
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    /// Library view
    Library,
    /// Queue view
    Queue,
    /// Playlists view
    Playlists,
    /// Settings view
    Settings,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config, audio_engine: AudioEngine) -> Self {
        let volume = config.audio.volume;
        let theme_name = &config.general.theme;

        let mut theme_manager = ThemeManager::new();
        // Try to set the configured theme
        if theme_manager.set_theme(theme_name).is_err() {
            // Fall back to monokai if theme not found
            let _ = theme_manager.set_theme("monokai");
        }

        Self {
            config,
            audio_engine,
            database: None,
            theme_manager,
            playback_state: PlaybackState {
                is_playing: false,
                volume,
                is_muted: false,
                shuffle: false,
                repeat: RepeatMode::Off,
                current_track: None,
                position: 0.0,
                duration: 0.0,
            },
            library: Library {
                tracks: Vec::new(),
                queue: Vec::new(),
                queue_index: 0,
                playlists: Vec::new(),
                selected_index: 0,
                filter: String::new(),
                filtered_tracks: Vec::new(),
            },
            ui_state: UiState {
                view: View::Library,
                show_help: false,
                search_mode: false,
                search_query: String::new(),
                status_message: None,
                error_message: None,
            },
            visualizer_state: VisualizerState::new(32),
            artwork_cache: ArtworkCache::default(),
            current_track_artwork: None,
            start_time: Instant::now(),
        }
    }

    /// Toggle playback
    pub fn toggle_playback(&mut self) {
        if self.playback_state.is_playing {
            self.audio_engine.pause();
            self.playback_state.is_playing = false;
        } else {
            self.audio_engine.play();
            self.playback_state.is_playing = true;
        }
    }

    /// Skip to next track
    pub fn next_track(&mut self) {
        if self.library.queue.is_empty() {
            return;
        }

        match self.playback_state.repeat {
            RepeatMode::One => {
                // Replay current track
                self.play_track_at_index(self.library.queue_index);
            }
            RepeatMode::All => {
                // Next track, wrap around
                let next_index = (self.library.queue_index + 1) % self.library.queue.len();
                self.library.queue_index = next_index;
                self.play_track_at_index(next_index);
            }
            RepeatMode::Off => {
                // Next track if available
                if self.library.queue_index + 1 < self.library.queue.len() {
                    self.library.queue_index += 1;
                    self.play_track_at_index(self.library.queue_index);
                } else {
                    // End of queue
                    self.stop_playback();
                }
            }
        }
    }

    /// Go to previous track
    pub fn previous_track(&mut self) {
        if self.library.queue.is_empty() {
            return;
        }

        // If more than 3 seconds into track, restart current track
        if self.playback_state.position > 3.0 {
            self.seek_to(0.0);
            return;
        }

        let prev_index = if self.library.queue_index > 0 {
            self.library.queue_index - 1
        } else if self.playback_state.repeat == RepeatMode::All {
            self.library.queue.len() - 1
        } else {
            0
        };

        self.library.queue_index = prev_index;
        self.play_track_at_index(prev_index);
    }

    /// Play track at given queue index
    fn play_track_at_index(&mut self, index: usize) {
        if index >= self.library.queue.len() {
            return;
        }

        let track = &self.library.queue[index];
        if let Err(e) = self.audio_engine.play_file(&track.path) {
            self.ui_state.error_message = Some(format!("Failed to play track: {e}"));
            return;
        }

        self.playback_state.current_track = Some(track.clone());
        self.playback_state.is_playing = true;
        self.playback_state.position = 0.0;
        self.playback_state.duration = track.duration;

        // Update status message
        self.ui_state.status_message = Some(format!("Playing: {} - {}", track.artist, track.title));

        // Clear error if any
        self.ui_state.error_message = None;
    }

    /// Stop playback completely
    fn stop_playback(&mut self) {
        self.audio_engine.stop();
        self.playback_state.is_playing = false;
        self.playback_state.current_track = None;
    }

    /// Increase volume
    pub fn volume_up(&mut self) {
        let new_volume = (self.playback_state.volume + 0.05).min(1.0);
        self.playback_state.volume = new_volume;
        self.audio_engine.set_volume(new_volume);
    }

    /// Decrease volume
    pub fn volume_down(&mut self) {
        let new_volume = (self.playback_state.volume - 0.05).max(0.0);
        self.playback_state.volume = new_volume;
        self.audio_engine.set_volume(new_volume);
    }

    /// Toggle mute
    pub fn toggle_mute(&mut self) {
        self.playback_state.is_muted = !self.playback_state.is_muted;
        self.audio_engine.set_muted(self.playback_state.is_muted);
    }

    /// Toggle shuffle mode
    pub fn toggle_shuffle(&mut self) {
        self.playback_state.shuffle = !self.playback_state.shuffle;
        if self.playback_state.shuffle {
            self.shuffle_queue();
        }
        self.ui_state.status_message = Some(if self.playback_state.shuffle {
            "Shuffle: ON".to_string()
        } else {
            "Shuffle: OFF".to_string()
        });
    }

    /// Toggle repeat mode
    pub fn toggle_repeat(&mut self) {
        self.playback_state.repeat = match self.playback_state.repeat {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        let mode_str = match self.playback_state.repeat {
            RepeatMode::Off => "Repeat: OFF",
            RepeatMode::All => "Repeat: ALL",
            RepeatMode::One => "Repeat: ONE",
        };
        self.ui_state.status_message = Some(mode_str.to_string());
    }

    /// Shuffle the queue (keeping current track first)
    fn shuffle_queue(&mut self) {
        if self.library.queue.len() <= 1 {
            return;
        }

        let current_track = if self.library.queue_index < self.library.queue.len() {
            Some(self.library.queue[self.library.queue_index].clone())
        } else {
            None
        };

        // Fisher-Yates shuffle
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.library.queue.shuffle(&mut rng);

        // Move current track to front if it exists
        if let Some(track) = current_track {
            if let Some(pos) = self.library.queue.iter().position(|t| t.id == track.id) {
                self.library.queue.swap(0, pos);
                self.library.queue_index = 0;
            }
        }
    }

    /// Seek to position
    fn seek_to(&mut self, _position: f64) {
        // TODO: Implement seeking in audio engine
    }

    /// Navigate up in the library list
    pub fn navigate_up(&mut self) {
        if self.library.selected_index > 0 {
            self.library.selected_index -= 1;
        }
    }

    /// Navigate down in the library list
    pub fn navigate_down(&mut self) {
        let max_index = if self.ui_state.search_mode {
            self.library.filtered_tracks.len()
        } else {
            self.library.tracks.len()
        };

        if max_index > 0 && self.library.selected_index < max_index - 1 {
            self.library.selected_index += 1;
        }
    }

    /// Select and play current track
    pub fn select_current_track(&mut self) {
        let tracks = if self.ui_state.search_mode {
            &self.library.filtered_tracks
        } else {
            &self.library.tracks
        };

        if let Some(track) = tracks.get(self.library.selected_index) {
            // Set queue to all visible tracks
            self.library.queue = tracks.clone();
            self.library.queue_index = self.library.selected_index;
            self.play_track_at_index(self.library.selected_index);
        }
    }

    /// Enter search mode
    pub fn enter_search_mode(&mut self) {
        self.ui_state.search_mode = true;
        self.ui_state.search_query.clear();
        self.library.selected_index = 0;
    }

    /// Exit search mode
    pub fn exit_search_mode(&mut self) {
        self.ui_state.search_mode = false;
        self.ui_state.search_query.clear();
        self.library.filtered_tracks.clear();
        self.library.selected_index = 0;
    }

    /// Check if in search mode
    pub fn is_in_search_mode(&self) -> bool {
        self.ui_state.search_mode
    }

    /// Append character to search query
    pub fn append_search_char(&mut self, c: char) {
        self.ui_state.search_query.push(c);
        self.apply_search_filter();
    }

    /// Remove last character from search query
    pub fn remove_search_char(&mut self) {
        self.ui_state.search_query.pop();
        self.apply_search_filter();
    }

    /// Apply search filter to tracks
    fn apply_search_filter(&mut self) {
        let query = self.ui_state.search_query.to_lowercase();
        self.library.filtered_tracks = self
            .library
            .tracks
            .iter()
            .filter(|track| {
                track.title.to_lowercase().contains(&query)
                    || track.artist.to_lowercase().contains(&query)
                    || track.album.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
        self.library.selected_index = 0;
    }

    /// Toggle help display
    pub fn toggle_help(&mut self) {
        self.ui_state.show_help = !self.ui_state.show_help;
    }

    /// Cycle to next theme
    pub fn next_theme(&mut self) {
        let theme = self.theme_manager.next_theme();
        self.ui_state.status_message = Some(format!("Theme: {}", theme.name));
    }

    /// Set theme by name
    pub fn set_theme(&mut self, name: &str) {
        if let Ok(()) = self.theme_manager.set_theme(name) {
            self.ui_state.status_message = Some(format!("Theme: {}", name));
        }
    }

    /// Cycle visualization mode
    pub fn next_visualization_mode(&mut self) {
        self.visualizer_state.next_mode();
        let mode_name = self.visualizer_state.mode.name();
        self.ui_state.status_message = Some(format!("Visualizer: {}", mode_name));
    }

    /// Set visualization mode by number (1-4)
    pub fn set_visualization_mode(&mut self, mode: u8) {
        use crate::ui::visualizer::VisualizationMode;
        let viz_mode = match mode {
            1 => VisualizationMode::Bars,
            2 => VisualizationMode::Waveform,
            3 => VisualizationMode::Circular,
            4 => VisualizationMode::Off,
            _ => return,
        };
        self.visualizer_state.set_mode(viz_mode);
        self.ui_state.status_message = Some(format!("Visualizer: {}", viz_mode.name()));
    }

    /// Update application state (called each frame)
    pub fn update(&mut self) {
        // Update playback position
        // This would normally query the audio engine for actual position
        // For now, we'll estimate based on time elapsed

        // Clear status message after some time
        // In production, we'd track message timestamp
    }

    /// Load music library from database
    pub fn load_library(&mut self) -> anyhow::Result<()> {
        if let Some(db) = &self.database {
            let tracks = db.get_all_tracks()?;
            self.library.tracks = tracks;
        }
        Ok(())
    }
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            volume: 0.6,
            is_muted: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            current_track: None,
            position: 0.0,
            duration: 0.0,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            view: View::Library,
            show_help: false,
            search_mode: false,
            search_query: String::new(),
            status_message: None,
            error_message: None,
        }
    }
}
