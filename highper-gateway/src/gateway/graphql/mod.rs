//! GraphQL Gateway with schema stitching
//!
//! Provides GraphQL query routing, schema stitching from multiple backends,
//! query batching, and caching capabilities.

pub mod schema;
pub mod stitcher;
pub mod executor;
pub mod cache;
pub mod analyzer;

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
        let document = parse_query(&req.query)
            .context("Failed to parse GraphQL query")?;

        // B5 — depth/complexity analyzer. Reads thresholds from
        // `runtime_config::current().graphql`; when `enforce` is true
        // and either the depth or complexity limit is exceeded (or the
        // analyzer rejects the query — fragment cycle / unknown
        // fragment), reply with a structured GraphQL error and bypass
        // backend dispatch entirely.
        if let Some(rejection) = self.analyze_request(&document) {
            return Ok(GraphQLResponse {
                data: None,
                errors: Some(vec![GraphQLError {
                    message: rejection,
                    locations: None,
                    path: None,
                }]),
            });
        }

        // Check cache if enabled
        if self.config.enable_cache {
            let cache_key = self.generate_cache_key(&req);
            if let Some(cached_response) = self.cache.get(&cache_key).await {
                debug!("Cache hit for query");
                return Ok(cached_response);
            }
        }

        // Execute query - use federation if enabled and multiple backends configured
        let response = if self.config.enable_stitching && self.config.backends.len() > 1 {
            // Federation mode - analyze query, split across backends, and merge results
            debug!("Using federation mode with {} backends", self.config.backends.len());

            match self.stitcher.analyze_and_split_query(&req.query, &self.schema_registry) {
                Ok(fragments) => {
                    if fragments.is_empty() {
                        // No fragments - query doesn't match any backend fields
                        GraphQLResponse {
                            data: None,
                            errors: Some(vec![GraphQLError {
                                message: "Query fields don't match any configured backend".to_string(),
                                locations: None,
                                path: None,
                            }]),
                        }
                    } else {
                        // Execute fragments in parallel across backends
                        match self.executor.execute_federated(fragments, req.variables.clone()).await {
                            Ok(fragment_results) => {
                                // Merge results from all backends
                                match self.stitcher.merge_results(fragment_results) {
                                    Ok(merged_data) => {
                                        GraphQLResponse {
                                            data: Some(merged_data),
                                            errors: None,
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to merge results: {}", e);
                                        GraphQLResponse {
                                            data: None,
                                            errors: Some(vec![GraphQLError {
                                                message: format!("Result merge failed: {}", e),
                                                locations: None,
                                                path: None,
                                            }]),
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Federation execution failed: {}", e);
                                GraphQLResponse {
                                    data: None,
                                    errors: Some(vec![GraphQLError {
                                        message: format!("Execution failed: {}", e),
                                        locations: None,
                                        path: None,
                                    }]),
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Query analysis failed: {}", e);
                    GraphQLResponse {
                        data: None,
                        errors: Some(vec![GraphQLError {
                            message: format!("Query analysis failed: {}", e),
                            locations: None,
                            path: None,
                        }]),
                    }
                }
            }
        } else if !self.config.backends.is_empty() {
            // Single backend mode - execute directly on first backend
            debug!("Using single backend mode");
            self.executor.execute_simple(&self.config.backends[0], req.query.clone(), req.variables.clone()).await?
        } else {
            // No backends configured
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

    /// Apply depth + complexity limits from `runtime_config::current()
    /// .graphql`. Returns `Some(rejection_message)` if the query must
    /// be refused; otherwise `None`. When `enforce` is false, limit
    /// breaches are logged but the request proceeds.
    fn analyze_request(
        &self,
        document: &async_graphql_parser::types::ExecutableDocument,
    ) -> Option<String> {
        use crate::runtime_config;

        let cfg = runtime_config::try_current();
        let (max_depth, max_complexity, enforce) = match cfg.as_ref() {
            Some(c) => (
                *c.graphql.max_depth.get(),
                *c.graphql.max_complexity.get(),
                *c.graphql.enforce.get(),
            ),
            None => (15, 1000, true), // safe defaults when runtime_config not yet installed
        };

        match analyzer::analyze(document) {
            Ok(stats) => {
                if stats.depth > max_depth {
                    let msg = format!(
                        "GraphQL query depth {} exceeds limit {}",
                        stats.depth, max_depth
                    );
                    if enforce {
                        warn!("{} — rejecting", msg);
                        return Some(msg);
                    }
                    warn!("{} — log-only (enforce=false)", msg);
                }
                if stats.complexity > max_complexity {
                    let msg = format!(
                        "GraphQL query complexity {} exceeds limit {}",
                        stats.complexity, max_complexity
                    );
                    if enforce {
                        warn!("{} — rejecting", msg);
                        return Some(msg);
                    }
                    warn!("{} — log-only (enforce=false)", msg);
                }
                None
            }
            Err(e) => {
                let msg = format!("GraphQL query rejected by analyzer: {}", e);
                if enforce {
                    warn!("{}", msg);
                    Some(msg)
                } else {
                    warn!("{} — log-only (enforce=false)", msg);
                    None
                }
            }
        }
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

    #[tokio::test]
    async fn test_federation_mode_detection() {
        // Test with federation enabled and multiple backends
        let config_federated = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![
                GraphQLBackend {
                    name: "users".to_string(),
                    url: "http://localhost:8081/graphql".to_string(),
                    namespace: None,
                    type_mappings: vec![],
                },
                GraphQLBackend {
                    name: "posts".to_string(),
                    url: "http://localhost:8082/graphql".to_string(),
                    namespace: None,
                    type_mappings: vec![],
                },
            ],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config_federated, client).await.unwrap();

        // Verify gateway was created with federation config
        assert!(gateway.config.enable_stitching);
        assert_eq!(gateway.config.backends.len(), 2);
    }

    #[tokio::test]
    async fn test_single_backend_mode() {
        // Test with federation disabled or single backend
        let config_single = GraphQLConfig {
            enable_stitching: false,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![
                GraphQLBackend {
                    name: "api".to_string(),
                    url: "http://localhost:8081/graphql".to_string(),
                    namespace: None,
                    type_mappings: vec![],
                },
            ],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config_single, client).await.unwrap();

        // Verify single backend mode
        assert!(!gateway.config.enable_stitching);
        assert_eq!(gateway.config.backends.len(), 1);
    }

    #[tokio::test]
    async fn test_no_backends_configured() {
        let config_empty = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config_empty, client).await.unwrap();

        let req = GraphQLRequest {
            query: "{ hello }".to_string(),
            operation_name: None,
            variables: None,
        };

        let response = gateway.handle_request(req).await.unwrap();

        // Should return error about no backends
        assert!(response.data.is_none());
        assert!(response.errors.is_some());
        let errors = response.errors.unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("No backends configured"));
    }

    #[tokio::test]
    async fn test_introspection_disabled() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: false,
            backends: vec![],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config, client).await.unwrap();

        let response = gateway.handle_introspection().await.unwrap();

        // Should return error
        assert!(response.data.is_none());
        assert!(response.errors.is_some());
        let errors = response.errors.unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("Introspection is disabled"));
    }

    #[tokio::test]
    async fn test_batch_request_disabled() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config, client).await.unwrap();

        let requests = vec![
            GraphQLRequest {
                query: "{ hello }".to_string(),
                operation_name: None,
                variables: None,
            },
        ];

        let result = gateway.handle_batch(requests).await;

        // Should return error about batching disabled
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Batching is disabled"));
    }

    #[tokio::test]
    async fn test_batch_request_size_limit() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: Duration::from_secs(300),
            enable_batching: true,
            max_batch_size: 2,
            introspection_enabled: true,
            backends: vec![],
        };

        let client = Arc::new(Client::new());
        let gateway = GraphQLGateway::new(config, client).await.unwrap();

        let requests = vec![
            GraphQLRequest {
                query: "{ hello }".to_string(),
                operation_name: None,
                variables: None,
            },
            GraphQLRequest {
                query: "{ world }".to_string(),
                operation_name: None,
                variables: None,
            },
            GraphQLRequest {
                query: "{ foo }".to_string(),
                operation_name: None,
                variables: None,
            },
        ];

        let result = gateway.handle_batch(requests).await;

        // Should return error about batch size
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Batch size exceeds maximum"));
    }
}
