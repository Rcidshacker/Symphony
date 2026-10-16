//! Plugin Registry
//!
//! Finds the plugins installed in the plugin directory (one folder each, with `plugin.toml`
//! and `plugin.wasm`) and removes them again.

use super::{PluginError, PluginManifest};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Plugin registry for managing plugins
pub struct PluginRegistry {
    /// Plugin directory
    plugin_dir: PathBuf,
    /// Installed plugins (from directory scan)
    available: HashMap<String, PluginInfo>,
}

/// Information about an installed plugin
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
    /// Repository URL
    pub repository: Option<String>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new(plugin_dir: PathBuf) -> Result<Self, PluginError> {
        let mut registry = Self {
            plugin_dir,
            available: HashMap::new(),
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

    /// Scan the plugin directory
    pub fn scan_plugins(&mut self) -> Result<(), PluginError> {
        self.available.clear();

        if !self.plugin_dir.exists() {
            std::fs::create_dir_all(&self.plugin_dir)?;
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.plugin_dir)? {
            let path = entry?.path();
            let manifest_path = path.join("plugin.toml");
            if !path.is_dir() || !manifest_path.exists() {
                continue;
            }
            match Self::load_plugin_info(&manifest_path, &path) {
                Ok(info) => {
                    self.available.insert(info.name.clone(), info);
                }
                Err(e) => warn!("Failed to load plugin info from {:?}: {}", path, e),
            }
        }

        info!("Found {} plugins", self.available.len());
        Ok(())
    }

    /// Load plugin info from manifest
    fn load_plugin_info(manifest_path: &Path, plugin_dir: &Path) -> Result<PluginInfo, PluginError> {
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

    /// Get plugin info by name
    pub fn get_plugin(&self, name: &str) -> Option<&PluginInfo> {
        self.available.get(name)
    }

    /// Check if plugin is installed
    pub fn is_installed(&self, name: &str) -> bool {
        self.available.get(name).map(|p| p.installed).unwrap_or(false)
    }

    /// Delete a plugin's folder
    pub fn uninstall(&mut self, name: &str) -> Result<(), PluginError> {
        let info = self
            .available
            .get(name)
            .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

        std::fs::remove_dir_all(&info.path)?;
        info!("Uninstalled plugin: {}", name);

        self.scan_plugins()
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

    fn install(dir: &Path, name: &str) {
        let folder = dir.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("plugin.toml"),
            format!("name = \"{name}\"\nversion = \"1.2.3\"\nauthor = \"me\"\ndescription = \"d\"\n"),
        )
        .unwrap();
    }

    #[test]
    fn scans_installed_plugins_and_skips_folders_without_a_manifest() {
        let dir = tempdir().unwrap();
        install(dir.path(), "one");
        std::fs::create_dir_all(dir.path().join("not-a-plugin")).unwrap();

        let registry = PluginRegistry::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(registry.list_installed().len(), 1);
        assert_eq!(registry.get_plugin("one").unwrap().version, "1.2.3");
        assert!(registry.is_installed("one"));
        assert!(!registry.is_installed("not-a-plugin"));
    }

    #[test]
    fn uninstall_removes_the_folder() {
        let dir = tempdir().unwrap();
        install(dir.path(), "one");
        let mut registry = PluginRegistry::new(dir.path().to_path_buf()).unwrap();

        registry.uninstall("one").unwrap();
        assert!(!dir.path().join("one").exists());
        assert!(!registry.is_installed("one"));
        assert!(registry.uninstall("one").is_err());
    }
}
