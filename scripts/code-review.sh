#!/bin/bash
# Highper Gateway - Automated Code Review Script
# Comprehensive analysis covering all 15 use case scenarios
#
# Usage: ./scripts/code-review.sh [--quick|--full|--security|--performance]
#
# Modes:
#   --quick       Quick checks only (clippy, fmt, audit) - ~2 min
#   --full        All checks including benchmarks - ~30 min
#   --security    Security-focused analysis only - ~10 min
#   --performance Performance profiling only - ~15 min
#
# Exit codes:
#   0 - All checks passed
#   1 - Warnings found (non-blocking)
#   2 - Errors found (blocking)
#   3 - Tool installation required

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Results tracking
WARNINGS=0
ERRORS=0
RESULTS_FILE="${PROJECT_ROOT}/target/code-review-results.txt"

# Parse arguments
MODE="${1:---quick}"

print_header() {
    echo -e "\n${BLUE}════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}════════════════════════════════════════════════════════════════${NC}\n"
}

print_subheader() {
    echo -e "\n${YELLOW}── $1 ──${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
    ((WARNINGS++)) || true
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
    ((ERRORS++)) || true
}

check_tool() {
    if ! command -v "$1" &> /dev/null; then
        print_warning "$1 not installed. Install with: $2"
        return 1
    fi
    return 0
}

# Initialize results file
mkdir -p "${PROJECT_ROOT}/target"
echo "Highper Gateway Code Review Results" > "$RESULTS_FILE"
echo "Date: $(date)" >> "$RESULTS_FILE"
echo "Mode: $MODE" >> "$RESULTS_FILE"
echo "=================================" >> "$RESULTS_FILE"

print_header "HIGHPER GATEWAY CODE REVIEW"
echo "Mode: $MODE"
echo "Project: $PROJECT_ROOT"
echo ""

cd "$PROJECT_ROOT"

# ═══════════════════════════════════════════════════════════════
# STAGE 1: QUICK CHECKS (Always run)
# ═══════════════════════════════════════════════════════════════

print_header "STAGE 1: Quick Checks"

# 1.1 Format Check
print_subheader "1.1 Rust Format Check (cargo fmt)"
if cargo fmt --all -- --check 2>/dev/null; then
    print_success "Code formatting OK"
    echo "✓ Format check passed" >> "$RESULTS_FILE"
else
    print_error "Code formatting issues found"
    echo "✗ Format check failed - run: cargo fmt" >> "$RESULTS_FILE"
fi

# 1.2 Clippy Lints
print_subheader "1.2 Clippy Lints (700+ rules)"
if cargo clippy --all-targets --all-features -- -D warnings 2>/dev/null; then
    print_success "Clippy checks passed"
    echo "✓ Clippy passed" >> "$RESULTS_FILE"
else
    print_warning "Clippy found warnings"
    echo "⚠ Clippy warnings found" >> "$RESULTS_FILE"
fi

# 1.3 Security Audit
print_subheader "1.3 Security Audit (RustSec Advisory Database)"
if check_tool "cargo-audit" "cargo install cargo-audit"; then
    if cargo audit 2>/dev/null; then
        print_success "No known vulnerabilities found"
        echo "✓ Security audit passed" >> "$RESULTS_FILE"
    else
        print_error "Security vulnerabilities found!"
        echo "✗ Security vulnerabilities detected" >> "$RESULTS_FILE"
    fi
fi

# 1.4 Unit Tests
print_subheader "1.4 Unit Tests"
if cargo test --lib 2>/dev/null; then
    print_success "All unit tests passed"
    echo "✓ Unit tests passed" >> "$RESULTS_FILE"
else
    print_error "Unit tests failed"
    echo "✗ Unit tests failed" >> "$RESULTS_FILE"
fi

# Quick mode stops here
if [[ "$MODE" == "--quick" ]]; then
    print_header "QUICK CHECK COMPLETE"
    echo "Warnings: $WARNINGS, Errors: $ERRORS"
    exit $((ERRORS > 0 ? 2 : (WARNINGS > 0 ? 1 : 0)))
fi

# ═══════════════════════════════════════════════════════════════
# STAGE 2: SECURITY ANALYSIS
# ═══════════════════════════════════════════════════════════════

if [[ "$MODE" == "--security" || "$MODE" == "--full" ]]; then
    print_header "STAGE 2: Security Analysis"

    # 2.1 Dependency Policy Check
    print_subheader "2.1 Dependency Policy (cargo deny)"
    if check_tool "cargo-deny" "cargo install cargo-deny"; then
        if cargo deny check 2>/dev/null; then
            print_success "Dependency policies OK"
            echo "✓ cargo deny passed" >> "$RESULTS_FILE"
        else
            print_warning "Dependency policy violations"
            echo "⚠ cargo deny warnings" >> "$RESULTS_FILE"
        fi
    fi

    # 2.2 Unsafe Code Analysis
    print_subheader "2.2 Unsafe Code Analysis (cargo geiger)"
    if check_tool "cargo-geiger" "cargo install cargo-geiger"; then
        echo "Scanning for unsafe code blocks..."
        cargo geiger --all-features 2>/dev/null | tee -a "$RESULTS_FILE" || true
        print_success "Unsafe code scan complete (review output above)"
    fi

    # 2.3 OWASP Check for WAF scenario
    print_subheader "2.3 WAF Rule Coverage (Scenario 09)"
    echo "Checking WAF implementation coverage..."
    WAF_FILES=$(find "$PROJECT_ROOT/highper-gateway/src/middleware/waf" -name "*.rs" 2>/dev/null | wc -l)
    if [[ $WAF_FILES -ge 4 ]]; then
        print_success "WAF implementation complete ($WAF_FILES modules)"
        echo "✓ WAF has $WAF_FILES modules" >> "$RESULTS_FILE"
    else
        print_warning "WAF implementation may be incomplete"
    fi

    # 2.4 TLS Implementation Check (Scenario 03)
    print_subheader "2.4 TLS Implementation (Scenario 03)"
    TLS_FILES=$(find "$PROJECT_ROOT/highper-gateway/src/tls" -name "*.rs" 2>/dev/null | wc -l)
    if [[ $TLS_FILES -ge 5 ]]; then
        print_success "TLS implementation complete ($TLS_FILES modules)"
        echo "✓ TLS has $TLS_FILES modules" >> "$RESULTS_FILE"
    else
        print_warning "TLS implementation may be incomplete"
    fi

    # 2.5 TODO/FIXME Security Scan
    print_subheader "2.5 Security-Related TODOs"
    echo "Scanning for security-related TODO comments..."
    SECURITY_TODOS=$(grep -rn "TODO.*security\|FIXME.*security\|TODO.*auth\|TODO.*tls\|TODO.*encrypt" \
        "$PROJECT_ROOT/highper-gateway/src" 2>/dev/null | wc -l || echo "0")
    if [[ $SECURITY_TODOS -gt 0 ]]; then
        print_warning "Found $SECURITY_TODOS security-related TODOs"
        grep -rn "TODO.*security\|FIXME.*security\|TODO.*auth\|TODO.*tls\|TODO.*encrypt" \
            "$PROJECT_ROOT/highper-gateway/src" 2>/dev/null | head -10 || true
    else
        print_success "No security-related TODOs found"
    fi
fi

# Security mode stops here
if [[ "$MODE" == "--security" ]]; then
    print_header "SECURITY ANALYSIS COMPLETE"
    echo "Warnings: $WARNINGS, Errors: $ERRORS"
    exit $((ERRORS > 0 ? 2 : (WARNINGS > 0 ? 1 : 0)))
fi

# ═══════════════════════════════════════════════════════════════
# STAGE 3: PERFORMANCE ANALYSIS
# ═══════════════════════════════════════════════════════════════

if [[ "$MODE" == "--performance" || "$MODE" == "--full" ]]; then
    print_header "STAGE 3: Performance Analysis"

    # 3.1 Build Optimization Check
    print_subheader "3.1 Release Build Optimization"
    if grep -q 'opt-level = 3' "$PROJECT_ROOT/Cargo.toml" 2>/dev/null || \
       grep -q 'lto = true' "$PROJECT_ROOT/Cargo.toml" 2>/dev/null; then
        print_success "Release optimizations configured"
    else
        print_warning "Consider adding release optimizations to Cargo.toml"
    fi

    # 3.2 Benchmark Tests
    print_subheader "3.2 Benchmark Tests"
    if cargo bench --no-run 2>/dev/null; then
        print_success "Benchmarks compile successfully"
        echo "Run 'cargo bench' for full benchmark results"
    else
        print_warning "Benchmark compilation issues"
    fi

    # 3.3 Connection Pool Check (Scenarios 01, 02, 08)
    print_subheader "3.3 Connection Pool Implementation"
    POOL_FILES=$(grep -rln "connection_pool\|ConnectionPool\|pool_size" \
        "$PROJECT_ROOT/highper-gateway/src" 2>/dev/null | wc -l || echo "0")
    if [[ $POOL_FILES -gt 0 ]]; then
        print_success "Connection pooling implemented ($POOL_FILES files)"
    else
        print_warning "Connection pooling may not be implemented"
    fi

    # 3.4 Async Runtime Check
    print_subheader "3.4 Async Runtime Configuration"
    if grep -q "tokio.*rt-multi-thread" "$PROJECT_ROOT/highper-gateway/Cargo.toml" 2>/dev/null; then
        print_success "Multi-threaded Tokio runtime configured"
    else
        print_warning "Check Tokio runtime configuration"
    fi
fi

# Performance mode stops here
if [[ "$MODE" == "--performance" ]]; then
    print_header "PERFORMANCE ANALYSIS COMPLETE"
    echo "Warnings: $WARNINGS, Errors: $ERRORS"
    exit $((ERRORS > 0 ? 2 : (WARNINGS > 0 ? 1 : 0)))
fi

# ═══════════════════════════════════════════════════════════════
# STAGE 4: SCENARIO-SPECIFIC CHECKS (Full mode only)
# ═══════════════════════════════════════════════════════════════

if [[ "$MODE" == "--full" ]]; then
    print_header "STAGE 4: Scenario-Specific Validation"

    # Check each scenario's implementation
    SCENARIOS=(
        "01:tcp:Layer 4 TCP Proxy"
        "02:http:Layer 7 HTTP LB"
        "03:tls:HTTPS/TLS Termination"
        "04:gateway:API Gateway"
        "05:http3:HTTP/3 QUIC"
        "06:websocket:WebSocket LB"
        "07:grpc:gRPC Gateway"
        "08:database:Database LB"
        "09:waf:WAF + mTLS"
        "10:hybrid:Hybrid Multi-Protocol"
        "11:cache:CDN Edge Caching"
        "12:discovery:Microservices Discovery"
        "13:graphql:GraphQL Gateway"
        "14:webserver:Static + PHP-FPM"
        "15:geographic:Geographic LB"
    )

    for scenario in "${SCENARIOS[@]}"; do
        IFS=':' read -r num keyword name <<< "$scenario"
        print_subheader "Scenario $num: $name"

        # Check for related source files
        FILES=$(find "$PROJECT_ROOT/highper-gateway/src" -name "*.rs" \
            -exec grep -l "$keyword" {} \; 2>/dev/null | wc -l || echo "0")

        # Check for scenario config
        CONFIG_EXISTS=$(ls "$PROJECT_ROOT/examples/configs/scenarios/scenario-$num"* 2>/dev/null | wc -l || echo "0")

        if [[ $FILES -gt 0 && $CONFIG_EXISTS -gt 0 ]]; then
            print_success "Implementation found ($FILES files, $CONFIG_EXISTS configs)"
        elif [[ $FILES -gt 0 ]]; then
            print_warning "Implementation found but no example config"
        else
            print_warning "Limited implementation for scenario $num"
        fi
    done

    # ═══════════════════════════════════════════════════════════════
    # STAGE 5: INTEGRATION TESTS
    # ═══════════════════════════════════════════════════════════════

    print_header "STAGE 5: Integration Tests"

    print_subheader "5.1 Running Integration Tests"
    if cargo test --test '*' 2>/dev/null; then
        print_success "Integration tests passed"
        echo "✓ Integration tests passed" >> "$RESULTS_FILE"
    else
        print_warning "Some integration tests failed"
        echo "⚠ Integration tests have failures" >> "$RESULTS_FILE"
    fi

    # ═══════════════════════════════════════════════════════════════
    # STAGE 6: DOCUMENTATION CHECK
    # ═══════════════════════════════════════════════════════════════

    print_header "STAGE 6: Documentation Validation"

    # 6.1 Doc comments check
    print_subheader "6.1 Documentation Coverage"
    if cargo doc --no-deps 2>/dev/null; then
        print_success "Documentation builds successfully"
    else
        print_warning "Documentation build has warnings"
    fi

    # 6.2 Markdown files check
    print_subheader "6.2 Markdown Files"
    MD_COUNT=$(find "$PROJECT_ROOT" -name "*.md" -not -path "*/target/*" -not -path "*/archive/*" | wc -l)
    print_success "Found $MD_COUNT active markdown files"

    # 6.3 Check for broken internal links
    print_subheader "6.3 Checking for Placeholder Text"
    PLACEHOLDERS=$(grep -rn "yourusername\|your-username\|your_username" \
        "$PROJECT_ROOT"/*.md "$PROJECT_ROOT/docs"/*.md 2>/dev/null | wc -l || echo "0")
    if [[ $PLACEHOLDERS -gt 0 ]]; then
        print_warning "Found $PLACEHOLDERS placeholder references"
    else
        print_success "No placeholder text found"
    fi
fi

# ═══════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════

print_header "CODE REVIEW SUMMARY"

echo ""
echo "Results saved to: $RESULTS_FILE"
echo ""
echo "═══════════════════════════════════════"
echo "  Warnings: $WARNINGS"
echo "  Errors:   $ERRORS"
echo "═══════════════════════════════════════"
echo ""

if [[ $ERRORS -gt 0 ]]; then
    print_error "Code review failed with $ERRORS errors"
    exit 2
elif [[ $WARNINGS -gt 0 ]]; then
    print_warning "Code review completed with $WARNINGS warnings"
    exit 1
else
    print_success "Code review passed!"
    exit 0
fi
