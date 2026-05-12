//! Plugin system (Phase 5)
//!
//! WASM-based plugin system for extensibility. Plugins run in a sandboxed
//! environment and can respond to events, access track info, and integrate
//! with external services.
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │          Plugin Runtime                         │
//! ├─────────────────────────────────────────────────┤
//! │  Engine (Wasmtime)                              │
//! │    └── Compiles and runs WASM modules           │
//! │                                                  │
//! │  Linker (Host Functions)                        │
//! │    ├── host_log()                               │
//! │    ├── host_get_track()                         │
//! │    ├── host_set_discord_presence()              │
//! │    └── host_scrobble()                          │
//! │                                                  │
//! │  Plugin Registry                                │
//! │    └── Manages loaded plugins                   │
//! └─────────────────────────────────────────────────┘
//! ```
//!
//! # Creating a Plugin
//! ```rust,no_run
//! // In your plugin's lib.rs (compiled to WASM)
//!
//! #[no_mangle]
//! pub extern "C" fn plugin_init() {
//!     log("My plugin initialized!");
//! }
//!
//! #[no_mangle]
//! pub extern "C" fn on_event(event_ptr: i32) {
//!     // Handle event
//! }
//! ```
//!
//! # Plugin Manifest (plugin.toml)
//! ```toml
//! name = "my-plugin"
//! version = "1.0.0"
//! author = "Your Name"
//! description = "My awesome plugin"
//! permissions = ["track_info", "discord"]
//! ```

pub mod api;
pub mod registry;
pub mod runtime;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export main types
pub use api::{HostFunctions, PluginApi};
pub use registry::PluginRegistry;
pub use runtime::PluginRuntime;

/// Plugin event types that can be sent to plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// A new track started playing
    TrackChanged(TrackInfo),
    /// Playback state changed (play/pause/stop)
    PlaybackStateChanged(PlaybackState),
    /// Volume was changed
    VolumeChanged(f32),
    /// Playlist was modified
    PlaylistChanged {
        playlist_id: String,
        action: PlaylistAction,
    },
    /// Application started
    AppStarted,
    /// Application is shutting down
    AppStopped,
    /// A track completed
    TrackCompleted {
        track: TrackInfo,
        duration_played_secs: u64,
    },
    /// Seek position changed
    SeekChanged { position_secs: u64 },
    /// Custom event for plugin-specific data
    Custom {
        event_type: String,
        data: HashMap<String, String>,
    },
}

impl PluginEvent {
    /// Get event type as string
    pub fn event_type(&self) -> &'static str {
        match self {
            PluginEvent::TrackChanged(_) => "track_changed",
            PluginEvent::PlaybackStateChanged(_) => "playback_state_changed",
            PluginEvent::VolumeChanged(_) => "volume_changed",
            PluginEvent::PlaylistChanged { .. } => "playlist_changed",
            PluginEvent::AppStarted => "app_started",
            PluginEvent::AppStopped => "app_stopped",
            PluginEvent::TrackCompleted { .. } => "track_completed",
            PluginEvent::SeekChanged { .. } => "seek_changed",
            PluginEvent::Custom { .. } => "custom",
        }
    }

    /// Serialize event to JSON for transmission to plugins
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Track information passed to plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Album name (if available)
    pub album: Option<String>,
    /// Track duration in seconds
    pub duration: u64,
    /// Current playback position in seconds
    pub position: u64,
    /// Track ID
    pub id: String,
    /// Source type
    pub source: String,
    /// Album art URL (if available)
    pub album_art_url: Option<String>,
}

impl TrackInfo {
    /// Create a new track info
    pub fn new(id: impl Into<String>, title: impl Into<String>, artist: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album: None,
            duration: 0,
            position: 0,
            source: "local".to_string(),
            album_art_url: None,
        }
    }

    /// Set the album
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Set the duration
    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = duration;
        self
    }

    /// Set the position
    pub fn with_position(mut self, position: u64) -> Self {
        self.position = position;
        self
    }
}

/// Playback state for plugins
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    /// Currently playing
    Playing,
    /// Paused
    Paused,
    /// Stopped
    Stopped,
}

impl std::fmt::Display for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackState::Playing => write!(f, "Playing"),
            PlaybackState::Paused => write!(f, "Paused"),
            PlaybackState::Stopped => write!(f, "Stopped"),
        }
    }
}

/// Playlist modification action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaylistAction {
    /// Track added to playlist
    Added,
    /// Track removed from playlist
    Removed,
    /// Playlist created
    Created,
    /// Playlist deleted
    Deleted,
    /// Playlist reordered
    Reordered,
}

/// Plugin metadata from manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Plugin version (semver)
    pub version: String,
    /// Plugin author
    pub author: String,
    /// Brief description
    pub description: String,
    /// Required permissions
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Plugin website/repository
    #[serde(default)]
    pub homepage: Option<String>,
    /// Minimum Symphony version required
    #[serde(default)]
    pub min_symphony_version: Option<String>,
    /// Whether the plugin is enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl PluginManifest {
    /// Load manifest from a TOML file
    pub fn from_file(path: &std::path::Path) -> Result<Self, PluginError> {
        let content = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Check if plugin has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
}

/// Plugin permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Access current track information
    #[serde(rename = "track_info")]
    TrackInfo,
    /// Access playback state
    #[serde(rename = "playback_state")]
    PlaybackState,
    /// Modify playback (play/pause/skip)
    #[serde(rename = "playback_control")]
    PlaybackControl,
    /// Access library contents
    #[serde(rename = "library")]
    Library,
    /// Modify library (add/remove tracks)
    #[serde(rename = "library_write")]
    LibraryWrite,
    /// Access playlists
    #[serde(rename = "playlists")]
    Playlists,
    /// Modify playlists
    #[serde(rename = "playlists_write")]
    PlaylistsWrite,
    /// Set Discord Rich Presence
    #[serde(rename = "discord")]
    Discord,
    /// Scrobble to Last.fm
    #[serde(rename = "lastfm")]
    Lastfm,
    /// Make network requests
    #[serde(rename = "network")]
    Network,
    /// Read configuration
    #[serde(rename = "config_read")]
    ConfigRead,
    /// Write configuration
    #[serde(rename = "config_write")]
    ConfigWrite,
    /// Show notifications
    #[serde(rename = "notifications")]
    Notifications,
    /// Custom permission (for third-party plugins)
    #[serde(rename = "custom")]
    Custom(String),
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::TrackInfo => write!(f, "track_info"),
            Permission::PlaybackState => write!(f, "playback_state"),
            Permission::PlaybackControl => write!(f, "playback_control"),
            Permission::Library => write!(f, "library"),
            Permission::LibraryWrite => write!(f, "library_write"),
            Permission::Playlists => write!(f, "playlists"),
            Permission::PlaylistsWrite => write!(f, "playlists_write"),
            Permission::Discord => write!(f, "discord"),
            Permission::Lastfm => write!(f, "lastfm"),
            Permission::Network => write!(f, "network"),
            Permission::ConfigRead => write!(f, "config_read"),
            Permission::ConfigWrite => write!(f, "config_write"),
            Permission::Notifications => write!(f, "notifications"),
            Permission::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Plugin state in the runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is loaded and running
    Active,
    /// Plugin is loaded but disabled
    Disabled,
    /// Plugin failed to load
    Error,
    /// Plugin is loading
    Loading,
}

/// Plugin errors
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("WASM error: {0}")]
    WasmError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Plugin initialization failed: {0}")]
    InitFailed(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Host function error: {0}")]
    HostFunctionError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_event_serialization() {
        let event = PluginEvent::TrackChanged(TrackInfo::new("123", "Test Song", "Test Artist"));
        let json = event.to_json().unwrap();
        assert!(json.contains("Test Song"));
    }

    #[test]
    fn test_track_info_builder() {
        let track = TrackInfo::new("id", "Title", "Artist")
            .with_album("Album")
            .with_duration(180);

        assert_eq!(track.title, "Title");
        assert_eq!(track.album, Some("Album".to_string()));
        assert_eq!(track.duration, 180);
    }

    #[test]
    fn test_playback_state_display() {
        assert_eq!(PlaybackState::Playing.to_string(), "Playing");
        assert_eq!(PlaybackState::Paused.to_string(), "Paused");
        assert_eq!(PlaybackState::Stopped.to_string(), "Stopped");
    }
}
