//! GraphQL schema stitching
//!
//! Combines multiple GraphQL schemas into a unified schema,
//! analyzes queries, routes fields to appropriate backends,
//! and merges results.

use anyhow::{Context, Result};
use async_graphql_parser::parse_query;
use serde_json::Value;
use std::collections::HashMap;
use tracing::debug;

use super::schema::{SchemaRegistry, TypeDefinition};
use super::GraphQLConfig;

/// Schema stitcher for combining multiple GraphQL schemas
pub struct SchemaStitcher {
    config: GraphQLConfig,
}

/// Query split result - represents a query fragment for a specific backend
#[derive(Debug, Clone)]
pub struct QueryFragment {
    /// Backend name
    pub backend: String,

    /// The GraphQL query string for this backend
    pub query: String,

    /// Fields requested from this backend
    pub fields: Vec<String>,

    /// Path in the result where these fields belong
    pub result_path: Vec<String>,
}

/// Field routing information
#[derive(Debug, Clone)]
struct FieldRoute {
    /// Backend that owns this field
    backend: String,

    /// Type that contains this field
    type_name: String,

    /// Field name
    field_name: String,
}

impl SchemaStitcher {
    /// Create a new schema stitcher
    pub fn new(config: GraphQLConfig) -> Self {
        Self { config }
    }

    // Simplified schema stitching - placeholder for future enhancement
    // Complex query analysis and splitting is not implemented yet

    /// Stitch schemas together into a unified schema
    pub fn stitch_schemas(&self, registry: &SchemaRegistry) -> Result<Value> {
        let schemas = registry.get_all_schemas();

        let mut stitched_types = HashMap::new();
        let mut stitched_queries = Vec::new();
        let mut stitched_mutations = Vec::new();

        for schema in schemas {
            // Merge types
            for type_def in &schema.types {
                let type_name = if let Some(namespace) = self.get_namespace(&schema.name) {
                    format!("{}_{}", namespace, type_def.name)
                } else {
                    type_def.name.clone()
                };

                stitched_types.insert(type_name, type_def.clone());
            }

            // Extract query fields
            if let Some(query_type) = schema.types.iter().find(|t| t.name == "Query") {
                stitched_queries.extend(query_type.fields.clone());
            }

            // Extract mutation fields
            if let Some(mutation_type) = schema.types.iter().find(|t| t.name == "Mutation") {
                stitched_mutations.extend(mutation_type.fields.clone());
            }
        }

        // Build unified schema representation
        let mut schema_json = serde_json::json!({
            "__schema": {
                "types": [],
                "queryType": { "name": "Query" },
                "mutationType": null,
            }
        });

        // Add query type
        if !stitched_queries.is_empty() {
            let query_fields: Vec<Value> = stitched_queries
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "type": { "name": f.type_name, "kind": "SCALAR" }
                    })
                })
                .collect();

            schema_json["__schema"]["types"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "kind": "OBJECT",
                    "name": "Query",
                    "fields": query_fields
                }));
        }

        // Add mutation type
        if !stitched_mutations.is_empty() {
            schema_json["__schema"]["mutationType"] = serde_json::json!({ "name": "Mutation" });

            let mutation_fields: Vec<Value> = stitched_mutations
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "type": { "name": f.type_name, "kind": "SCALAR" }
                    })
                })
                .collect();

            schema_json["__schema"]["types"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "kind": "OBJECT",
                    "name": "Mutation",
                    "fields": mutation_fields
                }));
        }

        // Add all other types
        for (type_name, type_def) in stitched_types {
            let fields: Vec<Value> = type_def
                .fields
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "type": { "name": f.type_name, "kind": "SCALAR" }
                    })
                })
                .collect();

            schema_json["__schema"]["types"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "kind": "OBJECT",
                    "name": type_name,
                    "fields": fields
                }));
        }

        Ok(schema_json)
    }

    fn get_namespace(&self, backend_name: &str) -> Option<String> {
        self.config
            .backends
            .iter()
            .find(|b| b.name == backend_name)
            .and_then(|b| b.namespace.clone())
    }

    /// Analyze a GraphQL query and split it across backends
    ///
    /// This is the core federation function that:
    /// 1. Parses the query into an AST
    /// 2. Determines which backend owns each field
    /// 3. Splits the query into fragments for each backend
    ///
    /// Note: This is a simplified implementation for demonstration.
    /// Production would use full AST traversal.
    pub fn analyze_and_split_query(
        &self,
        query: &str,
        registry: &SchemaRegistry,
    ) -> Result<Vec<QueryFragment>> {
        // Validate query syntax
        let _document = parse_query(query).context("Failed to parse GraphQL query")?;

        // Build field routing map
        let field_routes = self.build_field_routing(registry);

        // For now, use simple string-based field extraction
        // This is a simplified approach for demonstration
        let fragments = self.split_query_simple(query, &field_routes)?;

        debug!("Split query into {} fragments", fragments.len());

        Ok(fragments)
    }

    /// Simplified query splitting using string parsing
    ///
    /// This extracts field names from the query string and routes them to backends.
    /// Production implementation should use full AST traversal.
    fn split_query_simple(
        &self,
        query: &str,
        field_routes: &HashMap<String, FieldRoute>,
    ) -> Result<Vec<QueryFragment>> {
        let mut fragments_by_backend: HashMap<String, QueryFragment> = HashMap::new();

        // Extract field names from query (simplified)
        // Match pattern: fieldName or fieldName { ... }
        let field_pattern = regex::Regex::new(r"(\w+)\s*(\{|$)").unwrap();

        for cap in field_pattern.captures_iter(query) {
            let field_name = &cap[1];

            // Skip GraphQL keywords
            if matches!(
                field_name,
                "query" | "mutation" | "subscription" | "fragment" | "on"
            ) {
                continue;
            }

            // Try to find which backend owns this field (assume Query type)
            let field_key = format!("Query.{}", field_name);

            if let Some(route) = field_routes.get(&field_key) {
                let backend = &route.backend;

                let fragment = fragments_by_backend
                    .entry(backend.clone())
                    .or_insert_with(|| QueryFragment {
                        backend: backend.clone(),
                        query: String::new(),
                        fields: Vec::new(),
                        result_path: vec![],
                    });

                if !fragment.fields.contains(&field_name.to_string()) {
                    fragment.fields.push(field_name.to_string());
                }
            }
        }

        Ok(fragments_by_backend.into_values().collect())
    }

    /// Build a map of field → backend routing
    fn build_field_routing(&self, registry: &SchemaRegistry) -> HashMap<String, FieldRoute> {
        let mut routes = HashMap::new();

        for schema in registry.get_all_schemas() {
            for type_def in &schema.types {
                for field in &type_def.fields {
                    let key = format!("{}.{}", type_def.name, field.name);

                    routes.insert(
                        key,
                        FieldRoute {
                            backend: schema.name.clone(),
                            type_name: type_def.name.clone(),
                            field_name: field.name.clone(),
                        },
                    );
                }
            }
        }

        debug!("Built routing map with {} field routes", routes.len());

        routes
    }


    /// Build a query string for a backend fragment
    pub fn build_fragment_query(&self, fragment: &QueryFragment) -> String {
        if fragment.fields.is_empty() {
            return String::new();
        }

        // Simple query construction: { field1 field2 field3 }
        let fields_str = fragment.fields.join(" ");
        format!("{{ {} }}", fields_str)
    }

    /// Merge results from multiple backend queries
    ///
    /// Takes results from different backends and combines them into a unified response
    pub fn merge_results(&self, fragment_results: Vec<(QueryFragment, Value)>) -> Result<Value> {
        if fragment_results.is_empty() {
            return Ok(Value::Null);
        }

        // Start with an empty merged result
        let mut merged = serde_json::json!({});

        for (fragment, result) in fragment_results {
            debug!("Merging result from backend: {}", fragment.backend);

            // Merge this result into the combined result
            if let Some(data) = result.get("data") {
                self.merge_value_at_path(&mut merged, data, &fragment.result_path)?;
            }

            // Collect errors if any
            if let Some(errors) = result.get("errors") {
                if merged.get("errors").is_none() {
                    merged["errors"] = serde_json::json!([]);
                }

                if let Some(merged_errors) = merged["errors"].as_array_mut() {
                    if let Some(err_array) = errors.as_array() {
                        merged_errors.extend(err_array.clone());
                    }
                }
            }
        }

        Ok(merged)
    }

    /// Merge a value at a specific path in the result tree
    fn merge_value_at_path(&self, target: &mut Value, source: &Value, path: &[String]) -> Result<()> {
        if path.is_empty() {
            // At the root level, merge objects
            if let (Some(target_obj), Some(source_obj)) = (target.as_object_mut(), source.as_object()) {
                for (key, value) in source_obj {
                    target_obj.insert(key.clone(), value.clone());
                }
            }
        } else {
            // Navigate to the path and merge there
            let mut current = target;

            for (i, segment) in path.iter().enumerate() {
                if i == path.len() - 1 {
                    // Last segment - do the merge
                    if let Some(obj) = current.as_object_mut() {
                        if let Some(existing) = obj.get_mut(segment) {
                            // Merge with existing
                            if let (Some(existing_obj), Some(source_obj)) =
                                (existing.as_object_mut(), source.as_object())
                            {
                                for (key, value) in source_obj {
                                    existing_obj.insert(key.clone(), value.clone());
                                }
                            }
                        } else {
                            // Insert new
                            obj.insert(segment.clone(), source.clone());
                        }
                    }
                } else {
                    // Navigate deeper
                    if !current.get(segment).is_some() {
                        current[segment] = serde_json::json!({});
                    }
                    current = &mut current[segment];
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::graphql::GraphQLBackend;

    #[test]
    fn test_stitcher_creation() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: std::time::Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let stitcher = SchemaStitcher::new(config);
        assert!(true); // Just verify it compiles
    }

    #[test]
    fn test_fragment_query_building() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: std::time::Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let stitcher = SchemaStitcher::new(config);

        let fragment = QueryFragment {
            backend: "users".to_string(),
            query: String::new(),
            fields: vec!["id".to_string(), "name".to_string(), "email".to_string()],
            result_path: vec![],
        };

        let query = stitcher.build_fragment_query(&fragment);
        assert_eq!(query, "{ id name email }");
    }

    #[test]
    fn test_merge_results_single() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: std::time::Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let stitcher = SchemaStitcher::new(config);

        let fragment = QueryFragment {
            backend: "users".to_string(),
            query: String::new(),
            fields: vec!["id".to_string(), "name".to_string()],
            result_path: vec![],
        };

        let result = serde_json::json!({
            "data": {
                "id": "123",
                "name": "Alice"
            }
        });

        let merged = stitcher.merge_results(vec![(fragment, result)]).unwrap();

        assert!(merged.get("id").is_some());
        assert_eq!(merged["id"], "123");
        assert_eq!(merged["name"], "Alice");
    }

    #[test]
    fn test_merge_results_multiple_backends() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: std::time::Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let stitcher = SchemaStitcher::new(config);

        let fragment1 = QueryFragment {
            backend: "users".to_string(),
            query: String::new(),
            fields: vec!["id".to_string(), "name".to_string()],
            result_path: vec![],
        };

        let result1 = serde_json::json!({
            "data": {
                "id": "123",
                "name": "Alice"
            }
        });

        let fragment2 = QueryFragment {
            backend: "posts".to_string(),
            query: String::new(),
            fields: vec!["title".to_string(), "content".to_string()],
            result_path: vec![],
        };

        let result2 = serde_json::json!({
            "data": {
                "title": "Hello World",
                "content": "This is a post"
            }
        });

        let merged = stitcher
            .merge_results(vec![(fragment1, result1), (fragment2, result2)])
            .unwrap();

        // Should have fields from both backends
        assert_eq!(merged["id"], "123");
        assert_eq!(merged["name"], "Alice");
        assert_eq!(merged["title"], "Hello World");
        assert_eq!(merged["content"], "This is a post");
    }

    #[test]
    fn test_merge_results_with_errors() {
        let config = GraphQLConfig {
            enable_stitching: true,
            enable_cache: false,
            cache_ttl: std::time::Duration::from_secs(300),
            enable_batching: false,
            max_batch_size: 10,
            introspection_enabled: true,
            backends: vec![],
        };

        let stitcher = SchemaStitcher::new(config);

        let fragment = QueryFragment {
            backend: "users".to_string(),
            query: String::new(),
            fields: vec!["id".to_string()],
            result_path: vec![],
        };

        let result = serde_json::json!({
            "data": null,
            "errors": [{
                "message": "User not found"
            }]
        });

        let merged = stitcher.merge_results(vec![(fragment, result)]).unwrap();

        assert!(merged.get("errors").is_some());
        assert!(merged["errors"].is_array());
    }
}
