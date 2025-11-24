//! Aggregation executor for parallel backend calls

use super::config::{AggregationConfig, BackendCall, ErrorStrategy};
use crate::proxy::Client;
use anyhow::Result;
use hyper::{HeaderMap, Method, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Result of a single backend call
#[derive(Debug, Clone)]
pub struct BackendResult {
    /// Backend name
    pub name: String,

    /// Whether the call succeeded
    pub success: bool,

    /// HTTP status code
    pub status: StatusCode,

    /// Response body (JSON value)
    pub body: Option<serde_json::Value>,

    /// Error message if failed
    pub error: Option<String>,

    /// Response time in milliseconds
    pub duration_ms: f64,
}

/// Aggregation executor
pub struct AggregationExecutor {
    config: AggregationConfig,
    client: Client,
    upstreams: Arc<HashMap<String, String>>, // upstream_name -> base_url
}

impl AggregationExecutor {
    /// Create new aggregation executor
    pub fn new(
        config: AggregationConfig,
        client: Client,
        upstreams: Arc<HashMap<String, String>>,
    ) -> Self {
        Self {
            config,
            client,
            upstreams,
        }
    }

    /// Execute all backend calls and return results
    pub async fn execute(
        &self,
        request_headers: &HeaderMap,
        path_params: &HashMap<String, String>,
    ) -> Result<Vec<BackendResult>> {
        info!(
            "Executing {} backend calls (parallel: {})",
            self.config.backends.len(),
            self.config.parallel
        );

        if self.config.parallel {
            self.execute_parallel(request_headers, path_params).await
        } else {
            self.execute_sequential(request_headers, path_params)
                .await
        }
    }

    /// Execute all calls in parallel
    async fn execute_parallel(
        &self,
        request_headers: &HeaderMap,
        path_params: &HashMap<String, String>,
    ) -> Result<Vec<BackendResult>> {
        let mut tasks = Vec::new();

        for backend in &self.config.backends {
            let backend_clone = backend.clone();
            let client = self.client.clone();
            let headers = request_headers.clone();
            let params = path_params.clone();
            let upstreams = self.upstreams.clone();
            let timeout_ms = self.config.timeout_ms;

            let task = tokio::spawn(async move {
                Self::execute_single_call(
                    &backend_clone,
                    &client,
                    &headers,
                    &params,
                    &upstreams,
                    timeout_ms,
                )
                .await
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(result) => results.push(result),
                Err(e) => {
                    error!("Backend task failed: {}", e);
                    // Return error result
                    results.push(BackendResult {
                        name: "unknown".to_string(),
                        success: false,
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        body: None,
                        error: Some(format!("Task join error: {}", e)),
                        duration_ms: 0.0,
                    });
                }
            }
        }

        self.handle_errors(&results)?;

        Ok(results)
    }

    /// Execute all calls sequentially
    async fn execute_sequential(
        &self,
        request_headers: &HeaderMap,
        path_params: &HashMap<String, String>,
    ) -> Result<Vec<BackendResult>> {
        let mut results = Vec::new();

        for backend in &self.config.backends {
            let result = Self::execute_single_call(
                backend,
                &self.client,
                request_headers,
                path_params,
                &self.upstreams,
                self.config.timeout_ms,
            )
            .await;

            results.push(result);
        }

        self.handle_errors(&results)?;

        Ok(results)
    }

    /// Execute a single backend call
    async fn execute_single_call(
        backend: &BackendCall,
        client: &Client,
        request_headers: &HeaderMap,
        path_params: &HashMap<String, String>,
        upstreams: &HashMap<String, String>,
        timeout_ms: u64,
    ) -> BackendResult {
        let start = std::time::Instant::now();

        debug!("Executing backend call: {}", backend.name);

        // Get upstream base URL
        let base_url = match upstreams.get(&backend.upstream) {
            Some(url) => url,
            None => {
                warn!("Upstream not found: {}", backend.upstream);
                return BackendResult {
                    name: backend.name.clone(),
                    success: false,
                    status: StatusCode::BAD_GATEWAY,
                    body: None,
                    error: Some(format!("Upstream '{}' not found", backend.upstream)),
                    duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                };
            }
        };

        // Interpolate path parameters
        let path = Self::interpolate_path(&backend.path, path_params);

        // Build headers
        let mut headers = request_headers.clone();
        for (key, value) in &backend.headers {
            if let Ok(header_name) = hyper::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = hyper::header::HeaderValue::from_str(value) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        // Parse method
        let method = match backend.method.parse::<Method>() {
            Ok(m) => m,
            Err(e) => {
                error!("Invalid HTTP method '{}': {}", backend.method, e);
                return BackendResult {
                    name: backend.name.clone(),
                    success: false,
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    body: None,
                    error: Some(format!("Invalid method: {}", e)),
                    duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                };
            }
        };

        // Execute request with timeout
        let result = timeout(
            Duration::from_millis(timeout_ms),
            client.forward(base_url, method, &path, headers),
        )
        .await;

        match result {
            Ok(Ok(response)) => {
                let status = response.status();
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

                // Read response body
                use http_body_util::BodyExt;
                let body_result = response.into_body().collect().await.map(|c| c.to_bytes());

                match body_result {
                    Ok(body_bytes) => {
                        // Try to parse as JSON
                        let json_value = serde_json::from_slice(&body_bytes).ok();

                        // Apply JSONPath extraction if configured
                        let extracted = if let Some(extract_path) = &backend.extract {
                            Self::extract_json_path(&json_value, extract_path)
                        } else {
                            json_value
                        };

                        BackendResult {
                            name: backend.name.clone(),
                            success: status.is_success(),
                            status,
                            body: extracted,
                            error: if status.is_success() {
                                None
                            } else {
                                Some(format!("HTTP {}", status))
                            },
                            duration_ms,
                        }
                    }
                    Err(e) => {
                        error!("Failed to read response body for {}: {}", backend.name, e);
                        BackendResult {
                            name: backend.name.clone(),
                            success: false,
                            status,
                            body: None,
                            error: Some(format!("Failed to read body: {}", e)),
                            duration_ms,
                        }
                    }
                }
            }
            Ok(Err(e)) => {
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                error!("Backend call failed for {}: {}", backend.name, e);

                // Use fallback if available
                let body = backend.fallback.clone();

                BackendResult {
                    name: backend.name.clone(),
                    success: false,
                    status: StatusCode::BAD_GATEWAY,
                    body,
                    error: Some(format!("Request failed: {}", e)),
                    duration_ms,
                }
            }
            Err(_) => {
                let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
                warn!("Backend call timeout for {}", backend.name);

                // Use fallback if available
                let body = backend.fallback.clone();

                BackendResult {
                    name: backend.name.clone(),
                    success: false,
                    status: StatusCode::GATEWAY_TIMEOUT,
                    body,
                    error: Some("Request timeout".to_string()),
                    duration_ms,
                }
            }
        }
    }

    /// Interpolate path parameters in path template
    fn interpolate_path(path: &str, params: &HashMap<String, String>) -> String {
        let mut result = path.to_string();

        for (key, value) in params {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }

        result
    }

    /// Extract value using JSONPath (simplified implementation)
    fn extract_json_path(
        value: &Option<serde_json::Value>,
        path: &str,
    ) -> Option<serde_json::Value> {
        // TODO: Full JSONPath implementation
        // For now, support simple dot notation: "data.user.name"
        let value = value.as_ref()?;

        if path == "$" || path.is_empty() {
            return Some(value.clone());
        }

        let parts: Vec<&str> = path.trim_start_matches("$.").split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current.clone())
    }

    /// Handle errors based on error strategy
    fn handle_errors(&self, results: &[BackendResult]) -> Result<()> {
        match self.config.error_strategy {
            ErrorStrategy::FailFast => {
                // Find first failed required backend
                for (backend_config, result) in self.config.backends.iter().zip(results.iter()) {
                    if backend_config.required && !result.success {
                        return Err(anyhow::anyhow!(
                            "Required backend '{}' failed: {}",
                            result.name,
                            result.error.as_deref().unwrap_or("unknown error")
                        ));
                    }
                }
                Ok(())
            }
            ErrorStrategy::Partial => {
                // Allow failures for non-required backends
                for (backend_config, result) in self.config.backends.iter().zip(results.iter()) {
                    if backend_config.required && !result.success {
                        return Err(anyhow::anyhow!(
                            "Required backend '{}' failed: {}",
                            result.name,
                            result.error.as_deref().unwrap_or("unknown error")
                        ));
                    }
                }
                Ok(())
            }
            ErrorStrategy::Include | ErrorStrategy::Ignore => {
                // Never fail, include errors in response or ignore them
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_path() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "123".to_string());
        params.insert("type".to_string(), "user".to_string());

        let path = "/api/{type}/{id}";
        let result = AggregationExecutor::interpolate_path(path, &params);
        assert_eq!(result, "/api/user/123");
    }

    #[test]
    fn test_extract_json_path_simple() {
        let json = serde_json::json!({
            "data": {
                "user": {
                    "name": "John"
                }
            }
        });

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.data.user.name");
        assert_eq!(result, Some(serde_json::json!("John")));
    }

    #[test]
    fn test_extract_json_path_root() {
        let json = serde_json::json!({"key": "value"});

        let result = AggregationExecutor::extract_json_path(&Some(json.clone()), "$");
        assert_eq!(result, Some(json));
    }

    #[test]
    fn test_extract_json_path_not_found() {
        let json = serde_json::json!({"key": "value"});

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.missing");
        assert_eq!(result, None);
    }

    #[test]
    fn test_backend_result_creation() {
        let result = BackendResult {
            name: "test".to_string(),
            success: true,
            status: StatusCode::OK,
            body: Some(serde_json::json!({"data": "test"})),
            error: None,
            duration_ms: 42.5,
        };

        assert_eq!(result.name, "test");
        assert!(result.success);
        assert_eq!(result.status, StatusCode::OK);
        assert!(result.body.is_some());
        assert!(result.error.is_none());
    }
}
