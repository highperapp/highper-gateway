//! Request/Response validation using JSON Schema
//!
//! Provides JSON Schema validation for request and response bodies.
//! Supports both inline schemas and schema files.

use crate::config::{RequestValidation, ResponseValidation, ValidationAction, ValidationConfig};
use bytes::Bytes;
use jsonschema::{JSONSchema, ValidationError};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, warn};

/// Validation error details
#[derive(Debug, Clone)]
pub struct ValidationErrorDetail {
    /// JSON path where the error occurred
    pub path: String,
    /// Error message
    pub message: String,
}

impl std::fmt::Display for ValidationErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// Result of validation
#[derive(Debug)]
pub enum ValidationResult {
    /// Validation passed
    Valid,
    /// Validation failed with errors
    Invalid(Vec<ValidationErrorDetail>),
    /// Validation skipped (not applicable)
    Skipped,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid | ValidationResult::Skipped)
    }

    pub fn errors(&self) -> Option<&[ValidationErrorDetail]> {
        match self {
            ValidationResult::Invalid(errors) => Some(errors),
            _ => None,
        }
    }
}

/// Compiled schema cache for performance
pub struct SchemaCache {
    schemas: RwLock<HashMap<String, Arc<JSONSchema>>>,
}

impl SchemaCache {
    pub fn new() -> Self {
        Self {
            schemas: RwLock::new(HashMap::new()),
        }
    }

    /// Get or compile a schema from a file path
    pub fn get_or_compile_file(&self, path: &str) -> Result<Arc<JSONSchema>, String> {
        // Check cache first
        {
            let cache = self.schemas.read();
            if let Some(schema) = cache.get(path) {
                return Ok(Arc::clone(schema));
            }
        }

        // Load and compile schema
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read schema file {}: {}", path, e))?;

        let schema_value: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse schema file {}: {}", path, e))?;

        let compiled = JSONSchema::compile(&schema_value)
            .map_err(|e| format!("Failed to compile schema {}: {}", path, e))?;

        let compiled = Arc::new(compiled);

        // Cache it
        {
            let mut cache = self.schemas.write();
            cache.insert(path.to_string(), Arc::clone(&compiled));
        }

        Ok(compiled)
    }

    /// Compile an inline schema
    pub fn compile_inline(&self, schema: &Value) -> Result<Arc<JSONSchema>, String> {
        // Generate a cache key from the schema
        let key = format!("inline:{}", serde_json::to_string(schema).unwrap_or_default());

        // Check cache first
        {
            let cache = self.schemas.read();
            if let Some(compiled) = cache.get(&key) {
                return Ok(Arc::clone(compiled));
            }
        }

        // Compile schema
        let compiled = JSONSchema::compile(schema)
            .map_err(|e| format!("Failed to compile inline schema: {}", e))?;

        let compiled = Arc::new(compiled);

        // Cache it
        {
            let mut cache = self.schemas.write();
            cache.insert(key, Arc::clone(&compiled));
        }

        Ok(compiled)
    }

    /// Clear the schema cache
    pub fn clear(&self) {
        let mut cache = self.schemas.write();
        cache.clear();
    }
}

impl Default for SchemaCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Request validator
pub struct RequestValidator {
    cache: Arc<SchemaCache>,
}

impl RequestValidator {
    pub fn new(cache: Arc<SchemaCache>) -> Self {
        Self { cache }
    }

    /// Validate a request body against the configured schema
    pub fn validate(
        &self,
        config: &RequestValidation,
        content_type: Option<&str>,
        body: &Bytes,
    ) -> ValidationResult {
        // Check if content type should be validated
        if let Some(ct) = content_type {
            let should_validate = config.content_types.iter().any(|allowed| {
                ct.starts_with(allowed) || ct.contains(allowed)
            });

            if !should_validate {
                debug!("Skipping validation for content type: {}", ct);
                return ValidationResult::Skipped;
            }
        } else {
            // No content type header, skip validation
            return ValidationResult::Skipped;
        }

        // Empty body is valid for some schemas, let schema decide
        if body.is_empty() {
            // Parse as null for empty body
            return self.validate_value(config, &Value::Null);
        }

        // Parse the body as JSON
        let value: Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                return ValidationResult::Invalid(vec![ValidationErrorDetail {
                    path: "".to_string(),
                    message: format!("Invalid JSON: {}", e),
                }]);
            }
        };

        self.validate_value(config, &value)
    }

    fn validate_value(&self, config: &RequestValidation, value: &Value) -> ValidationResult {
        // Get the schema
        let schema = if let Some(ref path) = config.schema_file {
            match self.cache.get_or_compile_file(path) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to load schema: {}", e);
                    return ValidationResult::Invalid(vec![ValidationErrorDetail {
                        path: "".to_string(),
                        message: format!("Schema error: {}", e),
                    }]);
                }
            }
        } else if let Some(ref inline) = config.json_schema {
            match self.cache.compile_inline(inline) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to compile inline schema: {}", e);
                    return ValidationResult::Invalid(vec![ValidationErrorDetail {
                        path: "".to_string(),
                        message: format!("Schema error: {}", e),
                    }]);
                }
            }
        } else {
            // No schema configured
            return ValidationResult::Skipped;
        };

        // Validate - use is_valid first for efficiency, then collect errors if needed
        if schema.is_valid(value) {
            ValidationResult::Valid
        } else {
            // Collect errors - need to call validate again to get error details
            let details: Vec<ValidationErrorDetail> = schema
                .validate(value)
                .err()
                .map(|errors| {
                    errors
                        .map(|e| ValidationErrorDetail {
                            path: e.instance_path.to_string(),
                            message: e.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            ValidationResult::Invalid(details)
        }
    }
}

/// Response validator
pub struct ResponseValidator {
    cache: Arc<SchemaCache>,
}

impl ResponseValidator {
    pub fn new(cache: Arc<SchemaCache>) -> Self {
        Self { cache }
    }

    /// Validate a response body against the configured schema
    pub fn validate(
        &self,
        config: &ResponseValidation,
        content_type: Option<&str>,
        body: &Bytes,
    ) -> ValidationResult {
        // Only validate JSON responses
        if let Some(ct) = content_type {
            if !ct.contains("application/json") {
                return ValidationResult::Skipped;
            }
        } else {
            return ValidationResult::Skipped;
        }

        if body.is_empty() {
            return ValidationResult::Valid;
        }

        // Parse the body as JSON
        let value: Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                return ValidationResult::Invalid(vec![ValidationErrorDetail {
                    path: "".to_string(),
                    message: format!("Invalid JSON response: {}", e),
                }]);
            }
        };

        // Get the schema
        let schema = if let Some(ref path) = config.schema_file {
            match self.cache.get_or_compile_file(path) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to load response schema: {}", e);
                    return ValidationResult::Invalid(vec![ValidationErrorDetail {
                        path: "".to_string(),
                        message: format!("Schema error: {}", e),
                    }]);
                }
            }
        } else if let Some(ref inline) = config.json_schema {
            match self.cache.compile_inline(inline) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to compile inline response schema: {}", e);
                    return ValidationResult::Invalid(vec![ValidationErrorDetail {
                        path: "".to_string(),
                        message: format!("Schema error: {}", e),
                    }]);
                }
            }
        } else {
            return ValidationResult::Skipped;
        };

        // Validate - use is_valid first for efficiency, then collect errors if needed
        if schema.is_valid(&value) {
            ValidationResult::Valid
        } else {
            // Collect errors
            let details: Vec<ValidationErrorDetail> = schema
                .validate(&value)
                .err()
                .map(|errors| {
                    errors
                        .map(|e| ValidationErrorDetail {
                            path: e.instance_path.to_string(),
                            message: e.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            ValidationResult::Invalid(details)
        }
    }
}

/// Create a JSON error response for validation failures
pub fn create_validation_error_response(errors: &[ValidationErrorDetail], include_details: bool) -> String {
    if include_details {
        let error_list: Vec<serde_json::Value> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "message": e.message
                })
            })
            .collect();

        serde_json::json!({
            "error": "Validation failed",
            "code": "VALIDATION_ERROR",
            "details": error_list
        }).to_string()
    } else {
        serde_json::json!({
            "error": "Request validation failed",
            "code": "VALIDATION_ERROR"
        }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_cache() {
        let cache = SchemaCache::new();

        let schema = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        });

        // First compilation
        let compiled1 = cache.compile_inline(&schema).unwrap();
        // Should get cached version
        let compiled2 = cache.compile_inline(&schema).unwrap();

        // Both should be the same Arc
        assert!(Arc::ptr_eq(&compiled1, &compiled2));
    }

    #[test]
    fn test_request_validation_valid() {
        let cache = Arc::new(SchemaCache::new());
        let validator = RequestValidator::new(cache);

        let config = RequestValidation {
            schema_file: None,
            json_schema: Some(serde_json::json!({
                "type": "object",
                "required": ["name", "email"],
                "properties": {
                    "name": { "type": "string", "minLength": 1 },
                    "email": { "type": "string", "format": "email" }
                }
            })),
            content_types: vec!["application/json".to_string()],
            on_failure: ValidationAction::Reject,
            include_errors: true,
        };

        let body = Bytes::from(r#"{"name": "John", "email": "john@example.com"}"#);
        let result = validator.validate(&config, Some("application/json"), &body);

        assert!(result.is_valid());
    }

    #[test]
    fn test_request_validation_invalid() {
        let cache = Arc::new(SchemaCache::new());
        let validator = RequestValidator::new(cache);

        let config = RequestValidation {
            schema_file: None,
            json_schema: Some(serde_json::json!({
                "type": "object",
                "required": ["name", "email"],
                "properties": {
                    "name": { "type": "string", "minLength": 1 },
                    "email": { "type": "string" }
                }
            })),
            content_types: vec!["application/json".to_string()],
            on_failure: ValidationAction::Reject,
            include_errors: true,
        };

        // Missing required field
        let body = Bytes::from(r#"{"name": "John"}"#);
        let result = validator.validate(&config, Some("application/json"), &body);

        assert!(!result.is_valid());
        let errors = result.errors().unwrap();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_request_validation_skip_non_json() {
        let cache = Arc::new(SchemaCache::new());
        let validator = RequestValidator::new(cache);

        let config = RequestValidation {
            schema_file: None,
            json_schema: Some(serde_json::json!({ "type": "object" })),
            content_types: vec!["application/json".to_string()],
            on_failure: ValidationAction::Reject,
            include_errors: true,
        };

        let body = Bytes::from("plain text");
        let result = validator.validate(&config, Some("text/plain"), &body);

        // Should skip validation for non-JSON content
        assert!(matches!(result, ValidationResult::Skipped));
    }

    #[test]
    fn test_request_validation_invalid_json() {
        let cache = Arc::new(SchemaCache::new());
        let validator = RequestValidator::new(cache);

        let config = RequestValidation {
            schema_file: None,
            json_schema: Some(serde_json::json!({ "type": "object" })),
            content_types: vec!["application/json".to_string()],
            on_failure: ValidationAction::Reject,
            include_errors: true,
        };

        let body = Bytes::from("not valid json {");
        let result = validator.validate(&config, Some("application/json"), &body);

        assert!(!result.is_valid());
        let errors = result.errors().unwrap();
        assert!(errors[0].message.contains("Invalid JSON"));
    }

    #[test]
    fn test_validation_error_response() {
        let errors = vec![
            ValidationErrorDetail {
                path: "/email".to_string(),
                message: "\"email\" is a required property".to_string(),
            },
            ValidationErrorDetail {
                path: "/age".to_string(),
                message: "must be a positive integer".to_string(),
            },
        ];

        let response = create_validation_error_response(&errors, true);
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["code"], "VALIDATION_ERROR");
        assert_eq!(parsed["details"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_validation_error_response_without_details() {
        let errors = vec![ValidationErrorDetail {
            path: "/name".to_string(),
            message: "required".to_string(),
        }];

        let response = create_validation_error_response(&errors, false);
        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(parsed["code"], "VALIDATION_ERROR");
        assert!(parsed.get("details").is_none());
    }

    #[test]
    fn test_type_validation() {
        let cache = Arc::new(SchemaCache::new());
        let validator = RequestValidator::new(cache);

        let config = RequestValidation {
            schema_file: None,
            json_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "age": { "type": "integer", "minimum": 0 },
                    "active": { "type": "boolean" }
                }
            })),
            content_types: vec!["application/json".to_string()],
            on_failure: ValidationAction::Reject,
            include_errors: true,
        };

        // Invalid type
        let body = Bytes::from(r#"{"age": "not a number", "active": true}"#);
        let result = validator.validate(&config, Some("application/json"), &body);

        assert!(!result.is_valid());
    }
}
