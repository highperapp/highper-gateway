//! Dynamic route management
//!
//! Allows routes to be added, updated, and removed at runtime via the Admin API

use super::RouteDefinition;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Route manager for dynamic route updates
pub struct RouteManager {
    routes: Arc<RwLock<HashMap<String, RouteDefinition>>>,
}

impl RouteManager {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new route
    pub async fn add_route(&self, route: RouteDefinition) -> Result<(), String> {
        let mut routes = self.routes.write().await;

        if routes.contains_key(&route.name) {
            return Err(format!("Route '{}' already exists", route.name));
        }

        routes.insert(route.name.clone(), route);
        Ok(())
    }

    /// Update an existing route
    pub async fn update_route(&self, name: &str, route: RouteDefinition) -> Result<(), String> {
        let mut routes = self.routes.write().await;

        if !routes.contains_key(name) {
            return Err(format!("Route '{}' not found", name));
        }

        routes.insert(name.to_string(), route);
        Ok(())
    }

    /// Remove a route
    pub async fn remove_route(&self, name: &str) -> Result<(), String> {
        let mut routes = self.routes.write().await;

        if routes.remove(name).is_none() {
            return Err(format!("Route '{}' not found", name));
        }

        Ok(())
    }

    /// Get a route by name
    pub async fn get_route(&self, name: &str) -> Option<RouteDefinition> {
        let routes = self.routes.read().await;
        routes.get(name).cloned()
    }

    /// List all routes
    pub async fn list_routes(&self) -> Vec<RouteDefinition> {
        let routes = self.routes.read().await;
        routes.values().cloned().collect()
    }

    /// Enable a route
    pub async fn enable_route(&self, name: &str) -> Result<(), String> {
        let mut routes = self.routes.write().await;

        if let Some(route) = routes.get_mut(name) {
            route.enabled = true;
            Ok(())
        } else {
            Err(format!("Route '{}' not found", name))
        }
    }

    /// Disable a route
    pub async fn disable_route(&self, name: &str) -> Result<(), String> {
        let mut routes = self.routes.write().await;

        if let Some(route) = routes.get_mut(name) {
            route.enabled = false;
            Ok(())
        } else {
            Err(format!("Route '{}' not found", name))
        }
    }
}
