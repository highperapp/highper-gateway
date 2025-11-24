//! Plugin registry for managing loaded plugins

use super::trait_def::{BoxedPlugin, Plugin};
use super::types::*;
use super::{PluginError, Result};
use dashmap::DashMap;
use std::sync::Arc;

/// Plugin entry in the registry
struct PluginEntry {
    /// The plugin instance
    plugin: BoxedPlugin,

    /// Plugin priority (higher = executes earlier)
    priority: u32,

    /// Whether the plugin is enabled
    enabled: bool,

    /// Plugin statistics
    stats: Arc<parking_lot::RwLock<PluginStats>>,
}

/// Plugin registry manages all loaded plugins
pub struct PluginRegistry {
    /// Map of plugin name to plugin entry
    plugins: Arc<DashMap<String, PluginEntry>>,

    /// Sorted list of plugins by priority (cached)
    sorted_cache: Arc<parking_lot::RwLock<Vec<String>>>,

    /// Whether the sorted cache is dirty
    cache_dirty: Arc<parking_lot::RwLock<bool>>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(DashMap::new()),
            sorted_cache: Arc::new(parking_lot::RwLock::new(Vec::new())),
            cache_dirty: Arc::new(parking_lot::RwLock::new(true)),
        }
    }

    /// Register a plugin
    pub fn register(&self, plugin: BoxedPlugin, priority: u32) -> Result<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(PluginError::AlreadyLoaded(name));
        }

        let entry = PluginEntry {
            plugin,
            priority,
            enabled: true,
            stats: Arc::new(parking_lot::RwLock::new(PluginStats::default())),
        };

        self.plugins.insert(name.clone(), entry);

        // Mark cache as dirty
        *self.cache_dirty.write() = true;

        tracing::info!("Registered plugin: {} (priority: {})", name, priority);

        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister(&self, name: &str) -> Result<BoxedPlugin> {
        self.plugins
            .remove(name)
            .map(|(_, entry)| {
                *self.cache_dirty.write() = true;
                tracing::info!("Unregistered plugin: {}", name);
                entry.plugin
            })
            .ok_or_else(|| PluginError::NotFound(name.to_string()))
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<BoxedPlugin> {
        self.plugins.get(name).map(|entry| entry.plugin.clone())
    }

    /// Check if a plugin exists
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Enable a plugin
    pub fn enable(&self, name: &str) -> Result<()> {
        if let Some(mut entry) = self.plugins.get_mut(name) {
            entry.enabled = true;
            tracing::info!("Enabled plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    /// Disable a plugin
    pub fn disable(&self, name: &str) -> Result<()> {
        if let Some(mut entry) = self.plugins.get_mut(name) {
            entry.enabled = false;
            tracing::info!("Disabled plugin: {}", name);
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    /// Get all enabled plugins sorted by priority (highest first)
    pub fn get_enabled_plugins(&self) -> Vec<BoxedPlugin> {
        // Check if cache needs update
        if *self.cache_dirty.read() {
            self.rebuild_cache();
        }

        let sorted = self.sorted_cache.read();
        sorted
            .iter()
            .filter_map(|name| {
                self.plugins.get(name).and_then(|entry| {
                    if entry.enabled {
                        Some(entry.plugin.clone())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Rebuild the priority-sorted cache
    fn rebuild_cache(&self) {
        let mut entries: Vec<(String, u32)> = self
            .plugins
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().priority))
            .collect();

        // Sort by priority (descending - higher priority first)
        entries.sort_by(|a, b| b.1.cmp(&a.1));

        let sorted_names: Vec<String> = entries.into_iter().map(|(name, _)| name).collect();

        *self.sorted_cache.write() = sorted_names;
        *self.cache_dirty.write() = false;

        tracing::debug!("Rebuilt plugin priority cache");
    }

    /// Get plugin statistics
    pub fn get_stats(&self, name: &str) -> Option<PluginStats> {
        self.plugins
            .get(name)
            .map(|entry| entry.stats.read().clone())
    }

    /// Update plugin statistics
    pub fn update_stats<F>(&self, name: &str, f: F)
    where
        F: FnOnce(&mut PluginStats),
    {
        if let Some(entry) = self.plugins.get(name) {
            let mut stats = entry.stats.write();
            f(&mut stats);
        }
    }

    /// List all plugin names
    pub fn list_plugins(&self) -> Vec<String> {
        self.plugins.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Get plugin metadata
    pub fn get_metadata(&self, name: &str) -> Option<PluginMetadata> {
        self.plugins.get(name).map(|entry| entry.plugin.metadata())
    }

    /// Get all plugin metadata
    pub fn list_metadata(&self) -> Vec<PluginMetadata> {
        self.plugins
            .iter()
            .map(|entry| entry.plugin.metadata())
            .collect()
    }

    /// Get plugin count
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Get enabled plugin count
    pub fn enabled_count(&self) -> usize {
        self.plugins.iter().filter(|entry| entry.enabled).count()
    }

    /// Clear all plugins
    pub fn clear(&self) {
        self.plugins.clear();
        *self.sorted_cache.write() = Vec::new();
        *self.cache_dirty.write() = true;
        tracing::info!("Cleared all plugins from registry");
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::trait_def::NoOpPlugin;
    use super::*;

    #[test]
    fn test_registry_register() {
        let registry = PluginRegistry::new();
        let plugin: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));

        assert!(registry.register(plugin, 50).is_ok());
        assert!(registry.contains("test1"));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_registry_duplicate_register() {
        let registry = PluginRegistry::new();
        let plugin1: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));
        let plugin2: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));

        assert!(registry.register(plugin1, 50).is_ok());
        assert!(registry.register(plugin2, 60).is_err());
    }

    #[test]
    fn test_registry_unregister() {
        let registry = PluginRegistry::new();
        let plugin: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));

        registry.register(plugin, 50).unwrap();
        assert!(registry.contains("test1"));

        let removed = registry.unregister("test1");
        assert!(removed.is_ok());
        assert!(!registry.contains("test1"));
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_registry_priority_sorting() {
        let registry = PluginRegistry::new();

        let plugin1: BoxedPlugin = Arc::new(NoOpPlugin::new("low"));
        let plugin2: BoxedPlugin = Arc::new(NoOpPlugin::new("high"));
        let plugin3: BoxedPlugin = Arc::new(NoOpPlugin::new("medium"));

        registry.register(plugin1, 10).unwrap();
        registry.register(plugin2, 100).unwrap();
        registry.register(plugin3, 50).unwrap();

        let enabled = registry.get_enabled_plugins();
        assert_eq!(enabled.len(), 3);

        // Should be sorted by priority: high (100), medium (50), low (10)
        assert_eq!(enabled[0].name(), "high");
        assert_eq!(enabled[1].name(), "medium");
        assert_eq!(enabled[2].name(), "low");
    }

    #[test]
    fn test_registry_enable_disable() {
        let registry = PluginRegistry::new();
        let plugin: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));

        registry.register(plugin, 50).unwrap();
        assert_eq!(registry.enabled_count(), 1);

        registry.disable("test1").unwrap();
        assert_eq!(registry.enabled_count(), 0);

        let enabled = registry.get_enabled_plugins();
        assert_eq!(enabled.len(), 0);

        registry.enable("test1").unwrap();
        assert_eq!(registry.enabled_count(), 1);
    }

    #[test]
    fn test_registry_stats() {
        let registry = PluginRegistry::new();
        let plugin: BoxedPlugin = Arc::new(NoOpPlugin::new("test1"));

        registry.register(plugin, 50).unwrap();

        // Update stats
        registry.update_stats("test1", |stats| {
            stats.record_success(100);
        });

        let stats = registry.get_stats("test1").unwrap();
        assert_eq!(stats.requests_total, 1);
        assert_eq!(stats.requests_success, 1);
    }

    #[test]
    fn test_registry_list() {
        let registry = PluginRegistry::new();

        let plugin1: BoxedPlugin = Arc::new(NoOpPlugin::new("plugin1"));
        let plugin2: BoxedPlugin = Arc::new(NoOpPlugin::new("plugin2"));

        registry.register(plugin1, 50).unwrap();
        registry.register(plugin2, 60).unwrap();

        let names = registry.list_plugins();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"plugin1".to_string()));
        assert!(names.contains(&"plugin2".to_string()));
    }

    #[test]
    fn test_registry_clear() {
        let registry = PluginRegistry::new();

        let plugin1: BoxedPlugin = Arc::new(NoOpPlugin::new("plugin1"));
        let plugin2: BoxedPlugin = Arc::new(NoOpPlugin::new("plugin2"));

        registry.register(plugin1, 50).unwrap();
        registry.register(plugin2, 60).unwrap();

        assert_eq!(registry.count(), 2);

        registry.clear();
        assert_eq!(registry.count(), 0);
    }
}
