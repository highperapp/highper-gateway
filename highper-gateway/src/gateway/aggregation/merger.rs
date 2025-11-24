//! Response merger for aggregation

use super::config::{ErrorStrategy, MergeStrategy};
use super::executor::BackendResult;
use hyper::StatusCode;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Response merger
pub struct ResponseMerger {
    merge_strategy: MergeStrategy,
    error_strategy: ErrorStrategy,
}

/// Merged response
#[derive(Debug, Clone)]
pub struct MergedResponse {
    /// HTTP status code
    pub status: StatusCode,

    /// Merged response body
    pub body: Value,

    /// Whether all backends succeeded
    pub all_success: bool,

    /// Number of successful backends
    pub success_count: usize,

    /// Total number of backends
    pub total_count: usize,
}

impl ResponseMerger {
    /// Create new response merger
    pub fn new(merge_strategy: MergeStrategy, error_strategy: ErrorStrategy) -> Self {
        Self {
            merge_strategy,
            error_strategy,
        }
    }

    /// Merge backend results into a single response
    pub fn merge(&self, results: Vec<BackendResult>) -> MergedResponse {
        let total_count = results.len();
        let success_count = results.iter().filter(|r| r.success).count();
        let all_success = success_count == total_count;

        debug!(
            "Merging {} backend results ({} successful)",
            total_count, success_count
        );

        // Determine HTTP status code
        let status = self.determine_status(&results, all_success);

        // Merge response bodies
        let body = match &self.merge_strategy {
            MergeStrategy::Object => self.merge_as_object(&results),
            MergeStrategy::Array => self.merge_as_array(&results),
            MergeStrategy::First => self.merge_first(&results),
            MergeStrategy::Custom { template } => {
                self.merge_custom(&results, template)
            }
        };

        MergedResponse {
            status,
            body,
            all_success,
            success_count,
            total_count,
        }
    }

    /// Determine HTTP status code for merged response
    fn determine_status(&self, results: &[BackendResult], all_success: bool) -> StatusCode {
        if all_success {
            return StatusCode::OK;
        }

        match self.error_strategy {
            ErrorStrategy::FailFast | ErrorStrategy::Partial => {
                // If we got here, only non-required backends failed
                // Return 207 Multi-Status to indicate partial success
                StatusCode::MULTI_STATUS
            }
            ErrorStrategy::Include => {
                // Include errors in response with 207 Multi-Status
                StatusCode::MULTI_STATUS
            }
            ErrorStrategy::Ignore => {
                // Ignore errors, return 200 OK if we have any success
                if results.iter().any(|r| r.success) {
                    StatusCode::OK
                } else {
                    StatusCode::BAD_GATEWAY
                }
            }
        }
    }

    /// Merge results as a JSON object with backend names as keys
    fn merge_as_object(&self, results: &[BackendResult]) -> Value {
        let mut merged = serde_json::Map::new();

        for result in results {
            let value = match &self.error_strategy {
                ErrorStrategy::Include => {
                    // Include error information
                    if result.success {
                        result.body.clone().unwrap_or(Value::Null)
                    } else {
                        json!({
                            "error": result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                            "status": result.status.as_u16(),
                        })
                    }
                }
                _ => {
                    // Only include successful results
                    if result.success {
                        result.body.clone().unwrap_or(Value::Null)
                    } else {
                        continue;
                    }
                }
            };

            merged.insert(result.name.clone(), value);
        }

        Value::Object(merged)
    }

    /// Merge results as a JSON array
    fn merge_as_array(&self, results: &[BackendResult]) -> Value {
        let mut merged = Vec::new();

        for result in results {
            match &self.error_strategy {
                ErrorStrategy::Include => {
                    // Include both success and error results
                    if result.success {
                        if let Some(body) = &result.body {
                            merged.push(body.clone());
                        }
                    } else {
                        merged.push(json!({
                            "error": result.error.clone().unwrap_or_else(|| "Unknown error".to_string()),
                            "status": result.status.as_u16(),
                            "backend": result.name.clone(),
                        }));
                    }
                }
                _ => {
                    // Only include successful results
                    if result.success {
                        if let Some(body) = &result.body {
                            merged.push(body.clone());
                        }
                    }
                }
            }
        }

        Value::Array(merged)
    }

    /// Take the first successful result
    fn merge_first(&self, results: &[BackendResult]) -> Value {
        for result in results {
            if result.success {
                if let Some(body) = &result.body {
                    debug!("Using first successful result from: {}", result.name);
                    return body.clone();
                }
            }
        }

        warn!("No successful results found, returning null");
        Value::Null
    }

    /// Custom merge using template (simplified)
    fn merge_custom(&self, results: &[BackendResult], template: &str) -> Value {
        // TODO: Full JSONPath template implementation
        // For now, just log and fall back to object merge
        warn!(
            "Custom merge template not fully implemented: {}",
            template
        );
        self.merge_as_object(results)
    }
}

impl MergedResponse {
    /// Convert to JSON bytes for HTTP response
    pub fn to_json_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.body).unwrap_or_else(|e| {
            warn!("Failed to serialize merged response: {}", e);
            b"{}".to_vec()
        })
    }

    /// Get metadata about the merge
    pub fn metadata(&self) -> Value {
        json!({
            "total_backends": self.total_count,
            "successful_backends": self.success_count,
            "all_success": self.all_success,
            "status": self.status.as_u16(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_success_result(name: &str, data: Value) -> BackendResult {
        BackendResult {
            name: name.to_string(),
            success: true,
            status: StatusCode::OK,
            body: Some(data),
            error: None,
            duration_ms: 10.0,
        }
    }

    fn create_error_result(name: &str, error: &str) -> BackendResult {
        BackendResult {
            name: name.to_string(),
            success: false,
            status: StatusCode::BAD_GATEWAY,
            body: None,
            error: Some(error.to_string()),
            duration_ms: 5.0,
        }
    }

    #[test]
    fn test_merge_as_object_all_success() {
        let merger = ResponseMerger::new(MergeStrategy::Object, ErrorStrategy::FailFast);

        let results = vec![
            create_success_result("user", json!({"id": 1, "name": "Alice"})),
            create_success_result("orders", json!({"count": 5})),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.status, StatusCode::OK);
        assert!(merged.all_success);
        assert_eq!(merged.success_count, 2);

        let body = merged.body.as_object().unwrap();
        assert_eq!(body.get("user").unwrap()["name"], "Alice");
        assert_eq!(body.get("orders").unwrap()["count"], 5);
    }

    #[test]
    fn test_merge_as_array() {
        let merger = ResponseMerger::new(MergeStrategy::Array, ErrorStrategy::Ignore);

        let results = vec![
            create_success_result("backend1", json!({"data": "a"})),
            create_success_result("backend2", json!({"data": "b"})),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.status, StatusCode::OK);
        let body = merged.body.as_array().unwrap();
        assert_eq!(body.len(), 2);
        assert_eq!(body[0]["data"], "a");
        assert_eq!(body[1]["data"], "b");
    }

    #[test]
    fn test_merge_first_successful() {
        let merger = ResponseMerger::new(MergeStrategy::First, ErrorStrategy::Ignore);

        let results = vec![
            create_error_result("backend1", "timeout"),
            create_success_result("backend2", json!({"result": "success"})),
            create_success_result("backend3", json!({"result": "also success"})),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.body["result"], "success");
    }

    #[test]
    fn test_merge_with_errors_include_strategy() {
        let merger = ResponseMerger::new(MergeStrategy::Object, ErrorStrategy::Include);

        let results = vec![
            create_success_result("user", json!({"id": 1})),
            create_error_result("orders", "service unavailable"),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.status, StatusCode::MULTI_STATUS);
        assert_eq!(merged.success_count, 1);
        assert!(!merged.all_success);

        let body = merged.body.as_object().unwrap();
        assert_eq!(body.get("user").unwrap()["id"], 1);
        assert!(body.get("orders").unwrap()["error"].is_string());
    }

    #[test]
    fn test_merge_with_errors_ignore_strategy() {
        let merger = ResponseMerger::new(MergeStrategy::Object, ErrorStrategy::Ignore);

        let results = vec![
            create_success_result("user", json!({"id": 1})),
            create_error_result("orders", "service unavailable"),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.status, StatusCode::OK);
        let body = merged.body.as_object().unwrap();
        assert!(body.contains_key("user"));
        assert!(!body.contains_key("orders")); // Error ignored
    }

    #[test]
    fn test_merged_response_to_json_bytes() {
        let merger = ResponseMerger::new(MergeStrategy::Object, ErrorStrategy::FailFast);

        let results = vec![create_success_result("test", json!({"key": "value"}))];

        let merged = merger.merge(results);
        let bytes = merged.to_json_bytes();

        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed["test"]["key"], "value");
    }

    #[test]
    fn test_merged_response_metadata() {
        let merger = ResponseMerger::new(MergeStrategy::Object, ErrorStrategy::Partial);

        let results = vec![
            create_success_result("backend1", json!({})),
            create_success_result("backend2", json!({})),
            create_error_result("backend3", "error"),
        ];

        let merged = merger.merge(results);
        let metadata = merged.metadata();

        assert_eq!(metadata["total_backends"], 3);
        assert_eq!(metadata["successful_backends"], 2);
        assert_eq!(metadata["all_success"], false);
    }

    #[test]
    fn test_all_errors_returns_bad_gateway() {
        let merger = ResponseMerger::new(MergeStrategy::First, ErrorStrategy::Ignore);

        let results = vec![
            create_error_result("backend1", "error1"),
            create_error_result("backend2", "error2"),
        ];

        let merged = merger.merge(results);

        assert_eq!(merged.status, StatusCode::BAD_GATEWAY);
        assert_eq!(merged.success_count, 0);
    }
}
