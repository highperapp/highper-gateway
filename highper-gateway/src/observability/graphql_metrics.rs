//! GraphQL-specific metrics
//!
//! This module provides metrics specifically for GraphQL:
//! - Per-query tracking by operation name
//! - Query complexity scoring
//! - Query depth measurement
//! - Resolver latency per field
//! - Batched query stats
//! - GraphQL errors

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};
use std::time::Duration;

/// Initialize GraphQL-specific metrics descriptions
pub fn describe_graphql_metrics() {
    // Query metrics
    describe_counter!(
        "graphql_queries_total",
        "Total GraphQL queries by operation name"
    );
    describe_histogram!(
        "graphql_query_duration_seconds",
        "GraphQL query execution time by operation"
    );

    // Query complexity metrics
    describe_histogram!(
        "graphql_query_complexity",
        "GraphQL query complexity score"
    );
    describe_histogram!(
        "graphql_query_depth",
        "GraphQL query depth (nested levels)"
    );
    describe_counter!(
        "graphql_complexity_exceeded_total",
        "Times query complexity limit was exceeded"
    );
    describe_counter!(
        "graphql_depth_exceeded_total",
        "Times query depth limit was exceeded"
    );

    // Resolver metrics
    describe_histogram!(
        "graphql_resolver_duration_seconds",
        "GraphQL resolver execution time by field"
    );
    describe_counter!(
        "graphql_resolver_calls_total",
        "Total resolver calls by field"
    );
    describe_counter!(
        "graphql_resolver_errors_total",
        "Total resolver errors by field"
    );

    // Batched query metrics
    describe_counter!(
        "graphql_batched_queries_total",
        "Total batched GraphQL queries"
    );
    describe_histogram!(
        "graphql_batch_size",
        "Number of queries in a batch"
    );
    describe_histogram!(
        "graphql_batch_duration_seconds",
        "Total duration of batched queries"
    );

    // Error metrics
    describe_counter!(
        "graphql_errors_total",
        "Total GraphQL errors by type"
    );
    describe_counter!(
        "graphql_validation_errors_total",
        "Total query validation errors"
    );
    describe_counter!(
        "graphql_execution_errors_total",
        "Total query execution errors"
    );

    // Cache metrics
    describe_counter!(
        "graphql_cache_hits_total",
        "Total GraphQL cache hits"
    );
    describe_counter!(
        "graphql_cache_misses_total",
        "Total GraphQL cache misses"
    );

    // Field selection metrics
    describe_counter!(
        "graphql_fields_selected_total",
        "Total fields selected in queries"
    );

    // Introspection queries
    describe_counter!(
        "graphql_introspection_queries_total",
        "Total introspection queries"
    );

    // Subscription metrics (if supported)
    describe_gauge!(
        "graphql_subscriptions_active",
        "Number of active GraphQL subscriptions"
    );
    describe_counter!(
        "graphql_subscriptions_total",
        "Total GraphQL subscriptions created"
    );
}

/// Record a GraphQL query
pub fn record_query(
    operation_name: Option<&str>,
    operation_type: &str,
    duration: Duration,
    complexity: u32,
    depth: u32,
) {
    let operation = operation_name.unwrap_or("anonymous");
    let duration_secs = duration.as_secs_f64();

    counter!(
        "graphql_queries_total",
        "operation" => operation.to_string(),
        "type" => operation_type.to_string(),
    ).increment(1);

    histogram!(
        "graphql_query_duration_seconds",
        "operation" => operation.to_string(),
        "type" => operation_type.to_string(),
    ).record(duration_secs);

    histogram!(
        "graphql_query_complexity",
        "operation" => operation.to_string(),
    ).record(complexity as f64);

    histogram!(
        "graphql_query_depth",
        "operation" => operation.to_string(),
    ).record(depth as f64);

}

/// Record query complexity exceeded
pub fn record_complexity_exceeded(operation_name: Option<&str>, _complexity: u32, _limit: u32) {
    let operation = operation_name.unwrap_or("anonymous");

    counter!(
        "graphql_complexity_exceeded_total",
        "operation" => operation.to_string(),
    ).increment(1);
}

/// Record query depth exceeded
pub fn record_depth_exceeded(operation_name: Option<&str>, _depth: u32, _limit: u32) {
    let operation = operation_name.unwrap_or("anonymous");

    counter!(
        "graphql_depth_exceeded_total",
        "operation" => operation.to_string(),
    ).increment(1);
}

/// Record resolver execution
pub fn record_resolver(
    parent_type: &str,
    field_name: &str,
    duration: Duration,
    error: bool,
) {
    let duration_secs = duration.as_secs_f64();
    let field = format!("{}.{}", parent_type, field_name);

    counter!(
        "graphql_resolver_calls_total",
        "field" => field.clone(),
    ).increment(1);

    histogram!(
        "graphql_resolver_duration_seconds",
        "field" => field.clone(),
    ).record(duration_secs);

    if error {
        counter!(
            "graphql_resolver_errors_total",
            "field" => field.clone(),
        ).increment(1);
    }
}

/// Record batched query
pub fn record_batch(batch_size: usize, duration: Duration) {
    counter!("graphql_batched_queries_total").increment(1);
    histogram!("graphql_batch_size").record(batch_size as f64);
    histogram!("graphql_batch_duration_seconds").record(duration.as_secs_f64());

}

/// Record GraphQL error
pub fn record_error(error_type: &str, operation_name: Option<&str>) {
    let operation = operation_name.unwrap_or("anonymous");

    counter!(
        "graphql_errors_total",
        "type" => error_type.to_string(),
        "operation" => operation.to_string(),
    ).increment(1);

    match error_type {
        "validation" => {
            counter!(
                "graphql_validation_errors_total",
                "operation" => operation.to_string(),
            ).increment(1)
        }
        "execution" => {
            counter!(
                "graphql_execution_errors_total",
                "operation" => operation.to_string(),
            ).increment(1)
        }
        _ => {}
    }

}

/// Record cache hit/miss
pub fn record_cache_result(operation_name: Option<&str>, hit: bool) {
    let operation = operation_name.unwrap_or("anonymous");

    if hit {
        counter!(
            "graphql_cache_hits_total",
            "operation" => operation.to_string(),
        ).increment(1);

    } else {
        counter!(
            "graphql_cache_misses_total",
            "operation" => operation.to_string(),
        ).increment(1);

    }
}

/// Record fields selected
pub fn record_fields_selected(operation_name: Option<&str>, count: usize) {
    let operation = operation_name.unwrap_or("anonymous");

    counter!(
        "graphql_fields_selected_total",
        "operation" => operation.to_string(),
    ).increment(count as u64);

}

/// Record introspection query
pub fn record_introspection() {
    counter!("graphql_introspection_queries_total").increment(1);

}

/// Record subscription lifecycle
pub fn record_subscription_open(operation_name: Option<&str>) {
    let operation = operation_name.unwrap_or("anonymous");

    gauge!("graphql_subscriptions_active").increment(1.0);
    counter!(
        "graphql_subscriptions_total",
        "operation" => operation.to_string(),
    ).increment(1);

}

/// Record subscription close
pub fn record_subscription_close(operation_name: Option<&str>) {
    let _operation = operation_name.unwrap_or("anonymous");

    gauge!("graphql_subscriptions_active").decrement(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphql_metrics_recording() {
        // Initialize metrics
        describe_graphql_metrics();

        // Test query recording
        record_query(
            Some("GetUser"),
            "query",
            Duration::from_millis(50),
            100,
            5,
        );

        // Test complexity/depth exceeded
        record_complexity_exceeded(Some("ComplexQuery"), 5000, 1000);
        record_depth_exceeded(Some("DeepQuery"), 20, 10);

        // Test resolver
        record_resolver("User", "posts", Duration::from_millis(20), false);
        record_resolver("Post", "author", Duration::from_millis(10), true);

        // Test batch
        record_batch(5, Duration::from_millis(100));

        // Test errors
        record_error("validation", Some("InvalidQuery"));
        record_error("execution", Some("FailedQuery"));

        // Test cache
        record_cache_result(Some("GetUser"), true);
        record_cache_result(Some("GetPost"), false);

        // Test fields
        record_fields_selected(Some("GetUser"), 15);

        // Test introspection
        record_introspection();

        // Test subscriptions
        record_subscription_open(Some("OnUserUpdated"));
        record_subscription_close(Some("OnUserUpdated"));

        assert!(true);
    }
}
