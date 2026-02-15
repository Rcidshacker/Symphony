//! Plugin API and Host Functions
//!
//! Defines the interface between plugins and the host application.

use super::{Permission, PluginError, PluginManifest, TrackInfo};
use std::collections::HashMap;
use std::sync::Arc;

/// Host functions available to plugins
///
/// These functions are imported by the plugin and implemented by the host.
/// Plugins call these to interact with Symphony.
pub struct HostFunctions {
    /// Callbacks for host functions
    callbacks: HostCallbacks,
}

/// Host function callbacks
struct HostCallbacks {
    /// Log a message
    log: Arc<dyn Fn(&str) + Send + Sync>,
    /// Get current track info
    get_track: Arc<dyn Fn() -> Option<TrackInfo> + Send + Sync>,
    /// Get current volume
    get_volume: Arc<dyn Fn() -> f32 + Send + Sync>,
    /// Set Discord presence
    set_discord_presence: Arc<dyn Fn(&str, &str, &str, u64) + Send + Sync>,
    /// Scrobble to Last.fm
    scrobble: Arc<dyn Fn(&str, &str, &str) + Send + Sync>,
    /// Update now playing on Last.fm
    update_now_playing: Arc<dyn Fn(&str, &str, &str) + Send + Sync>,
    /// Show notification
    show_notification: Arc<dyn Fn(&str, &str) + Send + Sync>,
    /// Make HTTP request
    http_request: Arc<dyn Fn(&str, &str, &str) -> Result<String, String> + Send + Sync>,
    /// Get configuration value
    get_config: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
    /// Set configuration value
    set_config: Arc<dyn Fn(&str, &str) + Send + Sync>,
}

impl std::fmt::Debug for HostFunctions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostFunctions").finish()
    }
}

impl HostFunctions {
    /// Create host functions with default (no-op) callbacks
    pub fn new() -> Self {
        Self {
            callbacks: HostCallbacks {
                log: Arc::new(|msg| println!("[Plugin] {}", msg)),
                get_track: Arc::new(|| None),
                get_volume: Arc::new(|| 0.6),
                set_discord_presence: Arc::new(|_, _, _, _| {}),
                scrobble: Arc::new(|_, _, _| {}),
                update_now_playing: Arc::new(|_, _, _| {}),
                show_notification: Arc::new(|_, _| {}),
                http_request: Arc::new(|_, _, _| Err("HTTP not implemented".to_string())),
                get_config: Arc::new(|_| None),
                set_config: Arc::new(|_, _| {}),
            },
        }
    }

    /// Set log callback
    pub fn with_log<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.callbacks.log = Arc::new(callback);
        self
    }

    /// Set get_track callback
    pub fn with_get_track<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> Option<TrackInfo> + Send + Sync + 'static,
    {
        self.callbacks.get_track = Arc::new(callback);
        self
    }

    /// Set get_volume callback
    pub fn with_get_volume<F>(mut self, callback: F) -> Self
    where
        F: Fn() -> f32 + Send + Sync + 'static,
    {
        self.callbacks.get_volume = Arc::new(callback);
        self
    }

    /// Set Discord presence callback
    pub fn with_discord<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str, u64) + Send + Sync + 'static,
    {
        self.callbacks.set_discord_presence = Arc::new(callback);
        self
    }

    /// Set Last.fm scrobble callback
    pub fn with_scrobble<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str) + Send + Sync + 'static,
    {
        self.callbacks.scrobble = Arc::new(callback);
        self
    }

    /// Set now playing callback
    pub fn with_now_playing<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str) + Send + Sync + 'static,
    {
        self.callbacks.update_now_playing = Arc::new(callback);
        self
    }

    /// Set notification callback
    pub fn with_notification<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.callbacks.show_notification = Arc::new(callback);
        self
    }

    /// Set HTTP request callback
    pub fn with_http<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, &str) -> Result<String, String> + Send + Sync + 'static,
    {
        self.callbacks.http_request = Arc::new(callback);
        self
    }

    // --- Host function implementations ---

    /// Log a message from a plugin
    pub fn log(&self, message: &str) {
        (self.callbacks.log)(message);
    }

    /// Get current track info
    pub fn get_track(&self) -> Option<TrackInfo> {
        (self.callbacks.get_track)()
    }

    /// Get current volume
    pub fn get_volume(&self) -> f32 {
        (self.callbacks.get_volume)()
    }

    /// Set Discord Rich Presence
    pub fn set_discord_presence(
        &self,
        title: &str,
        artist: &str,
        album: &str,
        duration_secs: u64,
    ) {
        (self.callbacks.set_discord_presence)(title, artist, album, duration_secs);
    }

    /// Scrobble to Last.fm
    pub fn scrobble(&self, title: &str, artist: &str, album: &str) {
        (self.callbacks.scrobble)(title, artist, album);
    }

    /// Update now playing on Last.fm
    pub fn update_now_playing(&self, title: &str, artist: &str, album: &str) {
        (self.callbacks.update_now_playing)(title, artist, album);
    }

    /// Show a notification
    pub fn show_notification(&self, title: &str, body: &str) {
        (self.callbacks.show_notification)(title, body);
    }

    /// Make an HTTP request
    pub fn http_request(
        &self,
        method: &str,
        url: &str,
        body: &str,
    ) -> Result<String, PluginError> {
        (self.callbacks.http_request)(method, url, body)
            .map_err(PluginError::HostFunctionError)
    }

    /// Get a configuration value
    pub fn get_config(&self, key: &str) -> Option<String> {
        (self.callbacks.get_config)(key)
    }

    /// Set a configuration value
    pub fn set_config(&self, key: &str, value: &str) {
        (self.callbacks.set_config)(key, value);
    }
}

impl Default for HostFunctions {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin API - interface for plugins to call host functions
///
/// This is the Rust API that plugins would use.
/// For WASM plugins, these are exposed as C functions.
pub struct PluginApi {
    manifest: PluginManifest,
    permissions: Vec<Permission>,
}

impl PluginApi {
    /// Create a new plugin API instance
    pub fn new(manifest: PluginManifest) -> Self {
        let permissions = manifest.permissions.clone();
        Self { manifest, permissions }
    }

    /// Check if plugin has a permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Get plugin name
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Get plugin version
    pub fn version(&self) -> &str {
        &self.manifest.version
    }
}

/// WASM plugin API macros
///
/// These macros help plugin authors define the required exports.
#[macro_export]
macro_rules! declare_plugin {
    ($name:literal, $init:expr, $on_event:expr) => {
        #[no_mangle]
        pub extern "C" fn plugin_init() {
            $init();
        }

        #[no_mangle]
        pub extern "C" fn on_event(event_ptr: i32, event_len: i32) {
            $on_event(event_ptr, event_len);
        }

        #[no_mangle]
        pub extern "C" fn plugin_name() -> *const u8 {
            $name.as_ptr()
        }

        #[no_mangle]
        pub extern "C" fn plugin_name_len() -> i32 {
            $name.len() as i32
        }
    };
}

/// Plugin configuration stored per-plugin
#[derive(Debug, Clone, Default)]
pub struct PluginConfig {
    /// Configuration key-value pairs
    values: HashMap<String, String>,
}

impl PluginConfig {
    /// Create a new plugin config
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Get a config value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.values.get(key)
    }

    /// Set a config value
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    /// Remove a config value
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.values.remove(key)
    }

    /// Check if key exists
    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Get all keys
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_functions_creation() {
        let host = HostFunctions::new();
        assert_eq!(host.get_volume(), 0.6);
    }

    #[test]
    fn test_host_functions_custom() {
        let host = HostFunctions::new()
            .with_get_volume(|| 0.8);

        assert_eq!(host.get_volume(), 0.8);
    }

    #[test]
    fn test_plugin_config() {
        let mut config = PluginConfig::new();
        config.set("api_key", "test123");

        assert_eq!(config.get("api_key"), Some(&"test123".to_string()));
        assert!(config.contains("api_key"));
        assert!(!config.contains("missing"));
    }
}
