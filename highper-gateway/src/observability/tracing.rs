//! Distributed tracing with OpenTelemetry
//!
//! Provides trace context propagation, span creation, and export to backends like Jaeger.

use crate::config::TracingConfig;
use opentelemetry::{
    global,
    trace::{TraceError, TracerProvider as _, Tracer},
    KeyValue,
};
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tracing::{info, warn};
use hyper::{HeaderMap, StatusCode};

/// Initialize distributed tracing
///
/// Sets up OpenTelemetry with the configured exporter (Jaeger, etc.)
pub fn init_tracing(config: &TracingConfig) -> Result<(), TraceError> {
    if !config.enabled {
        info!("Distributed tracing is disabled");
        return Ok(());
    }

    info!(
        "Initializing distributed tracing: exporter={}, endpoint={}",
        config.exporter, config.endpoint
    );

    // Build resource with service name and attributes
    let mut resource_kvs = vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ];

    // Add custom resource attributes
    for (key, value) in &config.resource_attributes {
        resource_kvs.push(KeyValue::new(key.clone(), value.clone()));
    }

    let resource = Resource::new(resource_kvs);

    // Initialize tracer based on exporter type
    let tracer_provider = match config.exporter.as_str() {
        "jaeger" => init_jaeger_tracer(config, resource)?,
        "stdout" => init_stdout_tracer(config, resource)?,
        _ => {
            warn!(
                "Unknown tracing exporter '{}', falling back to stdout",
                config.exporter
            );
            init_stdout_tracer(config, resource)?
        }
    };

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider.clone());

    // Create tracing layer for tracing-subscriber integration
    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer("highper-gateway"));

    // Initialize tracing subscriber with OpenTelemetry layer
    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| TraceError::Other(Box::new(e)))?;

    info!("Distributed tracing initialized successfully");

    Ok(())
}

/// Initialize Jaeger exporter (for production)
fn init_jaeger_tracer(
    config: &TracingConfig,
    resource: Resource,
) -> Result<TracerProvider, TraceError> {
    info!(
        "Configuring Jaeger exporter: endpoint={}, service={}",
        config.endpoint, config.service_name
    );

    // Build Jaeger exporter manually to get TracerProvider
    let agent_endpoint = config.endpoint.clone();

    // Create exporter for batch processing
    let exporter = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(agent_endpoint)
        .with_service_name(&config.service_name)
        .build_sync_agent_exporter()
        .map_err(|e| TraceError::Other(Box::new(e)))?;

    // Build TracerProvider with batch exporter
    Ok(TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    config.sample_rate,
                ))))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .build())
}

/// Initialize stdout exporter (for testing/development)
fn init_stdout_tracer(
    config: &TracingConfig,
    resource: Resource,
) -> Result<TracerProvider, TraceError> {
    use opentelemetry_stdout::SpanExporter;

    info!("Configuring stdout exporter for development");

    let exporter = SpanExporter::default();

    Ok(TracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    config.sample_rate,
                ))))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .build())
}

/// Shutdown tracing and flush remaining spans
pub async fn shutdown_tracing() {
    info!("Shutting down distributed tracing...");
    global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_config() -> TracingConfig {
        TracingConfig {
            enabled: true,
            exporter: "stdout".to_string(),
            endpoint: "http://localhost:14268/api/traces".to_string(),
            sample_rate: 1.0,
            service_name: "test-proxy".to_string(),
            resource_attributes: HashMap::new(),
        }
    }

    #[test]
    fn test_tracing_config_disabled() {
        let config = TracingConfig {
            enabled: false,
            ..create_test_config()
        };

        let result = init_tracing(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tracing_config_creation() {
        let config = create_test_config();
        assert_eq!(config.enabled, true);
        assert_eq!(config.exporter, "stdout");
        assert_eq!(config.sample_rate, 1.0);
    }
}

/// Record HTTP request attributes in current span
///
/// Uses tracing macros which integrate with OpenTelemetry
pub fn record_http_request(method: &str, path: &str, host: Option<&str>, client_ip: Option<&str>) {
    

    // Record attributes using tracing
    tracing::info_span!(
        "http_request",
        http.method = %method,
        http.target = %path,
        http.host = host.unwrap_or(""),
        client.ip = client_ip.unwrap_or(""),
        otel.kind = "server"
    );
}

/// Record HTTP response attributes in current span
pub fn record_http_response(status: StatusCode, duration_ms: f64) {
    // Add response attributes to current span
    tracing::Span::current().record("http.status_code", status.as_u16());
    tracing::Span::current().record("http.response_time_ms", duration_ms);

    // Log the response
    if status.is_server_error() {
        tracing::error!(http.status_code = %status, "Server error");
    } else if status.is_client_error() {
        tracing::warn!(http.status_code = %status, "Client error");
    } else {
        tracing::debug!(http.status_code = %status, "Request completed");
    }
}

/// Extract trace context from HTTP headers
///
/// Uses W3C Trace Context propagation format.
pub fn extract_trace_context(headers: &HeaderMap) -> opentelemetry::Context {
    use opentelemetry::propagation::{Extractor, TextMapPropagator};
    
    struct HeaderExtractor<'a>(&'a HeaderMap);
    
    impl<'a> Extractor for HeaderExtractor<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|v| v.to_str().ok())
        }
        
        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|k| k.as_str()).collect()
        }
    }
    
    let extractor = HeaderExtractor(headers);
    let propagator = opentelemetry_sdk::propagation::TraceContextPropagator::new();
    
    propagator.extract(&extractor)
}

/// Inject trace context into HTTP headers
///
/// Uses W3C Trace Context propagation format.
pub fn inject_trace_context(headers: &mut HeaderMap, cx: &opentelemetry::Context) {
    use opentelemetry::propagation::{Injector, TextMapPropagator};
    
    struct HeaderInjector<'a>(&'a mut HeaderMap);
    
    impl<'a> Injector for HeaderInjector<'a> {
        fn set(&mut self, key: &str, value: String) {
            if let Ok(header_name) = hyper::header::HeaderName::from_bytes(key.as_bytes()) {
                if let Ok(header_value) = hyper::header::HeaderValue::from_str(&value) {
                    self.0.insert(header_name, header_value);
                }
            }
        }
    }
    
    let mut injector = HeaderInjector(headers);
    let propagator = opentelemetry_sdk::propagation::TraceContextPropagator::new();
    
    propagator.inject_context(cx, &mut injector);
}

#[cfg(test)]
mod span_tests {
    use super::*;

    #[test]
    fn test_record_http_request() {
        // Just verify function compiles and doesn't panic
        record_http_request("GET", "/api/users", Some("example.com"), Some("127.0.0.1"));
    }

    #[test]
    fn test_record_http_response() {
        // Just verify function compiles and doesn't panic
        record_http_response(StatusCode::OK, 42.5);
    }

    #[test]
    fn test_extract_trace_context() {
        let mut headers = HeaderMap::new();
        headers.insert("traceparent", "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".parse().unwrap());

        let ctx = extract_trace_context(&headers);
        // Context should be extracted successfully
        drop(ctx);
    }
}
