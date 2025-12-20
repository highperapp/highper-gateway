# DSL Directives Analysis - Missing Features

**Date**: December 20, 2025
**Purpose**: Complete analysis of missing DSL directives for all 15 scenarios
**Goal**: Enable full DSL support for easy adoption and configuration

---

## Executive Summary

**Current DSL Support**: ~70% (basic directives)
**Missing**: ~30% (advanced features: WAF, Cache, GraphQL, PHP-FPM, Geographic, Service Discovery)

**Impact**:
- Scenarios 9, 11, 12, 13, 14, 15 cannot be fully configured via DSL
- Users must fall back to YAML for advanced features
- Reduces ease of adoption

**Timeline to Complete**:
- DSL Grammar & Parser: 15-20 hours
- Testing: 5-8 hours
- **Total**: 20-28 hours

---

## Missing Directives by Feature

### 1. WAF (Web Application Firewall) - Scenario 09

**Config Structure**:
```rust
pub struct WafConfig {
    pub enabled: bool,
    pub mode: WafMode,  // custom, coraza, modsecurity, aws
    pub block_mode: bool,
    pub custom: Option<CustomWafConfig>,
    pub max_body_size: usize,
    pub coraza: Option<CorazaConfig>,
    pub modsecurity: Option<ModSecurityConfig>,
    pub aws: Option<AwsWafConfig>,
}
```

**Required DSL Directives**:

```dsl
# Basic WAF
waf enabled
waf mode=custom block
waf mode=modsecurity log_only
waf max_body_size=10485760  # 10MB

# Custom WAF rules
waf_rule sql_injection enabled
waf_rule xss enabled
waf_rule path_traversal enabled
waf_rule rate_limit requests=100 window=60s

# ModSecurity
waf mode=modsecurity {
    rules_file /etc/modsecurity/rules.conf
    paranoia_level 2
    audit_log /var/log/modsec_audit.log
}

# AWS WAF
waf mode=aws {
    web_acl_id arn:aws:wafv2:us-east-1:123456789012:regional/webacl/...
    region us-east-1
    api_mode managed
}

# Coraza
waf mode=coraza {
    rules_dir /etc/coraza/rules
    audit_log /var/log/coraza_audit.log
}
```

**Grammar Rules to Add**:
```pest
waf_directive = {
    "waf" ~ waf_option+ ~ (waf_block | newline)
}

waf_option = {
    "enabled"
  | "mode=" ~ waf_mode
  | "block"
  | "log_only"
  | "max_body_size=" ~ number
}

waf_mode = { "custom" | "modsecurity" | "coraza" | "aws" }

waf_block = {
    "{" ~ newline* ~
    (waf_engine_directive)* ~ newline* ~
    "}" ~ newline*
}

waf_rule_directive = {
    "waf_rule" ~ waf_rule_type ~ waf_rule_option* ~ newline
}

waf_rule_type = {
    "sql_injection"
  | "xss"
  | "path_traversal"
  | "rate_limit"
  | "user_agent"
  | "method"
}
```

**Estimated Effort**: 4-6 hours

---

### 2. Cache - Scenario 11

**Config Structure**:
```rust
pub struct CacheConfig {
    pub enabled: bool,
    pub default_ttl: Duration,
    pub max_size: usize,
    pub cleanup_interval: Duration,
    pub cache_only_success: bool,
    pub methods: Vec<String>,
    pub key_headers: Vec<String>,
}
```

**Required DSL Directives**:

```dsl
# Basic cache
cache enabled
cache ttl=300s
cache max_size=10000
cache cleanup_interval=60s

# Advanced cache options
cache methods GET HEAD
cache key_headers Authorization Accept-Language
cache only_success

# Cache control
cache control {
    respect_cache_control true
    ignore_query_string false
    vary_headers Host Accept-Encoding
}

# Per-route cache
/api/* {
    cache ttl=60s
    proxy backend:8080
}

/static/* {
    cache ttl=3600s
    proxy cdn:8080
}
```

**Grammar Rules to Add**:
```pest
cache_directive = {
    "cache" ~ cache_option+ ~ (cache_block | newline)
}

cache_option = {
    "enabled"
  | "ttl=" ~ duration
  | "max_size=" ~ number
  | "cleanup_interval=" ~ duration
  | "methods" ~ method_list
  | "key_headers" ~ header_list
  | "only_success"
}

cache_block = {
    "{" ~ newline* ~
    (cache_control_directive)* ~ newline* ~
    "}" ~ newline*
}

method_list = { http_method+ }
http_method = { "GET" | "POST" | "PUT" | "DELETE" | "HEAD" | "OPTIONS" }

header_list = { header_name+ }
```

**Estimated Effort**: 3-4 hours

---

### 3. GraphQL - Scenario 13

**Config Structure**:
```rust
pub struct GraphQLConfig {
    pub enable_stitching: bool,
    pub enable_cache: bool,
    pub cache_ttl: Duration,
    pub enable_batching: bool,
    pub max_batch_size: usize,
    pub introspection_enabled: bool,
    pub backends: Vec<GraphQLBackend>,
}

pub struct GraphQLBackend {
    pub name: String,
    pub url: String,
    pub namespace: Option<String>,
}
```

**Required DSL Directives**:

```dsl
# Basic GraphQL
graphql enabled
graphql endpoint=/graphql
graphql introspection

# Federation/Stitching
graphql stitching {
    backend users {
        url http://users-service:4001/graphql
        namespace users
    }

    backend posts {
        url http://posts-service:4002/graphql
        namespace posts
    }
}

# Caching & Batching
graphql cache ttl=300s
graphql batching max_size=10

# Example route
/graphql {
    graphql
    cors
    rate_limit 1000 per 60s
}
```

**Grammar Rules to Add**:
```pest
graphql_directive = {
    "graphql" ~ graphql_option* ~ (graphql_block | newline)
}

graphql_option = {
    "enabled"
  | "endpoint=" ~ quoted_string
  | "introspection"
  | "cache" ~ "ttl=" ~ duration
  | "batching" ~ "max_size=" ~ number
}

graphql_block = {
    "{" ~ newline* ~
    (graphql_stitching_directive | graphql_backend_directive)* ~ newline* ~
    "}" ~ newline*
}

graphql_stitching_directive = {
    "stitching" ~ "{" ~ newline* ~
    graphql_backend_block* ~
    "}" ~ newline*
}

graphql_backend_block = {
    "backend" ~ identifier ~ "{" ~ newline* ~
    (graphql_backend_option ~ newline)* ~
    "}" ~ newline*
}

graphql_backend_option = {
    "url" ~ quoted_string
  | "namespace" ~ identifier
}
```

**Estimated Effort**: 3-5 hours

---

### 4. PHP-FPM - Scenario 14

**Config Structure**:
```rust
pub struct PhpFpmConfig {
    pub socket: String,
    pub pool_size: usize,
    pub connect_timeout: u64,
    pub read_timeout: u64,
    pub write_timeout: u64,
    pub keepalive_timeout: u64,
    pub script_extensions: Vec<String>,
}
```

**Required DSL Directives**:

```dsl
# Basic PHP-FPM
php_fpm enabled
php_fpm socket=/var/run/php/php8.2-fpm.sock
php_fpm pool_size=50

# Timeouts
php_fpm connect_timeout=5s
php_fpm read_timeout=60s
php_fpm write_timeout=60s
php_fpm keepalive=90s

# Static files + PHP
http://php.example.com:8080 {
    root /var/www/html
    index index.php index.html

    # Static files
    /static/* {
        static_files
        try_files $uri $uri/ =404
    }

    # PHP scripts
    /*.php {
        php_fpm socket=/var/run/php/php8.2-fpm.sock
        php_fpm script_extensions .php .phtml
    }
}
```

**Grammar Rules to Add**:
```pest
php_fpm_directive = {
    "php_fpm" ~ php_fpm_option+ ~ newline
}

php_fpm_option = {
    "enabled"
  | "socket=" ~ quoted_string
  | "pool_size=" ~ number
  | "connect_timeout=" ~ duration
  | "read_timeout=" ~ duration
  | "write_timeout=" ~ duration
  | "keepalive=" ~ duration
  | "script_extensions" ~ file_extension+
}

file_extension = @{ "." ~ (ASCII_ALPHANUMERIC)+ }

static_files_directive = {
    "static_files" ~ newline
}

root_directive = {
    "root" ~ quoted_string ~ newline
}

index_directive = {
    "index" ~ filename+ ~ newline
}

filename = @{ (ASCII_ALPHANUMERIC | "." | "-" | "_")+ }

try_files_directive = {
    "try_files" ~ try_files_pattern+ ~ newline
}

try_files_pattern = { "$uri" | "$uri/" | variable | quoted_string }
```

**Estimated Effort**: 4-6 hours

---

### 5. Geographic Routing - Scenario 15

**Config Structure**:
```rust
// In LoadBalancingConfig
pub algorithm: Algorithm,

pub enum Algorithm {
    Geographic,
    // ... other algorithms
}

// Geographic provider config (exists in codebase)
```

**Required DSL Directives**:

```dsl
# Geographic routing
lb geographic
geoip provider=maxmind
geoip database=/var/lib/GeoIP/GeoLite2-City.mmdb
geoip fallback=least_conn

# Backend locations
https://api.example.com {
    upstream us-east {
        server 10.0.1.10:8080 location 40.7128,-74.0060
        server 10.0.1.11:8080 location 40.7128,-74.0060
    }

    upstream eu-west {
        server 10.0.2.10:8080 location 51.5074,-0.1278
        server 10.0.2.11:8080 location 51.5074,-0.1278
    }

    upstream asia-pacific {
        server 10.0.3.10:8080 location 35.6762,139.6503
        server 10.0.3.11:8080 location 35.6762,139.6503
    }

    lb geographic
    geoip provider=maxmind database=/var/lib/GeoIP/GeoLite2-City.mmdb
}
```

**Grammar Rules to Add**:
```pest
geoip_directive = {
    "geoip" ~ geoip_option+ ~ newline
}

geoip_option = {
    "provider=" ~ geoip_provider
  | "database=" ~ quoted_string
  | "fallback=" ~ lb_algorithm
}

geoip_provider = { "maxmind" | "ip2location" }

// Update backend to support location
backend_with_location = {
    "server" ~ backend ~ "location" ~ coordinates
}

coordinates = {
    number ~ "," ~ number  // latitude,longitude
}

// Update lb_algorithm to include geographic
lb_algorithm = {
    "round_robin"
  | "least_conn"
  | "ip_hash"
  | "random"
  | "weighted"
  | "consistent_hash"
  | "geographic"
}
```

**Estimated Effort**: 3-4 hours

---

### 6. Service Discovery - Scenario 12

**Config Structure**:
```rust
// Service discovery config (may be in separate module)
pub struct ServiceDiscoveryConfig {
    pub provider: ServiceDiscoveryProvider,
    pub consul: Option<ConsulConfig>,
    pub etcd: Option<EtcdConfig>,
}
```

**Required DSL Directives**:

```dsl
# Consul service discovery
service_discovery consul {
    address http://consul:8500
    token your-consul-token
    datacenter dc1
    service my-backend-service
    tag production
    health_check true
}

# etcd service discovery
service_discovery etcd {
    endpoints http://etcd1:2379,http://etcd2:2379,http://etcd3:2379
    prefix /services/backends
    username admin
    password secret
}

# Example microservices setup
https://api.example.com {
    service_discovery consul {
        address http://localhost:8500
        service user-service
        health_check true
    }

    lb least_conn
    circuit_breaker threshold=5 timeout=30s
}
```

**Grammar Rules to Add**:
```pest
service_discovery_directive = {
    "service_discovery" ~ sd_provider ~ sd_block
}

sd_provider = { "consul" | "etcd" | "kubernetes" }

sd_block = {
    "{" ~ newline* ~
    (sd_option ~ newline)* ~
    "}" ~ newline*
}

sd_option = {
    "address" ~ quoted_string
  | "endpoints" ~ quoted_string
  | "token" ~ quoted_string
  | "datacenter" ~ identifier
  | "service" ~ identifier
  | "tag" ~ identifier
  | "health_check" ~ boolean
  | "prefix" ~ quoted_string
  | "username" ~ quoted_string
  | "password" ~ quoted_string
}

boolean = { "true" | "false" }
identifier = @{ (ASCII_ALPHANUMERIC | "-" | "_")+ }
```

**Estimated Effort**: 4-5 hours

---

## Additional Minor Directives

### 7. Advanced TLS Options

**Current**: Basic TLS support
**Missing**:
- `tls_ciphers` - Cipher suite selection
- `tls_min_version` / `tls_max_version` - Version constraints
- `tls_client_auth` - mTLS client certificate requirements
- `tls_ocsp_stapling` - OCSP configuration
- `tls_session_cache` - Session resumption

**Example DSL**:
```dsl
https://secure.example.com {
    tls cert=/path/to/cert.pem key=/path/to/key.pem
    tls_protocols TLSv1.2 TLSv1.3
    tls_ciphers ECDHE-RSA-AES256-GCM-SHA384:ECDHE-RSA-AES128-GCM-SHA256
    tls_client_auth required verify_depth=2
    tls_ocsp_stapling enabled
    tls_session_cache enabled size=1024
}
```

**Estimated Effort**: 2-3 hours

---

### 8. mTLS Directives

**Config Structure**: Part of TLS config
**Missing**:
- `mtls` - Enable mutual TLS
- `mtls_ca_cert` - CA certificate for client verification
- `mtls_verify_depth` - Verification chain depth
- `mtls_crl` - Certificate revocation list

**Example DSL**:
```dsl
https://secure.example.com {
    tls cert=/path/to/cert.pem key=/path/to/key.pem
    mtls enabled
    mtls_ca_cert /path/to/ca.pem
    mtls_verify_depth 3
    mtls_crl /path/to/crl.pem
    mtls_require_cert true
}
```

**Estimated Effort**: 2 hours

---

## Implementation Priority

### Phase 1: High Priority (Core Features) - 15-20 hours
1. **Cache directives** (3-4h) - Scenario 11
2. **WAF directives** (4-6h) - Scenario 09
3. **GraphQL directives** (3-5h) - Scenario 13
4. **PHP-FPM directives** (4-6h) - Scenario 14

**Scenarios Unlocked**: 4 (9, 11, 13, 14)

### Phase 2: Medium Priority (Advanced) - 8-12 hours
5. **Geographic routing** (3-4h) - Scenario 15
6. **Service Discovery** (4-5h) - Scenario 12
7. **Advanced TLS** (2-3h) - All HTTPS scenarios

**Scenarios Unlocked**: 2 (12, 15)
**Scenarios Enhanced**: All HTTPS/TLS scenarios

### Phase 3: Optional (Polish) - 2-3 hours
8. **mTLS directives** (2h) - Scenario 09
9. **Documentation** (1h)

---

## Testing Strategy

### Per-Directive Testing
1. **Unit tests**: Grammar parsing for each new directive
2. **Integration tests**: Full config parsing with new directives
3. **Scenario tests**: Each scenario config validates and runs

### Test Matrix

| Directive | Parse Test | Integration Test | Runtime Test |
|-----------|------------|------------------|--------------|
| waf | ✅ | ✅ | ✅ |
| cache | ✅ | ✅ | ✅ |
| graphql | ✅ | ✅ | ✅ |
| php_fpm | ✅ | ✅ | ✅ |
| geoip | ✅ | ✅ | ✅ |
| service_discovery | ✅ | ✅ | ⚠️ (needs external services) |

### Validation Commands
```bash
# Parse validation
./highper-gateway validate -c configs/scenarios/scenario-09-waf-mtls.proxy

# Runtime test
./highper-gateway start -c configs/scenarios/scenario-11-cdn-caching.proxy --test-mode

# Load test
./scripts/test-scenario.sh 11
```

---

## Implementation Checklist

### Grammar (.pest file)
- [ ] Add WAF grammar rules
- [ ] Add Cache grammar rules
- [ ] Add GraphQL grammar rules
- [ ] Add PHP-FPM grammar rules
- [ ] Add Geographic routing rules
- [ ] Add Service Discovery rules
- [ ] Add Advanced TLS rules
- [ ] Add mTLS rules

### Parser (dsl_parser.rs)
- [ ] Implement WAF directive parsing
- [ ] Implement Cache directive parsing
- [ ] Implement GraphQL directive parsing
- [ ] Implement PHP-FPM directive parsing
- [ ] Implement Geographic directive parsing
- [ ] Implement Service Discovery parsing
- [ ] Implement Advanced TLS parsing
- [ ] Implement mTLS parsing

### AST (dsl_ast.rs)
- [ ] Add WAF AST structures
- [ ] Add Cache AST structures
- [ ] Add GraphQL AST structures
- [ ] Add PHP-FPM AST structures
- [ ] Add Geographic AST structures
- [ ] Add Service Discovery AST structures

### Converter (dsl_converter.rs)
- [ ] Convert WAF directives to schema
- [ ] Convert Cache directives to schema
- [ ] Convert GraphQL directives to schema
- [ ] Convert PHP-FPM directives to schema
- [ ] Convert Geographic directives to schema
- [ ] Convert Service Discovery to schema

### Tests
- [ ] Grammar parse tests (pest)
- [ ] Parser unit tests
- [ ] Converter tests
- [ ] Full scenario integration tests
- [ ] Runtime validation tests

### Scenarios
- [ ] Update scenario 09 (WAF + mTLS)
- [ ] Update scenario 11 (Cache)
- [ ] Update scenario 12 (Service Discovery)
- [ ] Update scenario 13 (GraphQL)
- [ ] Update scenario 14 (PHP-FPM)
- [ ] Update scenario 15 (Geographic)

---

## Success Criteria

### Functional Requirements
- ✅ All 6 missing features have DSL directives
- ✅ All directives parse correctly
- ✅ All directives convert to correct schema
- ✅ All 15 scenarios can be configured via DSL
- ✅ DSL configs work identically to YAML

### Quality Requirements
- ✅ 100% test coverage for new directives
- ✅ Clear error messages for invalid syntax
- ✅ Documentation for all new directives
- ✅ Examples in each scenario config

### Performance Requirements
- Parse time < 10ms for typical config
- No memory leaks in parser
- Efficient AST representation

---

## Timeline Estimate

| Phase | Tasks | Hours | Days (8h/day) |
|-------|-------|-------|---------------|
| **Phase 1** | Cache, WAF, GraphQL, PHP-FPM | 15-20h | 2-3 days |
| **Phase 2** | Geographic, Service Discovery, Advanced TLS | 8-12h | 1-2 days |
| **Phase 3** | mTLS, Documentation | 2-3h | 0.5 days |
| **Testing** | All scenarios, integration | 5-8h | 1 day |
| **Total** | | **30-43h** | **4.5-6 days** |

---

## Conclusion

Completing DSL support for all advanced features will:

1. **Enable 6 more scenarios** to use DSL (currently simplified)
2. **Improve adoption** - Users prefer simple DSL over verbose YAML
3. **Reduce errors** - DSL syntax is more intuitive
4. **Better documentation** - DSL examples are clearer

**Recommended Approach**:
- Start with Phase 1 (Cache, WAF, GraphQL, PHP-FPM) → 4 scenarios
- Move to Phase 2 once Phase 1 is validated
- Phase 3 is optional polish

**Next Action**: Begin implementing Cache directives (simplest, 3-4 hours)

---

**Status**: ✅ Analysis Complete
**Document**: DSL_DIRECTIVES_ANALYSIS.md
**Date**: December 20, 2025
