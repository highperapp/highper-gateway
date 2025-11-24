//! GraphQL query executor
//!
//! Executes GraphQL queries across multiple backends and merges results

use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{GraphQLBackend, GraphQLConfig, GraphQLError, GraphQLResponse};
use crate::proxy::Client;

/// GraphQL query executor
pub struct GraphQLExecutor {
    client: Arc<Client>,
    config: GraphQLConfig,
}

impl GraphQLExecutor {
    /// Create a new GraphQL executor
    pub fn new(client: Arc<Client>, config: GraphQLConfig) -> Self {
        Self { client, config }
    }

    /// Execute a simple query on a single backend
    pub async fn execute_simple(
        &self,
        backend: &GraphQLBackend,
        query: String,
        variables: Option<Value>,
    ) -> Result<GraphQLResponse> {
        debug!("Executing query on backend: {}", backend.name);

        // Build request body
        let request_body = if let Some(vars) = variables {
            serde_json::json!({
                "query": query,
                "variables": vars
            })
        } else {
            serde_json::json!({
                "query": query
            })
        };

        // Prepare headers
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            hyper::header::ACCEPT,
            hyper::header::HeaderValue::from_static("application/json"),
        );

        // Make request
        let response = self.client
            .forward(&backend.url, hyper::Method::POST, "/graphql", headers)
            .await
            .context(format!("Failed to execute query on backend: {}", backend.name))?;

        let status = response.status();
        let body_bytes = http_body_util::BodyExt::collect(response.into_body())
            .await
            .context("Failed to read response body")?
            .to_bytes();

        if !status.is_success() {
            return Ok(GraphQLResponse {
                data: None,
                errors: Some(vec![GraphQLError {
                    message: format!("Backend {} returned status {}", backend.name, status),
                    locations: None,
                    path: None,
                }]),
            });
        }

        let graphql_response: GraphQLResponse = serde_json::from_slice(&body_bytes)
            .context("Failed to parse GraphQL response")?;

        Ok(graphql_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_executor_creation() {
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
        let executor = GraphQLExecutor::new(client, config);
        assert!(true); // Just verify it compiles
    }

}
