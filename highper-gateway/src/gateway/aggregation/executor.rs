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
            client.forward(base_url, method, &path, headers, None), // No body for aggregation requests
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

    /// Extract value using JSONPath with full feature support
    ///
    /// Supports:
    /// - Root: `$`
    /// - Dot notation: `$.data.user.name`
    /// - Bracket notation: `$['data']['user']['name']`
    /// - Array indexing: `$.users[0]`, `$.users[1]`
    /// - Array slicing: `$.users[0:3]`, `$.users[:2]`, `$.users[1:]`
    /// - Wildcards: `$.users[*].name`, `$.*`
    /// - Recursive descent: `$..name` (finds all 'name' fields at any depth)
    /// - Filters: `$.users[?(@.age > 18)]`
    fn extract_json_path(
        value: &Option<serde_json::Value>,
        path: &str,
    ) -> Option<serde_json::Value> {
        let value = value.as_ref()?;

        if path == "$" || path.is_empty() {
            return Some(value.clone());
        }

        // Parse and evaluate JSONPath
        match Self::evaluate_jsonpath(value, path) {
            Ok(results) => {
                if results.is_empty() {
                    None
                } else if results.len() == 1 {
                    Some(results[0].clone())
                } else {
                    // Multiple results - return as array
                    Some(serde_json::Value::Array(results))
                }
            }
            Err(e) => {
                warn!("JSONPath evaluation failed for '{}': {}", path, e);
                None
            }
        }
    }

    /// Evaluate JSONPath expression
    fn evaluate_jsonpath(
        value: &serde_json::Value,
        path: &str,
    ) -> Result<Vec<serde_json::Value>> {
        let path = path.trim();

        // Handle root
        if path == "$" {
            return Ok(vec![value.clone()]);
        }

        // Check for recursive descent first (before stripping prefix)
        if path.starts_with("$..") {
            let field = path.trim_start_matches("$..");
            return Ok(Self::recursive_descent(value, field));
        }

        // Remove leading $. or $
        let path = path.strip_prefix("$.").or_else(|| path.strip_prefix("$")).unwrap_or(path);

        // Parse path segments
        let segments = Self::parse_jsonpath_segments(path)?;

        // Evaluate segments
        let mut current_values = vec![value.clone()];

        for segment in segments {
            let mut next_values = Vec::new();

            for val in &current_values {
                match &segment {
                    PathSegment::Field(field) => {
                        if let Some(v) = val.get(field) {
                            next_values.push(v.clone());
                        }
                    }
                    PathSegment::Index(index) => {
                        if let Some(arr) = val.as_array() {
                            let idx = if *index < 0 {
                                (arr.len() as i64 + index) as usize
                            } else {
                                *index as usize
                            };
                            if idx < arr.len() {
                                next_values.push(arr[idx].clone());
                            }
                        }
                    }
                    PathSegment::Slice(start, end) => {
                        if let Some(arr) = val.as_array() {
                            let len = arr.len() as i64;
                            let start = start.unwrap_or(0).max(0) as usize;
                            let end = end.unwrap_or(len).min(len) as usize;

                            for item in &arr[start..end] {
                                next_values.push(item.clone());
                            }
                        }
                    }
                    PathSegment::Wildcard => {
                        if let Some(obj) = val.as_object() {
                            for v in obj.values() {
                                next_values.push(v.clone());
                            }
                        } else if let Some(arr) = val.as_array() {
                            for item in arr {
                                next_values.push(item.clone());
                            }
                        }
                    }
                    PathSegment::Filter(expr) => {
                        if let Some(arr) = val.as_array() {
                            for item in arr {
                                if Self::evaluate_filter(item, expr) {
                                    next_values.push(item.clone());
                                }
                            }
                        }
                    }
                    PathSegment::RecursiveDescent(field) => {
                        next_values.extend(Self::recursive_descent(val, field));
                    }
                }
            }

            current_values = next_values;
        }

        Ok(current_values)
    }

    /// Parse JSONPath segments
    fn parse_jsonpath_segments(path: &str) -> Result<Vec<PathSegment>> {
        let mut segments = Vec::new();
        let mut chars = path.chars().peekable();
        let mut current_field = String::new();

        while let Some(ch) = chars.next() {
            match ch {
                '.' => {
                    // Check for recursive descent (..)
                    if chars.peek() == Some(&'.') {
                        chars.next(); // consume second '.'

                        // Parse field name after ..
                        let mut field = String::new();
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch == '.' || next_ch == '[' {
                                break;
                            }
                            field.push(chars.next().unwrap());
                        }

                        segments.push(PathSegment::RecursiveDescent(field));
                    } else {
                        // Regular field separator
                        if !current_field.is_empty() {
                            segments.push(PathSegment::Field(current_field.clone()));
                            current_field.clear();
                        }
                    }
                }
                '[' => {
                    // Save current field if any
                    if !current_field.is_empty() {
                        segments.push(PathSegment::Field(current_field.clone()));
                        current_field.clear();
                    }

                    // Parse bracket content
                    let mut bracket_content = String::new();
                    let mut bracket_depth = 1;

                    while let Some(next_ch) = chars.next() {
                        if next_ch == '[' {
                            bracket_depth += 1;
                            bracket_content.push(next_ch);
                        } else if next_ch == ']' {
                            bracket_depth -= 1;
                            if bracket_depth == 0 {
                                break;
                            }
                            bracket_content.push(next_ch);
                        } else {
                            bracket_content.push(next_ch);
                        }
                    }

                    segments.push(Self::parse_bracket_content(&bracket_content)?);
                }
                _ => {
                    current_field.push(ch);
                }
            }
        }

        // Add final field if any
        if !current_field.is_empty() {
            segments.push(PathSegment::Field(current_field));
        }

        Ok(segments)
    }

    /// Parse bracket content: index, slice, wildcard, or filter
    fn parse_bracket_content(content: &str) -> Result<PathSegment> {
        let content = content.trim().trim_matches('\'').trim_matches('"');

        // Wildcard
        if content == "*" {
            return Ok(PathSegment::Wildcard);
        }

        // Filter expression
        if content.starts_with("?(") && content.ends_with(')') {
            let filter_expr = content[2..content.len()-1].to_string();
            return Ok(PathSegment::Filter(filter_expr));
        }

        // Slice notation: start:end
        if content.contains(':') {
            let parts: Vec<&str> = content.split(':').collect();
            let start = if parts[0].is_empty() {
                None
            } else {
                Some(parts[0].parse::<i64>()?)
            };
            let end = if parts.len() > 1 && !parts[1].is_empty() {
                Some(parts[1].parse::<i64>()?)
            } else {
                None
            };
            return Ok(PathSegment::Slice(start, end));
        }

        // Index
        if let Ok(index) = content.parse::<i64>() {
            return Ok(PathSegment::Index(index));
        }

        // Field name in bracket notation
        Ok(PathSegment::Field(content.to_string()))
    }

    /// Evaluate filter expression
    fn evaluate_filter(value: &serde_json::Value, expr: &str) -> bool {
        // Simple filter evaluation
        // Supports: @.field op value
        // Examples: @.age > 18, @.name == "John", @.active == true

        let expr = expr.trim().replace("@.", "");

        // Check for >= first (before >)
        if let Some(pos) = expr.find(">=") {
            let field = expr[..pos].trim();
            let right = expr[pos + 2..].trim();

            if let Some(field_value) = value.get(field) {
                if let (Some(a), Ok(b)) = (field_value.as_f64(), right.parse::<f64>()) {
                    return a >= b;
                }
            }
            return false;
        }

        // Check for <= (before <)
        if let Some(pos) = expr.find("<=") {
            let field = expr[..pos].trim();
            let right = expr[pos + 2..].trim();

            if let Some(field_value) = value.get(field) {
                if let (Some(a), Ok(b)) = (field_value.as_f64(), right.parse::<f64>()) {
                    return a <= b;
                }
            }
            return false;
        }

        // Check for == (equality)
        if let Some(pos) = expr.find("==") {
            let field = expr[..pos].trim();
            let right = expr[pos + 2..].trim().trim_matches('"').trim_matches('\'');

            if let Some(field_value) = value.get(field) {
                let right_value = if right == "true" {
                    serde_json::Value::Bool(true)
                } else if right == "false" {
                    serde_json::Value::Bool(false)
                } else if let Ok(num) = right.parse::<i64>() {
                    serde_json::Value::Number(num.into())
                } else if let Ok(num) = right.parse::<f64>() {
                    serde_json::Number::from_f64(num)
                        .map(serde_json::Value::Number)
                        .unwrap_or_else(|| serde_json::Value::String(right.to_string()))
                } else {
                    serde_json::Value::String(right.to_string())
                };

                return field_value == &right_value;
            }
            return false;
        }

        // Check for != (inequality)
        if let Some(pos) = expr.find("!=") {
            let field = expr[..pos].trim();
            let right = expr[pos + 2..].trim().trim_matches('"').trim_matches('\'');

            if let Some(field_value) = value.get(field) {
                let right_value = if right == "true" {
                    serde_json::Value::Bool(true)
                } else if right == "false" {
                    serde_json::Value::Bool(false)
                } else if let Ok(num) = right.parse::<i64>() {
                    serde_json::Value::Number(num.into())
                } else if let Ok(num) = right.parse::<f64>() {
                    serde_json::Number::from_f64(num)
                        .map(serde_json::Value::Number)
                        .unwrap_or_else(|| serde_json::Value::String(right.to_string()))
                } else {
                    serde_json::Value::String(right.to_string())
                };

                return field_value != &right_value;
            }
            return false;
        }

        // Check for > (greater than)
        if let Some(pos) = expr.find('>') {
            let field = expr[..pos].trim();
            let right = expr[pos + 1..].trim();

            if let Some(field_value) = value.get(field) {
                if let (Some(a), Ok(b)) = (field_value.as_f64(), right.parse::<f64>()) {
                    return a > b;
                }
            }
            return false;
        }

        // Check for < (less than)
        if let Some(pos) = expr.find('<') {
            let field = expr[..pos].trim();
            let right = expr[pos + 1..].trim();

            if let Some(field_value) = value.get(field) {
                if let (Some(a), Ok(b)) = (field_value.as_f64(), right.parse::<f64>()) {
                    return a < b;
                }
            }
            return false;
        }

        false
    }

    /// Recursive descent - find all matching fields at any depth
    fn recursive_descent(value: &serde_json::Value, field: &str) -> Vec<serde_json::Value> {
        let mut results = Vec::new();

        fn traverse(value: &serde_json::Value, field: &str, results: &mut Vec<serde_json::Value>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, val) in map {
                        // If field name matches, collect this value
                        if key == field {
                            results.push(val.clone());
                        }
                        // Continue traversing into nested structures
                        traverse(val, field, results);
                    }
                }
                serde_json::Value::Array(arr) => {
                    for item in arr {
                        traverse(item, field, results);
                    }
                }
                _ => {}
            }
        }

        traverse(value, field, &mut results);
        results
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

/// JSONPath segment types
#[derive(Debug, Clone)]
enum PathSegment {
    /// Field access: .field or ['field']
    Field(String),
    /// Array index: [0], [1], [-1]
    Index(i64),
    /// Array slice: [0:3], [:2], [1:]
    Slice(Option<i64>, Option<i64>),
    /// Wildcard: [*] or .*
    Wildcard,
    /// Filter: [?(@.age > 18)]
    Filter(String),
    /// Recursive descent: ..field
    RecursiveDescent(String),
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
    fn test_extract_json_path_array_index() {
        let json = serde_json::json!({
            "users": [
                {"name": "Alice"},
                {"name": "Bob"},
                {"name": "Charlie"}
            ]
        });

        let result = AggregationExecutor::extract_json_path(&Some(json.clone()), "$.users[0].name");
        assert_eq!(result, Some(serde_json::json!("Alice")));

        let result = AggregationExecutor::extract_json_path(&Some(json.clone()), "$.users[1].name");
        assert_eq!(result, Some(serde_json::json!("Bob")));

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.users[-1].name");
        assert_eq!(result, Some(serde_json::json!("Charlie")));
    }

    #[test]
    fn test_extract_json_path_array_slice() {
        let json = serde_json::json!({
            "numbers": [1, 2, 3, 4, 5]
        });

        let result = AggregationExecutor::extract_json_path(&Some(json.clone()), "$.numbers[0:3]");
        assert_eq!(result, Some(serde_json::json!([1, 2, 3])));

        let result = AggregationExecutor::extract_json_path(&Some(json.clone()), "$.numbers[:2]");
        assert_eq!(result, Some(serde_json::json!([1, 2])));

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.numbers[2:]");
        assert_eq!(result, Some(serde_json::json!([3, 4, 5])));
    }

    #[test]
    fn test_extract_json_path_wildcard() {
        let json = serde_json::json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25},
                {"name": "Charlie", "age": 35}
            ]
        });

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.users[*].name");
        assert_eq!(result, Some(serde_json::json!(["Alice", "Bob", "Charlie"])));
    }

    #[test]
    fn test_extract_json_path_filter() {
        let json = serde_json::json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 17},
                {"name": "Charlie", "age": 25}
            ]
        });

        let result = AggregationExecutor::extract_json_path(&Some(json), "$.users[?(@.age > 18)]");
        assert!(result.is_some());
        let results = result.unwrap();
        assert!(results.is_array());
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 2); // Alice and Charlie
    }

    #[test]
    fn test_extract_json_path_recursive_descent() {
        let json = serde_json::json!({
            "store": {
                "book": [
                    {"title": "Book 1", "price": 10.0},
                    {"title": "Book 2", "price": 20.0}
                ],
                "bicycle": {
                    "price": 100.0
                }
            }
        });

        let result = AggregationExecutor::extract_json_path(&Some(json), "$..price");
        assert!(result.is_some());
        let results = result.unwrap();
        assert!(results.is_array());
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 3); // All prices
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
