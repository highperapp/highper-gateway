//! Response and Request Transformation
//!
//! Provides JSON transformation capabilities for modifying request/response bodies.
//! Supports field renaming, removal, addition, flattening, and extraction.

use crate::config::{RequestTransform, ResponseTransform};
use bytes::Bytes;
use serde_json::{Map, Value};
use tracing::{debug, warn};

/// Result of transformation
#[derive(Debug)]
pub enum TransformResult {
    /// Transformation applied successfully
    Transformed(Bytes),
    /// Transformation skipped (not applicable)
    Skipped,
    /// Transformation failed with error
    Error(String),
}

/// Transform a response body according to configuration
pub fn transform_response(
    config: &ResponseTransform,
    content_type: Option<&str>,
    body: &Bytes,
) -> TransformResult {
    // Check if content type should be transformed
    if let Some(ct) = content_type {
        let should_transform = config.content_types.iter().any(|allowed| {
            ct.starts_with(allowed) || ct.contains(allowed)
        });

        if !should_transform {
            debug!("Skipping transformation for content type: {}", ct);
            return TransformResult::Skipped;
        }
    } else {
        return TransformResult::Skipped;
    }

    if body.is_empty() {
        return TransformResult::Skipped;
    }

    // Parse the body as JSON
    let mut value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse JSON for transformation: {}", e);
            return TransformResult::Error(format!("Invalid JSON: {}", e));
        }
    };

    // Apply transformations
    if config.map_array && value.is_array() {
        // Apply transformations to each array element
        if let Value::Array(arr) = &mut value {
            for item in arr.iter_mut() {
                apply_response_transforms(config, item);
            }
        }
    } else {
        apply_response_transforms(config, &mut value);
    }

    // Serialize back to bytes
    match serde_json::to_vec(&value) {
        Ok(bytes) => TransformResult::Transformed(Bytes::from(bytes)),
        Err(e) => TransformResult::Error(format!("Failed to serialize: {}", e)),
    }
}

/// Apply all response transformation rules to a JSON value
fn apply_response_transforms(config: &ResponseTransform, value: &mut Value) {
    // 1. Extract nested field as root (do this first)
    if let Some(extract_path) = &config.extract {
        if let Some(extracted) = get_nested_value(value, extract_path) {
            *value = extracted.clone();
        }
    }

    // 2. Flatten nested objects
    if !config.flatten.is_empty() {
        apply_flatten(value, &config.flatten);
    }

    // 3. Pick only specific fields (whitelist)
    if !config.pick.is_empty() {
        apply_pick(value, &config.pick);
    }

    // 4. Remove fields
    for path in &config.remove {
        remove_field(value, path);
    }

    // 5. Rename fields
    for (old_name, new_name) in &config.rename {
        rename_field(value, old_name, new_name);
    }

    // 6. Add fields
    for (path, add_value) in &config.add {
        add_field(value, path, add_value.clone());
    }
}

/// Transform a request body according to configuration
pub fn transform_request(
    config: &RequestTransform,
    content_type: Option<&str>,
    body: &Bytes,
) -> TransformResult {
    // Check if content type should be transformed
    if let Some(ct) = content_type {
        let should_transform = config.content_types.iter().any(|allowed| {
            ct.starts_with(allowed) || ct.contains(allowed)
        });

        if !should_transform {
            return TransformResult::Skipped;
        }
    } else {
        return TransformResult::Skipped;
    }

    if body.is_empty() {
        return TransformResult::Skipped;
    }

    // Parse the body as JSON
    let mut value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return TransformResult::Error(format!("Invalid JSON: {}", e));
        }
    };

    // Apply transformations
    apply_request_transforms(config, &mut value);

    // Serialize back to bytes
    match serde_json::to_vec(&value) {
        Ok(bytes) => TransformResult::Transformed(Bytes::from(bytes)),
        Err(e) => TransformResult::Error(format!("Failed to serialize: {}", e)),
    }
}

/// Apply all request transformation rules to a JSON value
fn apply_request_transforms(config: &RequestTransform, value: &mut Value) {
    // Remove fields
    for path in &config.remove {
        remove_field(value, path);
    }

    // Rename fields
    for (old_name, new_name) in &config.rename {
        rename_field(value, old_name, new_name);
    }

    // Add fields
    for (path, add_value) in &config.add {
        add_field(value, path, add_value.clone());
    }
}

/// Get a nested value by dot-separated path
fn get_nested_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            Value::Object(obj) => {
                current = obj.get(part)?;
            }
            Value::Array(arr) => {
                let index: usize = part.parse().ok()?;
                current = arr.get(index)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

/// Remove a field by path (supports dot notation)
fn remove_field(value: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        // Simple field removal
        if let Value::Object(obj) = value {
            obj.remove(path);
        }
    } else {
        // Navigate to parent and remove
        let parent_path = &parts[..parts.len() - 1];
        let field_name = parts.last().unwrap();

        if let Some(parent) = get_nested_value_mut(value, parent_path) {
            if let Value::Object(obj) = parent {
                obj.remove(*field_name);
            }
        }
    }
}

/// Rename a field (at root level)
fn rename_field(value: &mut Value, old_name: &str, new_name: &str) {
    if let Value::Object(obj) = value {
        if let Some(val) = obj.remove(old_name) {
            obj.insert(new_name.to_string(), val);
        }
    }
}

/// Add a field by path
fn add_field(value: &mut Value, path: &str, new_value: Value) {
    let parts: Vec<&str> = path.split('.').collect();

    if parts.len() == 1 {
        // Simple field addition
        if let Value::Object(obj) = value {
            obj.insert(path.to_string(), new_value);
        }
    } else {
        // Navigate to parent and add
        let parent_path = &parts[..parts.len() - 1];
        let field_name = parts.last().unwrap();

        if let Some(parent) = get_nested_value_mut(value, parent_path) {
            if let Value::Object(obj) = parent {
                obj.insert((*field_name).to_string(), new_value);
            }
        }
    }
}

/// Get a mutable reference to a nested value
fn get_nested_value_mut<'a>(value: &'a mut Value, parts: &[&str]) -> Option<&'a mut Value> {
    let mut current = value;

    for part in parts {
        match current {
            Value::Object(obj) => {
                current = obj.get_mut(*part)?;
            }
            Value::Array(arr) => {
                let index: usize = part.parse().ok()?;
                current = arr.get_mut(index)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

/// Flatten nested objects by bringing their fields to the root
fn apply_flatten(value: &mut Value, paths: &[String]) {
    if let Value::Object(obj) = value {
        let mut fields_to_add = Map::new();
        let mut keys_to_remove = Vec::new();

        for path in paths {
            if let Some(nested_value) = obj.get(path) {
                if let Value::Object(nested_obj) = nested_value {
                    for (k, v) in nested_obj {
                        fields_to_add.insert(k.clone(), v.clone());
                    }
                    keys_to_remove.push(path.clone());
                }
            }
        }

        // Remove the nested objects
        for key in keys_to_remove {
            obj.remove(&key);
        }

        // Add the flattened fields
        for (k, v) in fields_to_add {
            obj.insert(k, v);
        }
    }
}

/// Pick only specific fields (whitelist)
fn apply_pick(value: &mut Value, fields: &[String]) {
    if let Value::Object(obj) = value {
        let picked: Map<String, Value> = obj
            .iter()
            .filter(|(k, _)| fields.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        *obj = picked;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rename_field() {
        let config = ResponseTransform {
            rename: [("user_name".to_string(), "username".to_string())]
                .iter()
                .cloned()
                .collect(),
            ..Default::default()
        };

        let body = Bytes::from(r#"{"user_name": "john", "age": 30}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["username"], "john");
            assert!(value.get("user_name").is_none());
            assert_eq!(value["age"], 30);
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_remove_field() {
        let config = ResponseTransform {
            remove: vec!["password".to_string(), "internal_id".to_string()],
            ..Default::default()
        };

        let body = Bytes::from(r#"{"name": "john", "password": "secret", "internal_id": 123}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["name"], "john");
            assert!(value.get("password").is_none());
            assert!(value.get("internal_id").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_add_field() {
        let config = ResponseTransform {
            add: [
                ("api_version".to_string(), Value::String("2.0".to_string())),
                ("processed".to_string(), Value::Bool(true)),
            ]
            .iter()
            .cloned()
            .collect(),
            ..Default::default()
        };

        let body = Bytes::from(r#"{"name": "john"}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["name"], "john");
            assert_eq!(value["api_version"], "2.0");
            assert_eq!(value["processed"], true);
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_flatten() {
        let config = ResponseTransform {
            flatten: vec!["user".to_string()],
            ..Default::default()
        };

        let body = Bytes::from(r#"{"id": 1, "user": {"name": "john", "email": "john@example.com"}}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["id"], 1);
            assert_eq!(value["name"], "john");
            assert_eq!(value["email"], "john@example.com");
            assert!(value.get("user").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_pick() {
        let config = ResponseTransform {
            pick: vec!["id".to_string(), "name".to_string()],
            ..Default::default()
        };

        let body = Bytes::from(r#"{"id": 1, "name": "john", "email": "john@example.com", "password": "secret"}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["id"], 1);
            assert_eq!(value["name"], "john");
            assert!(value.get("email").is_none());
            assert!(value.get("password").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_extract() {
        let config = ResponseTransform {
            extract: Some("data".to_string()),
            ..Default::default()
        };

        let body = Bytes::from(r#"{"status": "ok", "data": {"id": 1, "name": "john"}}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["id"], 1);
            assert_eq!(value["name"], "john");
            assert!(value.get("status").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_map_array() {
        let config = ResponseTransform {
            remove: vec!["password".to_string()],
            map_array: true,
            ..Default::default()
        };

        let body = Bytes::from(r#"[{"name": "john", "password": "x"}, {"name": "jane", "password": "y"}]"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            let arr = value.as_array().unwrap();
            assert_eq!(arr.len(), 2);
            assert!(arr[0].get("password").is_none());
            assert!(arr[1].get("password").is_none());
            assert_eq!(arr[0]["name"], "john");
            assert_eq!(arr[1]["name"], "jane");
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_skip_non_json() {
        let config = ResponseTransform::default();

        let body = Bytes::from("plain text");
        let result = transform_response(&config, Some("text/plain"), &body);

        assert!(matches!(result, TransformResult::Skipped));
    }

    #[test]
    fn test_combined_transforms() {
        let config = ResponseTransform {
            rename: [("user_name".to_string(), "name".to_string())]
                .iter()
                .cloned()
                .collect(),
            remove: vec!["internal_id".to_string()],
            add: [("api_version".to_string(), Value::String("v2".to_string()))]
                .iter()
                .cloned()
                .collect(),
            ..Default::default()
        };

        let body = Bytes::from(r#"{"user_name": "john", "internal_id": 123, "email": "john@example.com"}"#);
        let result = transform_response(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["name"], "john");
            assert_eq!(value["email"], "john@example.com");
            assert_eq!(value["api_version"], "v2");
            assert!(value.get("user_name").is_none());
            assert!(value.get("internal_id").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }

    #[test]
    fn test_request_transform() {
        let config = RequestTransform {
            rename: [("userName".to_string(), "user_name".to_string())]
                .iter()
                .cloned()
                .collect(),
            remove: vec!["csrf_token".to_string()],
            add: [("source".to_string(), Value::String("api".to_string()))]
                .iter()
                .cloned()
                .collect(),
            ..Default::default()
        };

        let body = Bytes::from(r#"{"userName": "john", "csrf_token": "abc123"}"#);
        let result = transform_request(&config, Some("application/json"), &body);

        if let TransformResult::Transformed(transformed) = result {
            let value: Value = serde_json::from_slice(&transformed).unwrap();
            assert_eq!(value["user_name"], "john");
            assert_eq!(value["source"], "api");
            assert!(value.get("userName").is_none());
            assert!(value.get("csrf_token").is_none());
        } else {
            panic!("Expected Transformed result");
        }
    }
}
