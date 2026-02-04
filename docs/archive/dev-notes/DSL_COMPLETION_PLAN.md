# DSL Completion Plan

**Date**: November 25, 2025
**Status**: 40% Complete, 60% Remaining
**Estimated Time**: 24 hours (3 days)

---

## Current Status Assessment

### What's Complete ✅

#### 1. Grammar Definition (100%)
**File**: `src/config/dsl.pest`
**Lines**: 229
**Status**: ✅ Complete Caddy-like grammar

**Features Defined**:
- ✅ Global directives (log, admin, metrics)
- ✅ Site addressing (HTTP/HTTPS domains, TCP ports)
- ✅ Simple one-line proxy syntax
- ✅ Block syntax with nested directives
- ✅ Path-based routing
- ✅ All major directives:
  - proxy, tcp, lb (load balancing)
  - pool (connection pooling)
  - health (health checks)
  - tls (TLS/HTTPS configuration)
  - cors, websocket, grpc
  - compress, rate_limit, timeout
  - headers (header_up, header_down)
  - tls-passthrough

**Quality**: Production-grade grammar

#### 2. AST Structures (100%)
**File**: `src/config/dsl_ast.rs`
**Estimated Lines**: 400+
**Status**: ✅ Complete type definitions

**Structures Defined**:
- ✅ Config, GlobalConfig, Site, Route
- ✅ SiteAddress (HTTP, TCP)
- ✅ All directive enums
- ✅ Backend, PoolConfig, HealthCheckConfig
- ✅ TlsConfig, CorsConfig, Compression
- ✅ LoadBalancingAlgorithm
- ✅ Duration, number parsing helpers

**Quality**: Well-structured, type-safe

#### 3. Parser Implementation (70%)
**File**: `src/config/dsl_parser.rs`
**Status**: ⚠️ Partially complete

**What Works**:
- ✅ Global directive parsing (log, admin, metrics)
- ✅ Site address parsing (HTTP/TCP)
- ✅ Simple proxy syntax
- ✅ Backend parsing
- ✅ Scheme and protocol parsing

**What's Missing**:
- ⏭️ Complete directive parsing (only partial)
- ⏭️ Route block parsing (structure exists, directives incomplete)
- ⏭️ Duration parsing
- ⏭️ Quoted string handling
- ⏭️ Variable substitution
- ⏭️ Error messages and validation

### What's Missing ⏭️

#### 4. DSL to Config Converter (30%)
**File**: `src/config/dsl_converter.rs`
**Status**: ⏭️ Needs significant work

**Expected Functionality**:
- Convert DSL AST → Runtime Config struct
- Map directives to middleware configuration
- Handle default values
- Validate configurations
- Generate upstreams, routes, listeners

**Current Gap**: Basic structure exists, implementation incomplete

#### 5. Tests (40%)
**Files**: `tests/dsl_integration.rs`, `tests/dsl_cli_integration.rs`
**Status**: ⏭️ Tests exist but many fail

**Test Coverage**:
- ✅ Test structure defined
- ⏭️ Most tests currently fail (parser/converter incomplete)
- ⏭️ Need comprehensive passing tests

#### 6. Example Files (0%)
**Location**: `examples/*.proxy`
**Status**: ❌ Not created yet

**Needed**:
- simple.proxy
- https-auto-tls.proxy
- load-balancing.proxy
- database-tcp-proxy.proxy
- microservices.proxy
- development.proxy

#### 7. Documentation (20%)
**Status**: ⏭️ Minimal documentation

**Needed**:
- DSL syntax guide
- Migration guide (TOML → DSL)
- Examples and best practices
- CLI usage documentation

#### 8. Migration Tools (0%)
**Status**: ❌ Not started

**Needed**:
- TOML to DSL converter
- YAML to DSL converter
- Validation tool
- CLI integration

---

## Completion Roadmap

### Phase 1: Complete Parser (8 hours)

#### Task 1.1: Finish Directive Parsing (4h)
**Files**: `src/config/dsl_parser.rs`

**Work Required**:
```rust
// Complete implementations for:
fn parse_site_block() -> Result<(Vec<Route>, Vec<Directive>)>
fn parse_route() -> Result<Route>
fn parse_route_block() -> Result<Vec<Directive>>
fn parse_directive() -> Result<Directive>

// Specific directive parsers:
fn parse_lb_directive() -> Result<LoadBalancingAlgorithm>
fn parse_pool_directive() -> Result<PoolConfig>
fn parse_health_directive() -> Result<HealthCheckConfig>
fn parse_tls_directive() -> Result<TlsConfig>
fn parse_cors_directive() -> Result<CorsConfig>
fn parse_compress_directive() -> Result<CompressionConfig>
fn parse_rate_limit_directive() -> Result<RateLimitConfig>
fn parse_timeout_directive() -> Result<Duration>
fn parse_headers_directive() -> Result<HeaderDirective>

// Helper parsers:
fn parse_duration(s: &str) -> Result<Duration>
fn parse_quoted_string(s: &str) -> String
fn parse_number(s: &str) -> Result<u64>
```

**Priority**: HIGH
**Complexity**: Medium
**Estimated**: 4 hours

#### Task 1.2: Error Handling & Validation (2h)
**Files**: `src/config/dsl_parser.rs`

**Work Required**:
- Better error messages with line/column numbers
- Validation during parsing
- Meaningful error contexts
- Suggested fixes

**Example**:
```rust
// Before:
Err(anyhow!("Invalid scheme"))

// After:
Err(anyhow!("Invalid scheme '{}' at line {}. Expected: http://, https://, or grpc://",
    scheme, line_num))
```

**Priority**: HIGH
**Complexity**: Low
**Estimated**: 2 hours

#### Task 1.3: Testing Parser (2h)
**Files**: `tests/dsl_integration.rs`

**Work Required**:
- Fix existing failing tests
- Add tests for each directive
- Add error case tests
- Validate all grammar rules work

**Priority**: HIGH
**Complexity**: Low
**Estimated**: 2 hours

---

### Phase 2: Complete Converter (10 hours)

#### Task 2.1: Core Converter Implementation (6h)
**Files**: `src/config/dsl_converter.rs`

**Work Required**:
```rust
// Main conversion function
pub fn convert_dsl_to_config(dsl: DslConfig) -> Result<RuntimeConfig> {
    let mut config = RuntimeConfig::default();

    // 1. Convert global settings
    convert_global_config(&dsl.global, &mut config)?;

    // 2. Convert each site to listener + routes + upstreams
    for site in dsl.sites {
        convert_site(site, &mut config)?;
    }

    // 3. Validate final configuration
    validate_config(&config)?;

    Ok(config)
}

fn convert_site(site: Site, config: &mut RuntimeConfig) -> Result<()> {
    // Create listener from site address
    let listener = create_listener(&site.address)?;
    config.listeners.push(listener);

    // Create upstream from proxy backends
    let upstream = create_upstream(&site)?;
    config.upstreams.push(upstream);

    // Create routes
    for route in site.routes {
        let runtime_route = create_route(route, &upstream.name)?;
        config.routes.push(runtime_route);
    }

    // Apply directives to configuration
    apply_directives(&site.directives, config)?;

    Ok(())
}

fn create_listener(address: &SiteAddress) -> Result<Listener> {
    // Convert site address to listener configuration
}

fn create_upstream(site: &Site) -> Result<Upstream> {
    // Extract backends from proxy directive
    // Apply load balancing, health checks, etc.
}

fn create_route(route: Route, upstream: &str) -> Result<RuntimeRoute> {
    // Convert DSL route to runtime route
}

fn apply_directives(directives: &[Directive], config: &mut RuntimeConfig) -> Result<()> {
    // Apply each directive to configuration
    for directive in directives {
        match directive {
            Directive::LoadBalancing(algo) => {
                // Update upstream load balancing
            }
            Directive::HealthCheck(health) => {
                // Configure health checks
            }
            Directive::Tls(tls) => {
                // Configure TLS
            }
            Directive::Cors(cors) => {
                // Enable CORS middleware
            }
            Directive::RateLimit(limit) => {
                // Enable rate limiting middleware
            }
            // ... other directives
        }
    }
    Ok(())
}
```

**Priority**: CRITICAL
**Complexity**: High
**Estimated**: 6 hours

#### Task 2.2: Middleware Configuration (2h)
**Files**: `src/config/dsl_converter.rs`

**Work Required**:
```rust
fn configure_middleware(directives: &[Directive]) -> MiddlewareConfig {
    let mut middleware = MiddlewareConfig::default();

    for directive in directives {
        match directive {
            Directive::Cors(cors) => {
                middleware.cors = Some(convert_cors_config(cors));
            }
            Directive::Compress(algos) => {
                middleware.compression = Some(convert_compression_config(algos));
            }
            Directive::RateLimit(limit) => {
                middleware.rate_limit = Some(convert_rate_limit_config(limit));
            }
            Directive::Timeout(duration) => {
                middleware.timeout = Some(*duration);
            }
            _ => {}
        }
    }

    middleware
}
```

**Priority**: HIGH
**Complexity**: Medium
**Estimated**: 2 hours

#### Task 2.3: Testing Converter (2h)
**Files**: `tests/dsl_integration.rs`

**Work Required**:
- Test DSL → Config conversion
- Validate all directives convert correctly
- Test edge cases
- Verify runtime config is valid

**Priority**: HIGH
**Complexity**: Low
**Estimated**: 2 hours

---

### Phase 3: CLI & Migration Tools (4 hours)

#### Task 3.1: CLI Integration (2h)
**Files**: `src/main.rs`, `src/config/mod.rs`

**Work Required**:
```bash
# Load DSL config
highper-gateway --config config.proxy

# Convert TOML to DSL
highper-gateway convert config.toml > config.proxy

# Validate DSL
highper-gateway validate config.proxy

# Show generated config (for debugging)
highper-gateway show-config config.proxy
```

**Implementation**:
```rust
// In main.rs
match args.subcommand {
    Subcommand::Start { config } => {
        // Detect format from extension
        let runtime_config = if config.ends_with(".proxy") {
            // Load DSL
            let dsl_text = std::fs::read_to_string(&config)?;
            let dsl_config = parse_dsl(&dsl_text)?;
            convert_dsl_to_config(dsl_config)?
        } else {
            // Load TOML/YAML
            load_config(&config)?
        };

        start_server(runtime_config).await?;
    }
    Subcommand::Convert { input, output } => {
        convert_config(&input, &output)?;
    }
    Subcommand::Validate { config } => {
        validate_dsl_file(&config)?;
    }
}
```

**Priority**: MEDIUM
**Complexity**: Low
**Estimated**: 2 hours

#### Task 3.2: Migration Tools (2h)
**Files**: `src/config/dsl_generator.rs`

**Work Required**:
```rust
pub fn toml_to_dsl(toml_config: &RuntimeConfig) -> Result<String> {
    let mut dsl = String::new();

    // Generate global directives
    if let Some(log) = &toml_config.log_level {
        dsl.push_str(&format!("log {}\n", log));
    }

    // Generate sites
    for listener in &toml_config.listeners {
        dsl.push_str(&generate_site(listener, toml_config)?);
    }

    Ok(dsl)
}

fn generate_site(listener: &Listener, config: &RuntimeConfig) -> Result<String> {
    let mut site = String::new();

    // Site address
    site.push_str(&format!("{}:{} {{\n", listener.host, listener.port));

    // Routes and directives
    for route in config.routes_for_listener(listener) {
        site.push_str(&generate_route(route)?);
    }

    site.push_str("}\n\n");
    Ok(site)
}
```

**Priority**: MEDIUM
**Complexity**: Medium
**Estimated**: 2 hours

---

### Phase 4: Examples & Documentation (2 hours)

#### Task 4.1: Create Example Files (1h)
**Files**: `examples/*.proxy`

**Examples Needed**:

**simple.proxy**:
```
# Simple HTTP proxy
localhost:8080 proxy backend:3000

log info
```

**https-auto-tls.proxy**:
```
# HTTPS with automatic TLS
https://example.com {
    tls user@example.com
    proxy backend:3000
    compress gzip br
    cors
}
```

**load-balancing.proxy**:
```
# Load balanced API
api.example.com {
    proxy srv1:8080 srv2:8080 srv3:8080
    lb least_conn
    health interval=10s path="/health"
    rate_limit 1000 per 1s
}
```

**database-tcp-proxy.proxy**:
```
# MySQL TCP proxy
:3306 mysql {
    proxy db1:3306 db2:3306
    lb least_conn
    health interval=5s
}
```

**microservices.proxy**:
```
# Microservices gateway
https://api.example.com {
    tls internal

    /users/* {
        proxy users-service:8001
        rate_limit 100 per 1s
    }

    /orders/* {
        proxy orders-service:8002
        rate_limit 50 per 1s
    }

    /products/* {
        proxy products-service:8003
        rate_limit 200 per 1s
    }
}
```

**development.proxy**:
```
# Development environment
localhost:8080 {
    proxy localhost:3000
    log debug
    cors origins="*"
}

admin localhost:9090
metrics on
```

**Priority**: MEDIUM
**Complexity**: Low
**Estimated**: 1 hour

#### Task 4.2: Write DSL Documentation (1h)
**Files**: `docs/DSL_SYNTAX_GUIDE.md`

**Content**:
1. Overview and benefits
2. Basic syntax
3. Global directives reference
4. Site addressing
5. Directives reference (all)
6. Examples for each feature
7. Migration guide
8. Best practices

**Priority**: MEDIUM
**Complexity**: Low
**Estimated**: 1 hour

---

## Detailed Task Breakdown

### Week 1: Parser & Converter (18 hours)

**Day 1: Parser Completion (8h)**
- Morning: Complete directive parsing (4h)
- Afternoon: Add error handling (2h) + test parser (2h)

**Day 2: Converter Core (8h)**
- Morning: Core converter implementation (4h)
- Afternoon: Middleware configuration (2h) + testing (2h)

**Day 3: Integration (2h)**
- Morning: End-to-end integration testing (2h)

### Week 2: CLI, Tools & Docs (6 hours)

**Day 4: CLI & Tools (4h)**
- Morning: CLI integration (2h)
- Afternoon: Migration tools (2h)

**Day 5: Examples & Docs (2h)**
- Morning: Create example files (1h)
- Afternoon: Write documentation (1h)

**Total**: 24 hours

---

## Success Criteria

### Parser Complete ✅
- [ ] All directive types parse successfully
- [ ] Error messages are helpful with line numbers
- [ ] All grammar rules tested
- [ ] 100% of test DSL files parse

### Converter Complete ✅
- [ ] DSL converts to valid RuntimeConfig
- [ ] All directives map to correct config
- [ ] Middleware configured correctly
- [ ] Generated config matches expected output

### CLI & Tools Complete ✅
- [ ] Can load .proxy files directly
- [ ] TOML → DSL conversion works
- [ ] DSL validation command works
- [ ] show-config command helpful

### Examples & Docs Complete ✅
- [ ] 6 example files created and tested
- [ ] DSL syntax guide complete
- [ ] Migration guide written
- [ ] Best practices documented

### Integration Tests Passing ✅
- [ ] All DSL integration tests pass
- [ ] CLI integration tests pass
- [ ] Example files load and run correctly
- [ ] Converter produces valid configs

---

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_parse_duration() {
    assert_eq!(parse_duration("10s"), Ok(Duration::from_secs(10)));
    assert_eq!(parse_duration("5m"), Ok(Duration::from_secs(300)));
    assert_eq!(parse_duration("1h"), Ok(Duration::from_secs(3600)));
}

#[test]
fn test_parse_lb_directive() {
    let dsl = "lb round_robin";
    let result = parse_directive_from_str(dsl);
    assert!(matches!(result, Ok(Directive::LoadBalancing(LoadBalancingAlgorithm::RoundRobin))));
}
```

### Integration Tests
```rust
#[test]
fn test_simple_proxy_conversion() {
    let dsl = "localhost:8080 proxy backend:3000";
    let config = parse_and_convert_dsl(dsl).unwrap();

    assert_eq!(config.listeners.len(), 1);
    assert_eq!(config.upstreams.len(), 1);
    assert_eq!(config.upstreams[0].servers.len(), 1);
}
```

### E2E Tests
```bash
# Test loading DSL file
./target/release/highper-gateway --config examples/simple.proxy &
sleep 2
curl http://localhost:8080/test
killall highper-gateway

# Test conversion
./target/release/highper-gateway convert config.toml > config.proxy
./target/release/highper-gateway validate config.proxy
```

---

## Risk Mitigation

### Risk 1: Parser Complexity
**Risk**: Directive parsing more complex than expected
**Mitigation**: Start with simple directives, add complex ones incrementally
**Fallback**: Mark complex directives as "experimental" for v1.0

### Risk 2: Converter Edge Cases
**Risk**: Unexpected configuration combinations
**Mitigation**: Comprehensive test suite with real-world examples
**Fallback**: Document known limitations

### Risk 3: Time Overrun
**Risk**: 24 hours not sufficient
**Mitigation**: Prioritize critical features first (parser + converter)
**Fallback**: Ship with basic DSL, add advanced features in v1.1

---

## Next Steps

### Immediate (Start Now)
1. Begin Task 1.1: Complete directive parsing
2. Set up parser test harness
3. Fix failing integration tests one by one

### This Week
- Complete Phase 1 (Parser) - 8 hours
- Complete Phase 2 (Converter) - 10 hours
- Total: 18 hours (Days 1-3)

### Next Week
- Complete Phase 3 (CLI & Tools) - 4 hours
- Complete Phase 4 (Examples & Docs) - 2 hours
- Total: 6 hours (Days 4-5)

---

**Timeline**: 5 working days (24 hours total)
**Priority**: HIGH (short-term goal for v1.0)
**Status**: Ready to begin Phase 1

---

*Created: November 25, 2025*
*Next Update: After Phase 1 completion*

---
