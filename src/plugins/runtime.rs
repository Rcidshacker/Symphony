//! WASM Plugin Runtime
//!
//! Manages the lifecycle of WASM plugins using Wasmtime.

use super::{PluginError, PluginEvent, PluginManifest, PluginStatus, TrackInfo};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Note: The actual wasmtime implementation would require the wasmtime crate.
// This is a simplified version that demonstrates the architecture.
// In production, uncomment the wasmtime dependency in Cargo.toml.

/// Plugin runtime state
#[derive(Debug, Default)]
struct PluginState {
    /// Plugin name
    name: String,
    /// Shared track info
    current_track: Option<TrackInfo>,
    /// Shared volume
    volume: f32,
    /// Memory buffer for passing data
    memory_buffer: Vec<u8>,
}

/// A loaded plugin
#[derive(Debug)]
struct LoadedPlugin {
    /// Plugin manifest
    manifest: PluginManifest,
    /// Plugin status
    status: PluginStatus,
    /// Path to plugin WASM file
    wasm_path: PathBuf,
    /// Whether the plugin is enabled
    enabled: bool,
    /// Plugin state data
    state: PluginState,
    /// Last error (if any)
    last_error: Option<String>,
}

/// Plugin runtime - manages WASM plugin lifecycle
pub struct PluginRuntime {
    /// Loaded plugins indexed by name
    plugins: HashMap<String, LoadedPlugin>,
    /// Plugin directory
    plugin_dir: PathBuf,
    /// Whether plugins are enabled globally
    enabled: bool,
    /// Host callback for Discord presence
    discord_callback: Option<Arc<dyn Fn(&str, &str, &str) + Send + Sync>>,
    /// Host callback for Last.fm scrobble
    lastfm_callback: Option<Arc<dyn Fn(&str, &str, &str) + Send + Sync>>,
    /// Host callback for notifications
    notify_callback: Option<Arc<dyn Fn(&str, &str) + Send + Sync>>,
}

impl std::fmt::Debug for PluginRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRuntime")
            .field("plugins", &self.plugins.keys().collect::<Vec<_>>())
            .field("plugin_dir", &self.plugin_dir)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl PluginRuntime {
    /// Create a new plugin runtime
    pub fn new(plugin_dir: PathBuf) -> Result<Self, PluginError> {
        // Ensure plugin directory exists
        if !plugin_dir.exists() {
            std::fs::create_dir_all(&plugin_dir)?;
        }

        Ok(Self {
            plugins: HashMap::new(),
            plugin_dir,
            enabled: true,
            discord_callback: None,
            lastfm_callback: None,
            notify_callback: None,
        })
    }

    /// Create runtime with default plugin directory
    pub fn with_defaults() -> Result<Self, PluginError> {
        let plugin_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony")
            .join("plugins");

        Self::new(plugin_dir)
    }

    /// Enable or disable plugins globally
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Scan plugin directory and load all plugins
    pub async fn load_all_plugins(&mut self) -> Result<Vec<String>, PluginError> {
        let mut loaded = Vec::new();

        if !self.plugin_dir.exists() {
            return Ok(loaded);
        }

        let entries = std::fs::read_dir(&self.plugin_dir)?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                // Look for plugin.toml
                let manifest_path = path.join("plugin.toml");
                let wasm_path = path.join("plugin.wasm");

                if manifest_path.exists() {
                    match self.load_plugin(&manifest_path, &wasm_path).await {
                        Ok(name) => {
                            loaded.push(name);
                        }
                        Err(e) => {
                            warn!("Failed to load plugin from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        info!("Loaded {} plugins", loaded.len());
        Ok(loaded)
    }

    /// Load a plugin from manifest and WASM file
    pub async fn load_plugin(
        &mut self,
        manifest_path: &Path,
        wasm_path: &Path,
    ) -> Result<String, PluginError> {
        // Read and parse manifest
        let manifest_content = std::fs::read_to_string(manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

        let name = manifest.name.clone();

        // Check if already loaded
        if self.plugins.contains_key(&name) {
            return Err(PluginError::InitFailed(format!(
                "Plugin {} already loaded",
                name
            )));
        }

        info!("Loading plugin: {} v{}", name, manifest.version);

        // Verify WASM file exists
        if !wasm_path.exists() {
            // Create placeholder for built-in plugins that don't have WASM yet
            let plugin = LoadedPlugin {
                manifest,
                status: PluginStatus::Active,
                wasm_path: wasm_path.to_path_buf(),
                enabled: true,
                state: PluginState {
                    name: name.clone(),
                    ..Default::default()
                },
                last_error: None,
            };

            self.plugins.insert(name.clone(), plugin);
            return Ok(name);
        }

        // In production, this would:
        // 1. Read WASM bytes
        // 2. Compile module with wasmtime
        // 3. Create linker with host functions
        // 4. Instantiate module
        // 5. Call plugin_init()

        let plugin = LoadedPlugin {
            manifest,
            status: PluginStatus::Active,
            wasm_path: wasm_path.to_path_buf(),
            enabled: true,
            state: PluginState {
                name: name.clone(),
                ..Default::default()
            },
            last_error: None,
        };

        self.plugins.insert(name.clone(), plugin);

        // Call plugin init
        self.call_plugin_init(&name)?;

        info!("Plugin {} loaded successfully", name);
        Ok(name)
    }

    /// Unload a plugin
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        if let Some(plugin) = self.plugins.remove(name) {
            info!("Unloaded plugin: {}", name);
            // In production, would call plugin_shutdown() first
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(name.to_string()))
        }
    }

    /// Reload a plugin (useful for development)
    pub async fn reload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        let manifest_path = plugin.wasm_path.parent().unwrap().join("plugin.toml");
        let wasm_path = plugin.wasm_path.clone();

        self.unload_plugin(name)?;
        self.load_plugin(&manifest_path, &wasm_path).await?;

        Ok(())
    }

    /// Trigger an event for all plugins
    pub fn trigger_event(&mut self, event: &PluginEvent) {
        if !self.enabled {
            return;
        }

        let event_json = match event.to_json() {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize event: {}", e);
                return;
            }
        };

        let plugin_names: Vec<String> = self.plugins.keys().cloned().collect();

        for name in plugin_names {
            let (enabled, status, has_permission) = if let Some(plugin) = self.plugins.get(&name) {
                (
                    plugin.enabled,
                    plugin.status,
                    self.has_event_permission(&plugin.manifest, event),
                )
            } else {
                continue;
            };

            if !enabled || status != PluginStatus::Active {
                continue;
            }

            if !has_permission {
                continue;
            }

            debug!("Sending event {:?} to plugin {}", event.event_type(), name);

            // Handle built-in plugin logic directly (simplified)
            self.handle_builtin_plugin_event(&name, event, &event_json);
        }
    }

    /// Check if plugin has permission for event
    fn has_event_permission(&self, manifest: &PluginManifest, event: &PluginEvent) -> bool {
        use super::Permission;

        let required_permission = match event {
            PluginEvent::TrackChanged(_) => Permission::TrackInfo,
            PluginEvent::PlaybackStateChanged(_) => Permission::PlaybackState,
            PluginEvent::VolumeChanged(_) => Permission::PlaybackState,
            PluginEvent::PlaylistChanged { .. } => Permission::Playlists,
            PluginEvent::TrackCompleted { .. } => Permission::TrackInfo,
            PluginEvent::SeekChanged { .. } => Permission::PlaybackState,
            PluginEvent::AppStarted | PluginEvent::AppStopped => return true,
            PluginEvent::Custom { .. } => return true,
        };

        manifest.has_permission(&required_permission)
    }

    /// Handle events for built-in plugins (simplified implementation)
    fn handle_builtin_plugin_event(&mut self, name: &str, event: &PluginEvent, _event_json: &str) {
        match name {
            "discord" => {
                if let PluginEvent::TrackChanged(track) = event {
                    if let Some(callback) = &self.discord_callback {
                        let album = track.album.as_deref().unwrap_or("");
                        callback(&track.title, &track.artist, album);
                    }
                }
            }
            "lastfm" => {
                if let PluginEvent::TrackCompleted { track, .. } = event {
                    if let Some(callback) = &self.lastfm_callback {
                        let album = track.album.as_deref().unwrap_or("");
                        callback(&track.title, &track.artist, album);
                    }
                }
            }
            "notifications" => {
                if let PluginEvent::TrackChanged(track) = event {
                    if let Some(callback) = &self.notify_callback {
                        callback(&track.title, &track.artist);
                    }
                }
            }
            _ => {}
        }
    }

    /// Call plugin initialization function
    fn call_plugin_init(&mut self, name: &str) -> Result<(), PluginError> {
        // In production, would call the plugin_init() exported function
        debug!("Initializing plugin: {}", name);
        Ok(())
    }

    /// Enable a plugin
    pub fn enable_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = true;
            plugin.status = PluginStatus::Active;
            info!("Enabled plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(name.to_string()))
        }
    }

    /// Disable a plugin
    pub fn disable_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        if let Some(plugin) = self.plugins.get_mut(name) {
            plugin.enabled = false;
            plugin.status = PluginStatus::Disabled;
            info!("Disabled plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(name.to_string()))
        }
    }

    /// Get list of loaded plugins
    pub fn list_plugins(&self) -> Vec<(&String, &PluginManifest, PluginStatus)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| (name, &plugin.manifest, plugin.status))
            .collect()
    }

    /// Get plugin status
    pub fn get_plugin_status(&self, name: &str) -> Option<PluginStatus> {
        self.plugins.get(name).map(|p| p.status)
    }

    /// Check if a plugin is enabled
    pub fn is_plugin_enabled(&self, name: &str) -> bool {
        self.plugins
            .get(name)
            .map(|p| p.enabled && p.status == PluginStatus::Active)
            .unwrap_or(false)
    }

    /// Get plugin manifest
    pub fn get_plugin_manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name).map(|p| &p.manifest)
    }

    /// Set Discord presence callback
    pub fn set_discord_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, &str, &str) + Send + Sync + 'static,
    {
        self.discord_callback = Some(Arc::new(callback));
    }

    /// Set Last.fm scrobble callback
    pub fn set_lastfm_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, &str, &str) + Send + Sync + 'static,
    {
        self.lastfm_callback = Some(Arc::new(callback));
    }

    /// Set notification callback
    pub fn set_notify_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.notify_callback = Some(Arc::new(callback));
    }

    /// Update shared track info (called by host)
    pub fn update_track_info(&mut self, track: TrackInfo) {
        for plugin in self.plugins.values_mut() {
            plugin.state.current_track = Some(track.clone());
        }
    }

    /// Update shared volume (called by host)
    pub fn update_volume(&mut self, volume: f32) {
        for plugin in self.plugins.values_mut() {
            plugin.state.volume = volume;
        }
    }
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::with_defaults().expect("Failed to create default plugin runtime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_runtime_creation() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = PluginRuntime::new(dir.path().to_path_buf());
        assert!(runtime.is_ok());
    }

    #[test]
    fn test_plugin_enable_disable() {
        let mut runtime = PluginRuntime::with_defaults().unwrap();

        // Create a test plugin
        let manifest = PluginManifest {
            name: "test".to_string(),
            version: "1.0".to_string(),
            author: "test".to_string(),
            description: "test".to_string(),
            permissions: vec![],
            homepage: None,
            min_symphony_version: None,
            enabled: true,
        };

        runtime.plugins.insert(
            "test".to_string(),
            LoadedPlugin {
                manifest,
                status: PluginStatus::Active,
                wasm_path: PathBuf::from("test.wasm"),
                enabled: true,
                state: PluginState::default(),
                last_error: None,
            },
        );

        assert!(runtime.is_plugin_enabled("test"));

        runtime.disable_plugin("test").unwrap();
        assert!(!runtime.is_plugin_enabled("test"));

        runtime.enable_plugin("test").unwrap();
        assert!(runtime.is_plugin_enabled("test"));
    }
}
