#!/bin/bash
# Generate comprehensive load test report from metadata and results
# Usage: ./generate-report.sh <results-directory>

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1" >&2; }

# Arguments
RESULTS_DIR="${1:-}"

if [ -z "$RESULTS_DIR" ] || [ ! -d "$RESULTS_DIR" ]; then
    log_error "Usage: $0 <results-directory>"
    log_error "Example: $0 results/load-test-20250127-120000"
    exit 1
fi

METADATA_FILE="$RESULTS_DIR/metadata.json"
if [ ! -f "$METADATA_FILE" ]; then
    log_error "Metadata file not found: $METADATA_FILE"
    exit 1
fi

log_info "============================================"
log_info "Generating Load Test Report"
log_info "============================================"
log_info "Results directory: $RESULTS_DIR"
log_info "============================================"

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_FILE="$SCRIPT_DIR/report-template.md"
OUTPUT_FILE="$RESULTS_DIR/report.md"

# Extract values from metadata.json using jq
extract_json() {
    local path="$1"
    jq -r "$path // \"N/A\"" "$METADATA_FILE"
}

extract_json_number() {
    local path="$1"
    jq -r "$path // 0" "$METADATA_FILE" | awk '{printf "%.2f", $1}'
}

log_info "Extracting metadata..."

# Test identification
TEST_ID=$(extract_json '.test_id')
TEST_DATE=$(extract_json '.timestamp.start')
TEST_DURATION=$(extract_json '.timestamp.duration_seconds')
TEST_DURATION_MINUTES=$(echo "$TEST_DURATION / 60" | bc -l | awk '{printf "%.1f", $1}')

# Scenario
SCENARIO_NUMBER=$(extract_json '.scenario.number')
SCENARIO_NAME=$(extract_json '.scenario.name')
SCENARIO_DESCRIPTION=$(extract_json '.scenario.description')
TARGET_RPS=$(extract_json_number '.scenario.target_rps')
TARGET_CONNECTIONS=$(extract_json_number '.scenario.target_connections')
TARGET_P99=$(extract_json_number '.scenario.target_p99_latency_ms')

# Infrastructure
PROVIDER=$(extract_json '.infrastructure.provider')
REGION=$(extract_json '.infrastructure.region')

# Proxy
PROXY_PLAN=$(extract_json '.infrastructure.proxy.plan')
PROXY_CPU=$(extract_json '.infrastructure.proxy.cpu_cores')
PROXY_MEMORY=$(extract_json '.infrastructure.proxy.memory_gb')
PROXY_IP=$(extract_json '.infrastructure.proxy.ip_address')
PROXY_COST=$(extract_json_number '.infrastructure.proxy.cost_per_hour')

# Backends
BACKEND_COUNT=$(extract_json '.infrastructure.backends.count')
BACKEND_PLAN=$(extract_json '.infrastructure.backends.plan')
BACKEND_CPU=$(extract_json '.infrastructure.backends.cpu_cores')
BACKEND_MEMORY=$(extract_json '.infrastructure.backends.memory_gb')
BACKEND_COST=$(extract_json_number '.infrastructure.backends.total_cost_per_hour')
BACKEND_IPS=$(jq -r '.infrastructure.backends.instances[].ip_address' "$METADATA_FILE" | tr '\n' ', ' | sed 's/,$//')

# Generators
GENERATOR_COUNT=$(extract_json '.infrastructure.generators.count')
GENERATOR_PLAN=$(extract_json '.infrastructure.generators.plan')
GENERATOR_CPU=$(extract_json '.infrastructure.generators.cpu_cores')
GENERATOR_MEMORY=$(extract_json '.infrastructure.generators.memory_gb')
GENERATOR_COST=$(extract_json_number '.infrastructure.generators.total_cost_per_hour')
GENERATOR_IPS=$(jq -r '.infrastructure.generators.instances[].ip_address' "$METADATA_FILE" | tr '\n' ', ' | sed 's/,$//')
LOAD_TEST_TOOL=$(extract_json '.infrastructure.generators.instances[0].tool')

# Total infrastructure
TOTAL_INSTANCES=$(extract_json '.infrastructure.total.instances')
TOTAL_CORES=$(extract_json '.infrastructure.total.cpu_cores')
TOTAL_MEMORY=$(extract_json '.infrastructure.total.memory_gb')
TOTAL_COST_PER_HOUR=$(extract_json_number '.infrastructure.total.cost_per_hour')
TEST_COST=$(extract_json_number '.infrastructure.total.test_cost')

# Software
GATEWAY_VERSION=$(extract_json '.software.gateway.version')
GIT_COMMIT=$(extract_json '.software.gateway.git_commit')
BACKEND_VERSION=$(extract_json '.software.backend.version')
BACKEND_FRAMEWORK=$(extract_json '.software.backend.framework')
LOAD_TEST_TOOL_VERSION=$(extract_json '.software.load_test_tool.version')
PROMETHEUS_VERSION=$(extract_json '.software.observability.prometheus_version')
GRAFANA_VERSION=$(extract_json '.software.observability.grafana_version')

# Configuration
MAX_CONNECTIONS=$(extract_json '.configuration.gateway.max_connections')
CONNECTION_TIMEOUT=$(extract_json '.configuration.gateway.connection_timeout')
KEEPALIVE_TIMEOUT=$(extract_json '.configuration.gateway.keepalive_timeout')
LB_ALGORITHM=$(extract_json '.configuration.gateway.load_balancer_algorithm')
RATE_LIMIT_RPS=$(extract_json '.configuration.gateway.rate_limit_rps')
RATE_LIMIT_BURST=$(extract_json '.configuration.gateway.rate_limit_burst')
BUFFER_POOL_ENABLED=$(extract_json '.configuration.gateway.buffer_pool_enabled')
BUFFER_SIZE=$(extract_json '.configuration.gateway.buffer_pool_size')
POOL_SIZE=$(extract_json '.configuration.gateway.pool_size')
POOL_MIN_IDLE=$(extract_json '.configuration.gateway.connection_pool.min_idle')
POOL_MAX_IDLE=$(extract_json '.configuration.gateway.connection_pool.max_idle')
POOL_MAX_OPEN=$(extract_json '.configuration.gateway.connection_pool.max_open')

# Results
TOTAL_REQUESTS=$(extract_json_number '.results.summary.total_requests')
SUCCESSFUL_REQUESTS=$(extract_json_number '.results.summary.successful_requests')
FAILED_REQUESTS=$(extract_json_number '.results.summary.failed_requests')
ACTUAL_RPS=$(extract_json_number '.results.summary.actual_rps')
SUCCESS_RATE=$(extract_json_number '.results.summary.success_rate_percent')
DATA_TRANSFERRED=$(extract_json_number '.results.summary.total_data_transferred_gb')

# Latency
MIN_LATENCY=$(extract_json_number '.results.latency.min_ms')
MAX_LATENCY=$(extract_json_number '.results.latency.max_ms')
MEAN_LATENCY=$(extract_json_number '.results.latency.mean_ms')
P50_LATENCY=$(extract_json_number '.results.latency.p50_ms')
P95_LATENCY=$(extract_json_number '.results.latency.p95_ms')
P99_LATENCY=$(extract_json_number '.results.latency.p99_ms')
P999_LATENCY=$(extract_json_number '.results.latency.p999_ms')
STDDEV_LATENCY=$(extract_json_number '.results.latency.stddev_ms')

# Resource utilization
PROXY_CPU_AVG=$(extract_json_number '.results.resource_utilization.proxy_cpu_avg')
PROXY_CPU_PEAK=$(extract_json_number '.results.resource_utilization.proxy_cpu_peak')
PROXY_MEM_AVG=$(extract_json_number '.results.resource_utilization.proxy_memory_avg')
PROXY_MEM_PEAK=$(extract_json_number '.results.resource_utilization.proxy_memory_peak')
BACKEND_CPU_AVG=$(extract_json_number '.results.resource_utilization.backend_cpu_avg')
BACKEND_MEM_AVG=$(extract_json_number '.results.resource_utilization.backend_memory_avg')

# Status determination
RPS_ACHIEVEMENT=$(echo "scale=2; ($ACTUAL_RPS / $TARGET_RPS) * 100" | bc -l | awk '{printf "%.1f", $1}')
if (( $(echo "$ACTUAL_RPS >= $TARGET_RPS" | bc -l) )); then
    RPS_STATUS="✅ Achieved"
else
    RPS_STATUS="⚠️  Below target"
fi

if (( $(echo "$P99_LATENCY <= $TARGET_P99" | bc -l) )); then
    P99_STATUS="✅ Within target"
else
    P99_STATUS="⚠️  Above target"
fi

if (( $(echo "$SUCCESS_RATE >= 99.99" | bc -l) )); then
    SUCCESS_STATUS="✅ Achieved"
else
    SUCCESS_STATUS="⚠️  Below target"
fi

# Generate report from template
log_info "Generating report..."

cat > "$OUTPUT_FILE" <<EOF
# Load Test Report: $SCENARIO_NAME

**Test ID**: \`$TEST_ID\`
**Date**: $TEST_DATE
**Duration**: $TEST_DURATION seconds
**Provider**: $PROVIDER ($REGION)

---

## Executive Summary

$SCENARIO_DESCRIPTION

| Metric | Target | Achieved | Status |
|--------|--------|----------|--------|
| **Requests/sec** | ${TARGET_RPS} | ${ACTUAL_RPS} | $RPS_STATUS |
| **P99 Latency** | < ${TARGET_P99}ms | ${P99_LATENCY}ms | $P99_STATUS |
| **Success Rate** | > 99.99% | ${SUCCESS_RATE}% | $SUCCESS_STATUS |

**Overall Result**: $([ "$RPS_STATUS" = "✅ Achieved" ] && [ "$P99_STATUS" = "✅ Within target" ] && echo "✅ SUCCESS" || echo "⚠️  PARTIAL SUCCESS")

---

## Test Configuration

### Infrastructure

#### Proxy Server (Gateway)
- **Plan**: $PROXY_PLAN - $PROXY_CPU cores, ${PROXY_MEMORY}GB RAM
- **IP**: $PROXY_IP
- **Cost**: \$$PROXY_COST/hour

#### Backend Servers (×$BACKEND_COUNT)
- **Plan**: $BACKEND_PLAN - $BACKEND_CPU cores, ${BACKEND_MEMORY}GB RAM
- **IPs**: $BACKEND_IPS
- **Total Cost**: \$$BACKEND_COST/hour

#### Load Generators (×$GENERATOR_COUNT)
- **Plan**: $GENERATOR_PLAN - $GENERATOR_CPU cores, ${GENERATOR_MEMORY}GB RAM
- **IPs**: $GENERATOR_IPS
- **Tool**: $LOAD_TEST_TOOL
- **Total Cost**: \$$GENERATOR_COST/hour

**Total Infrastructure**: $TOTAL_INSTANCES instances, $TOTAL_CORES cores, ${TOTAL_MEMORY}GB RAM
**Test Cost**: \$$TEST_COST ($TEST_DURATION_MINUTES minutes × \$$TOTAL_COST_PER_HOUR/hour)

---

## Results

### Throughput

| Metric | Value |
|--------|-------|
| **Total Requests** | $TOTAL_REQUESTS |
| **Successful Requests** | $SUCCESSFUL_REQUESTS |
| **Failed Requests** | $FAILED_REQUESTS |
| **Actual RPS** | $ACTUAL_RPS |
| **Success Rate** | ${SUCCESS_RATE}% |
| **Data Transferred** | ${DATA_TRANSFERRED}GB |

**Achievement**: ${RPS_ACHIEVEMENT}% of target ($TARGET_RPS RPS)

### Latency Distribution

| Percentile | Latency | Status vs Target |
|------------|---------|------------------|
| **Min** | ${MIN_LATENCY}ms | - |
| **P50 (Median)** | ${P50_LATENCY}ms | $([ $(echo "$P50_LATENCY <= 2.0" | bc -l) -eq 1 ] && echo "✅" || echo "⚠️" ) |
| **P95** | ${P95_LATENCY}ms | $([ $(echo "$P95_LATENCY <= 8.0" | bc -l) -eq 1 ] && echo "✅" || echo "⚠️" ) |
| **P99** | ${P99_LATENCY}ms | $P99_STATUS |
| **P99.9** | ${P999_LATENCY}ms | $([ $(echo "$P999_LATENCY <= 25.0" | bc -l) -eq 1 ] && echo "✅" || echo "⚠️" ) |
| **Max** | ${MAX_LATENCY}ms | - |
| **Mean** | ${MEAN_LATENCY}ms | - |
| **Std Dev** | ${STDDEV_LATENCY}ms | - |

### Resource Utilization

#### Proxy Server (Gateway)

| Resource | Average | Peak | Status |
|----------|---------|------|--------|
| **CPU** | ${PROXY_CPU_AVG}% | ${PROXY_CPU_PEAK}% | $([ $(echo "$PROXY_CPU_AVG < 70.0" | bc -l) -eq 1 ] && echo "✅ Good headroom" || echo "⚠️ High usage") |
| **Memory** | ${PROXY_MEM_AVG}% | ${PROXY_MEM_PEAK}% | $([ $(echo "$PROXY_MEM_AVG < 70.0" | bc -l) -eq 1 ] && echo "✅ Good headroom" || echo "⚠️ High usage") |

**Sizing Recommendation**: $([ $(echo "$PROXY_CPU_AVG < 50.0" | bc -l) -eq 1 ] && echo "Can downsize to save costs" || echo "Current sizing is appropriate")

---

## Artifacts

- **Metadata**: [metadata.json](metadata.json)
- **Vegeta Results**: [vegeta/](vegeta/) directory
- **Prometheus Snapshot**: [metrics/prometheus-snapshot.tar.gz](metrics/prometheus-snapshot.tar.gz)
- **Grafana Dashboards**: [metrics/grafana/](metrics/grafana/) directory
- **Logs**: [logs/](logs/) directory

---

**Generated**: $(date -u '+%Y-%m-%d %H:%M:%S UTC')
**Framework**: Highper Gateway Load Testing Framework
EOF

log_success "Report generated: $OUTPUT_FILE"

# Generate summary.txt
log_info "Generating summary..."
cat > "$RESULTS_DIR/summary.txt" <<EOF
═══════════════════════════════════════════════════════════════════════════════
HIGHPER GATEWAY LOAD TEST SUMMARY
═══════════════════════════════════════════════════════════════════════════════

Test ID: $TEST_ID
Scenario: $SCENARIO_NAME
Date: $TEST_DATE
Duration: $TEST_DURATION seconds

───────────────────────────────────────────────────────────────────────────────
PERFORMANCE RESULTS
───────────────────────────────────────────────────────────────────────────────

Throughput:
  Target RPS:      $TARGET_RPS
  Actual RPS:      $ACTUAL_RPS
  Achievement:     ${RPS_ACHIEVEMENT}%
  Success Rate:    ${SUCCESS_RATE}%

Latency:
  P50:  ${P50_LATENCY}ms
  P95:  ${P95_LATENCY}ms
  P99:  ${P99_LATENCY}ms (target: <${TARGET_P99}ms)
  P99.9: ${P999_LATENCY}ms

───────────────────────────────────────────────────────────────────────────────
INFRASTRUCTURE
───────────────────────────────────────────────────────────────────────────────

Total: $TOTAL_INSTANCES instances, $TOTAL_CORES cores, ${TOTAL_MEMORY}GB RAM
Cost: \$$TEST_COST for $TEST_DURATION_MINUTES minutes

Proxy:      $PROXY_PLAN - $PROXY_CPU cores, ${PROXY_MEMORY}GB RAM
Backends:   ${BACKEND_COUNT}× $BACKEND_PLAN - $BACKEND_CPU cores, ${BACKEND_MEMORY}GB RAM
Generators: ${GENERATOR_COUNT}× $GENERATOR_PLAN - $GENERATOR_CPU cores, ${GENERATOR_MEMORY}GB RAM

───────────────────────────────────────────────────────────────────────────────
RESOURCE UTILIZATION
───────────────────────────────────────────────────────────────────────────────

Proxy CPU:    Avg ${PROXY_CPU_AVG}%, Peak ${PROXY_CPU_PEAK}%
Proxy Memory: Avg ${PROXY_MEM_AVG}%, Peak ${PROXY_MEM_PEAK}%

───────────────────────────────────────────────────────────────────────────────
OVERALL STATUS: $([ "$RPS_STATUS" = "✅ Achieved" ] && [ "$P99_STATUS" = "✅ Within target" ] && echo "SUCCESS" || echo "PARTIAL SUCCESS")
───────────────────────────────────────────────────────────────────────────────

Full Report: report.md
EOF

log_success "Summary generated: $RESULTS_DIR/summary.txt"

log_success "============================================"
log_success "Report Generation Complete"
log_success "============================================"
log_info "View report: cat $OUTPUT_FILE"
log_info "View summary: cat $RESULTS_DIR/summary.txt"

exit 0
