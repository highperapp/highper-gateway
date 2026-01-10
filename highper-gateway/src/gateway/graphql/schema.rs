//! GraphQL schema registry
//!
//! Manages schemas from multiple GraphQL backends

use anyhow::{Context, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

use crate::proxy::Client;

/// Schema information for a GraphQL backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    /// Backend name
    pub name: String,

    /// Schema SDL (Schema Definition Language)
    pub sdl: String,

    /// Introspection result
    pub introspection: Value,

    /// Types defined in this schema
    pub types: Vec<TypeDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinition {
    pub name: String,
    pub kind: TypeKind,
    pub fields: Vec<FieldDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind {
    Object,
    Interface,
    Union,
    Enum,
    InputObject,
    Scalar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub type_name: String,
    pub args: Vec<ArgumentDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgumentDefinition {
    pub name: String,
    pub type_name: String,
}

/// Registry for managing schemas from multiple backends
pub struct SchemaRegistry {
    schemas: DashMap<String, Schema>,
}

impl SchemaRegistry {
    /// Create a new schema registry
    pub fn new() -> Self {
        Self {
            schemas: DashMap::new(),
        }
    }

    /// Load schema from a GraphQL backend
    pub async fn load_schema(&self, name: &str, url: &str, client: &Client) -> Result<()> {
        info!("Loading schema from backend: {} at {}", name, url);

        // GraphQL introspection query
        let introspection_query = r#"
            query IntrospectionQuery {
                __schema {
                    queryType { name }
                    mutationType { name }
                    subscriptionType { name }
                    types {
                        ...FullType
                    }
                    directives {
                        name
                        description
                        locations
                        args {
                            ...InputValue
                        }
                    }
                }
            }

            fragment FullType on __Type {
                kind
                name
                description
                fields(includeDeprecated: true) {
                    name
                    description
                    args {
                        ...InputValue
                    }
                    type {
                        ...TypeRef
                    }
                    isDeprecated
                    deprecationReason
                }
                inputFields {
                    ...InputValue
                }
                interfaces {
                    ...TypeRef
                }
                enumValues(includeDeprecated: true) {
                    name
                    description
                    isDeprecated
                    deprecationReason
                }
                possibleTypes {
                    ...TypeRef
                }
            }

            fragment InputValue on __InputValue {
                name
                description
                type { ...TypeRef }
                defaultValue
            }

            fragment TypeRef on __Type {
                kind
                name
                ofType {
                    kind
                    name
                    ofType {
                        kind
                        name
                        ofType {
                            kind
                            name
                            ofType {
                                kind
                                name
                                ofType {
                                    kind
                                    name
                                    ofType {
                                        kind
                                        name
                                        ofType {
                                            kind
                                            name
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        "#;

        let request_body = serde_json::json!({
            "query": introspection_query
        });

        // Serialize request body to JSON bytes
        let body_json = serde_json::to_vec(&request_body)
            .context("Failed to serialize introspection query")?;
        let body_bytes = bytes::Bytes::from(body_json);

        // Make request to backend
        let mut headers = hyper::HeaderMap::new();
        headers.insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("application/json"),
        );

        let response = client
            .forward(url, hyper::Method::POST, "/graphql", headers, Some(body_bytes))
            .await
            .context("Failed to fetch schema")?;

        let body_bytes = http_body_util::BodyExt::collect(response.into_body())
            .await
            .context("Failed to read response body")?
            .to_bytes();

        let introspection_result: Value = serde_json::from_slice(&body_bytes)
            .context("Failed to parse introspection result")?;

        // Parse schema types
        let types = self.parse_types(&introspection_result)?;

        // Generate SDL from introspection
        let sdl = self.generate_sdl(&introspection_result)?;

        let schema = Schema {
            name: name.to_string(),
            sdl,
            introspection: introspection_result,
            types,
        };

        self.schemas.insert(name.to_string(), schema);
        info!("Successfully loaded schema from {}", name);

        Ok(())
    }

    /// Get schema by name
    pub fn get_schema(&self, name: &str) -> Option<Schema> {
        self.schemas.get(name).map(|s| s.clone())
    }

    /// Get all schemas
    pub fn get_all_schemas(&self) -> Vec<Schema> {
        self.schemas.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Find type across all schemas
    pub fn find_type(&self, type_name: &str) -> Option<(String, TypeDefinition)> {
        for entry in self.schemas.iter() {
            let schema = entry.value();
            for type_def in &schema.types {
                if type_def.name == type_name {
                    return Some((schema.name.clone(), type_def.clone()));
                }
            }
        }
        None
    }

    fn parse_types(&self, introspection: &Value) -> Result<Vec<TypeDefinition>> {
        let mut types = Vec::new();

        if let Some(schema) = introspection.get("data").and_then(|d| d.get("__schema")) {
            if let Some(type_array) = schema.get("types").and_then(|t| t.as_array()) {
                for type_value in type_array {
                    if let Some(type_def) = self.parse_type(type_value) {
                        types.push(type_def);
                    }
                }
            }
        }

        Ok(types)
    }

    fn parse_type(&self, type_value: &Value) -> Option<TypeDefinition> {
        let name = type_value.get("name")?.as_str()?.to_string();
        let kind_str = type_value.get("kind")?.as_str()?;

        let kind = match kind_str {
            "OBJECT" => TypeKind::Object,
            "INTERFACE" => TypeKind::Interface,
            "UNION" => TypeKind::Union,
            "ENUM" => TypeKind::Enum,
            "INPUT_OBJECT" => TypeKind::InputObject,
            "SCALAR" => TypeKind::Scalar,
            _ => return None,
        };

        let mut fields = Vec::new();
        if let Some(field_array) = type_value.get("fields").and_then(|f| f.as_array()) {
            for field_value in field_array {
                if let Some(field_def) = self.parse_field(field_value) {
                    fields.push(field_def);
                }
            }
        }

        Some(TypeDefinition { name, kind, fields })
    }

    fn parse_field(&self, field_value: &Value) -> Option<FieldDefinition> {
        let name = field_value.get("name")?.as_str()?.to_string();
        let type_name = self.extract_type_name(field_value.get("type")?)?;

        let mut args = Vec::new();
        if let Some(arg_array) = field_value.get("args").and_then(|a| a.as_array()) {
            for arg_value in arg_array {
                if let Some(arg_def) = self.parse_argument(arg_value) {
                    args.push(arg_def);
                }
            }
        }

        Some(FieldDefinition {
            name,
            type_name,
            args,
        })
    }

    fn parse_argument(&self, arg_value: &Value) -> Option<ArgumentDefinition> {
        let name = arg_value.get("name")?.as_str()?.to_string();
        let type_name = self.extract_type_name(arg_value.get("type")?)?;

        Some(ArgumentDefinition { name, type_name })
    }

    fn extract_type_name(&self, type_value: &Value) -> Option<String> {
        if let Some(name) = type_value.get("name").and_then(|n| n.as_str()) {
            return Some(name.to_string());
        }

        if let Some(of_type) = type_value.get("ofType") {
            return self.extract_type_name(of_type);
        }

        None
    }

    fn generate_sdl(&self, introspection: &Value) -> Result<String> {
        // Simplified SDL generation from introspection
        // In production, use a proper SDL generator
        let mut sdl = String::new();

        if let Some(schema) = introspection.get("data").and_then(|d| d.get("__schema")) {
            if let Some(types) = schema.get("types").and_then(|t| t.as_array()) {
                for type_value in types {
                    if let Some(name) = type_value.get("name").and_then(|n| n.as_str()) {
                        // Skip introspection types
                        if name.starts_with("__") {
                            continue;
                        }

                        if let Some(kind) = type_value.get("kind").and_then(|k| k.as_str()) {
                            match kind {
                                "OBJECT" => {
                                    sdl.push_str(&format!("type {} {{\n", name));
                                    if let Some(fields) = type_value.get("fields").and_then(|f| f.as_array()) {
                                        for field in fields {
                                            if let Some(field_name) = field.get("name").and_then(|n| n.as_str()) {
                                                sdl.push_str(&format!("  {}: String\n", field_name));
                                            }
                                        }
                                    }
                                    sdl.push_str("}\n\n");
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(sdl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_registry() {
        let registry = SchemaRegistry::new();
        assert_eq!(registry.get_all_schemas().len(), 0);
    }

    #[test]
    fn test_type_parsing() {
        let type_value = serde_json::json!({
            "name": "User",
            "kind": "OBJECT",
            "fields": [
                {
                    "name": "id",
                    "type": {
                        "name": "ID",
                        "kind": "SCALAR"
                    },
                    "args": []
                }
            ]
        });

        let registry = SchemaRegistry::new();
        let type_def = registry.parse_type(&type_value);
        assert!(type_def.is_some());
        assert_eq!(type_def.unwrap().name, "User");
    }
}
