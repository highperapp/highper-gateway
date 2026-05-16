//! Distributed tracing and metrics with OpenTelemetry
//!
//! Provides trace context propagation, span creation, and export to backends like Jaeger.
//! Also supports OTLP (OpenTelemetry Protocol) for traces and metrics export.

use crate::config::{OtlpConfig, TracingConfig};
use hyper::{HeaderMap, StatusCode};
use opentelemetry::{
    global,
    trace::{TraceError, TracerProvider as _},
    KeyValue,
};
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

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
        "otlp" => init_otlp_tracer(config, resource)?,
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
    let telemetry_layer =
        tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer("highper-gateway"));

    // Initialize tracing subscriber with OpenTelemetry layer
    tracing_subscriber::registry()
        .with(telemetry_layer)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| TraceError::Other(Box::new(e)))?;

    info!("Distributed tracing initialized successfully");

    // Initialize OTLP metrics if using OTLP exporter and metrics are enabled
    if config.exporter == "otlp" && config.otlp.metrics_enabled {
        let metrics_config = OtlpMetricsConfig {
            enabled: true,
            endpoint: config.endpoint.clone(),
            service_name: config.service_name.clone(),
            export_interval_secs: config.otlp.export_interval_secs,
        };

        if let Err(e) = init_otlp_metrics(&metrics_config) {
            warn!("Failed to initialize OTLP metrics: {}", e);
        }
    }

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

/// Initialize OTLP exporter (OpenTelemetry Protocol)
///
/// Supports exporting traces via gRPC to any OTLP-compatible backend
/// (e.g., Jaeger, Tempo, Honeycomb, Datadog, etc.)
fn init_otlp_tracer(
    config: &TracingConfig,
    resource: Resource,
) -> Result<TracerProvider, TraceError> {
    use opentelemetry_otlp::WithExportConfig;

    info!(
        "Configuring OTLP trace exporter: endpoint={}, service={}",
        config.endpoint, config.service_name
    );

    // Create OTLP exporter with gRPC transport
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.endpoint)
        .build_span_exporter()
        .map_err(|e| TraceError::Other(Box::new(e)))?;

    // Build TracerProvider with batch exporter for better performance
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

// =====================================================
// OTLP Metrics Support
// =====================================================

use opentelemetry_sdk::metrics::SdkMeterProvider;
use std::sync::Arc;

/// OTLP Metrics configuration
#[derive(Debug, Clone)]
pub struct OtlpMetricsConfig {
    /// Enable OTLP metrics export
    pub enabled: bool,
    /// OTLP endpoint (defaults to http://localhost:4317)
    pub endpoint: String,
    /// Service name for metrics
    pub service_name: String,
    /// Export interval in seconds (defaults to 60)
    pub export_interval_secs: u64,
}

impl Default for OtlpMetricsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            service_name: "highper-gateway".to_string(),
            export_interval_secs: 60,
        }
    }
}

/// Global OTLP meter provider (stored for shutdown)
static OTLP_METER_PROVIDER: std::sync::OnceLock<Arc<SdkMeterProvider>> = std::sync::OnceLock::new();

/// Initialize OTLP metrics exporter
///
/// This sets up a periodic metrics export to an OTLP-compatible backend.
/// The exporter sends metrics at the configured interval.
pub fn init_otlp_metrics(
    config: &OtlpMetricsConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use opentelemetry_otlp::WithExportConfig;
    use std::time::Duration;

    if !config.enabled {
        info!("OTLP metrics export is disabled");
        return Ok(());
    }

    info!(
        "Initializing OTLP metrics exporter: endpoint={}, service={}, interval={}s",
        config.endpoint, config.service_name, config.export_interval_secs
    );

    // Build resource with service name
    let resource = Resource::new(vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    // Create OTLP metrics exporter using the metrics pipeline
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.endpoint);

    // Build meter provider with OTLP exporter
    // The pipeline configures periodic export automatically
    let meter_provider = opentelemetry_otlp::new_pipeline()
        .metrics(opentelemetry_sdk::runtime::Tokio)
        .with_exporter(exporter)
        .with_period(Duration::from_secs(config.export_interval_secs))
        .with_resource(resource)
        .build()?;

    // Store provider for shutdown
    let provider = Arc::new(meter_provider);
    let _ = OTLP_METER_PROVIDER.set(Arc::clone(&provider));

    info!("OTLP metrics exporter initialized successfully");
    Ok(())
}

/// Get the global OTLP meter for creating custom metrics
pub fn get_otlp_meter(
    name: impl Into<std::borrow::Cow<'static, str>>,
) -> opentelemetry::metrics::Meter {
    opentelemetry::global::meter(name)
}

/// Shutdown OTLP metrics and flush any pending exports
pub fn shutdown_otlp_metrics() {
    info!("Shutting down OTLP metrics exporter...");
    if let Some(provider) = OTLP_METER_PROVIDER.get() {
        if let Err(e) = provider.shutdown() {
            warn!("Error shutting down OTLP metrics provider: {}", e);
        }
    }
}

/// Record a counter metric via OTLP
pub fn record_otlp_counter(
    name: impl Into<std::borrow::Cow<'static, str>>,
    value: u64,
    attributes: &[KeyValue],
) {
    let meter = get_otlp_meter("highper-gateway");
    let counter = meter.u64_counter(name).init();
    counter.add(value, attributes);
}

/// Record a gauge metric via OTLP
pub fn record_otlp_gauge(
    name: impl Into<std::borrow::Cow<'static, str>>,
    value: f64,
    attributes: &[KeyValue],
) {
    let meter = get_otlp_meter("highper-gateway");
    let gauge = meter.f64_up_down_counter(name).init();
    gauge.add(value, attributes);
}

/// Record a histogram metric via OTLP
pub fn record_otlp_histogram(
    name: impl Into<std::borrow::Cow<'static, str>>,
    value: f64,
    attributes: &[KeyValue],
) {
    let meter = get_otlp_meter("highper-gateway");
    let histogram = meter.f64_histogram(name).init();
    histogram.record(value, attributes);
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
            otlp: OtlpConfig::default(),
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

    #[test]
    fn test_otlp_metrics_config_default() {
        let config = OtlpMetricsConfig::default();
        assert_eq!(config.enabled, false);
        assert_eq!(config.endpoint, "http://localhost:4317");
        assert_eq!(config.service_name, "highper-gateway");
        assert_eq!(config.export_interval_secs, 60);
    }

    #[test]
    fn test_otlp_config_default() {
        let config = OtlpConfig::default();
        assert_eq!(config.metrics_enabled, false);
        assert_eq!(config.export_interval_secs, 60);
        assert_eq!(config.compression, false);
        assert_eq!(config.timeout_secs, 30);
        assert!(config.headers.is_empty());
    }

    #[test]
    fn test_tracing_config_with_otlp() {
        let config = TracingConfig {
            enabled: true,
            exporter: "otlp".to_string(),
            endpoint: "http://localhost:4317".to_string(),
            sample_rate: 0.5,
            service_name: "test-service".to_string(),
            resource_attributes: HashMap::new(),
            otlp: OtlpConfig {
                metrics_enabled: true,
                export_interval_secs: 30,
                compression: true,
                headers: HashMap::new(),
                timeout_secs: 10,
            },
        };

        assert_eq!(config.exporter, "otlp");
        assert_eq!(config.otlp.metrics_enabled, true);
        assert_eq!(config.otlp.export_interval_secs, 30);
        assert_eq!(config.otlp.compression, true);
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
        headers.insert(
            "traceparent",
            "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"
                .parse()
                .unwrap(),
        );

        let ctx = extract_trace_context(&headers);
        // Context should be extracted successfully
        drop(ctx);
    }
}
