//! Configuration module
//!
//! Handles loading and saving of user configuration.

pub mod theme;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to write config: {0}")]
    WriteError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// General settings
    #[serde(default)]
    pub general: GeneralConfig,

    /// Audio settings
    #[serde(default)]
    pub audio: AudioConfig,

    /// Visualization settings
    #[serde(default)]
    pub visualizer: VisualizerConfig,

    /// AI settings
    #[serde(default)]
    pub ai: AiConfig,

    /// Streaming settings
    #[serde(default)]
    pub streaming: StreamingConfig,

    /// Plugin settings
    #[serde(default)]
    pub plugins: PluginsConfig,

    /// Lyrics settings
    #[serde(default)]
    pub lyrics: LyricsConfig,

    /// Integration settings
    #[serde(default)]
    pub integrations: IntegrationsConfig,

    /// Keybindings
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// General configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Music directory
    pub music_directory: PathBuf,

    /// Cache directory
    pub cache_directory: PathBuf,

    /// Database path
    pub database_path: PathBuf,

    /// Theme name
    pub theme: String,

    /// Default view on startup
    pub default_view: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        let _config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony");

        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony");

        Self {
            music_directory: dirs::audio_dir().unwrap_or_else(|| PathBuf::from("~/Music")),
            cache_directory: data_dir.join("cache"),
            database_path: data_dir.join("library.db"),
            theme: "monokai".to_string(),
            default_view: "library".to_string(),
        }
    }
}

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Output device (default = system default)
    pub output_device: String,

    /// Sample rate
    pub sample_rate: u32,

    /// Buffer size
    pub buffer_size: usize,

    /// Default volume (0.0 to 1.0)
    pub volume: f32,

    /// Normalize audio
    pub normalize_audio: bool,

    /// Audio effects
    #[serde(default)]
    pub effects: AudioEffects,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            output_device: "default".to_string(),
            sample_rate: 44100,
            buffer_size: 2048,
            volume: 0.6,
            normalize_audio: false,
            effects: AudioEffects::default(),
        }
    }
}

/// Audio effects configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioEffects {
    /// Enable equalizer
    pub equalizer: bool,

    /// Enable reverb
    pub reverb: bool,

    /// Bass boost factor (1.0 = normal)
    pub bass_boost: f32,
}

/// Visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizerConfig {
    /// Enable visualizer
    pub enabled: bool,

    /// Visualization mode
    pub mode: String,

    /// FFT size
    pub fft_size: usize,

    /// Smoothing factor
    pub smoothing: f32,

    /// Color scheme
    pub color_scheme: String,

    /// Enable beat detection
    pub beat_detection: bool,
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "spectrum".to_string(),
            fft_size: 4096,
            smoothing: 0.8,
            color_scheme: "rainbow".to_string(),
            beat_detection: true,
        }
    }
}

/// AI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Enable AI features
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// AI provider: "ollama" (local) or "openrouter" (cloud)
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Ollama configuration
    #[serde(default)]
    pub ollama: OllamaConfig,

    /// OpenRouter configuration
    #[serde(default)]
    pub openrouter: OpenRouterConfig,

    /// Enable recommendations
    #[serde(default = "default_true")]
    pub recommendations_enabled: bool,

    /// Enable natural language commands
    #[serde(default = "default_true")]
    pub natural_language_commands: bool,

    /// Cache AI responses
    #[serde(default = "default_true")]
    pub cache_responses: bool,
}

fn default_true() -> bool {
    true
}
fn default_provider() -> String {
    "ollama".to_string()
}

/// Ollama (local AI) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Base URL for Ollama API
    #[serde(default = "default_ollama_url")]
    pub base_url: String,

    /// Model name
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "gemma3:4b".to_string()
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_url(),
            model: default_ollama_model(),
        }
    }
}

/// OpenRouter (cloud AI) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    /// API key from openrouter.ai
    #[serde(default)]
    pub api_key: String,

    /// Model name
    #[serde(default = "default_openrouter_model")]
    pub model: String,

    /// Site URL for OpenRouter headers
    #[serde(default = "default_site_url")]
    pub site_url: String,

    /// App name for OpenRouter headers
    #[serde(default = "default_app_name")]
    pub app_name: String,
}

fn default_openrouter_model() -> String {
    "anthropic/claude-3.5-sonnet".to_string()
}
fn default_site_url() -> String {
    "https://github.com/Rcidshacker/Symphony".to_string()
}
fn default_app_name() -> String {
    "Symphony Music Player".to_string()
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openrouter_model(),
            site_url: default_site_url(),
            app_name: default_app_name(),
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider: "ollama".to_string(),
            ollama: OllamaConfig::default(),
            openrouter: OpenRouterConfig::default(),
            recommendations_enabled: true,
            natural_language_commands: true,
            cache_responses: true,
        }
    }
}

impl AiConfig {
    /// Get the AI provider configuration
    pub fn get_provider_config(&self) -> Result<crate::ai::AIProviderConfig, ConfigError> {
        match self.provider.as_str() {
            "ollama" => Ok(crate::ai::AIProviderConfig::Ollama {
                base_url: self.ollama.base_url.clone(),
                model: self.ollama.model.clone(),
            }),
            "openrouter" => {
                if self.openrouter.api_key.is_empty() {
                    return Err(ConfigError::InvalidConfig(
                        "OpenRouter API key not set. Get one from https://openrouter.ai/keys"
                            .to_string(),
                    ));
                }
                Ok(crate::ai::AIProviderConfig::OpenRouter {
                    api_key: self.openrouter.api_key.clone(),
                    model: self.openrouter.model.clone(),
                    site_url: Some(self.openrouter.site_url.clone()),
                    app_name: Some(self.openrouter.app_name.clone()),
                })
            }
            _ => Err(ConfigError::InvalidConfig(format!(
                "Unknown AI provider: {}. Use 'ollama' or 'openrouter'",
                self.provider
            ))),
        }
    }
}

/// Streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Enable streaming features
    #[serde(default)]
    pub enabled: bool,

    /// YouTube settings
    #[serde(default)]
    pub youtube: YouTubeStreamingConfig,

    /// Spotify settings
    #[serde(default)]
    pub spotify: SpotifyStreamingConfig,

    /// Cache settings
    #[serde(default)]
    pub cache: StreamingCacheConfig,

    /// Network settings
    #[serde(default)]
    pub network: NetworkConfig,
}

/// YouTube streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeStreamingConfig {
    /// Enable YouTube provider
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Default quality
    #[serde(default)]
    pub quality: String,

    /// yt-dlp path
    #[serde(default = "default_ytdlp_path")]
    pub ytdlp_path: String,
}

fn default_ytdlp_path() -> String {
    "yt-dlp".to_string()
}

impl Default for YouTubeStreamingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: "medium".to_string(),
            ytdlp_path: "yt-dlp".to_string(),
        }
    }
}

/// Spotify streaming configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyStreamingConfig {
    /// Enable Spotify provider
    #[serde(default)]
    pub enabled: bool,

    /// Spotify Client ID
    #[serde(default)]
    pub client_id: String,

    /// Spotify Client Secret
    #[serde(default)]
    pub client_secret: String,

    /// Redirect URI for OAuth
    #[serde(default = "default_spotify_redirect")]
    pub redirect_uri: String,

    /// Default quality
    #[serde(default)]
    pub quality: String,
}

fn default_spotify_redirect() -> String {
    "http://localhost:8888/callback".to_string()
}

impl Default for SpotifyStreamingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: default_spotify_redirect(),
            quality: "high".to_string(),
        }
    }
}

/// Streaming cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingCacheConfig {
    /// Enable caching
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Cache directory
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,

    /// Maximum hot tier size in MB
    #[serde(default = "default_hot_size")]
    pub max_hot_size_mb: u64,

    /// Maximum warm tier size in MB
    #[serde(default = "default_warm_size")]
    pub max_warm_size_mb: u64,

    /// Maximum cold tier size in MB
    #[serde(default = "default_cold_size")]
    pub max_cold_size_mb: u64,

    /// Enable predictive prefetching
    #[serde(default = "default_true")]
    pub prefetch_enabled: bool,

    /// Number of tracks to prefetch
    #[serde(default = "default_prefetch_count")]
    pub prefetch_count: usize,
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("symphony")
        .join("streams")
}

fn default_hot_size() -> u64 {
    100
}
fn default_warm_size() -> u64 {
    2048
}
fn default_cold_size() -> u64 {
    20480
}
fn default_prefetch_count() -> usize {
    3
}

impl Default for StreamingCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_dir: default_cache_dir(),
            max_hot_size_mb: default_hot_size(),
            max_warm_size_mb: default_warm_size(),
            max_cold_size_mb: default_cold_size(),
            prefetch_enabled: true,
            prefetch_count: default_prefetch_count(),
        }
    }
}

/// Network configuration for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Maximum concurrent downloads
    #[serde(default = "default_max_downloads")]
    pub max_concurrent_downloads: usize,

    /// Bandwidth limit in Mbps (0 = unlimited)
    #[serde(default)]
    pub bandwidth_limit_mbps: u32,

    /// Prefer cached content
    #[serde(default = "default_true")]
    pub prefer_cached: bool,

    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_max_downloads() -> usize {
    2
}
fn default_timeout() -> u64 {
    30
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: default_max_downloads(),
            bandwidth_limit_mbps: 0,
            prefer_cached: true,
            timeout_secs: default_timeout(),
        }
    }
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            youtube: YouTubeStreamingConfig::default(),
            spotify: SpotifyStreamingConfig::default(),
            cache: StreamingCacheConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

// =============================================================================
// Phase 5: Advanced Features Configuration
// =============================================================================

/// Plugins configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Enable plugin system
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Plugin directory
    #[serde(default = "default_plugin_dir")]
    pub plugin_dir: PathBuf,

    /// Auto-load plugins on startup
    #[serde(default)]
    pub auto_load: Vec<String>,

    /// Discord plugin settings
    #[serde(default)]
    pub discord: DiscordPluginConfig,

    /// Last.fm plugin settings
    #[serde(default)]
    pub lastfm: LastfmPluginConfig,

    /// Notifications plugin settings
    #[serde(default)]
    pub notifications: NotificationsPluginConfig,
}

fn default_plugin_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("symphony")
        .join("plugins")
}

/// Discord Rich Presence plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordPluginConfig {
    /// Enable Discord integration
    #[serde(default)]
    pub enabled: bool,

    /// Show timestamp
    #[serde(default = "default_true")]
    pub show_timestamp: bool,

    /// Show album art
    #[serde(default = "default_true")]
    pub show_album_art: bool,
}

impl Default for DiscordPluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_timestamp: true,
            show_album_art: true,
        }
    }
}

/// Last.fm plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastfmPluginConfig {
    /// Enable Last.fm integration
    #[serde(default)]
    pub enabled: bool,

    /// API key
    #[serde(default)]
    pub api_key: String,

    /// API secret
    #[serde(default)]
    pub api_secret: String,

    /// Username
    #[serde(default)]
    pub username: String,

    /// Scrobble threshold (percentage)
    #[serde(default = "default_scrobble_threshold")]
    pub scrobble_threshold: u8,
}

fn default_scrobble_threshold() -> u8 {
    50
}

impl Default for LastfmPluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            api_secret: String::new(),
            username: String::new(),
            scrobble_threshold: default_scrobble_threshold(),
        }
    }
}

/// Notifications plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsPluginConfig {
    /// Enable notifications
    #[serde(default)]
    pub enabled: bool,

    /// Notify on track change
    #[serde(default = "default_true")]
    pub on_track_change: bool,

    /// Notify on playlist end
    #[serde(default = "default_true")]
    pub on_playlist_end: bool,
}

impl Default for NotificationsPluginConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            on_track_change: true,
            on_playlist_end: true,
        }
    }
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            plugin_dir: default_plugin_dir(),
            auto_load: Vec::new(),
            discord: DiscordPluginConfig::default(),
            lastfm: LastfmPluginConfig::default(),
            notifications: NotificationsPluginConfig::default(),
        }
    }
}

/// Lyrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsConfig {
    /// Enable lyrics system
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Lyrics cache directory
    #[serde(default = "default_lyrics_cache_dir")]
    pub cache_dir: PathBuf,

    /// Auto-fetch lyrics
    #[serde(default = "default_true")]
    pub auto_fetch: bool,

    /// Show translation
    #[serde(default)]
    pub show_translation: bool,

    /// Karaoke mode
    #[serde(default = "default_true")]
    pub karaoke_mode: bool,

    /// Context lines before/after current
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
}

fn default_lyrics_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("symphony")
        .join("lyrics")
}

fn default_context_lines() -> usize {
    2
}

impl Default for LyricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cache_dir: default_lyrics_cache_dir(),
            auto_fetch: true,
            show_translation: false,
            karaoke_mode: true,
            context_lines: default_context_lines(),
        }
    }
}

/// Integrations configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationsConfig {
    /// Git integration settings
    #[serde(default)]
    pub git: GitIntegrationConfig,

    /// Pomodoro timer settings
    #[serde(default)]
    pub pomodoro: PomodoroConfig,

    /// Status line settings
    #[serde(default)]
    pub statusline: StatusLineConfig,
}

/// Git integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitIntegrationConfig {
    /// Enable Git integration
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Adapt music based on activity
    #[serde(default)]
    pub adapt_music: bool,

    /// Check interval in seconds
    #[serde(default = "default_git_check_interval")]
    pub check_interval: u64,
}

fn default_git_check_interval() -> u64 {
    60
}

impl Default for GitIntegrationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            adapt_music: false,
            check_interval: default_git_check_interval(),
        }
    }
}

/// Pomodoro timer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PomodoroConfig {
    /// Enable Pomodoro timer
    #[serde(default)]
    pub enabled: bool,

    /// Work session duration in minutes
    #[serde(default = "default_work_minutes")]
    pub work_minutes: u64,

    /// Break duration in minutes
    #[serde(default = "default_break_minutes")]
    pub break_minutes: u64,

    /// Long break duration in minutes
    #[serde(default = "default_long_break_minutes")]
    pub long_break_minutes: u64,

    /// Auto-start break
    #[serde(default)]
    pub auto_start_break: bool,

    /// Auto-start work
    #[serde(default)]
    pub auto_start_work: bool,

    /// Show notifications
    #[serde(default = "default_true")]
    pub notifications: bool,
}

fn default_work_minutes() -> u64 {
    25
}
fn default_break_minutes() -> u64 {
    5
}
fn default_long_break_minutes() -> u64 {
    15
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            work_minutes: default_work_minutes(),
            break_minutes: default_break_minutes(),
            long_break_minutes: default_long_break_minutes(),
            auto_start_break: false,
            auto_start_work: false,
            notifications: true,
        }
    }
}

/// Status line configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusLineConfig {
    /// Enable tmux status line
    #[serde(default)]
    pub tmux_enabled: bool,

    /// Enable zsh status line
    #[serde(default)]
    pub zsh_enabled: bool,

    /// Status file path
    #[serde(default)]
    pub status_file: String,
}

impl Default for StatusLineConfig {
    fn default() -> Self {
        Self {
            tmux_enabled: false,
            zsh_enabled: false,
            status_file: String::new(),
        }
    }
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            git: GitIntegrationConfig::default(),
            pomodoro: PomodoroConfig::default(),
            statusline: StatusLineConfig::default(),
        }
    }
}

/// Keybindings configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    pub play_pause: String,
    pub skip_next: String,
    pub skip_prev: String,
    pub volume_up: String,
    pub volume_down: String,
    pub search: String,
    pub ai_command: String,
    pub toggle_lyrics: String,
    pub toggle_karaoke: String,
    pub translate_lyrics: String,
    pub pomodoro_start: String,
    pub pomodoro_break: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            play_pause: "Space".to_string(),
            skip_next: "]".to_string(),
            skip_prev: "[".to_string(),
            volume_up: "+".to_string(),
            volume_down: "-".to_string(),
            search: "/".to_string(),
            ai_command: "a".to_string(),
            toggle_lyrics: "l".to_string(),
            toggle_karaoke: "K".to_string(),
            translate_lyrics: "T".to_string(),
            pomodoro_start: "P".to_string(),
            pomodoro_break: "B".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from file, or create default
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents =
            toml::to_string_pretty(self).map_err(|e| ConfigError::WriteError(e.to_string()))?;

        std::fs::write(&config_path, contents)?;

        Ok(())
    }

    /// Get configuration file path
    fn config_path() -> Result<PathBuf, ConfigError> {
        Ok(dirs::config_dir()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
            })?
            .join("symphony")
            .join("config.toml"))
    }

    /// Get the default configuration path
    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony")
            .join("config.toml")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            audio: AudioConfig::default(),
            visualizer: VisualizerConfig::default(),
            ai: AiConfig::default(),
            streaming: StreamingConfig::default(),
            plugins: PluginsConfig::default(),
            lyrics: LyricsConfig::default(),
            integrations: IntegrationsConfig::default(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.audio.volume, 0.6);
        assert_eq!(config.visualizer.fft_size, 4096);
        assert_eq!(config.ai.provider, "ollama");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(config.audio.volume, deserialized.audio.volume);
    }
}
