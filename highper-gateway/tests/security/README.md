# Highper Gateway Security Testing Framework

Comprehensive vulnerability analysis and penetration testing framework for all 15 deployment scenarios.

## Overview

This framework provides:
- **199 attack payloads** across 9 vulnerability categories
- **124 E2E security tests** covering all deployment scenarios
- **CVSS 3.1 scoring** for vulnerability assessment
- **OWASP Top 10 2021** coverage
- **Automated test orchestration** via shell script

## Quick Start

```bash
# Run unit tests (fast, no gateway required)
cargo test --test security_pentest

# Run full E2E security suite (requires gateway)
cargo test --test security_pentest -- --ignored --test-threads=1

# Run with verbose output
cargo test --test security_pentest -- --ignored --test-threads=1 --nocapture

# Use automation script
./scripts/pentest-runner.sh all
```

## Test Structure

```
tests/security/
├── mod.rs                 # Main module
├── common.rs              # Test harnesses, utilities, CVSS calculator
├── payloads.rs            # Attack payloads in Rust
├── README.md              # This file
├── payloads/              # Text payloads for external tools
│   ├── sql_injection.txt
│   ├── xss.txt
│   ├── path_traversal.txt
│   ├── ssrf.txt
│   └── headers.txt
└── scenarios/             # Per-scenario tests
    ├── scenario_01_tcp.rs
    ├── scenario_02_http.rs
    ├── scenario_03_tls.rs
    ├── scenario_04_api_gateway.rs
    ├── scenario_05_http3.rs
    ├── scenario_06_websocket.rs
    ├── scenario_07_grpc.rs
    ├── scenario_08_database.rs
    ├── scenario_09_waf_mtls.rs
    ├── scenario_10_hybrid.rs
    ├── scenario_11_cdn_cache.rs
    ├── scenario_12_microservices.rs
    ├── scenario_13_graphql.rs
    ├── scenario_14_static_php.rs
    └── scenario_15_geo_routing.rs
```

## Scenario Coverage

| # | Scenario | Risk | Key Tests |
|---|----------|------|-----------|
| 01 | TCP Layer 4 LB | Medium | Connection exhaustion, slowloris, RST attacks |
| 02 | HTTP Layer 7 LB | Medium | Request smuggling, CRLF, header injection |
| 03 | TLS Termination | Low | Cipher suites, protocol versions, certificates |
| 04 | API Gateway | Medium | JWT attacks, IDOR, injection, SSRF |
| 05 | HTTP/3 QUIC | Low | Amplification, 0-RTT replay, stream flooding |
| 06 | WebSocket | Medium | CSWSH, message injection, session hijacking |
| 07 | gRPC | Medium | Metadata injection, stream exhaustion |
| 08 | Database LB | High | Pool poisoning, credential exposure |
| 09 | WAF + mTLS | Low | WAF bypass, certificate spoofing |
| 10 | Hybrid | Medium | Protocol confusion, cross-protocol attacks |
| 11 | CDN Caching | Medium | Cache poisoning, web cache deception |
| 12 | Microservices | Medium | Service hopping, SSRF, cascading failures |
| 13 | GraphQL | High | Introspection, depth attacks, batch attacks |
| 14 | Static + PHP | High | Path traversal, LFI/RFI, file upload |
| 15 | Geo Routing | Low | IP spoofing, routing manipulation |

## Attack Categories

| Category | Payloads | Description |
|----------|----------|-------------|
| SQL Injection | 40 | Classic, UNION, time-based, encoded |
| XSS | 37 | Reflected, DOM, SVG, template injection |
| Path Traversal | 26 | Basic, encoded, null byte, Windows |
| Command Injection | 34 | Basic, time-based, encoded, bash bypass |
| SSRF | 24 | Localhost bypass, cloud metadata, schemes |
| Request Smuggling | 5 | CL.TE, TE.CL, TE.TE |
| CRLF Injection | 8 | Header injection, response splitting |
| Header Attacks | 19 | IP spoofing, host poisoning, cache |
| GraphQL | 6 | Introspection, depth, batch |

## Running Specific Tests

```bash
# By scenario
cargo test --test security_pentest scenario_04 -- --ignored --nocapture

# By attack type
cargo test --test security_pentest sql_injection -- --ignored
cargo test --test security_pentest xss -- --ignored
cargo test --test security_pentest path_traversal -- --ignored
cargo test --test security_pentest ssrf -- --ignored

# By vulnerability ID
cargo test --test security_pentest test_http_01 -- --ignored  # Request smuggling
cargo test --test security_pentest test_waf_01 -- --ignored   # WAF bypass
```

## OWASP Top 10 Coverage

| OWASP 2021 | Test IDs | Scenarios |
|------------|----------|-----------|
| A01: Broken Access Control | API-02, GQL-05, MS-01, PHP-01 | 04, 12, 13, 14 |
| A02: Cryptographic Failures | TLS-01 to TLS-07, DB-02 | 03, 04-09 |
| A03: Injection | HTTP-02, WAF-01-03, PHP-02-04 | All HTTP |
| A04: Insecure Design | DoS tests, CACHE-01-05 | All |
| A05: Security Misconfiguration | Config audits | All |
| A06: Vulnerable Components | Dependency audit | All |
| A07: Authentication Failures | API-01, MTLS-01-03 | 04, 09, 12, 13 |
| A08: Data Integrity Failures | CACHE-01, JWT attacks | 04, 11-13 |
| A09: Logging Failures | Audit validation | All |
| A10: SSRF | MS-04, GQL-04, PHP-03 | 04, 12-14 |

## Using Payloads with External Tools

The `payloads/` directory contains text files for use with external tools:

```bash
# With ffuf
ffuf -w tests/security/payloads/sql_injection.txt -u "http://target/api?id=FUZZ"

# With Burp Intruder
# Import payloads from tests/security/payloads/*.txt

# With curl
while read payload; do
  curl -s "http://target/api?q=$payload" | grep -i error
done < tests/security/payloads/sql_injection.txt
```

## Automation Script

The `scripts/pentest-runner.sh` script provides automated test orchestration:

```bash
./scripts/pentest-runner.sh help     # Show usage
./scripts/pentest-runner.sh all      # Full test suite
./scripts/pentest-runner.sh quick    # CI/CD mode (fast)
./scripts/pentest-runner.sh 1        # Phase 1: Automated scanning
./scripts/pentest-runner.sh 2        # Phase 2: Protocol testing
./scripts/pentest-runner.sh 2 04     # Phase 2 for scenario 04 only
./scripts/pentest-runner.sh 3        # Phase 3: Advanced testing
./scripts/pentest-runner.sh 4        # Phase 4: Report generation
```

## Test Harness

The framework provides a `SecurityTestHarness` for E2E testing:

```rust
use crate::security::common::*;

#[tokio::test]
#[ignore]
async fn test_custom_vulnerability() {
    let config = TestConfigBuilder::http_proxy()
        .with_waf()
        .with_rate_limit(100, 60)
        .build();

    let harness = SecurityTestHarness::new(&config).await.unwrap();
    let client = SecurityHttpClient::new(&harness.url(""));

    // Test your vulnerability
    let (status, body, _) = client
        .get_with_headers("/api?id=' OR '1'='1", HashMap::new())
        .await
        .unwrap();

    assert_eq!(status, StatusCode::FORBIDDEN, "SQLi should be blocked");
}
```

## CVSS Scoring

Calculate CVSS 3.1 scores for findings:

```rust
use crate::security::common::calculate_cvss_score;

// Remote Code Execution: Network/Low/None/None/Changed/High/High/High
let score = calculate_cvss_score("N", "L", "N", "N", "C", "H", "H", "H");
assert!(score >= 9.0); // Critical

// SQL Injection: Network/Low/Low/None/Unchanged/High/High/None
let score = calculate_cvss_score("N", "L", "L", "N", "U", "H", "H", "N");
assert!(score >= 7.0 && score < 9.0); // High
```

## Reporting

Test results template is available at `docs/SECURITY_TESTING_RESULTS.md`.

Reports are generated in `security-reports/TIMESTAMP/` when using the automation script.

## Contributing

When adding new security tests:

1. Add payloads to `payloads.rs` or `payloads/*.txt`
2. Create test functions in the appropriate scenario file
3. Mark E2E tests with `#[ignore]` attribute
4. Include CVSS scoring for findings
5. Update this README with new coverage

## References

- [OWASP Top 10 2021](https://owasp.org/Top10/)
- [CVSS 3.1 Calculator](https://www.first.org/cvss/calculator/3.1)
- [CWE Database](https://cwe.mitre.org/)
- [OWASP Testing Guide](https://owasp.org/www-project-web-security-testing-guide/)
