//! Integration tests for YAML/JSON → DSL migration tool

use anyhow::Result;
use highper_gateway::config::{dsl_converter, dsl_generator, dsl_parser, load_config};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test simple proxy configuration migration
#[test]
fn test_simple_proxy_migration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("simple.yaml");

    // Create a simple YAML config
    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    // Load and convert to DSL
    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Verify DSL contains expected elements
    assert!(dsl.contains("localhost:8080"));
    assert!(dsl.contains("proxy"));
    assert!(dsl.contains("backend:3000"));
    assert!(dsl.contains("log info"));

    // Verify DSL is much shorter
    let yaml_lines = yaml_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let dsl_lines = dsl.lines().filter(|l| !l.trim().is_empty()).count();
    assert!(
        dsl_lines < yaml_lines / 2,
        "DSL should be at least 2x shorter"
    );

    Ok(())
}

/// Test migration with admin API
#[test]
fn test_admin_api_migration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("admin.yaml");

    let yaml_content = r#"
server:
  bind: ["0.0.0.0:8080"]

admin:
  enabled: true
  bind: "127.0.0.1:9090"

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: debug
  metrics:
    enabled: true
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Verify admin directive (should extract port only)
    assert!(dsl.contains("admin :9090"));
    assert!(dsl.contains("log debug"));
    assert!(dsl.contains("metrics on"));

    Ok(())
}

/// Test DSL generation preserves key configuration elements
#[test]
fn test_round_trip_conversion() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("test.yaml");

    // Start with YAML
    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

admin:
  enabled: true
  bind: "127.0.0.1:9090"

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    // Load as config
    let config = load_config(&yaml_path)?;

    // Generate DSL
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Verify all key configuration elements are present in DSL
    assert!(dsl.contains("log info"), "Should have log directive");
    assert!(dsl.contains("admin :9090"), "Should have admin directive");
    assert!(dsl.contains("localhost:8080"), "Should have server address");
    assert!(dsl.contains("proxy"), "Should have proxy directive");
    assert!(dsl.contains("backend:3000"), "Should have backend address");

    // Verify DSL is significantly shorter
    let yaml_lines = yaml_content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let dsl_lines = dsl.lines().filter(|l| !l.trim().is_empty()).count();

    assert!(
        dsl_lines < yaml_lines / 2,
        "DSL should be at least 2x shorter than YAML"
    );

    // Verify no protocol prefixes in output
    assert!(!dsl.contains("http://"), "Should strip http:// prefix");
    assert!(
        !dsl.contains("https://") || dsl.starts_with("https://"),
        "https:// should only appear as site protocol"
    );

    Ok(())
}

/// Test multi-backend migration
#[test]
fn test_multi_backend_migration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("multi.yaml");

    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend1:3000"
      - url: "http://backend2:3000"
      - url: "http://backend3:3000"
    load_balancing:
      algorithm: round_robin

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Should have all backends in single line
    assert!(dsl.contains("backend1:3000"));
    assert!(dsl.contains("backend2:3000"));
    assert!(dsl.contains("backend3:3000"));

    // Count lines - should be very compact
    let dsl_lines: Vec<&str> = dsl.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(dsl_lines.len() <= 3, "Should be 3 lines or less");

    Ok(())
}

/// Test TLS configuration migration
#[test]
fn test_tls_migration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("tls.yaml");

    let yaml_content = r#"
server:
  bind: ["0.0.0.0:8080"]
  tls_bind: ["0.0.0.0:8443"]

tls:
  certificates:
    - domain: "example.com"
      cert_file: "cert.pem"
      key_file: "key.pem"

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Print DSL for debugging
    println!("Generated DSL:\n{}", dsl);

    // Should generate site with TLS address
    // Note: Generator uses tls_bind if available
    assert!(
        dsl.contains("0.0.0.0:8443") || dsl.contains("https://"),
        "DSL should contain TLS address or https protocol"
    );
    assert!(dsl.contains("backend:3000"));

    Ok(())
}

/// Test path normalization
#[test]
fn test_path_normalization() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("paths.yaml");

    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: root
    match:
      paths: ["/"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Path "/" should be normalized to "/*"
    assert!(dsl.contains("/*"));

    Ok(())
}

/// Test configuration size reduction
#[test]
fn test_size_reduction() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("size.yaml");

    // Create a moderately complex YAML config
    let yaml_content = r#"
server:
  bind: ["0.0.0.0:8080"]
  workers: "4"

admin:
  enabled: true
  bind: "127.0.0.1:9090"

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"
        weight: 1
    load_balancing:
      algorithm: round_robin
    health_check:
      active:
        enabled: true
        interval: 10s

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend
    timeout:
      connect: 5s
      request: 30s

observability:
  logging:
    level: info
    format: json
  metrics:
    enabled: true
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Calculate reduction
    let yaml_lines = yaml_content
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .count();
    let dsl_lines = dsl.lines().filter(|l| !l.trim().is_empty()).count();

    let reduction_ratio = yaml_lines as f64 / dsl_lines as f64;

    // Should be at least 3x reduction
    assert!(
        reduction_ratio >= 3.0,
        "Expected at least 3x reduction, got {:.1}x",
        reduction_ratio
    );

    Ok(())
}

/// Test that generated DSL can be parsed
#[test]
fn test_generated_dsl_parses() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("parse.yaml");

    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    // Generate DSL
    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Write DSL to file
    let dsl_path = temp_dir.path().join("generated.proxy");
    fs::write(&dsl_path, &dsl)?;

    // Try to parse the generated DSL
    let parse_result = dsl_parser::parse_dsl(&dsl);
    assert!(
        parse_result.is_ok(),
        "Generated DSL should parse successfully"
    );

    // Try to load as config
    let load_result = load_config(&dsl_path);
    assert!(load_result.is_ok(), "Generated DSL should load as config");

    Ok(())
}

/// Test empty/minimal configuration
#[test]
fn test_minimal_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let yaml_path = temp_dir.path().join("minimal.yaml");

    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content)?;

    let config = load_config(&yaml_path)?;
    let dsl = dsl_generator::generate_dsl(&config)?;

    // Should be very concise
    let lines: Vec<&str> = dsl.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() <= 3,
        "Minimal config should be 3 lines or less, got {}",
        lines.len()
    );

    Ok(())
}

/// Test extraction of backend addresses
#[test]
fn test_backend_address_extraction() {
    use highper_gateway::config::dsl_generator;

    // This is testing the internal logic, but we can't access private functions
    // So we test through the public API by checking the output

    let temp_dir = TempDir::new().unwrap();
    let yaml_path = temp_dir.path().join("extract.yaml");

    let yaml_content = r#"
server:
  bind: ["localhost:8080"]

upstreams:
  - name: backend
    servers:
      - url: "http://backend:3000"
      - url: "https://secure-backend:8443"

routes:
  - name: main
    match:
      paths: ["/*"]
    upstream: backend

observability:
  logging:
    level: info
"#;

    fs::write(&yaml_path, yaml_content).unwrap();

    let config = load_config(&yaml_path).unwrap();
    let dsl = dsl_generator::generate_dsl(&config).unwrap();

    // Should extract addresses without http:// or https://
    assert!(dsl.contains("backend:3000"));
    assert!(dsl.contains("secure-backend:8443"));
    assert!(!dsl.contains("http://"));
    assert!(!dsl.contains("https://"));
}
