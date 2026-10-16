//! Application state management
//!
//! This module contains the core application state and logic for Symphony.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::ai::{AIManager, MusicIntent};
use crate::audio::features::AudioFeatures;
use crate::audio::{FftSize, SpectrumAnalyzer};
use crate::integrations::git::GitMonitorConfig;
use crate::integrations::pomodoro::PomodoroConfig;
use crate::integrations::{GitMonitor, PomodoroEvent, PomodoroState, PomodoroTimer};
use crate::lyrics::display::LyricsDisplayConfig;
use crate::lyrics::fetcher::{LyricsFetcher, LyricsFetcherConfig};
use crate::lyrics::translate::translate_lines;
use crate::lyrics::{LrcParser, Lyrics, LyricsDisplay, LyricsError};

use crate::audio::AudioEngine;
use crate::config::theme::ThemeManager;
use crate::config::Config;
use crate::db::Database;
use crate::integrations::git::ActivityLevel;
use crate::streaming::StreamTrack;
use crate::ui::artwork::{self, ArtworkCache, ArtworkConfig, AsciiArt};
use crate::ui::visualizer::{VisualizationMode, VisualizerState};

#[path = "app_assistant.rs"]
mod assistant;
use assistant::SearchKind;

#[path = "app_plugins.rs"]
mod hooks;

#[path = "app_stream.rs"]
mod stream;
pub use stream::{import_stream_spec, track_from_stream};

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

    /// Time of the previous `update()` call (drives the playback position)
    last_tick: Instant,

    /// Real-time spectrum analysis of the audio being played
    analyzer: Option<SpectrumAnalyzer>,

    /// Measured loudness and tempo by file path (filled in the background)
    pub features: HashMap<String, AudioFeatures>,

    /// Tracks played this session, oldest first
    pub history: Vec<Track>,

    /// Synced lyrics for the current track
    pub lyrics: LyricsDisplay,

    /// Pomodoro timer
    pub pomodoro: PomodoroTimer,

    /// Git activity monitor (`None` when disabled in the config)
    pub git_monitor: Option<GitMonitor>,

    /// Runtime for async work such as AI requests
    runtime: Option<tokio::runtime::Runtime>,

    /// Plugins, Discord and Last.fm (idle until `start_integrations`)
    integrations: hooks::Integrations,

    /// Coding activity the current queue was built for (see `adapt_music`)
    adapted_level: Option<ActivityLevel>,

    /// Stream manager, created on first use
    stream: stream::SharedManager,

    /// Id of the stream track being fetched, if any
    stream_pending: Option<String>,

    /// AI results come back from the runtime over this channel
    ai_tx: Sender<BackgroundEvent>,
    ai_rx: Receiver<BackgroundEvent>,
}

/// Result of background work, handed back to the UI thread over a channel
enum BackgroundEvent {
    /// The AI understood a prompt
    Intent(MusicIntent),
    /// The AI request failed
    AiError(String),
    /// Online lyrics lookup for a track finished
    Lyrics {
        track_id: String,
        result: Result<Lyrics, LyricsError>,
    },
    /// AI translation of a track's lyrics finished
    Translation {
        track_id: String,
        result: Result<Vec<Option<String>>, String>,
    },
    /// A semantic search finished (ids of the matching tracks, best first)
    Semantic {
        query: String,
        kind: SearchKind,
        ids: Vec<String>,
        method: String,
        note: Option<String>,
    },
    /// The AI answered a question
    Answer(String),
    /// A track's audio analysis finished (`None` if it could not be decoded)
    Features {
        path: String,
        features: Option<AudioFeatures>,
    },
    /// A stream track finished downloading to the cache
    StreamReady { track_id: String, path: PathBuf },
    /// A stream track could not be fetched
    StreamFailed { track_id: String, error: String },
    /// A `symphony stream <query>` search finished
    StreamSearch {
        query: String,
        result: Result<Vec<StreamTrack>, String>,
    },
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

    /// Lyrics panel shown instead of the library
    pub show_lyrics: bool,

    /// AI prompt open
    pub ai_mode: bool,

    /// Text typed into the AI prompt
    pub ai_query: String,

    /// An AI request is in flight
    pub ai_pending: bool,

    /// The AI's answer to a question, shown in a popup until a key is pressed
    pub ai_answer: Option<String>,

    /// The list in the search view is a set of results (AI search or suggestions), not a typed query
    pub pinned_results: bool,

    /// Tracks still waiting for audio analysis
    pub analysis_remaining: usize,
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

        let visualizer_state = initial_visualizer(&config);

        let lyrics = LyricsDisplay::with_config(LyricsDisplayConfig {
            karaoke_mode: config.lyrics.karaoke_mode,
            show_translation: config.lyrics.show_translation,
            context_before: config.lyrics.context_lines,
            context_after: config.lyrics.context_lines,
            ..Default::default()
        });

        let pomo = &config.integrations.pomodoro;
        let pomodoro = PomodoroTimer::with_config(PomodoroConfig {
            work_duration: Duration::from_secs(pomo.work_minutes * 60),
            break_duration: Duration::from_secs(pomo.break_minutes * 60),
            long_break_duration: Duration::from_secs(pomo.long_break_minutes * 60),
            auto_start_break: pomo.auto_start_break,
            auto_start_work: pomo.auto_start_work,
            notifications: pomo.notifications,
            ..Default::default()
        });

        let git_monitor = config.integrations.git.enabled.then(|| {
            GitMonitor::with_config(GitMonitorConfig {
                check_interval: Duration::from_secs(config.integrations.git.check_interval),
                ..Default::default()
            })
        });

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .ok();
        let (ai_tx, ai_rx) = mpsc::channel();

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
                show_lyrics: false,
                ai_mode: false,
                ai_query: String::new(),
                ai_pending: false,
                ai_answer: None,
                pinned_results: false,
                analysis_remaining: 0,
            },
            visualizer_state,
            artwork_cache: ArtworkCache::default(),
            current_track_artwork: None,
            start_time: Instant::now(),
            last_tick: Instant::now(),
            analyzer: None,
            features: HashMap::new(),
            history: Vec::new(),
            lyrics,
            pomodoro,
            git_monitor,
            runtime,
            integrations: Default::default(),
            adapted_level: None,
            stream: Default::default(),
            stream_pending: None,
            ai_tx,
            ai_rx,
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
        self.playback_changed();
    }

    /// Skip to next track
    pub fn next_track(&mut self) {
        if self.library.queue.is_empty() {
            return;
        }
        if self.adapt_queue_for_git() {
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

        let track = self.library.queue[index].clone();
        self.track_finished();
        self.stream_pending = None;
        if stream::is_stream(&track) {
            self.start_stream(track);
            return;
        }
        let path = track.path.clone();
        self.begin_playback(&track, &path);
    }

    /// Start `track` from the audio file `track_path`; `false` if it could not be played
    fn begin_playback(&mut self, track: &Track, track_path: &Path) -> bool {
        if let Err(e) = self.audio_engine.play_file(track_path) {
            self.ui_state.error_message = Some(format!("Failed to play track: {e}"));
            return false;
        }

        self.playback_state.current_track = Some(track.clone());
        self.history.push(track.clone());
        if self.history.len() > 50 {
            self.history.remove(0);
        }
        self.playback_state.is_playing = true;
        self.playback_state.position = 0.0;
        self.playback_state.duration = track.duration;
        self.track_started(track);

        // Update status message
        self.ui_state.status_message = Some(format!("Playing: {} - {}", track.artist, track.title));

        // Clear error if any
        self.ui_state.error_message = None;

        self.load_lyrics_for(track_path);
        if !self.lyrics.has_lyrics() {
            self.fetch_lyrics_online(track);
        }
        self.load_artwork(&track.id, track_path);
        if let Some(analyzer) = &mut self.analyzer {
            analyzer.reset();
        }
        true
    }

    /// Stop playback completely
    fn stop_playback(&mut self) {
        self.track_finished();
        self.audio_engine.stop();
        self.playback_state.is_playing = false;
        self.playback_state.current_track = None;
        self.playback_stopped();
    }

    /// Increase volume
    pub fn volume_up(&mut self) {
        self.set_volume(self.playback_state.volume + 0.05);
    }

    /// Decrease volume
    pub fn volume_down(&mut self) {
        self.set_volume(self.playback_state.volume - 0.05);
    }

    /// Set the volume (clamped to 0..=1, rounded to whole percents)
    fn set_volume(&mut self, volume: f32) {
        let volume = ((volume.clamp(0.0, 1.0)) * 100.0).round() / 100.0;
        self.playback_state.volume = volume;
        self.audio_engine.set_volume(volume);
        self.ui_state.status_message =
            Some(format!("Volume: {}%", (volume * 100.0).round() as u32));
        self.volume_changed();
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
        } else {
            self.restore_queue_order();
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

    /// Put the queue back into library order, keeping the current track current
    fn restore_queue_order(&mut self) {
        let current_id = self
            .library
            .queue
            .get(self.library.queue_index)
            .map(|t| t.id.clone());

        let tracks = &self.library.tracks;
        self.library.queue.sort_by_key(|q| {
            tracks
                .iter()
                .position(|t| t.id == q.id)
                .unwrap_or(usize::MAX)
        });

        if let Some(id) = current_id {
            if let Some(pos) = self.library.queue.iter().position(|t| t.id == id) {
                self.library.queue_index = pos;
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

        if tracks.get(self.library.selected_index).is_some() {
            // Set queue to all visible tracks
            self.library.queue = tracks.clone();
            self.library.queue_index = self.library.selected_index;
            if self.playback_state.shuffle {
                // Keeps the selected track first, shuffles the rest
                self.shuffle_queue();
            }
            self.play_track_at_index(self.library.queue_index);
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
        self.ui_state.pinned_results = false;
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
        let viz_mode = match mode {
            1 => VisualizationMode::Bars,
            2 => VisualizationMode::Waveform,
            3 => VisualizationMode::Circular,
            4 => VisualizationMode::Minimal,
            5 => VisualizationMode::Off,
            _ => return,
        };
        self.visualizer_state.set_mode(viz_mode);
        self.ui_state.status_message = Some(format!("Visualizer: {}", viz_mode.name()));
    }

    /// Update application state (called each frame)
    pub fn update(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        self.poll_background();
        self.poll_integrations();
        self.poll_plugin_notifications();
        self.publish_status();
        self.update_visualizer();

        if self.playback_state.current_track.is_none() || !self.playback_state.is_playing {
            return;
        }

        // Rodio does not report the position, so advance it by wall-clock time while playing.
        let state = &mut self.playback_state;
        state.position += elapsed;
        if state.duration > 0.0 {
            state.position = state.position.min(state.duration);
        }
        self.lyrics
            .update_position((self.playback_state.position * 1000.0) as u64);

        // The sink drains when the track ends: move on to the next one.
        if self.audio_engine.is_empty() {
            self.audio_engine.mark_completed();
            self.next_track();
        }
    }

    // ---- text prompts (search / AI) ----

    /// A text prompt is open and should receive every key
    pub fn is_in_text_input(&self) -> bool {
        self.ui_state.search_mode || self.ui_state.ai_mode
    }

    /// Type a character into the open prompt
    pub fn append_input_char(&mut self, c: char) {
        if self.ui_state.ai_mode {
            self.ui_state.ai_query.push(c);
        } else {
            self.unpin_results();
            self.append_search_char(c);
        }
    }

    /// Typing over a list of AI results starts an ordinary search instead
    fn unpin_results(&mut self) {
        if self.ui_state.pinned_results {
            self.ui_state.pinned_results = false;
            self.ui_state.search_query.clear();
        }
    }

    /// Delete the last character of the open prompt
    pub fn remove_input_char(&mut self) {
        if self.ui_state.ai_mode {
            self.ui_state.ai_query.pop();
        } else if self.ui_state.pinned_results {
            self.unpin_results();
            self.apply_search_filter();
        } else {
            self.remove_search_char();
        }
    }

    /// Close the open prompt
    pub fn cancel_text_input(&mut self) {
        if self.ui_state.ai_mode {
            self.exit_ai_mode();
        } else {
            self.exit_search_mode();
        }
    }

    /// Enter in the open prompt: send the AI request, or play the selected search result
    pub fn confirm_text_input(&mut self) {
        if self.ui_state.ai_mode {
            self.submit_ai_query();
        } else {
            self.select_current_track();
        }
    }

    // ---- AI ----

    /// Open the AI prompt (`a`)
    pub fn enter_ai_mode(&mut self) {
        if !self.config.ai.enabled {
            self.ui_state.status_message =
                Some("AI is disabled: set enabled = true under [ai] in the config".to_string());
            return;
        }
        if self.ui_state.ai_pending {
            self.ui_state.status_message =
                Some("AI is still working on your last request".to_string());
            return;
        }
        self.ui_state.search_mode = false;
        self.ui_state.ai_mode = true;
        self.ui_state.ai_query.clear();
    }

    /// Run a prompt given on the command line (`symphony "play something calm"`)
    pub fn submit_prompt(&mut self, prompt: &str) {
        if !self.config.ai.enabled {
            self.ui_state.status_message =
                Some("AI is disabled: set enabled = true under [ai] in the config".to_string());
            return;
        }
        self.ui_state.ai_query = prompt.to_string();
        self.submit_ai_query();
    }

    /// Close the AI prompt without sending
    pub fn exit_ai_mode(&mut self) {
        self.ui_state.ai_mode = false;
        self.ui_state.ai_query.clear();
    }

    /// Send the typed prompt to the configured AI provider in the background
    pub fn submit_ai_query(&mut self) {
        let query = self.ui_state.ai_query.trim().to_string();
        self.exit_ai_mode();
        if query.is_empty() {
            return;
        }

        let provider = match self.config.ai.get_provider_config() {
            Ok(provider) => provider,
            Err(e) => {
                self.ui_state.error_message = Some(format!("AI: {e}"));
                return;
            }
        };
        let Some(runtime) = &self.runtime else {
            self.ui_state.error_message = Some("AI: could not start the async runtime".to_string());
            return;
        };

        let context = self.build_ai_context();
        let tx = self.ai_tx.clone();
        self.ui_state.ai_pending = true;
        self.ui_state.error_message = None;
        self.ui_state.status_message = Some(format!("AI: thinking about \"{query}\"..."));

        runtime.spawn(async move {
            let result = async {
                let manager = AIManager::new(provider).await?.with_context(context);
                manager.parse_command(&query).await
            }
            .await;
            let event = match result {
                Ok(intent) => BackgroundEvent::Intent(intent),
                Err(e) => BackgroundEvent::AiError(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    /// Collect finished background work: AI prompts, lyrics lookups, translations (every frame)
    fn poll_background(&mut self) {
        while let Ok(event) = self.ai_rx.try_recv() {
            match event {
                BackgroundEvent::Intent(intent) => {
                    self.ui_state.ai_pending = false;
                    self.apply_ai_intent(intent);
                }
                BackgroundEvent::AiError(e) => {
                    self.ui_state.ai_pending = false;
                    self.ui_state.error_message = Some(format!("AI: {e}"));
                }
                BackgroundEvent::Lyrics { track_id, result } => {
                    self.apply_fetched_lyrics(&track_id, result);
                }
                BackgroundEvent::Translation { track_id, result } => {
                    self.apply_translation(&track_id, result);
                }
                BackgroundEvent::Semantic {
                    query,
                    kind,
                    ids,
                    method,
                    note,
                } => self.apply_semantic(&query, kind, ids, &method, note),
                BackgroundEvent::Answer(text) => {
                    self.ui_state.ai_pending = false;
                    self.ui_state.status_message = None;
                    self.ui_state.ai_answer = Some(text);
                }
                BackgroundEvent::Features { path, features } => self.apply_features(path, features),
                BackgroundEvent::StreamReady { track_id, path } => {
                    self.apply_stream_ready(&track_id, path)
                }
                BackgroundEvent::StreamFailed { track_id, error } => {
                    self.apply_stream_failed(&track_id, error)
                }
                BackgroundEvent::StreamSearch { query, result } => {
                    self.apply_stream_search(&query, result)
                }
            }
        }
    }

    fn is_current_track(&self, track_id: &str) -> bool {
        self.playback_state
            .current_track
            .as_ref()
            .is_some_and(|t| t.id == track_id)
    }

    // ---- visualizer ----

    /// Feed the visualizer with the audio that was just played, or fade it out
    fn update_visualizer(&mut self) {
        // always drain so the tap buffer never holds stale audio
        let samples = self.audio_engine.drain_samples();
        if !self.config.visualizer.enabled || self.visualizer_state.mode == VisualizationMode::Off {
            return;
        }
        if samples.is_empty() {
            self.visualizer_state.decay();
            return;
        }

        let rate = self.audio_engine.sample_rate();
        if self.analyzer.as_ref().map(|a| a.sample_rate()) != Some(rate) {
            let mut analyzer =
                SpectrumAnalyzer::new(fft_size_from(self.config.visualizer.fft_size), rate);
            analyzer.set_smoothing(self.config.visualizer.smoothing);
            self.analyzer = Some(analyzer);
        }
        if let Some(analyzer) = &mut self.analyzer {
            analyzer.add_samples(&samples);
            if let Some(result) = analyzer.analyze() {
                self.visualizer_state.update(result);
            }
        }
    }

    // ---- album art ----

    /// Album art of the track that just started (embedded cover or cover file), cached by track
    pub fn load_artwork(&mut self, track_id: &str, audio_path: &Path) {
        if let Some(art) = self.artwork_cache.get(track_id) {
            self.current_track_artwork = Some(art.clone());
            return;
        }

        let config = ArtworkConfig {
            max_width: artwork::ART_COLS,
            max_height: artwork::ART_ROWS,
            ..ArtworkConfig::default()
        };
        self.current_track_artwork = artwork::extract_album_art(audio_path)
            .and_then(|bytes| AsciiArt::from_image(&bytes, &config).ok());
        if let Some(art) = &self.current_track_artwork {
            self.artwork_cache.insert(track_id.to_string(), art.clone());
        }
    }

    // ---- lyrics ----

    /// Show or hide the lyrics panel (`l`)
    pub fn toggle_lyrics(&mut self) {
        if !self.config.lyrics.enabled {
            self.ui_state.status_message =
                Some("Lyrics are disabled: set enabled = true under [lyrics]".to_string());
            return;
        }
        self.ui_state.show_lyrics = !self.ui_state.show_lyrics;
    }

    /// Look the track up on LRCLIB in the background (cached on disk; skipped for unknown artists)
    fn fetch_lyrics_online(&mut self, track: &Track) {
        let config = &self.config.lyrics;
        if !config.enabled
            || !config.auto_fetch
            || track.title.trim().is_empty()
            || track.artist == "Unknown Artist"
        {
            return;
        }
        let Some(runtime) = &self.runtime else { return };

        let fetcher = LyricsFetcher::new(LyricsFetcherConfig {
            cache_dir: config.cache_dir.clone(),
            lrclib_url: config.lrclib_url.clone(),
            ..LyricsFetcherConfig::default()
        });
        let (id, title, artist, album, duration) = (
            track.id.clone(),
            track.title.clone(),
            track.artist.clone(),
            track.album.clone(),
            track.duration,
        );
        let tx = self.ai_tx.clone();

        runtime.spawn(async move {
            let result = fetcher
                .fetch_track(&title, &artist, Some(&album), Some(duration))
                .await;
            let _ = tx.send(BackgroundEvent::Lyrics {
                track_id: id,
                result,
            });
        });
    }

    /// A lookup finished: show the lyrics if that track is still playing
    fn apply_fetched_lyrics(&mut self, track_id: &str, result: Result<Lyrics, LyricsError>) {
        if !self.is_current_track(track_id) {
            return;
        }
        match result {
            Ok(lyrics) => {
                self.lyrics.set_lyrics(lyrics);
                self.lyrics
                    .update_position((self.playback_state.position * 1000.0) as u64);
                self.ui_state.status_message =
                    Some("Lyrics found online, press l to show them".to_string());
            }
            Err(LyricsError::NotFound) => {}
            Err(e) => {
                self.ui_state.status_message = Some(format!("Lyrics: online lookup failed ({e})"));
            }
        }
    }

    /// Translate the lyrics with the AI provider (`x`); pressed again it shows/hides the translation
    pub fn translate_lyrics(&mut self) {
        if !self.lyrics.has_lyrics() {
            self.ui_state.status_message = Some(
                "No lyrics to translate: add a .lrc file or enable [lyrics] auto_fetch".to_string(),
            );
            return;
        }
        if self.lyrics.has_translations() {
            self.lyrics.toggle_translation();
            return;
        }
        if !self.config.ai.enabled {
            self.ui_state.status_message =
                Some("Translation needs the AI: set enabled = true under [ai]".to_string());
            return;
        }
        let provider = match self.config.ai.get_provider_config() {
            Ok(provider) => provider,
            Err(e) => {
                self.ui_state.error_message = Some(format!("AI: {e}"));
                return;
            }
        };
        let (Some(runtime), Some(track_id)) = (
            &self.runtime,
            self.playback_state
                .current_track
                .as_ref()
                .map(|t| t.id.clone()),
        ) else {
            return;
        };
        let Some(lines) = self.lyrics.lyrics().map(|l| {
            l.lines
                .iter()
                .map(|line| line.text.clone())
                .collect::<Vec<_>>()
        }) else {
            return;
        };

        let language = self.config.lyrics.translate_to.clone();
        self.ui_state.status_message = Some(format!("Translating lyrics to {language}..."));
        let tx = self.ai_tx.clone();
        runtime.spawn(async move {
            let result = translate_lines(provider, &language, &lines).await;
            let _ = tx.send(BackgroundEvent::Translation { track_id, result });
        });
    }

    /// A translation finished: attach it to the lyrics and show the lyrics panel
    fn apply_translation(&mut self, track_id: &str, result: Result<Vec<Option<String>>, String>) {
        if !self.is_current_track(track_id) {
            return;
        }
        match result {
            Ok(translations) => {
                let translated = translations.iter().flatten().count();
                self.lyrics.set_translations(translations);
                self.ui_state.show_lyrics = true;
                self.ui_state.status_message = Some(format!(
                    "Lyrics translated to {} ({translated} lines, x hides it)",
                    self.config.lyrics.translate_to
                ));
            }
            Err(e) => self.ui_state.error_message = Some(format!("Translation: {e}")),
        }
    }

    /// Load the `.lrc` file next to the audio file, if there is one
    pub fn load_lyrics_for(&mut self, audio_path: &Path) {
        if !self.config.lyrics.enabled {
            self.lyrics.clear();
            return;
        }
        match LrcParser::parse_file(&audio_path.with_extension("lrc")) {
            Ok(lyrics) => self.lyrics.set_lyrics(lyrics),
            Err(_) => self.lyrics.clear(),
        }
    }

    // ---- integrations ----

    /// Start or stop the Pomodoro timer (`w`)
    pub fn toggle_pomodoro(&mut self) {
        let message = match self.pomodoro.get_state() {
            PomodoroState::Working | PomodoroState::Break | PomodoroState::LongBreak => {
                self.pomodoro.stop();
                "Pomodoro stopped".to_string()
            }
            PomodoroState::WorkComplete => {
                self.pomodoro.start_break();
                "Pomodoro: break started".to_string()
            }
            _ => {
                self.pomodoro.start_work();
                format!(
                    "Pomodoro: work session started ({})",
                    self.pomodoro.format_remaining()
                )
            }
        };
        self.ui_state.status_message = Some(message);
    }

    /// Timer completion and git activity (called every frame; both are cheap or rate-limited)
    fn poll_integrations(&mut self) {
        if let Some(event) = self.pomodoro.check_completion() {
            self.ui_state.status_message = Some(match event {
                PomodoroEvent::WorkSessionComplete { session } => {
                    format!("Pomodoro: work session {session} done, press w for a break")
                }
                PomodoroEvent::BreakComplete => {
                    "Pomodoro: break over, press w for the next session".to_string()
                }
                PomodoroEvent::Started(_) | PomodoroEvent::Stopped { .. } => return,
            });
        }

        // ponytail: runs `git` on the UI thread once per check_interval (60 s default);
        // move to the runtime if the pause ever becomes visible.
        if let Some(git) = &mut self.git_monitor {
            git.update();
        }
    }

    /// Load music library from database
    pub fn load_library(&mut self) -> anyhow::Result<()> {
        if let Some(db) = &self.database {
            let tracks = db.get_all_tracks()?;
            self.library.tracks = tracks;
            self.features = db.all_features()?;
        }
        self.start_feature_analysis();
        Ok(())
    }
}

/// Visualizer state from the `[visualizer]` config section
fn initial_visualizer(config: &Config) -> VisualizerState {
    let mut state = VisualizerState::new(32);
    state.mode = if !config.visualizer.enabled {
        VisualizationMode::Off
    } else {
        match config.visualizer.mode.to_lowercase().as_str() {
            "waveform" => VisualizationMode::Waveform,
            "circular" | "circle" => VisualizationMode::Circular,
            "minimal" => VisualizationMode::Minimal,
            "off" | "none" => VisualizationMode::Off,
            _ => VisualizationMode::Bars, // "spectrum", "bars"
        }
    };
    state
}

/// Closest supported FFT size for the configured value
fn fft_size_from(size: usize) -> FftSize {
    match size {
        0..=2048 => FftSize::Size2048,
        2049..=4096 => FftSize::Size4096,
        _ => FftSize::Size8192,
    }
}

/// Tracks whose artist/title contains `artist` and whose genre contains `genre`
/// (case-insensitive; a missing filter matches everything)
fn filter_tracks(tracks: &[Track], artist: Option<&str>, genre: Option<&str>) -> Vec<Track> {
    let artist = artist.map(str::to_lowercase);
    let genre = genre.map(str::to_lowercase);

    tracks
        .iter()
        .filter(|t| {
            let artist_ok = artist.as_ref().is_none_or(|a| {
                t.artist.to_lowercase().contains(a) || t.title.to_lowercase().contains(a)
            });
            let genre_ok = genre.as_ref().is_none_or(|g| {
                t.genre
                    .as_ref()
                    .is_some_and(|tg| tg.to_lowercase().contains(g))
            });
            artist_ok && genre_ok
        })
        .cloned()
        .collect()
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
            show_lyrics: false,
            ai_mode: false,
            ai_query: String::new(),
            ai_pending: false,
            ai_answer: None,
            pinned_results: false,
            analysis_remaining: 0,
        }
    }
}
