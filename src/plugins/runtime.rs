//! WASM plugin runtime
//!
//! Every plugin is a WebAssembly module run by the `wasmi` interpreter, one sandboxed instance
//! per plugin. A plugin can only touch the outside world through the host functions below, each
//! of which checks the permissions declared in its `plugin.toml`. Calls are metered (a plugin
//! stuck in a loop is stopped and marked as failed) and memory is capped.
//!
//! # Plugin ABI (v1)
//!
//! Exports the plugin provides:
//! - `memory` (required)
//! - `alloc(len: i32) -> i32` (required for `on_event`): returns a buffer in `memory` for the host to fill
//! - `on_event(ptr: i32, len: i32)`: receives one event as JSON (see [`PluginEvent`])
//! - `plugin_init()` / `plugin_shutdown()`: optional lifecycle hooks
//!
//! Host functions in the `symphony` import module (strings are `ptr, len` pairs in plugin memory;
//! functions returning `i32` give `0` on success and `-1` when the permission is missing):
//! - `log(ptr, len)`
//! - `get_track(buf_ptr, buf_cap) -> i32`: current track as JSON, returns its length (`track_info`; 0 = nothing playing, -2 = buffer too small)
//! - `get_volume() -> f32`: volume 0.0-1.0, or -1.0 without permission (`playback_state`)
//! - `notify(title_ptr, title_len, body_ptr, body_len) -> i32` (`notifications`)
//! - `discord_presence(title_ptr, title_len, artist_ptr, artist_len) -> i32` (`discord`)
//! - `scrobble(title_ptr, title_len, artist_ptr, artist_len) -> i32` (`lastfm`)

use super::{HostFunctions, Permission, PluginError, PluginEvent, PluginManifest, PluginStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use wasmi::{Caller, Config, Engine, Extern, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Instructions a plugin may run per call before it is stopped
const FUEL_PER_CALL: u64 = 20_000_000;
/// Linear memory a plugin may use
const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;
/// Longest string a plugin may pass to the host
const MAX_STRING_BYTES: i32 = 64 * 1024;

/// What host functions see of the calling plugin
struct HostState {
    permissions: Vec<Permission>,
    host: Arc<HostFunctions>,
    /// Host calls refused for lack of a permission
    denied: u32,
    limits: StoreLimits,
}

/// A plugin's wasm instance
struct Guest {
    store: Store<HostState>,
    instance: Instance,
    memory: Memory,
    has_on_event: bool,
}

/// A loaded plugin
struct LoadedPlugin {
    manifest: PluginManifest,
    status: PluginStatus,
    wasm_path: PathBuf,
    enabled: bool,
    guest: Guest,
    last_error: Option<String>,
}

/// Plugin runtime - manages WASM plugin lifecycle
pub struct PluginRuntime {
    plugins: HashMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
    enabled: bool,
    /// Load only these plugins from the directory (all when empty)
    auto_load: Vec<String>,
    engine: Engine,
    linker: Linker<HostState>,
    host: Arc<HostFunctions>,
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
        if !plugin_dir.exists() {
            std::fs::create_dir_all(&plugin_dir)?;
        }

        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let mut linker = Linker::new(&engine);
        define_host_functions(&mut linker)?;

        Ok(Self {
            plugins: HashMap::new(),
            plugin_dir,
            enabled: true,
            auto_load: Vec::new(),
            engine,
            linker,
            host: Arc::new(HostFunctions::new()),
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

    /// Connect plugins to the application (affects plugins loaded afterwards)
    pub fn set_host(&mut self, host: HostFunctions) {
        self.host = Arc::new(host);
    }

    /// Only load the named plugins from the directory (all when empty)
    pub fn set_auto_load(&mut self, names: Vec<String>) {
        self.auto_load = names;
    }

    /// Enable or disable plugins globally
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Load every plugin folder (`plugin.toml` + `plugin.wasm`) in the plugin directory.
    /// A plugin that fails to load is skipped with a warning.
    pub async fn load_all_plugins(&mut self) -> Result<Vec<String>, PluginError> {
        let mut loaded = Vec::new();
        if !self.plugin_dir.exists() {
            return Ok(loaded);
        }

        for entry in std::fs::read_dir(&self.plugin_dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            let manifest_path = path.join("plugin.toml");
            if !path.is_dir() || !manifest_path.exists() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().to_string();
            if !self.auto_load.is_empty() && !self.auto_load.contains(&folder) {
                continue;
            }
            match self.load_plugin(&manifest_path, &path.join("plugin.wasm")).await {
                Ok(name) => loaded.push(name),
                Err(e) => warn!("Failed to load plugin from {:?}: {}", path, e),
            }
        }

        info!("Loaded {} plugins", loaded.len());
        Ok(loaded)
    }

    /// Load a plugin from its manifest and WASM file
    pub async fn load_plugin(
        &mut self,
        manifest_path: &Path,
        wasm_path: &Path,
    ) -> Result<String, PluginError> {
        let manifest_content = std::fs::read_to_string(manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&manifest_content)
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
        let name = manifest.name.clone();

        if self.plugins.contains_key(&name) {
            return Err(PluginError::InitFailed(format!("Plugin {name} already loaded")));
        }
        info!("Loading plugin: {} v{}", name, manifest.version);

        let bytes = std::fs::read(wasm_path)?;
        let guest = self.instantiate(&manifest, &bytes)?;

        let enabled = manifest.enabled;
        self.plugins.insert(
            name.clone(),
            LoadedPlugin {
                manifest,
                status: if enabled { PluginStatus::Active } else { PluginStatus::Disabled },
                wasm_path: wasm_path.to_path_buf(),
                enabled,
                guest,
                last_error: None,
            },
        );

        // plugin_init runs once; a plugin whose init traps is kept but marked failed
        if let Some(plugin) = self.plugins.get_mut(&name) {
            if let Err(e) = call_hook(&mut plugin.guest, "plugin_init") {
                plugin.status = PluginStatus::Error;
                plugin.last_error = Some(e);
            }
        }

        info!("Plugin {} loaded", name);
        Ok(name)
    }

    /// Compile and instantiate a plugin's module
    fn instantiate(&self, manifest: &PluginManifest, bytes: &[u8]) -> Result<Guest, PluginError> {
        let wasm_err = |e: &dyn std::fmt::Display| PluginError::WasmError(e.to_string());

        let module = Module::new(&self.engine, bytes).map_err(|e| wasm_err(&e))?;
        let mut store = Store::new(
            &self.engine,
            HostState {
                permissions: manifest.permissions.clone(),
                host: self.host.clone(),
                denied: 0,
                limits: StoreLimitsBuilder::new()
                    .memory_size(MAX_MEMORY_BYTES)
                    .instances(1)
                    .build(),
            },
        );
        store.limiter(|state| &mut state.limits);
        store.set_fuel(FUEL_PER_CALL).map_err(|e| wasm_err(&e))?;

        let instance = self
            .linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| wasm_err(&e))?;
        let memory = instance
            .get_export(&store, "memory")
            .and_then(Extern::into_memory)
            .ok_or_else(|| PluginError::WasmError("plugin must export `memory`".to_string()))?;

        let has_on_event = instance.get_export(&store, "on_event").is_some();
        if has_on_event && instance.get_typed_func::<i32, i32>(&store, "alloc").is_err() {
            return Err(PluginError::WasmError(
                "plugins with `on_event` must export `alloc(i32) -> i32`".to_string(),
            ));
        }

        Ok(Guest { store, instance, memory, has_on_event })
    }

    /// Unload a plugin, giving it a chance to clean up
    pub fn unload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let mut plugin = self
            .plugins
            .remove(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        if plugin.status == PluginStatus::Active {
            if let Err(e) = call_hook(&mut plugin.guest, "plugin_shutdown") {
                warn!("plugin_shutdown of {name} failed: {e}");
            }
        }
        info!("Unloaded plugin: {}", name);
        Ok(())
    }

    /// Reload a plugin (useful for development)
    pub async fn reload_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        let wasm_path = plugin.wasm_path.clone();
        let manifest_path = wasm_path.with_file_name("plugin.toml");

        self.unload_plugin(name)?;
        self.load_plugin(&manifest_path, &wasm_path).await?;
        Ok(())
    }

    /// Send an event to every enabled plugin that has the permission for it.
    /// A plugin that traps or runs out of fuel is marked failed and gets no more events.
    pub fn trigger_event(&mut self, event: &PluginEvent) {
        if !self.enabled {
            return;
        }
        let json = match event.to_json() {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize event: {}", e);
                return;
            }
        };

        for (name, plugin) in self.plugins.iter_mut() {
            if !plugin.enabled
                || plugin.status != PluginStatus::Active
                || !plugin.guest.has_on_event
                || !has_event_permission(&plugin.manifest, event)
            {
                continue;
            }
            debug!("Sending event {:?} to plugin {}", event.event_type(), name);
            if let Err(e) = deliver(&mut plugin.guest, &json) {
                warn!("Plugin {name} failed on {}: {e}", event.event_type());
                plugin.status = PluginStatus::Error;
                plugin.last_error = Some(e);
            }
        }
    }

    /// Enable a plugin
    pub fn enable_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        plugin.enabled = true;
        plugin.status = PluginStatus::Active;
        plugin.last_error = None;
        info!("Enabled plugin: {}", name);
        Ok(())
    }

    /// Disable a plugin
    pub fn disable_plugin(&mut self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;
        plugin.enabled = false;
        plugin.status = PluginStatus::Disabled;
        info!("Disabled plugin: {}", name);
        Ok(())
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

    /// Why a plugin was stopped, if it was
    pub fn plugin_error(&self, name: &str) -> Option<&str> {
        self.plugins.get(name).and_then(|p| p.last_error.as_deref())
    }

    /// How many host calls the plugin made without having the permission
    pub fn denied_calls(&self, name: &str) -> u32 {
        self.plugins.get(name).map_or(0, |p| p.guest.store.data().denied)
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
}

impl Drop for PluginRuntime {
    fn drop(&mut self) {
        for plugin in self.plugins.values_mut() {
            if plugin.status == PluginStatus::Active {
                // a plugin that cannot shut down cleanly is dropped regardless
                let _ = call_hook(&mut plugin.guest, "plugin_shutdown");
            }
        }
    }
}

/// Permission an event needs
fn has_event_permission(manifest: &PluginManifest, event: &PluginEvent) -> bool {
    let required = match event {
        PluginEvent::TrackChanged(_) | PluginEvent::TrackCompleted { .. } => Permission::TrackInfo,
        PluginEvent::PlaybackStateChanged(_)
        | PluginEvent::VolumeChanged(_)
        | PluginEvent::SeekChanged { .. } => Permission::PlaybackState,
        PluginEvent::PlaylistChanged { .. } => Permission::Playlists,
        PluginEvent::AppStarted | PluginEvent::AppStopped | PluginEvent::Custom { .. } => return true,
    };
    manifest.has_permission(&required)
}

/// Call an optional exported `fn()` hook; a missing hook is fine
fn call_hook(guest: &mut Guest, name: &str) -> Result<(), String> {
    let Ok(hook) = guest.instance.get_typed_func::<(), ()>(&guest.store, name) else {
        return Ok(());
    };
    guest.store.set_fuel(FUEL_PER_CALL).map_err(|e| e.to_string())?;
    hook.call(&mut guest.store, ()).map_err(|e| e.to_string())
}

/// Copy an event into plugin memory and call `on_event`
fn deliver(guest: &mut Guest, json: &str) -> Result<(), String> {
    let len = i32::try_from(json.len()).map_err(|_| "event too large".to_string())?;
    let alloc = guest
        .instance
        .get_typed_func::<i32, i32>(&guest.store, "alloc")
        .map_err(|e| e.to_string())?;
    let on_event = guest
        .instance
        .get_typed_func::<(i32, i32), ()>(&guest.store, "on_event")
        .map_err(|e| e.to_string())?;

    guest.store.set_fuel(FUEL_PER_CALL).map_err(|e| e.to_string())?;
    let ptr = alloc.call(&mut guest.store, len).map_err(|e| e.to_string())?;
    let offset = usize::try_from(ptr).map_err(|_| "alloc returned a negative pointer".to_string())?;
    guest
        .memory
        .write(&mut guest.store, offset, json.as_bytes())
        .map_err(|e| format!("alloc returned a bad buffer: {e}"))?;
    on_event.call(&mut guest.store, (ptr, len)).map_err(|e| e.to_string())
}

/// The calling plugin's memory
fn memory_of(caller: &Caller<'_, HostState>) -> Option<Memory> {
    caller.get_export("memory").and_then(Extern::into_memory)
}

/// A string from plugin memory (bounded, lossy UTF-8)
fn read_str(caller: &Caller<'_, HostState>, ptr: i32, len: i32) -> Option<String> {
    if ptr < 0 || !(0..=MAX_STRING_BYTES).contains(&len) {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    memory_of(caller)?.read(caller, ptr as usize, &mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Whether the plugin holds `permission`; a refusal is counted and logged
fn allowed(caller: &mut Caller<'_, HostState>, permission: Permission) -> bool {
    if caller.data().permissions.contains(&permission) {
        return true;
    }
    caller.data_mut().denied += 1;
    warn!("plugin tried to use a host function without the `{permission}` permission");
    false
}

fn define_host_functions(linker: &mut Linker<HostState>) -> Result<(), PluginError> {
    let link_err = |e: wasmi::errors::LinkerError| PluginError::WasmError(e.to_string());

    linker
        .func_wrap("symphony", "log", |caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            if let Some(message) = read_str(&caller, ptr, len) {
                caller.data().host.log(&message);
            }
        })
        .map_err(link_err)?;

    linker
        .func_wrap(
            "symphony",
            "get_track",
            |mut caller: Caller<'_, HostState>, buf: i32, cap: i32| -> i32 {
                if !allowed(&mut caller, Permission::TrackInfo) {
                    return -1;
                }
                let Some(track) = caller.data().host.get_track() else {
                    return 0;
                };
                let Ok(json) = serde_json::to_vec(&track) else {
                    return 0;
                };
                if buf < 0 || cap < 0 || json.len() > cap as usize {
                    return -2;
                }
                let Some(memory) = memory_of(&caller) else {
                    return -2;
                };
                match memory.write(&mut caller, buf as usize, &json) {
                    Ok(()) => json.len() as i32,
                    Err(_) => -2,
                }
            },
        )
        .map_err(link_err)?;

    linker
        .func_wrap("symphony", "get_volume", |mut caller: Caller<'_, HostState>| -> f32 {
            if !allowed(&mut caller, Permission::PlaybackState) {
                return -1.0;
            }
            caller.data().host.get_volume()
        })
        .map_err(link_err)?;

    linker
        .func_wrap(
            "symphony",
            "notify",
            |mut caller: Caller<'_, HostState>, tp: i32, tl: i32, bp: i32, bl: i32| -> i32 {
                if !allowed(&mut caller, Permission::Notifications) {
                    return -1;
                }
                if let (Some(title), Some(body)) = (read_str(&caller, tp, tl), read_str(&caller, bp, bl)) {
                    caller.data().host.show_notification(&title, &body);
                }
                0
            },
        )
        .map_err(link_err)?;

    linker
        .func_wrap(
            "symphony",
            "discord_presence",
            |mut caller: Caller<'_, HostState>, tp: i32, tl: i32, ap: i32, al: i32| -> i32 {
                if !allowed(&mut caller, Permission::Discord) {
                    return -1;
                }
                if let (Some(title), Some(artist)) = (read_str(&caller, tp, tl), read_str(&caller, ap, al)) {
                    caller.data().host.set_discord_presence(&title, &artist, "", 0);
                }
                0
            },
        )
        .map_err(link_err)?;

    linker
        .func_wrap(
            "symphony",
            "scrobble",
            |mut caller: Caller<'_, HostState>, tp: i32, tl: i32, ap: i32, al: i32| -> i32 {
                if !allowed(&mut caller, Permission::Lastfm) {
                    return -1;
                }
                if let (Some(title), Some(artist)) = (read_str(&caller, tp, tl), read_str(&caller, ap, al)) {
                    caller.data().host.scrobble(&title, &artist, "");
                }
                0
            },
        )
        .map_err(link_err)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_runtime_creation() {
        let dir = tempfile::tempdir().unwrap();
        assert!(PluginRuntime::new(dir.path().to_path_buf()).is_ok());
    }
}
