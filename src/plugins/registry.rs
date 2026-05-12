//! Plugin Registry
//!
//! Manages plugin discovery, installation, and updates.

use super::{PluginError, PluginManifest, PluginRuntime, PluginStatus};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Plugin registry for managing plugins
pub struct PluginRegistry {
    /// Plugin directory
    plugin_dir: PathBuf,
    /// Available plugins (from directory scan)
    available: HashMap<String, PluginInfo>,
    /// Official plugins
    official: Vec<OfficialPlugin>,
}

/// Information about an available plugin
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Author name
    pub author: String,
    /// Description
    pub description: String,
    /// Path to plugin directory
    pub path: PathBuf,
    /// Whether installed
    pub installed: bool,
    /// Whether this is an official plugin
    pub official: bool,
    /// Repository URL
    pub repository: Option<String>,
}

/// Official plugin definition
#[derive(Debug, Clone)]
pub struct OfficialPlugin {
    /// Plugin ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Default enabled
    pub default_enabled: bool,
    /// Required permissions
    pub permissions: Vec<String>,
    /// Whether it needs configuration
    pub needs_config: bool,
    /// Configuration fields
    pub config_fields: Vec<ConfigField>,
}

/// Configuration field definition
#[derive(Debug, Clone)]
pub struct ConfigField {
    /// Field key
    pub key: String,
    /// Display label
    pub label: String,
    /// Field type
    pub field_type: ConfigFieldType,
    /// Is required
    pub required: bool,
    /// Default value
    pub default: Option<String>,
    /// Help text
    pub help: Option<String>,
}

/// Configuration field types
#[derive(Debug, Clone)]
pub enum ConfigFieldType {
    /// Single line text
    Text,
    /// Password/API key (masked)
    Secret,
    /// Boolean toggle
    Boolean,
    /// Number
    Number,
    /// Select from options
    Select { options: Vec<String> },
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new(plugin_dir: PathBuf) -> Result<Self, PluginError> {
        let mut registry = Self {
            plugin_dir,
            available: HashMap::new(),
            official: Self::get_official_plugins(),
        };

        registry.scan_plugins()?;

        Ok(registry)
    }

    /// Create registry with defaults
    pub fn with_defaults() -> Result<Self, PluginError> {
        let plugin_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("symphony")
            .join("plugins");

        Self::new(plugin_dir)
    }

    /// Get list of official plugins
    fn get_official_plugins() -> Vec<OfficialPlugin> {
        vec![
            OfficialPlugin {
                id: "discord".to_string(),
                name: "Discord Rich Presence".to_string(),
                description: "Show what you're listening to on Discord".to_string(),
                default_enabled: false,
                permissions: vec!["track_info".to_string(), "discord".to_string()],
                needs_config: false,
                config_fields: vec![],
            },
            OfficialPlugin {
                id: "lastfm".to_string(),
                name: "Last.fm Scrobbler".to_string(),
                description: "Scrobble your plays to Last.fm".to_string(),
                default_enabled: false,
                permissions: vec!["track_info".to_string(), "lastfm".to_string(), "network".to_string()],
                needs_config: true,
                config_fields: vec![
                    ConfigField {
                        key: "api_key".to_string(),
                        label: "API Key".to_string(),
                        field_type: ConfigFieldType::Secret,
                        required: true,
                        default: None,
                        help: Some("Get your API key from https://www.last.fm/api/account/create".to_string()),
                    },
                    ConfigField {
                        key: "api_secret".to_string(),
                        label: "API Secret".to_string(),
                        field_type: ConfigFieldType::Secret,
                        required: true,
                        default: None,
                        help: None,
                    },
                    ConfigField {
                        key: "username".to_string(),
                        label: "Username".to_string(),
                        field_type: ConfigFieldType::Text,
                        required: true,
                        default: None,
                        help: None,
                    },
                ],
            },
            OfficialPlugin {
                id: "notifications".to_string(),
                name: "Desktop Notifications".to_string(),
                description: "Show notifications when tracks change".to_string(),
                default_enabled: false,
                permissions: vec!["track_info".to_string(), "notifications".to_string()],
                needs_config: false,
                config_fields: vec![],
            },
            OfficialPlugin {
                id: "stats".to_string(),
                name: "Listening Statistics".to_string(),
                description: "Track and visualize your listening habits".to_string(),
                default_enabled: true,
                permissions: vec!["track_info".to_string()],
                needs_config: false,
                config_fields: vec![],
            },
        ]
    }

    /// Scan plugin directory for available plugins
    pub fn scan_plugins(&mut self) -> Result<(), PluginError> {
        self.available.clear();

        // Ensure directory exists
        if !self.plugin_dir.exists() {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&self.plugin_dir)?;
            return Ok(());
        }

        // Scan plugin directories
        for entry in std::fs::read_dir(&self.plugin_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let manifest_path = path.join("plugin.toml");

                if manifest_path.exists() {
                    match self.load_plugin_info(&manifest_path, &path) {
                        Ok(info) => {
                            self.available.insert(info.name.clone(), info);
                        }
                        Err(e) => {
                            warn!("Failed to load plugin info from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        // Add official plugins not yet installed
        for official in &self.official {
            if !self.available.contains_key(&official.id) {
                self.available.insert(
                    official.id.clone(),
                    PluginInfo {
                        name: official.id.clone(),
                        version: "bundled".to_string(),
                        author: "Symphony".to_string(),
                        description: official.description.clone(),
                        path: self.plugin_dir.join(&official.id),
                        installed: false,
                        official: true,
                        repository: None,
                    },
                );
            } else {
                // Mark as official and installed
                if let Some(info) = self.available.get_mut(&official.id) {
                    info.official = true;
                    info.installed = true;
                }
            }
        }

        info!("Found {} available plugins", self.available.len());
        Ok(())
    }

    /// Load plugin info from manifest
    fn load_plugin_info(
        &self,
        manifest_path: &Path,
        plugin_dir: &Path,
    ) -> Result<PluginInfo, PluginError> {
        let content = std::fs::read_to_string(manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&content)
            .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

        Ok(PluginInfo {
            name: manifest.name,
            version: manifest.version,
            author: manifest.author,
            description: manifest.description,
            path: plugin_dir.to_path_buf(),
            installed: true,
            official: false,
            repository: manifest.homepage,
        })
    }

    /// Get list of all available plugins
    pub fn list_available(&self) -> Vec<&PluginInfo> {
        self.available.values().collect()
    }

    /// Get list of installed plugins
    pub fn list_installed(&self) -> Vec<&PluginInfo> {
        self.available.values().filter(|p| p.installed).collect()
    }

    /// Get list of official plugins
    pub fn list_official(&self) -> &[OfficialPlugin] {
        &self.official
    }

    /// Get plugin info by name
    pub fn get_plugin(&self, name: &str) -> Option<&PluginInfo> {
        self.available.get(name)
    }

    /// Get official plugin by ID
    pub fn get_official_plugin(&self, id: &str) -> Option<&OfficialPlugin> {
        self.official.iter().find(|p| p.id == id)
    }

    /// Check if plugin is installed
    pub fn is_installed(&self, name: &str) -> bool {
        self.available.get(name).map(|p| p.installed).unwrap_or(false)
    }

    /// Install an official plugin
    pub fn install_official(&mut self, id: &str) -> Result<(), PluginError> {
        let official = self
            .official
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| PluginError::PluginNotFound(id.to_string()))?;

        // Create plugin directory
        let plugin_dir = self.plugin_dir.join(id);
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&plugin_dir)?;

        // Create manifest
        let manifest = PluginManifest {
            name: official.id.clone(),
            version: "1.0.0".to_string(),
            author: "Symphony".to_string(),
            description: official.description.clone(),
            permissions: vec![],
            homepage: None,
            min_symphony_version: None,
            enabled: official.default_enabled,
        };

        let manifest_content = toml::to_string_pretty(&manifest)
            .map_err(|e| PluginError::ParseError(e.to_string()))?;

        std::fs::write(plugin_dir.join("plugin.toml"), manifest_content)?;

        info!("Installed official plugin: {}", id);

        // Rescan
        self.scan_plugins()?;

        Ok(())
    }

    /// Uninstall a plugin
    pub fn uninstall(&mut self, name: &str) -> Result<(), PluginError> {
        let info = self
            .available
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        if info.official {
            // Just remove the directory
            std::fs::remove_dir_all(&info.path)?;
            info!("Uninstalled plugin: {}", name);
        } else {
            // Remove plugin directory
            std::fs::remove_dir_all(&info.path)?;
            info!("Uninstalled plugin: {}", name);
        }

        // Rescan
        self.scan_plugins()?;

        Ok(())
    }

    /// Get plugin directory
    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_registry_creation() {
        let dir = tempdir().unwrap();
        let registry = PluginRegistry::new(dir.path().to_path_buf());
        assert!(registry.is_ok());
    }

    #[test]
    fn test_official_plugins_list() {
        let dir = tempdir().unwrap();
        let registry = PluginRegistry::new(dir.path().to_path_buf()).unwrap();

        let official = registry.list_official();
        assert!(!official.is_empty());
        assert!(official.iter().any(|p| p.id == "discord"));
        assert!(official.iter().any(|p| p.id == "lastfm"));
    }

    #[test]
    fn test_install_official_plugin() {
        let dir = tempdir().unwrap();
        let mut registry = PluginRegistry::new(dir.path().to_path_buf()).unwrap();

        registry.install_official("discord").unwrap();
        assert!(registry.is_installed("discord"));
    }
}
