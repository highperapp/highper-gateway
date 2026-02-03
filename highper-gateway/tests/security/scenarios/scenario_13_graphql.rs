//! Scenario 13: GraphQL Gateway Security Tests
//!
//! Tests for GraphQL-specific vulnerabilities:
//! - GQL-01: Introspection Disclosure
//! - GQL-02: Query Complexity DoS
//! - GQL-03: Batch Attack
//! - GQL-04: Field Suggestion Enumeration
//! - GQL-05: Authorization Bypass
//! - GQL-06: N+1 Query Abuse

use crate::security::common::*;
use crate::security::payloads::graphql;
use hyper::StatusCode;
use std::collections::HashMap;

/// GraphQL Gateway configuration
const GRAPHQL_CONFIG: &str = r#"
[server]
host = "127.0.0.1"
port = {{PROXY_PORT}}
workers = 2

[proxy]
backend_host = "127.0.0.1"
backend_port = {{BACKEND_PORT}}

[graphql]
enabled = true
introspection_enabled = true
max_depth = 10
max_complexity = 1000
max_batch_size = 10
query_cache_enabled = true
query_cache_ttl_secs = 300

[logging]
level = "warn"
"#;

/// Test GQL-01: Introspection Query
#[tokio::test]
#[ignore]
async fn test_gql_01_introspection_disclosure() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Introspection Disclosure Test:");

    for query in graphql::INTROSPECTION {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let result = client
            .post_with_body("/graphql", headers, query.as_bytes().to_vec())
            .await;

        if let Ok((status, body, _)) = result {
            let has_schema = body.contains("__schema")
                || body.contains("queryType")
                || body.contains("types");

            if status == StatusCode::OK && has_schema {
                println!("  [WARN] Introspection enabled - schema exposed");

                // Check what information is exposed
                if body.contains("password") || body.contains("secret") {
                    println!("    [CRITICAL] Sensitive field names in schema");
                }
                if body.contains("admin") || body.contains("internal") {
                    println!("    [INFO] Administrative types visible");
                }
            } else {
                println!("  [PASS] Introspection blocked or limited");
            }
        }
    }

    // Production recommendation
    println!("\n  Note: Introspection should be DISABLED in production");
}

/// Test GQL-02: Query Depth Attack
#[tokio::test]
#[ignore]
async fn test_gql_02_query_depth_attack() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Query Depth Attack Test:");
    println!("  Max depth configured: 10");

    // Generate deeply nested queries
    for depth in [5, 10, 15, 20, 50] {
        let mut query = String::from(r#"{"query": "{ user "#);
        for _ in 0..depth {
            query.push_str("{ posts { author ");
        }
        query.push_str("{ id }");
        for _ in 0..depth {
            query.push_str(" } }");
        }
        query.push_str(" }\"} ");

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let result = client
            .post_with_body("/graphql", headers, query.into_bytes())
            .await;

        if let Ok((status, body, response_time)) = result {
            let blocked = status == StatusCode::BAD_REQUEST
                || body.contains("depth")
                || body.contains("exceeded");

            println!(
                "  Depth {}: {} ({} ms) - {}",
                depth,
                status,
                response_time,
                if blocked { "BLOCKED" } else { "ALLOWED" }
            );

            if !blocked && depth > 10 {
                println!("    [WARN] Deep query accepted beyond limit");
            }
        }
    }
}

/// Test GQL-03: Alias-Based Batch Attack
#[tokio::test]
#[ignore]
async fn test_gql_03_batch_attack() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Batch Attack Test:");
    println!("  Max batch size: 10");

    // Test alias multiplication
    for count in [5, 10, 20, 100] {
        let mut query = String::from(r#"{"query": "{ "#);
        for i in 0..count {
            query.push_str(&format!("a{}: user(id: {}) {{ email }} ", i, i));
        }
        query.push_str(" }\"} ");

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let result = client
            .post_with_body("/graphql", headers, query.into_bytes())
            .await;

        if let Ok((status, body, response_time)) = result {
            let blocked = status == StatusCode::BAD_REQUEST
                || body.contains("batch")
                || body.contains("aliases")
                || body.contains("exceeded");

            println!(
                "  {} aliases: {} ({} ms) - {}",
                count,
                status,
                response_time,
                if blocked { "BLOCKED" } else { "ALLOWED" }
            );

            if !blocked && count > 10 {
                println!("    [WARN] Large batch accepted beyond limit");
            }
        }
    }

    // Test array batching
    let batch_query = r#"[
        {"query": "{ user(id: 1) { email } }"},
        {"query": "{ user(id: 2) { email } }"},
        {"query": "{ user(id: 3) { email } }"}
    ]"#;

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let result = client
        .post_with_body("/graphql", headers, batch_query.as_bytes().to_vec())
        .await;

    if let Ok((status, _, _)) = result {
        println!("  Array batching: {}", status);
    }
}

/// Test GQL-04: Field Suggestion Enumeration
#[tokio::test]
#[ignore]
async fn test_gql_04_field_enumeration() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Field Suggestion Test:");

    // Try invalid field names to get suggestions
    let typos = vec![
        ("pasword", "password"),
        ("secet", "secret"),
        ("creditCard", "credit_card"),
        ("ssn", "social_security"),
        ("apiKey", "api_key"),
    ];

    for (typo, expected) in typos {
        let query = format!(
            r#"{{"query": "{{ user {{ {} }} }}"}}"#,
            typo
        );

        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let result = client
            .post_with_body("/graphql", headers, query.into_bytes())
            .await;

        if let Ok((_, body, _)) = result {
            if body.contains("Did you mean") || body.contains(expected) {
                println!(
                    "  [INFO] '{}' suggests '{}'",
                    typo, expected
                );
            } else {
                println!("  '{}': no suggestion", typo);
            }
        }
    }
}

/// Test GQL-05: Authorization Bypass
#[tokio::test]
#[ignore]
async fn test_gql_05_authorization_bypass() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Authorization Bypass Test:");

    // Try to access protected fields directly
    let protected_queries = vec![
        r#"{"query": "{ user(id: 1) { password } }"}"#,
        r#"{"query": "{ user(id: 1) { creditCard { number cvv } } }"}"#,
        r#"{"query": "{ users { id email role } }"}"#,
        r#"{"query": "{ __type(name: \"User\") { fields { name } } }"}"#,
        r#"{"query": "mutation { deleteUser(id: 1) { success } }"}"#,
    ];

    for query in protected_queries {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        // No auth token

        let result = client
            .post_with_body("/graphql", headers, query.as_bytes().to_vec())
            .await;

        if let Ok((status, body, _)) = result {
            let has_data = body.contains("\"data\":")
                && !body.contains("null")
                && !body.contains("unauthorized");

            let query_preview = &query[..query.len().min(50)];
            println!(
                "  {}...: {} - {}",
                query_preview,
                status,
                if has_data {
                    "DATA RETURNED"
                } else {
                    "protected"
                }
            );
        }
    }
}

/// Test GQL-06: Query Complexity Attack
#[tokio::test]
#[ignore]
async fn test_gql_06_complexity_attack() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Complexity Attack Test:");
    println!("  Max complexity: 1000");

    // Query with high complexity due to list expansion
    let complex_query = r#"{"query": "{
        users(first: 100) {
            posts(first: 100) {
                comments(first: 100) {
                    author {
                        posts(first: 100) {
                            title
                        }
                    }
                }
            }
        }
    }"}"#;

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let result = client
        .post_with_body("/graphql", headers, complex_query.as_bytes().to_vec())
        .await;

    if let Ok((status, body, response_time)) = result {
        let blocked = body.contains("complexity")
            || body.contains("exceeded")
            || status == StatusCode::BAD_REQUEST;

        println!(
            "  High complexity query: {} ({} ms)",
            status, response_time
        );

        if blocked {
            println!("  [PASS] Complex query blocked");
        } else {
            println!("  [WARN] Complex query allowed - check limits");
        }
    }
}

/// Test GraphQL persisted queries
#[tokio::test]
#[ignore]
async fn test_gql_persisted_queries() {
    let harness = match SecurityTestHarness::new(GRAPHQL_CONFIG).await {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to create harness: {}", e);
            return;
        }
    };

    let client = SecurityHttpClient::new(&harness.url(""));

    println!("GraphQL Persisted Queries Test:");

    // Test if arbitrary queries can be executed or only persisted ones
    let query = r#"{"query": "{ __typename }", "extensions": {"persistedQuery": {"sha256Hash": "invalid"}}}"#;

    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());

    let result = client
        .post_with_body("/graphql", headers, query.as_bytes().to_vec())
        .await;

    if let Ok((status, body, _)) = result {
        if body.contains("PersistedQueryNotFound") {
            println!("  Persisted queries enforced");
        } else {
            println!("  Arbitrary queries allowed: {}", status);
        }
    }
}

/// Security audit for GraphQL
#[tokio::test]
#[ignore]
async fn test_graphql_security_audit() {
    println!("\n========== GraphQL Security Audit ==========\n");

    println!("Query Limits:");
    println!("  [✓] Max depth: 10");
    println!("  [✓] Max complexity: 1000");
    println!("  [✓] Max batch size: 10");
    println!("  [?] Query timeout configured");

    println!("\nInformation Disclosure:");
    println!("  [⚠] Introspection ENABLED (disable in production)");
    println!("  [?] Field suggestions disabled");
    println!("  [?] Debug mode disabled");

    println!("\nAuthorization:");
    println!("  [?] Field-level authorization");
    println!("  [?] Mutation authorization");
    println!("  [?] Subscription authorization");

    println!("\nCaching:");
    println!("  [✓] Query cache enabled");
    println!("  [✓] Cache TTL: 300s");
    println!("  [?] Cache key includes auth context");

    println!("\nRecommendations:");
    println!("  1. DISABLE introspection in production");
    println!("  2. Implement persisted queries");
    println!("  3. Add rate limiting per operation");
    println!("  4. Log and monitor complex queries");

    println!("\n=============================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_has_graphql() {
        assert!(GRAPHQL_CONFIG.contains("[graphql]"));
        assert!(GRAPHQL_CONFIG.contains("max_depth"));
        assert!(GRAPHQL_CONFIG.contains("max_complexity"));
    }
}
