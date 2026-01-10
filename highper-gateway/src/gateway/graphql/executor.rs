//! GraphQL query executor
//!
//! Executes GraphQL queries across multiple backends and merges results

use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, warn};

use super::stitcher::QueryFragment;
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

    /// Execute query fragments in parallel across multiple backends
    ///
    /// This is the core federation execution function
    pub async fn execute_federated(
        &self,
        fragments: Vec<QueryFragment>,
        variables: Option<Value>,
    ) -> Result<Vec<(QueryFragment, Value)>> {
        if fragments.is_empty() {
            return Ok(Vec::new());
        }

        debug!("Executing {} fragments in parallel", fragments.len());

        // Execute all fragments in parallel
        let mut tasks = Vec::new();

        for fragment in fragments {
            let backend = self.find_backend(&fragment.backend)?;
            let client = self.client.clone();
            let vars = variables.clone();

            // Build query from fragment fields
            let query = format!("{{ {} }}", fragment.fields.join(" "));

            debug!(
                "Executing fragment on backend {}: {}",
                backend.name, query
            );

            // Spawn async task for each backend
            let task = tokio::spawn(async move {
                let result = Self::execute_on_backend(&client, &backend, query, vars).await;
                (fragment.clone(), result)
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();

        for task in tasks {
            match task.await {
                Ok((fragment, result)) => match result {
                    Ok(response) => {
                        // Convert GraphQLResponse to serde_json::Value
                        let value = serde_json::to_value(response)?;
                        results.push((fragment, value));
                    }
                    Err(e) => {
                        warn!("Backend {} failed: {}", fragment.backend, e);
                        // Return error as GraphQL error format
                        let error_value = serde_json::json!({
                            "data": null,
                            "errors": [{
                                "message": e.to_string(),
                                "extensions": {
                                    "backend": fragment.backend
                                }
                            }]
                        });
                        results.push((fragment, error_value));
                    }
                },
                Err(e) => {
                    warn!("Task failed: {}", e);
                }
            }
        }

        debug!("Completed {} fragment executions", results.len());

        Ok(results)
    }

    /// Execute a query on a specific backend (helper method)
    async fn execute_on_backend(
        client: &Client,
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

        // Serialize request body to JSON bytes
        let body_json = serde_json::to_vec(&request_body)
            .context("Failed to serialize GraphQL request body")?;
        let body_bytes = bytes::Bytes::from(body_json);

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
        let response = client
            .forward(&backend.url, hyper::Method::POST, "/graphql", headers, Some(body_bytes))
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

    /// Find a backend by name
    fn find_backend(&self, name: &str) -> Result<GraphQLBackend> {
        self.config
            .backends
            .iter()
            .find(|b| b.name == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Backend not found: {}", name))
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

        // Serialize request body to JSON bytes
        let body_json = serde_json::to_vec(&request_body)
            .context("Failed to serialize GraphQL request body")?;
        let body_bytes = bytes::Bytes::from(body_json);

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
            .forward(&backend.url, hyper::Method::POST, "/graphql", headers, Some(body_bytes))
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
