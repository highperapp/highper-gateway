//! GraphQL Gateway with schema stitching
//!
//! Provides GraphQL query routing, schema stitching from multiple backends,
//! query batching, and caching capabilities.

pub mod schema;
pub mod stitcher;
pub mod executor;
pub mod cache;

use anyhow::{Context, Result};
use async_graphql_parser::parse_query;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::proxy::Client;

pub use schema::SchemaRegistry;
pub use stitcher::SchemaStitcher;
pub use executor::GraphQLExecutor;
pub use cache::QueryCache;

/// GraphQL gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLConfig {
    /// Enable schema stitching
    #[serde(default)]
    pub enable_stitching: bool,

    /// Enable query caching
    #[serde(default)]
    pub enable_cache: bool,

    /// Cache TTL
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: Duration,

    /// Enable query batching
    #[serde(default)]
    pub enable_batching: bool,

    /// Max batch size
    #[serde(default = "default_max_batch_size")]
    pub max_batch_size: usize,

    /// Schema introspection endpoint
    #[serde(default)]
    pub introspection_enabled: bool,

    /// Backend GraphQL endpoints
    pub backends: Vec<GraphQLBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLBackend {
    /// Backend name
    pub name: String,

    /// GraphQL endpoint URL
    pub url: String,

    /// Schema namespace (for stitching)
    pub namespace: Option<String>,

    /// Type mappings for stitching
    #[serde(default)]
    pub type_mappings: Vec<TypeMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMapping {
    /// Local type name
    pub local_type: String,

    /// Remote type name
    pub remote_type: String,

    /// Field mappings
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub local_field: String,
    pub remote_field: String,
}

fn default_cache_ttl() -> Duration {
    Duration::from_secs(300)
}

fn default_max_batch_size() -> usize {
    10
}

/// GraphQL request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
}

/// GraphQL response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locations: Option<Vec<ErrorLocation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLocation {
    pub line: usize,
    pub column: usize,
}

/// GraphQL Gateway handler
pub struct GraphQLGateway {
    config: GraphQLConfig,
    client: Arc<Client>,
    schema_registry: Arc<SchemaRegistry>,
    stitcher: Arc<SchemaStitcher>,
    executor: Arc<GraphQLExecutor>,
    cache: Arc<QueryCache>,
}

impl GraphQLGateway {
    /// Create a new GraphQL gateway
    pub async fn new(config: GraphQLConfig, client: Arc<Client>) -> Result<Self> {
        let schema_registry = Arc::new(SchemaRegistry::new());
        let stitcher = Arc::new(SchemaStitcher::new(config.clone()));
        let executor = Arc::new(GraphQLExecutor::new(client.clone(), config.clone()));
        let cache = Arc::new(QueryCache::new(config.cache_ttl));

        // Initialize schemas from backends
        for backend in &config.backends {
            info!("Loading GraphQL schema from backend: {}", backend.name);
            if let Err(e) = schema_registry.load_schema(&backend.name, &backend.url, client.as_ref()).await {
                warn!("Failed to load schema from {}: {}", backend.name, e);
            }
        }

        Ok(Self {
            config,
            client,
            schema_registry,
            stitcher,
            executor,
            cache,
        })
    }

    /// Handle GraphQL request
    pub async fn handle_request(&self, req: GraphQLRequest) -> Result<GraphQLResponse> {
        // Parse query
        let _document = parse_query(&req.query)
            .context("Failed to parse GraphQL query")?;

        // Check cache if enabled
        if self.config.enable_cache {
            let cache_key = self.generate_cache_key(&req);
            if let Some(cached_response) = self.cache.get(&cache_key).await {
                debug!("Cache hit for query");
                return Ok(cached_response);
            }
        }

        // Execute query on first backend (simplified for now)
        let response = if !self.config.backends.is_empty() {
            self.executor.execute_simple(&self.config.backends[0], req.query.clone(), req.variables.clone()).await?
        } else {
            GraphQLResponse {
                data: None,
                errors: Some(vec![GraphQLError {
                    message: "No backends configured".to_string(),
                    locations: None,
                    path: None,
                }]),
            }
        };

        // Cache response if enabled
        if self.config.enable_cache {
            let cache_key = self.generate_cache_key(&req);
            self.cache.set(cache_key, response.clone()).await;
        }

        Ok(response)
    }

    /// Handle introspection query
    pub async fn handle_introspection(&self) -> Result<GraphQLResponse> {
        if !self.config.introspection_enabled {
            return Ok(GraphQLResponse {
                data: None,
                errors: Some(vec![GraphQLError {
                    message: "Introspection is disabled".to_string(),
                    locations: None,
                    path: None,
                }]),
            });
        }

        // Build stitched schema for introspection
        let stitched_schema = self.stitcher.stitch_schemas(&self.schema_registry)?;

        Ok(GraphQLResponse {
            data: Some(stitched_schema),
            errors: None,
        })
    }

    /// Handle batch request
    pub async fn handle_batch(&self, requests: Vec<GraphQLRequest>) -> Result<Vec<GraphQLResponse>> {
        if !self.config.enable_batching {
            return Err(anyhow::anyhow!("Batching is disabled"));
        }

        if requests.len() > self.config.max_batch_size {
            return Err(anyhow::anyhow!("Batch size exceeds maximum"));
        }

        let mut responses = Vec::new();
        for req in requests {
            let response = self.handle_request(req).await?;
            responses.push(response);
        }

        Ok(responses)
    }

    fn generate_cache_key(&self, req: &GraphQLRequest) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&req.query);
        if let Some(ref vars) = req.variables {
            hasher.update(serde_json::to_string(vars).unwrap_or_default());
        }
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graphql_request_parsing() {
        let req = GraphQLRequest {
            query: "{ hello }".to_string(),
            operation_name: None,
            variables: None,
        };

        let result = parse_query(&req.query);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: true,
            cache_ttl: Duration::from_secs(300),
            enable_batching: true,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config, client).await.unwrap();

        let req1 = GraphQLRequest {
            query: "{ hello }".to_string(),
            operation_name: None,
            variables: None,
        };

        let req2 = GraphQLRequest {
            query: "{ hello }".to_string(),
            operation_name: None,
            variables: None,
        };

        let key1 = gateway.generate_cache_key(&req1);
        let key2 = gateway.generate_cache_key(&req2);

        assert_eq!(key1, key2);
    }
}
