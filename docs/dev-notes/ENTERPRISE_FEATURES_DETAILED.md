# Enterprise Features: Detailed Design & Implementation Plan
## Service Mesh Integration & Multi-Tenancy

**Document Version**: 1.0
**Date**: November 17, 2025
**Target Release**: v1.2 (Service Mesh), v2.0 (Multi-Tenancy)

---

## Table of Contents

1. [Service Mesh Integration](#i-service-mesh-integration)
2. [Multi-Tenancy Support](#ii-multi-tenancy-support)
3. [Implementation Roadmap](#iii-implementation-roadmap)
4. [Competitive Analysis](#iv-competitive-analysis)
5. [Reference Architecture](#v-reference-architecture)

---

## I. Service Mesh Integration

### Overview

**Service Mesh** is a dedicated infrastructure layer for handling service-to-service communication in microservices architectures. It provides:
- **Traffic management** (load balancing, routing, retries)
- **Security** (mTLS, authentication, authorization)
- **Observability** (metrics, tracing, logging)
- **Resilience** (circuit breaking, timeouts, fault injection)

### Why Service Mesh for Highper Gateway?

Current state: Highper Gateway can act as a **reverse proxy** or **API gateway**
Target state: Highper Gateway as a **sidecar proxy** in service mesh deployments

**Benefits**:
1. **Istio/Linkerd compatibility** - Drop-in replacement for Envoy
2. **Control plane integration** - xDS protocol support
3. **East-West traffic** - Service-to-service communication
4. **Zero-trust security** - Automatic mTLS between services
5. **Enterprise adoption** - Service mesh is standard in large organizations

---

### 1.1 Control Plane Integration (xDS Protocol)

#### What is xDS?

**xDS** (Discovery Service) is a set of APIs for dynamic configuration:
- **CDS** (Cluster Discovery Service) - Backend endpoints
- **EDS** (Endpoint Discovery Service) - Load balancing targets
- **LDS** (Listener Discovery Service) - Inbound ports/protocols
- **RDS** (Route Discovery Service) - Routing rules
- **SDS** (Secret Discovery Service) - TLS certificates

#### Current State vs Target

**Current State**: Static configuration (YAML file)
```yaml
# config.yaml - Static configuration
upstreams:
  - name: "user-service"
    servers:
      - url: "http://10.0.1.5:8080"  # Hardcoded
      - url: "http://10.0.1.6:8080"  # Hardcoded
```

**Target State**: Dynamic configuration (xDS gRPC stream)
```rust
// Dynamic configuration from Istio/Linkerd control plane
impl XdsClient {
    async fn watch_clusters(&self) -> impl Stream<Item = ClusterUpdate> {
        // gRPC stream from control plane (istiod, linkerd-controller)
        self.cds_stream.subscribe().await
    }

    async fn watch_endpoints(&self) -> impl Stream<Item = EndpointUpdate> {
        // Real-time endpoint updates (pods scaling up/down)
        self.eds_stream.subscribe().await
    }
}

// Configuration updates applied in real-time
async fn handle_cluster_update(update: ClusterUpdate) {
    match update.operation {
        Operation::Add => {
            // New service discovered
            load_balancer.add_backend(update.endpoint);
        }
        Operation::Remove => {
            // Service pod terminated
            load_balancer.remove_backend(update.endpoint);
        }
        Operation::Update => {
            // Service configuration changed
            load_balancer.update_backend(update.endpoint);
        }
    }
}
```

#### Architecture: Control Plane Integration

```
┌─────────────────────────────────────────────────────────────┐
│                    Kubernetes Cluster                        │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────────┐                                     │
│  │  Istio Control      │                                     │
│  │  Plane (istiod)     │                                     │
│  │                     │                                     │
│  │  - Service Discovery│                                     │
│  │  - Configuration    │                                     │
│  │  - Certificate CA   │                                     │
│  └──────────┬──────────┘                                     │
│             │ xDS gRPC (CDS, EDS, LDS, RDS, SDS)             │
│             │                                                 │
│  ┌──────────▼──────────────────────────────────────────┐    │
│  │                  Highper Gateway (Sidecar)               │    │
│  │                                                      │    │
│  │  ┌────────────────┐  ┌────────────────┐            │    │
│  │  │ XDS Client     │  │ Config Manager │            │    │
│  │  │ - CDS watch    │  │ - Dynamic      │            │    │
│  │  │ - EDS watch    │  │   routing      │            │    │
│  │  │ - LDS watch    │  │ - Hot reload   │            │    │
│  │  └────────────────┘  └────────────────┘            │    │
│  │                                                      │    │
│  │  ┌────────────────────────────────────────────┐    │    │
│  │  │         Service Proxy Logic                 │    │    │
│  │  │  - Load balancing                           │    │    │
│  │  │  - Circuit breaking                         │    │    │
│  │  │  - mTLS termination                         │    │    │
│  │  │  - Telemetry collection                     │    │    │
│  │  └────────────────────────────────────────────┘    │    │
│  └──────────────────────────────────────────────────────┘   │
│             ▲                                  │              │
│             │ :15006 (inbound)                 │ :15001       │
│             │                                  ▼ (outbound)   │
│  ┌──────────┴──────────┐          ┌────────────────────┐    │
│  │  Application Pod    │          │  Backend Service   │    │
│  │  (user-service)     │          │  (payment-service) │    │
│  │  :8080              │          │  :8080             │    │
│  └─────────────────────┘          └────────────────────┘    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

---

### 1.2 Sidecar Deployment Model

#### What is a Sidecar?

A **sidecar** is a container deployed alongside each application pod:
- Intercepts **all inbound traffic** (port 15006)
- Intercepts **all outbound traffic** (port 15001)
- Transparent to application (via iptables rules)

#### Deployment Example (Kubernetes)

```yaml
# Deployment with Highper Gateway sidecar
apiVersion: apps/v1
kind: Deployment
metadata:
  name: user-service
spec:
  template:
    metadata:
      annotations:
        # Inject Highper Gateway sidecar
        sidecar.istio.io/inject: "false"  # We inject manually
    spec:
      # Init container to set up iptables
      initContainers:
      - name: istio-init
        image: highper-gateway-init:1.0
        securityContext:
          capabilities:
            add: ["NET_ADMIN", "NET_RAW"]
        command:
        - /usr/local/bin/setup-iptables.sh
        # Redirects all TCP traffic to sidecar

      containers:
      # Application container
      - name: user-service
        image: user-service:v1.0
        ports:
        - containerPort: 8080
        # App thinks it's talking directly to other services
        # But traffic is intercepted by sidecar

      # Highper Gateway sidecar
      - name: highper-gateway-sidecar
        image: highper-gateway:1.2.0-sidecar
        args:
        - --mode=sidecar
        - --xds-server=istiod.istio-system.svc:15010
        - --pod-name=$(POD_NAME)
        - --pod-namespace=$(POD_NAMESPACE)
        env:
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        ports:
        - containerPort: 15001  # Outbound proxy
          name: proxy-outbound
        - containerPort: 15006  # Inbound proxy
          name: proxy-inbound
        - containerPort: 15020  # Health check
          name: health
        - containerPort: 15090  # Prometheus metrics
          name: metrics
        volumeMounts:
        - name: workload-certs
          mountPath: /etc/certs
          readOnly: true

      volumes:
      - name: workload-certs
        secret:
          secretName: istio.user-service
```

#### Traffic Flow with Sidecar

```
Client Request Flow:
1. Client sends request to user-service.default.svc:8080
2. iptables redirects to Highper Gateway sidecar :15006 (inbound)
3. Highper Gateway:
   - Terminates mTLS
   - Checks authorization policies
   - Applies rate limits
   - Records metrics
   - Forwards to localhost:8080 (application)
4. Application receives plain HTTP request

Outbound Request Flow:
1. Application calls payment-service.default.svc:8080
2. iptables redirects to Highper Gateway sidecar :15001 (outbound)
3. Highper Gateway:
   - Queries EDS for payment-service endpoints
   - Selects healthy endpoint (load balancing)
   - Establishes mTLS connection
   - Applies retries, timeouts, circuit breaking
   - Forwards request to backend sidecar :15006
4. Backend sidecar (payment-service) receives request
5. Backend sidecar forwards to payment-service:8080
```

---

### 1.3 Automatic mTLS (Mutual TLS)

#### Why mTLS in Service Mesh?

**Zero-trust security**: Every service-to-service connection is encrypted and authenticated
- **Encryption**: Prevents eavesdropping on internal network
- **Authentication**: Verifies identity of both client and server
- **Authorization**: Enforces fine-grained access policies

#### Certificate Management

```rust
// Automatic certificate rotation
pub struct WorkloadCertManager {
    // Certificate authority (Istio CA, cert-manager)
    ca_client: CertificateAuthorityClient,

    // Current workload identity
    workload_identity: WorkloadIdentity,

    // Certificates with auto-renewal
    cert_chain: Arc<RwLock<CertificateChain>>,
}

impl WorkloadCertManager {
    /// Fetch certificate from CA (SDS - Secret Discovery Service)
    pub async fn fetch_certificate(&self) -> Result<CertificateChain> {
        let csr = self.generate_csr()?;  // Certificate Signing Request

        // Request certificate from Istio CA
        let cert_response = self.ca_client
            .sign_certificate(SignCertificateRequest {
                csr: csr.to_pem()?,
                workload_identity: self.workload_identity.to_string(),
                ttl: Duration::from_hours(24),
            })
            .await?;

        Ok(CertificateChain {
            cert: cert_response.certificate,
            key: self.private_key.clone(),
            ca_cert: cert_response.ca_certificate,
            valid_until: Utc::now() + chrono::Duration::hours(24),
        })
    }

    /// Auto-renew certificate before expiration
    pub async fn auto_renew_loop(&self) {
        loop {
            let cert = self.cert_chain.read().await;
            let time_until_expiry = cert.valid_until - Utc::now();

            // Renew at 75% of lifetime (18 hours for 24h cert)
            let renew_at = time_until_expiry * 0.75;

            tokio::time::sleep(renew_at.to_std()?).await;

            info!("Certificate expiring soon, renewing...");
            match self.fetch_certificate().await {
                Ok(new_cert) => {
                    *self.cert_chain.write().await = new_cert;
                    info!("Certificate renewed successfully");
                }
                Err(e) => {
                    error!("Failed to renew certificate: {}", e);
                    // Retry with exponential backoff
                }
            }
        }
    }
}

// Workload identity: spiffe://cluster.local/ns/default/sa/user-service
#[derive(Debug, Clone)]
pub struct WorkloadIdentity {
    pub trust_domain: String,    // cluster.local
    pub namespace: String,        // default
    pub service_account: String,  // user-service
}

impl Display for WorkloadIdentity {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "spiffe://{}/ns/{}/sa/{}",
            self.trust_domain,
            self.namespace,
            self.service_account
        )
    }
}
```

#### mTLS Connection Flow

```rust
// Server side: Verify client certificate
pub async fn handle_mtls_connection(
    stream: TcpStream,
    cert_manager: Arc<WorkloadCertManager>,
    authz_policies: Arc<AuthorizationPolicies>,
) -> Result<()> {
    // Load current certificates
    let certs = cert_manager.cert_chain.read().await;

    // Configure TLS acceptor
    let tls_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_client_cert_verifier(Arc::new(WorkloadCertVerifier {
            ca_cert: certs.ca_cert.clone(),
        }))
        .with_single_cert(certs.cert_chain(), certs.key.clone())?;

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    // Perform TLS handshake
    let tls_stream = acceptor.accept(stream).await?;

    // Extract client identity from certificate
    let client_identity = extract_spiffe_identity(&tls_stream)?;

    info!("Authenticated client: {}", client_identity);

    // Check authorization policies
    if !authz_policies.is_allowed(&client_identity, &request) {
        warn!("Authorization denied for {}", client_identity);
        return Err(AuthzError::Denied);
    }

    // Forward to application
    forward_to_app(tls_stream).await
}

// Client side: Present client certificate
pub async fn connect_with_mtls(
    backend: &str,
    cert_manager: Arc<WorkloadCertManager>,
) -> Result<TlsStream<TcpStream>> {
    let certs = cert_manager.cert_chain.read().await;

    // Configure TLS connector
    let tls_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(certs.ca_cert.clone())
        .with_client_auth_cert(certs.cert_chain(), certs.key.clone())?;

    let connector = TlsConnector::from(Arc::new(tls_config));

    // Connect to backend sidecar
    let stream = TcpStream::connect(backend).await?;
    let domain = extract_domain(backend)?;

    // Perform mTLS handshake
    let tls_stream = connector.connect(domain, stream).await?;

    // Verify server identity (SPIFFE)
    verify_spiffe_identity(&tls_stream, expected_identity)?;

    Ok(tls_stream)
}
```

---

### 1.4 Traffic Management Features

#### Intelligent Load Balancing

**Standard load balancing** (current): Round-robin, least connections
**Service mesh load balancing** (target): Locality-aware, zone-aware, subset routing

```rust
// Locality-aware load balancing
pub struct LocalityAwareBalancer {
    // Endpoints grouped by locality (region/zone/subzone)
    localities: HashMap<Locality, Vec<Endpoint>>,

    // Current pod's locality
    local_locality: Locality,
}

impl LocalityAwareBalancer {
    pub fn select_endpoint(&self, request: &Request) -> Option<Endpoint> {
        // 1. Try same-zone endpoints first (lowest latency)
        if let Some(endpoint) = self.select_from_locality(&self.local_locality) {
            return Some(endpoint);
        }

        // 2. Try same-region, different zone (medium latency)
        if let Some(endpoint) = self.select_from_region(&self.local_locality.region) {
            return Some(endpoint);
        }

        // 3. Fallback to any healthy endpoint (high latency)
        self.select_from_any()
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Locality {
    pub region: String,   // us-west-2
    pub zone: String,     // us-west-2a
    pub subzone: String,  // rack-1
}
```

#### Traffic Splitting (Canary Deployments)

```rust
// Route configuration from control plane (RDS)
pub struct TrafficSplit {
    pub routes: Vec<WeightedRoute>,
}

#[derive(Debug, Clone)]
pub struct WeightedRoute {
    pub destination: String,  // user-service-v2
    pub weight: u32,          // 10 (10% of traffic)
    pub subset: Option<String>, // version=v2
}

impl TrafficSplit {
    pub fn select_route(&self, request: &Request) -> &WeightedRoute {
        // Consistent hashing based on request attributes
        let hash = self.compute_hash(request);

        // Select route based on weight distribution
        let mut cumulative = 0;
        for route in &self.routes {
            cumulative += route.weight;
            if hash % 100 < cumulative {
                return route;
            }
        }

        // Fallback to last route
        &self.routes.last().unwrap()
    }
}

// Example: Canary deployment
// 90% traffic to v1, 10% to v2
let split = TrafficSplit {
    routes: vec![
        WeightedRoute {
            destination: "user-service-v1".to_string(),
            weight: 90,
            subset: Some("version=v1".to_string()),
        },
        WeightedRoute {
            destination: "user-service-v2".to_string(),
            weight: 10,
            subset: Some("version=v2".to_string()),
        },
    ],
};
```

#### Fault Injection (Chaos Engineering)

```rust
// Inject faults for testing resilience
pub struct FaultInjection {
    pub delay: Option<FaultDelay>,
    pub abort: Option<FaultAbort>,
}

#[derive(Debug, Clone)]
pub struct FaultDelay {
    pub percentage: f32,      // 10% of requests
    pub fixed_delay: Duration, // 5 seconds delay
}

#[derive(Debug, Clone)]
pub struct FaultAbort {
    pub percentage: f32,  // 5% of requests
    pub http_status: u16, // 503 Service Unavailable
}

impl FaultInjection {
    pub async fn apply(&self, request: &mut Request) -> Result<()> {
        // Random selection
        let random = rand::random::<f32>() * 100.0;

        // Apply delay
        if let Some(delay) = &self.delay {
            if random < delay.percentage {
                warn!("Injecting delay: {:?}", delay.fixed_delay);
                tokio::time::sleep(delay.fixed_delay).await;
            }
        }

        // Apply abort
        if let Some(abort) = &self.abort {
            if random < abort.percentage {
                warn!("Injecting abort: {}", abort.http_status);
                return Err(FaultError::Aborted(abort.http_status));
            }
        }

        Ok(())
    }
}

// Configuration from Istio VirtualService
// apiVersion: networking.istio.io/v1beta1
// kind: VirtualService
// spec:
//   http:
//   - fault:
//       delay:
//         percentage:
//           value: 10
//         fixedDelay: 5s
//       abort:
//         percentage:
//           value: 5
//         httpStatus: 503
```

---

### 1.5 Enhanced Observability

#### Distributed Tracing Integration

```rust
// OpenTelemetry integration for service mesh
pub struct ServiceMeshTracer {
    tracer: opentelemetry::global::Tracer,
    mesh_metadata: MeshMetadata,
}

#[derive(Debug, Clone)]
pub struct MeshMetadata {
    pub pod_name: String,
    pub pod_namespace: String,
    pub workload_name: String,
    pub canonical_service: String,
    pub canonical_revision: String,
}

impl ServiceMeshTracer {
    pub fn create_span(&self, request: &Request) -> Span {
        let mut span = self.tracer
            .span_builder(&format!("{} {}", request.method(), request.uri()))
            .with_kind(SpanKind::Server)
            .start(&self.tracer);

        // Add mesh-specific attributes
        span.set_attribute(KeyValue::new("mesh.pod.name", self.mesh_metadata.pod_name.clone()));
        span.set_attribute(KeyValue::new("mesh.namespace", self.mesh_metadata.pod_namespace.clone()));
        span.set_attribute(KeyValue::new("mesh.workload", self.mesh_metadata.workload_name.clone()));
        span.set_attribute(KeyValue::new("mesh.service", self.mesh_metadata.canonical_service.clone()));
        span.set_attribute(KeyValue::new("mesh.version", self.mesh_metadata.canonical_revision.clone()));

        // Add standard HTTP attributes
        span.set_attribute(KeyValue::new("http.method", request.method().to_string()));
        span.set_attribute(KeyValue::new("http.url", request.uri().to_string()));
        span.set_attribute(KeyValue::new("http.host", request.headers().get("host").map(|h| h.to_str().unwrap_or("")).unwrap_or("")));

        span
    }
}
```

#### Service Mesh Metrics

**Standard Prometheus metrics** + **Service mesh-specific metrics**:

```rust
// Golden signals for service mesh
pub struct MeshMetrics {
    // Request rate
    request_count: Counter,

    // Error rate (4xx, 5xx)
    error_count: Counter,

    // Latency distribution
    request_duration: Histogram,

    // Saturation (connection pool usage)
    connection_pool_usage: Gauge,

    // Mesh-specific: upstream/downstream split
    upstream_request_count: Counter,
    downstream_request_count: Counter,

    // mTLS metrics
    mtls_handshake_count: Counter,
    mtls_handshake_errors: Counter,

    // Circuit breaker state
    circuit_breaker_state: Gauge,
}

// Example metrics
mesh_request_total{
  source_workload="user-service",
  source_namespace="default",
  destination_workload="payment-service",
  destination_namespace="default",
  response_code="200",
  connection_security_policy="mutual_tls"
} 15234

mesh_request_duration_seconds_bucket{
  source_workload="user-service",
  destination_workload="payment-service",
  le="0.005"
} 8432
```

---

### 1.6 Implementation Phases

#### Phase 1: Basic xDS Integration (v1.2)

**Scope**: Read-only xDS client
- ✅ CDS (Cluster Discovery) - Learn about upstream services
- ✅ EDS (Endpoint Discovery) - Get backend IP addresses
- ✅ Dynamic backend updates without restart

**Deliverables**:
1. xDS gRPC client (`src/servicemesh/xds_client.rs`)
2. Configuration hot-reload from control plane
3. Integration with existing load balancer
4. Basic Istio compatibility

**Effort**: 40 hours

**Testing**:
- Deploy Istio in Kubernetes
- Configure Highper Gateway as sidecar
- Verify service discovery works
- Test pod scaling (endpoints added/removed)

---

#### Phase 2: Full Data Plane (v1.3)

**Scope**: Complete xDS protocol + sidecar mode
- ✅ LDS (Listener Discovery) - Inbound/outbound listeners
- ✅ RDS (Route Discovery) - Dynamic routing rules
- ✅ SDS (Secret Discovery) - Automatic certificate rotation
- ✅ mTLS termination and origination

**Deliverables**:
1. Full xDS implementation (all discovery services)
2. Sidecar deployment mode
3. Automatic mTLS with certificate rotation
4. iptables initialization container
5. Workload identity (SPIFFE)

**Effort**: 80 hours

**Testing**:
- End-to-end mTLS between services
- Certificate rotation under load
- Authorization policy enforcement
- Traffic splitting (canary)

---

#### Phase 3: Advanced Features (v1.4)

**Scope**: Enterprise service mesh features
- ✅ Locality-aware load balancing
- ✅ Fault injection
- ✅ Traffic mirroring
- ✅ Request hedging
- ✅ Outlier detection

**Deliverables**:
1. Advanced traffic management
2. Chaos engineering features
3. Enhanced observability
4. Performance optimization

**Effort**: 60 hours

**Testing**:
- Multi-region deployment
- Fault injection scenarios
- Canary deployment workflows
- Performance benchmarks vs Envoy

---

## II. Multi-Tenancy Support

### Overview

**Multi-tenancy** allows a single Highper Gateway instance to serve multiple isolated tenants (customers, teams, environments) with:
- **Resource isolation** (CPU, memory, connections)
- **Configuration isolation** (routes, policies per tenant)
- **Data isolation** (logs, metrics separated)
- **Billing isolation** (usage tracking per tenant)

### Why Multi-Tenancy?

**Use Cases**:
1. **SaaS platforms** - Serve multiple customers from single infrastructure
2. **Development platforms** - Separate dev/staging/prod environments
3. **API marketplace** - Multiple API providers on shared gateway
4. **Managed services** - Hosting provider serving multiple clients

**Benefits**:
- **Cost efficiency** - Share infrastructure across tenants
- **Operational simplicity** - Manage one system instead of N
- **Resource optimization** - Better utilization through sharing
- **Centralized management** - Single control plane

---

### 2.1 Tenant Isolation Model

#### Tenant Identification

```rust
/// Tenant identifier extracted from request
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum TenantId {
    /// Hostname-based: api.tenant1.example.com
    Hostname(String),

    /// Subdomain: tenant1.api.example.com
    Subdomain(String),

    /// Header-based: X-Tenant-ID: tenant1
    Header(String),

    /// API key prefix: tk_tenant1_abc123
    ApiKeyPrefix(String),

    /// JWT claim: {"tenant_id": "tenant1"}
    JwtClaim(String),

    /// Path prefix: /tenants/tenant1/api
    PathPrefix(String),
}

/// Extract tenant from request
pub fn extract_tenant(request: &Request) -> Result<TenantId> {
    // 1. Check subdomain
    if let Some(host) = request.headers().get("host") {
        if let Ok(host_str) = host.to_str() {
            // api.tenant1.example.com -> tenant1
            if let Some(tenant) = parse_subdomain(host_str) {
                return Ok(TenantId::Hostname(tenant));
            }
        }
    }

    // 2. Check X-Tenant-ID header
    if let Some(tenant_header) = request.headers().get("x-tenant-id") {
        if let Ok(tenant) = tenant_header.to_str() {
            return Ok(TenantId::Header(tenant.to_string()));
        }
    }

    // 3. Check JWT claims
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(token) = extract_jwt(auth) {
            if let Some(tenant) = token.claims.get("tenant_id") {
                return Ok(TenantId::JwtClaim(tenant.clone()));
            }
        }
    }

    // 4. Check API key prefix
    if let Some(api_key) = request.headers().get("x-api-key") {
        if let Ok(key) = api_key.to_str() {
            // tk_tenant1_abc123 -> tenant1
            if let Some(tenant) = parse_api_key_prefix(key) {
                return Ok(TenantId::ApiKeyPrefix(tenant));
            }
        }
    }

    Err(TenantError::NoTenantFound)
}
```

#### Tenant Configuration

```yaml
# Multi-tenant configuration
tenants:
  # Tenant 1: SaaS customer "Acme Corp"
  - id: "acme-corp"
    display_name: "Acme Corporation"
    domains:
      - "api.acme-corp.example.com"
      - "acme.api.example.com"

    # Resource limits (prevent noisy neighbor)
    limits:
      max_requests_per_second: 1000
      max_concurrent_connections: 500
      max_request_body_size: 10485760  # 10 MB
      max_response_body_size: 52428800  # 50 MB

    # Dedicated upstream backends
    upstreams:
      - name: "acme-backend"
        servers:
          - url: "http://10.0.1.10:8080"
          - url: "http://10.0.1.11:8080"

    # Tenant-specific routes
    routes:
      - name: "acme-api"
        match_rules:
          paths: ["/api/*"]
        upstream: "acme-backend"

    # Authentication
    auth:
      jwt:
        issuer: "https://acme-corp.example.com"
        audience: "acme-api"
        public_key_url: "https://acme-corp.example.com/.well-known/jwks.json"

    # Observability (isolated metrics/logs)
    observability:
      log_level: "info"
      metrics_enabled: true
      tracing_sample_rate: 0.1  # 10% sampling

    # Billing & usage tracking
    billing:
      plan: "professional"
      billing_id: "cus_acme_12345"
      usage_reporting: true

  # Tenant 2: Internal team "Engineering"
  - id: "engineering"
    display_name: "Engineering Team"
    domains:
      - "eng.internal.example.com"

    limits:
      max_requests_per_second: 5000  # Higher limit for internal
      max_concurrent_connections: 2000

    upstreams:
      - name: "eng-services"
        servers:
          - url: "http://10.0.2.10:8080"

    auth:
      mtls:
        enabled: true
        client_ca: "/etc/certs/internal-ca.pem"

    billing:
      plan: "internal"
      billing_id: null  # No billing for internal
```

---

### 2.2 Resource Isolation

#### Per-Tenant Resource Limits

```rust
/// Resource quotas per tenant
#[derive(Debug, Clone)]
pub struct TenantQuota {
    // Rate limiting
    pub max_requests_per_second: u32,
    pub max_requests_per_minute: u32,
    pub max_requests_per_hour: u32,

    // Connection limits
    pub max_concurrent_connections: u32,
    pub max_connections_per_ip: u32,

    // Request/response size
    pub max_request_body_size: usize,
    pub max_response_body_size: usize,
    pub max_header_size: usize,

    // Backend connections
    pub max_backend_connections: u32,
    pub backend_timeout: Duration,

    // Memory limits (soft limits)
    pub max_memory_usage: usize,  // Bytes

    // CPU limits (enforced via cgroups in container)
    pub cpu_quota: Option<f64>,  // 1.0 = 1 CPU core
}

/// Tenant resource manager
pub struct TenantResourceManager {
    tenants: DashMap<TenantId, TenantResources>,
}

#[derive(Debug)]
struct TenantResources {
    quota: TenantQuota,

    // Current usage
    active_connections: AtomicU32,
    requests_this_second: AtomicU32,
    requests_this_minute: AtomicU32,
    memory_usage: AtomicUsize,

    // Rate limiters
    rate_limiter: Arc<RateLimiter>,

    // Circuit breaker (per tenant)
    circuit_breaker: Arc<CircuitBreaker>,
}

impl TenantResourceManager {
    /// Check if tenant can accept new request
    pub async fn check_quota(
        &self,
        tenant_id: &TenantId,
        request: &Request,
    ) -> Result<QuotaPermit, QuotaError> {
        let resources = self.tenants
            .get(tenant_id)
            .ok_or(QuotaError::TenantNotFound)?;

        // Check concurrent connections
        let current_conns = resources.active_connections.load(Ordering::Relaxed);
        if current_conns >= resources.quota.max_concurrent_connections {
            return Err(QuotaError::ConnectionLimitExceeded {
                current: current_conns,
                limit: resources.quota.max_concurrent_connections,
            });
        }

        // Check rate limit
        if !resources.rate_limiter.check_rate(tenant_id).await {
            return Err(QuotaError::RateLimitExceeded);
        }

        // Check request body size
        if let Some(content_length) = request.headers().get("content-length") {
            let size = content_length.to_str()?.parse::<usize>()?;
            if size > resources.quota.max_request_body_size {
                return Err(QuotaError::RequestTooLarge {
                    size,
                    limit: resources.quota.max_request_body_size,
                });
            }
        }

        // Check circuit breaker
        if !resources.circuit_breaker.allow_request() {
            return Err(QuotaError::CircuitBreakerOpen);
        }

        // Increment counters
        resources.active_connections.fetch_add(1, Ordering::Relaxed);
        resources.requests_this_second.fetch_add(1, Ordering::Relaxed);

        // Return permit that auto-decrements on drop (RAII)
        Ok(QuotaPermit {
            tenant_id: tenant_id.clone(),
            resources: resources.clone(),
        })
    }
}

/// RAII permit - auto-releases resources on drop
pub struct QuotaPermit {
    tenant_id: TenantId,
    resources: Arc<TenantResources>,
}

impl Drop for QuotaPermit {
    fn drop(&mut self) {
        self.resources.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}
```

#### CPU & Memory Isolation (cgroups)

```rust
/// Configure cgroup limits for tenant (Linux only)
#[cfg(target_os = "linux")]
pub fn set_tenant_cgroup_limits(tenant_id: &TenantId, quota: &TenantQuota) -> Result<()> {
    // Create cgroup for tenant
    let cgroup_path = format!("/sys/fs/cgroup/highper-gateway/tenant-{}", tenant_id);
    std::fs::create_dir_all(&cgroup_path)?;

    // Set CPU quota (1.0 = 1 core, 0.5 = 50% of 1 core)
    if let Some(cpu_quota) = quota.cpu_quota {
        let period = 100000;  // 100ms
        let quota_us = (cpu_quota * period as f64) as u64;

        std::fs::write(
            format!("{}/cpu.cfs_quota_us", cgroup_path),
            quota_us.to_string()
        )?;
        std::fs::write(
            format!("{}/cpu.cfs_period_us", cgroup_path),
            period.to_string()
        )?;
    }

    // Set memory limit
    std::fs::write(
        format!("{}/memory.max", cgroup_path),
        quota.max_memory_usage.to_string()
    )?;

    // Add current process to cgroup
    let pid = std::process::id();
    std::fs::write(
        format!("{}/cgroup.procs", cgroup_path),
        pid.to_string()
    )?;

    Ok(())
}
```

---

### 2.3 Configuration Isolation

#### Tenant-Specific Routing

```rust
/// Multi-tenant router
pub struct MultiTenantRouter {
    /// Tenant configurations
    tenants: DashMap<TenantId, TenantConfig>,

    /// Default/fallback tenant
    default_tenant: Option<TenantId>,
}

#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub id: TenantId,
    pub display_name: String,
    pub domains: Vec<String>,

    /// Tenant-specific routes
    pub routes: Vec<RouteConfig>,

    /// Tenant-specific upstreams
    pub upstreams: HashMap<String, UpstreamConfig>,

    /// Tenant-specific middleware
    pub middleware: Vec<Box<dyn Middleware>>,

    /// Tenant-specific policies
    pub policies: TenantPolicies,
}

impl MultiTenantRouter {
    pub async fn route_request(&self, request: &Request) -> Result<RouteMatch> {
        // 1. Extract tenant ID
        let tenant_id = extract_tenant(request)?;

        // 2. Get tenant configuration
        let tenant_config = self.tenants
            .get(&tenant_id)
            .ok_or(RouterError::TenantNotFound)?;

        // 3. Find matching route within tenant
        for route in &tenant_config.routes {
            if route.matches(request) {
                return Ok(RouteMatch {
                    tenant_id: tenant_id.clone(),
                    route: route.clone(),
                    upstream: tenant_config.upstreams
                        .get(&route.upstream)
                        .ok_or(RouterError::UpstreamNotFound)?
                        .clone(),
                });
            }
        }

        Err(RouterError::NoRouteMatched)
    }
}
```

#### Tenant-Specific Middleware

```rust
/// Middleware can be configured per tenant
pub trait TenantAwareMiddleware: Send + Sync {
    async fn process_request(
        &self,
        tenant_id: &TenantId,
        request: Request,
    ) -> Result<Request>;

    async fn process_response(
        &self,
        tenant_id: &TenantId,
        response: Response,
    ) -> Result<Response>;
}

// Example: Per-tenant WAF rules
pub struct TenantWAF {
    rules: DashMap<TenantId, WafRuleSet>,
}

impl TenantAwareMiddleware for TenantWAF {
    async fn process_request(
        &self,
        tenant_id: &TenantId,
        mut request: Request,
    ) -> Result<Request> {
        // Get tenant-specific WAF rules
        let rules = self.rules
            .get(tenant_id)
            .ok_or(WafError::NoRulesForTenant)?;

        // Apply rules
        if let Some(violation) = rules.check_request(&request) {
            warn!(
                "WAF violation for tenant {}: {:?}",
                tenant_id,
                violation
            );
            return Err(WafError::RequestBlocked(violation));
        }

        Ok(request)
    }
}
```

---

### 2.4 Data Isolation

#### Per-Tenant Observability

```rust
/// Tenant-specific metrics
pub struct TenantMetrics {
    tenant_id: TenantId,

    // Request metrics
    request_count: Counter,
    request_duration: Histogram,
    request_size: Histogram,
    response_size: Histogram,

    // Error metrics
    error_count: Counter,
    error_rate: Gauge,

    // Resource usage
    active_connections: Gauge,
    memory_usage: Gauge,
    cpu_usage: Gauge,

    // Billing metrics
    bandwidth_bytes: Counter,  // For billing
    compute_units: Counter,    // For billing
}

// Prometheus labels include tenant_id
http_requests_total{tenant_id="acme-corp", status="200"} 15234
http_request_duration_seconds{tenant_id="acme-corp", le="0.1"} 12543

// Per-tenant queries in Grafana
sum(rate(http_requests_total{tenant_id="acme-corp"}[5m]))
```

#### Per-Tenant Logging

```rust
/// Structured logging with tenant context
pub fn log_tenant_request(
    tenant_id: &TenantId,
    correlation_id: &CorrelationId,
    request: &Request,
) {
    info!(
        tenant_id = %tenant_id,
        correlation_id = %correlation_id,
        method = %request.method(),
        uri = %request.uri(),
        "Tenant request received"
    );
}

// Log output includes tenant_id
{
  "timestamp": "2025-11-17T10:30:00Z",
  "level": "INFO",
  "tenant_id": "acme-corp",
  "correlation_id": "01934567-89ab-7def-0123-456789abcdef",
  "method": "POST",
  "uri": "/api/users",
  "message": "Tenant request received"
}

// Log aggregation with tenant filtering
// Elasticsearch/Loki query: tenant_id:"acme-corp" AND level:ERROR
```

---

### 2.5 Billing & Usage Tracking

```rust
/// Usage tracking for billing
pub struct TenantUsageTracker {
    usage: DashMap<TenantId, TenantUsage>,

    /// Persist to database every N seconds
    persist_interval: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUsage {
    pub tenant_id: TenantId,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,

    // Request metrics
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,

    // Bandwidth
    pub ingress_bytes: u64,   // Request body bytes
    pub egress_bytes: u64,    // Response body bytes
    pub total_bytes: u64,     // ingress + egress

    // Compute
    pub cpu_seconds: f64,     // Total CPU time
    pub request_duration_seconds: f64,  // Total processing time

    // Advanced metrics
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub ssl_handshakes: u64,

    // Billing
    pub compute_units: f64,   // Normalized billing unit
    pub estimated_cost: f64,  // USD
}

impl TenantUsageTracker {
    /// Record request for billing
    pub async fn record_request(
        &self,
        tenant_id: &TenantId,
        request_size: u64,
        response_size: u64,
        duration: Duration,
        status: StatusCode,
    ) {
        let mut usage = self.usage
            .entry(tenant_id.clone())
            .or_insert_with(|| TenantUsage::new(tenant_id.clone()));

        usage.total_requests += 1;

        if status.is_success() {
            usage.successful_requests += 1;
        } else {
            usage.failed_requests += 1;
        }

        usage.ingress_bytes += request_size;
        usage.egress_bytes += response_size;
        usage.total_bytes += request_size + response_size;

        let duration_secs = duration.as_secs_f64();
        usage.request_duration_seconds += duration_secs;

        // Compute units: (duration * complexity_factor)
        // complexity_factor based on request type (simple GET = 1.0, complex POST = 2.0)
        let complexity_factor = self.calculate_complexity(request_size, status);
        usage.compute_units += duration_secs * complexity_factor;

        // Estimated cost: $0.0001 per compute unit + $0.05 per GB transfer
        usage.estimated_cost = (usage.compute_units * 0.0001)
                             + (usage.total_bytes as f64 / 1_073_741_824.0 * 0.05);
    }

    /// Generate billing report
    pub async fn generate_report(&self, tenant_id: &TenantId) -> Option<BillingReport> {
        let usage = self.usage.get(tenant_id)?;

        Some(BillingReport {
            tenant_id: tenant_id.clone(),
            period: (usage.period_start, usage.period_end),

            summary: BillingSummary {
                total_requests: usage.total_requests,
                total_bandwidth_gb: usage.total_bytes as f64 / 1_073_741_824.0,
                total_compute_units: usage.compute_units,
                estimated_cost_usd: usage.estimated_cost,
            },

            breakdown: vec![
                BillingLine {
                    item: "API Requests".to_string(),
                    quantity: usage.total_requests as f64,
                    unit: "requests".to_string(),
                    unit_price: 0.0001,
                    total: usage.total_requests as f64 * 0.0001,
                },
                BillingLine {
                    item: "Data Transfer".to_string(),
                    quantity: usage.total_bytes as f64 / 1_073_741_824.0,
                    unit: "GB".to_string(),
                    unit_price: 0.05,
                    total: usage.total_bytes as f64 / 1_073_741_824.0 * 0.05,
                },
            ],
        })
    }
}
```

---

### 2.6 Implementation Phases

#### Phase 1: Basic Multi-Tenancy (v2.0)

**Scope**: Tenant identification and configuration isolation
- ✅ Tenant extraction (hostname, header, JWT)
- ✅ Per-tenant configuration (routes, upstreams)
- ✅ Per-tenant resource limits (rate limiting, connections)
- ✅ Per-tenant metrics & logging

**Deliverables**:
1. Multi-tenant router (`src/tenancy/router.rs`)
2. Tenant configuration schema
3. Resource quota enforcement
4. Isolated observability

**Effort**: 60 hours

**Testing**:
- 10 tenants with different configs
- Resource limit enforcement
- Metrics/logs separation
- Configuration hot-reload per tenant

---

#### Phase 2: Advanced Isolation (v2.1)

**Scope**: Enhanced security and resource isolation
- ✅ cgroup-based CPU/memory limits
- ✅ Per-tenant WAF rules
- ✅ Per-tenant TLS certificates
- ✅ Tenant-specific middleware chains
- ✅ Cross-tenant request prevention

**Deliverables**:
1. cgroup integration (Linux)
2. Enhanced security policies
3. Tenant isolation tests
4. Performance benchmarks

**Effort**: 40 hours

---

#### Phase 3: Billing & Management (v2.2)

**Scope**: Usage tracking and tenant management
- ✅ Usage tracking for billing
- ✅ Cost estimation
- ✅ Billing API
- ✅ Tenant management API (CRUD)
- ✅ Usage dashboards

**Deliverables**:
1. Usage tracking system
2. Billing API (`/api/billing`)
3. Tenant admin panel
4. Usage reports & exports

**Effort**: 50 hours

---

## III. Implementation Roadmap

### Timeline Overview

```
v1.2 (Months 4-5): Service Mesh - Phase 1
├── xDS client (CDS, EDS)
├── Dynamic configuration
├── Basic Istio compatibility
└── Testing with Istio

v1.3 (Months 5-6): Service Mesh - Phase 2
├── Full xDS (LDS, RDS, SDS)
├── Sidecar mode
├── mTLS automation
└── Certificate rotation

v1.4 (Months 6-7): Service Mesh - Phase 3
├── Advanced traffic management
├── Fault injection
├── Locality-aware LB
└── Performance optimization

v2.0 (Months 8-9): Multi-Tenancy - Phase 1
├── Tenant identification
├── Configuration isolation
├── Resource quotas
└── Isolated observability

v2.1 (Months 9-10): Multi-Tenancy - Phase 2
├── cgroup isolation
├── Enhanced security
├── Tenant-specific middleware
└── Performance testing

v2.2 (Months 10-11): Multi-Tenancy - Phase 3
├── Usage tracking
├── Billing API
├── Tenant management
└── Admin dashboard
```

### Effort Summary

| Feature | Phases | Total Effort | Duration |
|---------|--------|--------------|----------|
| **Service Mesh** | 3 | 180 hours | 4-5 months |
| **Multi-Tenancy** | 3 | 150 hours | 3-4 months |
| **Total** | 6 | **330 hours** | **7-9 months** |

---

## IV. Competitive Analysis

### Service Mesh: vs Envoy

| Feature | Envoy | Highper Gateway (Target) | Advantage |
|---------|-------|---------------------|-----------|
| **xDS Protocol** | ✅ Full | ✅ Full (planned) | Equal |
| **Performance** | Excellent | ✅ Excellent+ | **Better** (Rust, SIMD, io_uring) |
| **Memory Safety** | C++ (unsafe) | ✅ Rust | **Better** (no CVEs like Envoy) |
| **Resource Usage** | ~50MB RAM | Target: ~30MB | **Better** |
| **Config Complexity** | High | Medium | **Better** |
| **Ecosystem** | Huge (Istio, etc) | ⚠️ Growing | ⚠️ Gap |
| **Maturity** | ✅ Production | ⚠️ New | ⚠️ Gap |

**Positioning**: "Envoy alternative with better security and performance"

### Multi-Tenancy: vs Kong

| Feature | Kong | Highper Gateway (Target) | Advantage |
|---------|------|---------------------|-----------|
| **Tenant Isolation** | ✅ Workspaces | ✅ Full (planned) | Equal |
| **Resource Limits** | ✅ Plugins | ✅ Built-in | Equal |
| **Billing Integration** | ✅ Enterprise | ✅ Planned | Equal |
| **Performance** | Good (Nginx+Lua) | ✅ Excellent | **Better** |
| **Cost** | $$$$ Enterprise | ✅ Open Source | **Better** |
| **Configuration** | Complex | ⚠️ Medium | ⚠️ Gap |

**Positioning**: "Kong alternative without enterprise licensing costs"

---

## V. Reference Architecture

### Service Mesh Deployment

```
┌────────────────────────────────────────────────────────────────┐
│                     Kubernetes Cluster                          │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Istio Control Plane                    │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐                │  │
│  │  │  Pilot   │  │  Citadel │  │ Galley   │                │  │
│  │  │  (xDS)   │  │  (CA)    │  │ (Config) │                │  │
│  │  └────┬─────┘  └────┬─────┘  └──────────┘                │  │
│  └───────┼─────────────┼───────────────────────────────────  │  │
│          │xDS gRPC     │SDS                                    │
│          ▼             ▼                                        │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │              Application Namespace                        │ │
│  │                                                            │ │
│  │  ┌─────────────────────────────────────────────────┐     │ │
│  │  │  Pod: frontend                                   │     │ │
│  │  │  ┌──────────────┐  ┌────────────────────────┐   │     │ │
│  │  │  │ Container:   │  │ Sidecar:               │   │     │ │
│  │  │  │ frontend-app │  │ highper-gateway             │   │     │ │
│  │  │  │ :8080        │◄─┤ :15001 (outbound)      │   │     │ │
│  │  │  └──────────────┘  │ :15006 (inbound)       │   │     │ │
│  │  │                    │ :15090 (metrics)       │   │     │ │
│  │  │                    └────────────────────────┘   │     │ │
│  │  └─────────────────────────────────────────────────┘     │ │
│  │                                                            │ │
│  │  ┌─────────────────────────────────────────────────┐     │ │
│  │  │  Pod: backend                                    │     │ │
│  │  │  ┌──────────────┐  ┌────────────────────────┐   │     │ │
│  │  │  │ Container:   │  │ Sidecar:               │   │     │ │
│  │  │  │ backend-app  │  │ highper-gateway             │   │     │ │
│  │  │  │ :8080        │◄─┤ :15001 (outbound)      │   │     │ │
│  │  │  └──────────────┘  │ :15006 (inbound)       │   │     │ │
│  │  │                    │ :15090 (metrics)       │   │     │ │
│  │  │                    └────────────────────────┘   │     │ │
│  │  └─────────────────────────────────────────────────┘     │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Traffic Flow (with mTLS):                                      │
│  frontend → sidecar (mTLS) → backend sidecar → backend-app      │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

### Multi-Tenant Deployment

```
┌────────────────────────────────────────────────────────────────┐
│                    Highper Gateway (Multi-Tenant)                    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │               Tenant Router & Isolation Layer            │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  Request → Extract Tenant ID → Route to Tenant     │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐ │
│  │ Tenant: acme    │  │ Tenant: beta    │  │ Tenant: corp   │ │
│  │                 │  │                 │  │                 │ │
│  │ Routes:         │  │ Routes:         │  │ Routes:        │ │
│  │ - /api/*        │  │ - /v1/*         │  │ - /internal/*  │ │
│  │                 │  │                 │  │                 │ │
│  │ Upstreams:      │  │ Upstreams:      │  │ Upstreams:     │ │
│  │ - 10.0.1.10:80  │  │ - 10.0.2.10:80  │  │ - 10.0.3.10:80 │ │
│  │ - 10.0.1.11:80  │  │ - 10.0.2.11:80  │  │ - 10.0.3.11:80 │ │
│  │                 │  │                 │  │                 │ │
│  │ Limits:         │  │ Limits:         │  │ Limits:        │ │
│  │ - 1k req/s      │  │ - 500 req/s     │  │ - 5k req/s     │ │
│  │ - 500 conns     │  │ - 200 conns     │  │ - 2k conns     │ │
│  │                 │  │                 │  │                 │ │
│  │ Metrics:        │  │ Metrics:        │  │ Metrics:       │ │
│  │ - 15k reqs      │  │ - 8k reqs       │  │ - 42k reqs     │ │
│  │ - $12.50        │  │ - $6.80         │  │ - $0 (internal)│ │
│  └─────────────────┘  └─────────────────┘  └────────────────┘ │
│                                                                  │
│  Observability (Tenant-Separated):                              │
│  - Prometheus: tenant_id label on all metrics                   │
│  - Logs: tenant_id field in structured logs                     │
│  - Tracing: tenant_id span attribute                            │
│  - Billing: Per-tenant usage tracking                           │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

---

## Conclusion

### Service Mesh Integration

**Value Proposition**:
- Drop-in replacement for Envoy in Istio/Linkerd
- Better performance and security (Rust, SIMD, io_uring)
- Lower resource footprint (~40% less memory than Envoy)
- Enterprise-ready service mesh features

**Timeline**: 7-9 months (v1.2 → v1.4)
**Effort**: 180 hours
**ROI**: Access to large enterprise market (service mesh adoption growing 40% YoY)

---

### Multi-Tenancy Support

**Value Proposition**:
- SaaS-ready multi-tenant API gateway
- Resource isolation and quota enforcement
- Built-in usage tracking and billing
- Lower cost than Kong Enterprise (open source)

**Timeline**: 3-4 months (v2.0 → v2.2)
**Effort**: 150 hours
**ROI**: Enables SaaS/platform businesses to use Highper Gateway

---

### Combined Impact

**Total Timeline**: 12-15 months for both features
**Total Effort**: 330 hours
**Market Positioning**: Enterprise-grade proxy with service mesh + multi-tenancy

**Target Customers**:
1. **Service Mesh**: Large enterprises with microservices (Fortune 500)
2. **Multi-Tenancy**: SaaS platforms and API marketplaces (startups to mid-market)

**Competitive Advantage**:
- Envoy + Kong features in single proxy
- Better performance and security (Rust)
- Open source (no enterprise licensing)
- Modern architecture (async, cloud-native)

---

*Last Updated: November 17, 2025*
*Document Version: 1.0*
*Target Audience: Technical decision-makers, architects, engineering leads*
