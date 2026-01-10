# Phase 2.4: OpenTelemetry Tracing - Implementation Specification

**Duration:** 2 weeks
**Priority:** Medium
**Difficulty:** Medium
**Impact:** +4% observability score

---

## Executive Summary

Implement distributed tracing using OpenTelemetry to track requests across microservices. This enables performance analysis, bottleneck identification, and debugging in distributed systems.

---

## What is Distributed Tracing?

```
Without Tracing:
Client → Proxy → Service A → Service B → Service C
        (black box - can't see internal flow)

With OpenTelemetry Tracing:
Client → Proxy (span 1) → Service A (span 2) → Service B (span 3)
                                                        ↓
                                              Service C (span 4)

Trace ID: abc123 connects all spans
Can see: timing, errors, dependencies
```

---

## Configuration

```yaml
observability:
  tracing:
    enabled: true
    exporter: "jaeger"  # jaeger, zipkin, otlp
    endpoint: "http://jaeger:14268/api/traces"
    sample_rate: 1.0  # 0.0 to 1.0 (0 = none, 1.0 = all)

    # Service name in traces
    service_name: "highper-gateway"

    # Additional attributes
    resource_attributes:
      environment: "production"
      version: "0.1.0"
```

---

## Implementation

```rust
// File: highper-gateway/src/observability/tracing.rs

use opentelemetry::{
    global,
    sdk::{
        export::trace::stdout,
        trace::{self, RandomIdGenerator, Sampler},
        Resource,
    },
    trace::{TraceError, Tracer},
    KeyValue,
};
use opentelemetry_jaeger::JaegerPipeline;

pub struct TracingConfig {
    pub enabled: bool,
    pub exporter: String,
    pub endpoint: String,
    pub sample_rate: f64,
    pub service_name: String,
}

pub fn init_tracing(config: &TracingConfig) -> Result<(), TraceError> {
    if !config.enabled {
        return Ok(());
    }

    let tracer = match config.exporter.as_str() {
        "jaeger" => init_jaeger(config)?,
        "zipkin" => init_zipkin(config)?,
        "otlp" => init_otlp(config)?,
        _ => return Err(TraceError::from("Unknown exporter")),
    };

    global::set_tracer_provider(tracer);

    Ok(())
}

fn init_jaeger(config: &TracingConfig) -> Result<opentelemetry::sdk::trace::TracerProvider, TraceError> {
    opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(&config.service_name)
        .with_endpoint(&config.endpoint)
        .install_batch(opentelemetry::runtime::Tokio)
}

// Middleware to create spans
pub async fn trace_request<B>(
    req: Request<B>,
    next: Next<B>,
) -> Response {
    use opentelemetry::trace::SpanKind;

    let tracer = global::tracer("highper-gateway");

    let mut span = tracer
        .span_builder(format!("{} {}", req.method(), req.uri().path()))
        .with_kind(SpanKind::Server)
        .start(&tracer);

    // Extract trace context from headers
    let parent_context = extract_trace_context(&req);
    let cx = Context::current_with_span(span);

    // Add attributes
    cx.span().set_attribute(KeyValue::new("http.method", req.method().to_string()));
    cx.span().set_attribute(KeyValue::new("http.url", req.uri().to_string()));

    // Execute request
    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();

    // Record response
    cx.span().set_attribute(KeyValue::new("http.status_code", response.status().as_u16() as i64));
    cx.span().set_attribute(KeyValue::new("http.response_time_ms", duration.as_millis() as i64));

    // Inject trace context into response headers
    inject_trace_context(&mut response, &cx);

    response
}

fn extract_trace_context<B>(req: &Request<B>) -> Context {
    use opentelemetry::propagation::Extractor;

    struct HeaderExtractor<'a>(&'a hyper::HeaderMap);

    impl<'a> Extractor for HeaderExtractor<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|v| v.to_str().ok())
        }

        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|k| k.as_str()).collect()
        }
    }

    let extractor = HeaderExtractor(req.headers());
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&extractor)
    })
}
```

---

## Dependencies

```toml
[dependencies]
# OpenTelemetry core
opentelemetry = { version = "0.22", features = ["trace", "metrics"] }
opentelemetry-jaeger = { version = "0.21", features = ["rt-tokio"] }
opentelemetry-zipkin = "0.20"
opentelemetry-otlp = "0.15"

# Tracing integration
tracing-opentelemetry = "0.23"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

---

## Acceptance Criteria

- [ ] Traces exported to Jaeger/Zipkin
- [ ] Trace context propagated via headers
- [ ] Spans created for each request
- [ ] Backend calls tracked as child spans
- [ ] Error spans marked
- [ ] Performance overhead < 5%

---

**Document Version:** 1.0
**Last Updated:** October 30, 2025
