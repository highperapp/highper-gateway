# Option 3: DSL Parser + PHP-FPM Implementation Plan

**Date**: December 20, 2025
**Goal**: Complete DSL parser for all 15 scenarios + finish PHP-FPM integration
**Timeline**: 30-43 hours (4.5-6 days at 8h/day)
**Priority**: Strategic - Enables easy adoption and broader deployment scenarios

---

## Strategic Rationale

### Why DSL First?

1. **Ease of Adoption**: Simple DSL syntax lowers barrier to entry
2. **Quick Experimentation**: Users can try scenarios without learning YAML intricacies
3. **Better Documentation**: DSL examples are clearer and more concise
4. **Load Testing Prep**: Simplifies writing 15 load test configurations for Option 1

### Why PHP-FPM?

1. **Market Expansion**: Opens PHP hosting market (huge deployment base)
2. **Nginx Replacement**: Directly compete with Nginx for web server workloads
3. **Scenario Completion**: Enables Scenario 14 (currently only 20% done)
4. **Real-world Use Case**: Validates static file + dynamic content handling

---

## Work Breakdown

### Analysis Complete ✅

**Documents Created**:
1. `DSL_DIRECTIVES_ANALYSIS.md` - Complete analysis of missing directives
2. `PHP_FPM_IMPLEMENTATION_STATUS.md` - Detailed PHP-FPM status

**Key Findings**:
- DSL: ~70% complete, missing 6 major feature directives
- PHP-FPM: ~65% complete (FastCGI 100%, integration 0%)
- Total effort: 30-43 hours (revised down from 50+ hours)

---

## Implementation Phases

### PHASE 1: DSL Grammar Extensions (15-20 hours)

**Goal**: Add all missing directives to DSL grammar and parser

#### 1.1 Cache Directives (3-4 hours) - Priority 1

**Files**:
- `highper-gateway/src/config/dsl.pest` - Grammar rules
- `highper-gateway/src/config/dsl_parser.rs` - Parser implementation
- `highper-gateway/src/config/dsl_ast.rs` - AST structures
- `highper-gateway/src/config/dsl_converter.rs` - Schema conversion

**Grammar to Add**:
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
```

**Testing**:
```bash
# Parse test
./highper-gateway validate -c configs/scenarios/scenario-11-cdn-caching.proxy

# Integration test
cargo test dsl::cache
```

---

#### 1.2 WAF Directives (4-6 hours) - Priority 1

**Grammar to Add**:
```pest
waf_directive = {
    "waf" ~ waf_option+ ~ (waf_block | newline)
}

waf_option = {
    "enabled"
  | "mode=" ~ waf_mode
  | "block" | "log_only"
  | "max_body_size=" ~ number
}

waf_mode = { "custom" | "modsecurity" | "coraza" | "aws" }

waf_rule_directive = {
    "waf_rule" ~ waf_rule_type ~ waf_rule_option* ~ newline
}
```

**Testing**:
```bash
./highper-gateway validate -c configs/scenarios/scenario-09-waf-mtls.proxy
```

---

#### 1.3 GraphQL Directives (3-5 hours) - Priority 1

**Grammar to Add**:
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

graphql_stitching_directive = {
    "stitching" ~ "{" ~ newline* ~
    graphql_backend_block* ~
    "}" ~ newline*
}
```

**Testing**:
```bash
./highper-gateway validate -c configs/scenarios/scenario-13-graphql.proxy
```

---

#### 1.4 PHP-FPM Directives (4-6 hours) - Priority 1

**Grammar to Add**:
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

root_directive = {
    "root" ~ quoted_string ~ newline
}

index_directive = {
    "index" ~ filename+ ~ newline
}

static_files_directive = {
    "static_files" ~ newline
}

try_files_directive = {
    "try_files" ~ try_files_pattern+ ~ newline
}
```

**Testing**:
```bash
./highper-gateway validate -c configs/scenarios/scenario-14-static-php-fpm.proxy
```

---

#### 1.5 Geographic Routing Directives (3-4 hours) - Priority 2

**Grammar to Add**:
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

backend_with_location = {
    "server" ~ backend ~ "location" ~ coordinates
}

coordinates = {
    number ~ "," ~ number  // latitude,longitude
}

// Update lb_algorithm
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

**Testing**:
```bash
./highper-gateway validate -c configs/scenarios/scenario-15-geo-routing.proxy
```

---

#### 1.6 Service Discovery Directives (4-5 hours) - Priority 2

**Grammar to Add**:
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
```

**Testing**:
```bash
./highper-gateway validate -c configs/scenarios/scenario-12-microservices-discovery.proxy
```

---

### PHASE 2: PHP-FPM Integration (13-19 hours)

**Goal**: Complete PHP-FPM and static file serving

#### 2.1 CGI Response Parser (2-3 hours)

**Location**: New file `highper-gateway/src/webserver/cgi_parser.rs`

**Implementation**:
```rust
pub struct CgiResponse {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub fn parse_cgi_response(data: Vec<u8>) -> Result<CgiResponse> {
    // 1. Find blank line separator
    let separator_pos = find_blank_line(&data)?;

    // 2. Parse headers
    let header_section = &data[..separator_pos];
    let (status, headers) = parse_cgi_headers(header_section)?;

    // 3. Extract body
    let body = Bytes::copy_from_slice(&data[separator_pos..]);

    Ok(CgiResponse {
        status,
        headers,
        body,
    })
}

fn parse_cgi_headers(data: &[u8]) -> Result<(u16, HeaderMap)> {
    let mut headers = HeaderMap::new();
    let mut status = 200;

    for line in data.split(|&b| b == b'\n') {
        let line = std::str::from_utf8(line)?.trim();
        if line.is_empty() { continue; }

        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();

            if name.eq_ignore_ascii_case("status") {
                // Parse status code
                status = value.split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(200);
            } else {
                headers.insert(
                    HeaderName::from_bytes(name.as_bytes())?,
                    HeaderValue::from_str(value)?
                );
            }
        }
    }

    Ok((status, headers))
}
```

**Tests**:
```rust
#[test]
fn test_parse_cgi_headers() {
    let data = b"Status: 200 OK\r\nContent-Type: text/html\r\n\r\n<html>body</html>";
    let response = parse_cgi_response(data.to_vec()).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers.get("content-type").unwrap(), "text/html");
}
```

---

#### 2.2 FastCGI Params Builder (3-4 hours)

**Location**: New file `highper-gateway/src/webserver/fastcgi_params.rs`

**Implementation**:
```rust
use http::Request;
use std::path::Path;

pub fn build_fastcgi_params(
    req: &Request<Body>,
    document_root: &str,
    script_filename: &Path,
) -> Vec<(String, String)> {
    let mut params = Vec::new();

    // Mandatory CGI/1.1 variables
    params.push(("GATEWAY_INTERFACE".into(), "CGI/1.1".into()));
    params.push(("SERVER_SOFTWARE".into(), "highper-gateway/1.0".into()));
    params.push(("SERVER_PROTOCOL".into(), format!("{:?}", req.version())));

    // Request variables
    params.push(("REQUEST_METHOD".into(), req.method().to_string()));
    params.push(("REQUEST_URI".into(), req.uri().to_string()));

    // Script variables
    params.push(("SCRIPT_FILENAME".into(), script_filename.display().to_string()));
    params.push(("SCRIPT_NAME".into(), extract_script_name(req.uri().path())));
    params.push(("DOCUMENT_ROOT".into(), document_root.to_string()));

    // Query string
    if let Some(query) = req.uri().query() {
        params.push(("QUERY_STRING".into(), query.to_string()));
    }

    // Content headers
    if let Some(content_type) = req.headers().get("content-type") {
        params.push(("CONTENT_TYPE".into(), content_type.to_str().unwrap_or("").to_string()));
    }
    if let Some(content_length) = req.headers().get("content-length") {
        params.push(("CONTENT_LENGTH".into(), content_length.to_str().unwrap_or("0").to_string()));
    }

    // Server variables
    if let Some(host) = req.headers().get("host") {
        let host_str = host.to_str().unwrap_or("localhost");
        let (name, port) = parse_host_port(host_str);
        params.push(("SERVER_NAME".into(), name));
        params.push(("SERVER_PORT".into(), port));
    }

    // HTTPS detection
    if req.uri().scheme() == Some(&http::uri::Scheme::HTTPS) {
        params.push(("HTTPS".into(), "on".into()));
    }

    // Client variables
    // TODO: Extract from socket peer address
    params.push(("REMOTE_ADDR".into(), "127.0.0.1".into()));
    params.push(("REMOTE_PORT".into(), "0".into()));

    // HTTP headers (with HTTP_ prefix)
    for (name, value) in req.headers().iter() {
        let header_name = format!("HTTP_{}", name.as_str().to_uppercase().replace('-', '_'));
        let header_value = value.to_str().unwrap_or("").to_string();
        params.push((header_name, header_value));
    }

    params
}

fn extract_script_name(path: &str) -> String {
    // Remove query string and return path
    path.split('?').next().unwrap_or("/").to_string()
}

fn parse_host_port(host: &str) -> (String, String) {
    if let Some(idx) = host.rfind(':') {
        (host[..idx].to_string(), host[idx+1..].to_string())
    } else {
        (host.to_string(), "80".to_string())
    }
}
```

**Tests**:
```rust
#[test]
fn test_build_fastcgi_params() {
    let req = Request::builder()
        .method("GET")
        .uri("http://example.com/index.php?foo=bar")
        .header("Host", "example.com")
        .body(Body::empty())
        .unwrap();

    let params = build_fastcgi_params(&req, "/var/www/html", Path::new("/var/www/html/index.php"));

    assert_params_contains(&params, "REQUEST_METHOD", "GET");
    assert_params_contains(&params, "SCRIPT_FILENAME", "/var/www/html/index.php");
    assert_params_contains(&params, "QUERY_STRING", "foo=bar");
}
```

---

#### 2.3 Handler Integration (8-12 hours)

**Location**: `highper-gateway/src/proxy/handler.rs`

**High-level Flow**:
```rust
impl Handler {
    pub async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>> {
        // 1. Check if this is a webserver request
        if let Some(webserver_config) = &self.config.webserver {
            if self.is_webserver_request(&req) {
                return self.handle_webserver_request(req, webserver_config).await;
            }
        }

        // 2. Otherwise, proxy to backend (existing logic)
        self.proxy_to_backend(req).await
    }

    async fn handle_webserver_request(
        &self,
        req: Request<Body>,
        config: &WebServerConfig,
    ) -> Result<Response<Body>> {
        // 1. Resolve file path
        let file_info = self.static_handler.resolve_path(req.uri().path())?;

        // 2. Check if it's a PHP file
        if file_info.is_php && config.enable_php_fpm {
            return self.handle_php_request(req, file_info, config).await;
        }

        // 3. Serve static file
        self.serve_static_file(req, file_info).await
    }

    async fn handle_php_request(
        &self,
        req: Request<Body>,
        file_info: FileInfo,
        config: &WebServerConfig,
    ) -> Result<Response<Body>> {
        // 1. Build FastCGI params
        let params = fastcgi_params::build_fastcgi_params(
            &req,
            &config.document_root,
            &file_info.path,
        );

        // 2. Get request body
        let body_bytes = hyper::body::to_bytes(req.into_body()).await?;

        // 3. Get PHP-FPM connection from pool
        let mut conn = self.php_fpm_pool
            .as_ref()
            .ok_or_else(|| anyhow!("PHP-FPM pool not configured"))?
            .get_connection()?;

        // 4. Execute FastCGI request
        let response_data = conn.execute(&params, &body_bytes).await?;

        // 5. Parse CGI response
        let cgi_response = cgi_parser::parse_cgi_response(response_data)?;

        // 6. Build HTTP response
        let mut response = Response::builder()
            .status(cgi_response.status);

        // Copy headers
        for (name, value) in cgi_response.headers.iter() {
            response = response.header(name, value);
        }

        Ok(response.body(Body::from(cgi_response.body))?)
    }

    async fn serve_static_file(
        &self,
        req: Request<Body>,
        file_info: FileInfo,
    ) -> Result<Response<Body>> {
        // 1. Check If-Modified-Since
        if let Some(ims) = req.headers().get("if-modified-since") {
            if self.is_not_modified(ims, &file_info.metadata)? {
                return Ok(Response::builder()
                    .status(304)
                    .body(Body::empty())?);
            }
        }

        // 2. Open file
        let file = File::open(&file_info.path)?;

        // 3. Get MIME type
        let mime_type = self.static_handler.get_mime_type(&file_info.path);

        // 4. Generate ETag
        let etag = self.static_handler.generate_etag(&file_info.metadata);

        // 5. Build response
        let mut response = Response::builder()
            .status(200)
            .header("Content-Type", mime_type)
            .header("Content-Length", file_info.metadata.len())
            .header("ETag", etag)
            .header("Last-Modified", format_http_date(file_info.metadata.modified()?));

        // 6. TODO: Check if sendfile is possible (requires raw socket)
        // For now, use regular file reading
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        Ok(response.body(Body::from(buffer))?)
    }
}
```

**Tasks**:
1. Add `static_handler` and `php_fpm_pool` fields to Handler struct (1h)
2. Implement `is_webserver_request()` routing logic (1h)
3. Implement `handle_webserver_request()` dispatcher (1h)
4. Implement `handle_php_request()` with CGI parsing (2-3h)
5. Implement `serve_static_file()` with conditionals (2-3h)
6. Add error handling and logging (1-2h)
7. Integration testing (2-3h)

---

### PHASE 3: Testing & Validation (5-8 hours)

**Goal**: Ensure all directives parse correctly and all scenarios work

#### 3.1 DSL Parse Tests (2-3 hours)

**Test each new directive**:
```rust
#[test]
fn test_cache_directive_parsing() {
    let dsl = r#"
        cache enabled ttl=300s max_size=10000
    "#;
    let result = parse_dsl(dsl);
    assert!(result.is_ok());
    // Assert cache config is correct
}

#[test]
fn test_waf_directive_parsing() {
    let dsl = r#"
        waf enabled mode=modsecurity block
    "#;
    let result = parse_dsl(dsl);
    assert!(result.is_ok());
}

// ... repeat for all directives
```

---

#### 3.2 Scenario Integration Tests (2-3 hours)

**Test all 15 scenarios**:
```bash
#!/bin/bash
# test-all-scenarios.sh

for i in {01..15}; do
    config_file="configs/scenarios/scenario-${i}*.proxy"
    echo "Testing scenario $i..."

    # Parse validation
    ./highper-gateway validate -c $config_file
    if [ $? -ne 0 ]; then
        echo "FAIL: Scenario $i parse error"
        exit 1
    fi

    echo "PASS: Scenario $i"
done

echo "All scenarios validated successfully!"
```

---

#### 3.3 Runtime Tests (1-2 hours)

**Test each scenario actually runs**:
```bash
#!/bin/bash
# runtime-test-scenario.sh <scenario_num>

scenario=$1
config="configs/scenarios/scenario-${scenario}*.proxy"

# Start gateway
./highper-gateway start -c $config &
PID=$!
sleep 3

# Test endpoint
curl http://localhost:8080/ || {
    echo "FAIL: Scenario $scenario runtime error"
    kill $PID
    exit 1
}

kill $PID
echo "PASS: Scenario $scenario runtime"
```

---

### PHASE 4: Documentation & Scenarios Update (2-3 hours)

**Goal**: Update all scenario configs with new DSL directives

#### 4.1 Update Scenario Configs (1-2 hours)

**Update each scenario to use new directives**:

1. **Scenario 09**: Add WAF + mTLS directives
2. **Scenario 11**: Add cache directives
3. **Scenario 12**: Add service_discovery directives
4. **Scenario 13**: Add graphql directives
5. **Scenario 14**: Add php_fpm + static_files directives
6. **Scenario 15**: Add geoip directives

---

#### 4.2 Documentation (1 hour)

**Create/update**:
- `DSL_REFERENCE.md` - Complete DSL syntax reference
- Update each scenario README with new directives
- Add examples to main README

---

## Timeline Summary

| Phase | Tasks | Hours | Dependencies |
|-------|-------|-------|--------------|
| **Phase 1.1** | Cache directives | 3-4h | None |
| **Phase 1.2** | WAF directives | 4-6h | None |
| **Phase 1.3** | GraphQL directives | 3-5h | None |
| **Phase 1.4** | PHP-FPM directives | 4-6h | None |
| **Phase 1.5** | Geographic directives | 3-4h | None |
| **Phase 1.6** | Service Discovery | 4-5h | None |
| **Phase 2.1** | CGI parser | 2-3h | None |
| **Phase 2.2** | Params builder | 3-4h | None |
| **Phase 2.3** | Handler integration | 8-12h | Phase 2.1, 2.2 |
| **Phase 3** | Testing & validation | 5-8h | Phase 1, 2 |
| **Phase 4** | Docs & scenarios | 2-3h | Phase 1, 2, 3 |
| **TOTAL** | | **30-43h** | **~5-6 days** |

---

## Parallel Execution Strategy

**Maximize efficiency by working in parallel where possible**:

### Week 1 (Days 1-3): DSL Directives
- Day 1: Cache + WAF directives (7-10h)
- Day 2: GraphQL + PHP-FPM directives (7-11h)
- Day 3: Geographic + Service Discovery (7-9h)

### Week 2 (Days 4-6): PHP-FPM Integration
- Day 4: CGI parser + Params builder (5-7h)
- Day 5-6: Handler integration + testing (13-20h)

**Total**: 5-6 days at 8 hours/day

---

## Risk Mitigation

### High Risk Items

1. **Handler Integration** (8-12h, touches critical code)
   - Mitigation: Create backup branch before starting
   - Test thoroughly with existing scenarios first
   - Use feature flag to toggle webserver mode

2. **DSL Grammar Breaking Changes**
   - Mitigation: Maintain backward compatibility
   - Test all existing configs after each directive addition

3. **PHP-FPM Compatibility**
   - Mitigation: Test with multiple PHP versions (7.4, 8.0, 8.1, 8.2)
   - Document supported PHP-FPM configurations

### Medium Risk Items

4. **Performance Regression**
   - Mitigation: Benchmark before/after handler changes
   - Ensure webserver check is O(1) operation

5. **Edge Cases in CGI Parsing**
   - Mitigation: Extensive unit tests
   - Test with real PHP applications (WordPress, Laravel)

---

## Success Criteria

### Functional Requirements
- ✅ All 6 new directive types parse correctly
- ✅ All 15 scenarios can be configured via DSL
- ✅ PHP-FPM requests execute successfully
- ✅ Static files serve correctly
- ✅ No regression in existing functionality

### Quality Requirements
- ✅ 100% test coverage for new directives
- ✅ All scenario configs validate
- ✅ All runtime tests pass
- ✅ Documentation complete

### Performance Requirements
- ✅ Parse time < 10ms for typical config
- ✅ Static file: 50K+ req/s (with sendfile)
- ✅ PHP-FPM: 5K+ req/s
- ✅ No performance regression for proxy workloads

---

## Validation Checklist

### Before Starting
- [ ] Backup current state: `git checkout -b backup/before-option3`
- [ ] Tag: `git tag v1.0-before-option3`
- [ ] Run full test suite: `cargo test --lib`
- [ ] Benchmark baseline: `./scripts/benchmark.sh`

### After Phase 1 (DSL)
- [ ] All new directives parse: `cargo test dsl::parse`
- [ ] All scenarios validate: `./scripts/test-all-scenarios.sh`
- [ ] No parse regressions: Test all existing configs
- [ ] Commit: `git commit -m "feat: Add DSL directives for WAF, Cache, GraphQL, PHP-FPM, Geo, SD"`

### After Phase 2 (PHP-FPM)
- [ ] CGI parser tests pass: `cargo test cgi_parser`
- [ ] Params builder tests pass: `cargo test fastcgi_params`
- [ ] Integration tests pass: `cargo test webserver::integration`
- [ ] Manual test: Serve static HTML file
- [ ] Manual test: Execute simple PHP script
- [ ] Commit: `git commit -m "feat: Complete PHP-FPM integration and static file serving"`

### After Phase 3 (Testing)
- [ ] All 687 existing tests still pass
- [ ] All 15 scenarios runtime tested
- [ ] Load test Scenario 14 (PHP-FPM)
- [ ] No memory leaks: `valgrind` or `heaptrack`

### After Phase 4 (Docs)
- [ ] DSL reference complete
- [ ] All scenario READMEs updated
- [ ] Main README updated
- [ ] Commit: `git commit -m "docs: Complete DSL reference and scenario documentation"`

### Final Validation
- [ ] Full test suite: `cargo test --lib` (687+ tests passing)
- [ ] Benchmark: No performance regression
- [ ] All 15 scenarios: Parse + Runtime
- [ ] Tag: `git tag v1.0-option3-complete`

---

## Next Steps After Completion

Once Option 3 is complete:

1. **Transition to Option 1** (Load Testing & Performance)
   - Use new DSL configs for all 15 load test scenarios
   - Easier to write test configurations
   - Clearer test case documentation

2. **Community Feedback**
   - Publish DSL documentation
   - Gather feedback on syntax
   - Iterate on usability

3. **Production Deployment**
   - Deploy with PHP-FPM for real workloads
   - Monitor performance and stability
   - Fix bugs as they arise

---

## Quick Start Guide

### To Begin Implementation

```bash
# 1. Create feature branch
git checkout -b feature/option3-dsl-php-fpm

# 2. Start with Cache directives (easiest)
cd highper-gateway
code src/config/dsl.pest  # Add cache grammar rules
code src/config/dsl_parser.rs  # Add cache parsing
code src/config/dsl_ast.rs  # Add cache AST
code src/config/dsl_converter.rs  # Add cache conversion

# 3. Test
cargo test dsl::cache

# 4. Move to next directive
# Repeat for WAF, GraphQL, PHP-FPM, Geographic, Service Discovery

# 5. Start PHP-FPM integration
code src/webserver/cgi_parser.rs  # Create CGI parser
code src/webserver/fastcgi_params.rs  # Create params builder
code src/proxy/handler.rs  # Integrate into handler

# 6. Test
cargo test webserver

# 7. Update scenarios
code configs/scenarios/scenario-11-cdn-caching.proxy
# ... etc

# 8. Validate
./scripts/test-all-scenarios.sh

# 9. Commit
git commit -m "feat: Complete Option 3 - DSL + PHP-FPM"
```

---

## Conclusion

Option 3 is **strategically important** for highper-gateway adoption:

**Benefits**:
- ✅ Easy adoption (simple DSL syntax)
- ✅ Broader market (PHP hosting)
- ✅ Better positioning (Nginx replacement)
- ✅ Cleaner load testing (for Option 1)

**Effort**: 30-43 hours (manageable in 1-2 weeks)

**Next Action**: Start with Phase 1.1 (Cache directives) - the easiest 3-4 hour task to build momentum

---

**Status**: ✅ Plan Complete
**Document**: OPTION3_IMPLEMENTATION_PLAN.md
**Date**: December 20, 2025
**Ready to Start**: Phase 1.1 - Cache Directives
