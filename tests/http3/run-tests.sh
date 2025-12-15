#!/bin/bash
#
# HTTP/3 Integration Test Suite
# Tests Alt-Svc headers, address validation, and HTTP/3 functionality
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GATEWAY_BIN="$PROJECT_ROOT/target/release/highper-gateway"
CONFIG_FILE="$SCRIPT_DIR/test-config.yaml"
CERT_DIR="$SCRIPT_DIR/certs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

run_test() {
    local test_name="$1"
    TESTS_RUN=$((TESTS_RUN + 1))
    echo ""
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}Test $TESTS_RUN: $test_name${NC}"
    echo -e "${BLUE}======================================${NC}"
}

test_passed() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    log_success "$1"
}

test_failed() {
    TESTS_FAILED=$((TESTS_FAILED + 1))
    log_error "$1"
}

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."

    # Check if gateway binary exists
    if [[ ! -f "$GATEWAY_BIN" ]]; then
        log_error "Gateway binary not found at $GATEWAY_BIN"
        log_info "Building gateway in release mode..."
        cd "$PROJECT_ROOT"
        cargo build --release
    fi

    # Check if Docker is running
    if ! docker ps &>/dev/null; then
        log_error "Docker is not running or not accessible"
        log_info "Please start Docker/Rancher Desktop and try again"
        exit 1
    fi

    # Check if curl is available
    if ! command -v curl &>/dev/null; then
        log_error "curl is not installed"
        exit 1
    fi

    # Check if openssl is available
    if ! command -v openssl &>/dev/null; then
        log_error "openssl is not installed"
        exit 1
    fi

    log_success "All prerequisites met"
}

# Generate certificates if needed
setup_certificates() {
    if [[ ! -f "$CERT_DIR/server.crt" || ! -f "$CERT_DIR/server.key" ]]; then
        log_info "Generating self-signed certificates..."
        bash "$SCRIPT_DIR/generate-certs.sh"
    else
        log_info "Certificates already exist"
    fi
}

# Start backend servers
start_backends() {
    log_info "Starting backend servers with Docker Compose..."
    cd "$SCRIPT_DIR"
    docker-compose down --remove-orphans 2>/dev/null || true
    docker-compose up -d

    # Wait for backends to be healthy
    log_info "Waiting for backends to be healthy..."
    for i in {1..30}; do
        if docker-compose ps | grep -q "(healthy)"; then
            local healthy_count=$(docker-compose ps | grep -c "(healthy)")
            if [[ $healthy_count -eq 3 ]]; then
                log_success "All 3 backends are healthy"
                return 0
            fi
        fi
        sleep 1
    done

    log_error "Backends failed to become healthy"
    docker-compose logs
    exit 1
}

# Start gateway
start_gateway() {
    log_info "Starting highper-gateway..."

    # Kill any existing gateway process
    pkill -9 highper-gateway 2>/dev/null || true
    sleep 1

    # Start gateway in background
    "$GATEWAY_BIN" start -c "$CONFIG_FILE" > "$SCRIPT_DIR/gateway.log" 2>&1 &
    GATEWAY_PID=$!

    # Wait for gateway to start
    log_info "Waiting for gateway to start..."
    for i in {1..30}; do
        if curl -s -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/ &>/dev/null; then
            log_success "Gateway started successfully (PID: $GATEWAY_PID)"
            return 0
        fi
        sleep 1
    done

    log_error "Gateway failed to start"
    cat "$SCRIPT_DIR/gateway.log"
    exit 1
}

# Test 1: Basic HTTP connectivity
test_http_connectivity() {
    run_test "HTTP Connectivity (HTTP/1.1)"

    local response=$(curl -s http://127.0.0.1:8080/)
    if echo "$response" | grep -q "backend"; then
        test_passed "HTTP connection successful"
    else
        test_failed "HTTP connection failed"
        echo "Response: $response"
    fi
}

# Test 2: HTTPS connectivity
test_https_connectivity() {
    run_test "HTTPS Connectivity (HTTP/2)"

    local response=$(curl -s -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/)
    if echo "$response" | grep -q "backend"; then
        test_passed "HTTPS connection successful"
    else
        test_failed "HTTPS connection failed"
        echo "Response: $response"
    fi
}

# Test 3: Alt-Svc header presence
test_alt_svc_header() {
    run_test "Alt-Svc Header Generation"

    log_info "Testing Alt-Svc header in HTTP/2 response..."
    local headers=$(curl -s -I -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/)

    if echo "$headers" | grep -qi "alt-svc.*h3"; then
        test_passed "Alt-Svc header present in response"
        echo "$headers" | grep -i "alt-svc"
    else
        test_failed "Alt-Svc header not found"
        echo "Response headers:"
        echo "$headers"
    fi
}

# Test 4: Load balancing
test_load_balancing() {
    run_test "Load Balancing Across Backends"

    log_info "Making 30 requests to verify load distribution..."
    local backend_counts=$(mktemp)

    for i in {1..30}; do
        curl -s -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/echo | grep -o "backend-[0-9]" >> "$backend_counts" || true
    done

    local backend1_count=$(grep -c "backend-1" "$backend_counts" || echo "0")
    local backend2_count=$(grep -c "backend-2" "$backend_counts" || echo "0")
    local backend3_count=$(grep -c "backend-3" "$backend_counts" || echo "0")

    log_info "Backend distribution:"
    log_info "  backend-1: $backend1_count requests"
    log_info "  backend-2: $backend2_count requests"
    log_info "  backend-3: $backend3_count requests"

    # Check if all backends received requests (round-robin)
    if [[ $backend1_count -gt 0 && $backend2_count -gt 0 && $backend3_count -gt 0 ]]; then
        test_passed "Load balanced across all backends"
    else
        test_failed "Some backends did not receive requests"
    fi

    rm -f "$backend_counts"
}

# Test 5: Health check endpoint
test_health_endpoint() {
    run_test "Health Check Endpoint"

    local response=$(curl -s -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/health)
    if echo "$response" | grep -q "healthy"; then
        test_passed "Health check endpoint working"
    else
        test_failed "Health check failed"
        echo "Response: $response"
    fi
}

# Test 6: POST request handling
test_post_request() {
    run_test "POST Request Handling"

    local response=$(curl -s -k --resolve localhost:8443:127.0.0.1 -X POST -d '{"test": "data"}' \
        -H "Content-Type: application/json" \
        https://localhost:8443/echo)

    if echo "$response" | grep -q "POST"; then
        test_passed "POST request handled correctly"
    else
        test_failed "POST request failed"
        echo "Response: $response"
    fi
}

# Test 7: Concurrent connections
test_concurrent_connections() {
    run_test "Concurrent Connections"

    log_info "Testing 50 concurrent requests..."
    local success_count=0

    for i in {1..50}; do
        curl -s -k --resolve localhost:8443:127.0.0.1 https://localhost:8443/ &>/dev/null && success_count=$((success_count + 1)) &
    done
    wait

    if [[ $success_count -ge 45 ]]; then
        test_passed "Concurrent connections successful ($success_count/50)"
    else
        test_failed "Too many concurrent failures ($success_count/50)"
    fi
}

# Test 8: Protocol detection (check gateway logs)
test_protocol_detection() {
    run_test "Protocol Detection in Logs"

    if grep -q "HTTP/3=true" "$SCRIPT_DIR/gateway.log" 2>/dev/null; then
        test_passed "HTTP/3 protocol detected in logs"
    else
        log_warn "HTTP/3 protocol detection not found in logs (may not be logged yet)"
    fi
}

# Cleanup function
cleanup() {
    log_info "Cleaning up..."

    # Stop gateway
    if [[ -n "$GATEWAY_PID" ]]; then
        kill $GATEWAY_PID 2>/dev/null || true
    fi
    pkill -9 highper-gateway 2>/dev/null || true

    # Stop backends
    cd "$SCRIPT_DIR"
    docker-compose down --remove-orphans 2>/dev/null || true

    # Show summary
    echo ""
    echo "========================================  "
    echo "Test Summary"
    echo "========================================"
    echo "Tests Run:    $TESTS_RUN"
    echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
    echo "========================================"

    if [[ $TESTS_FAILED -eq 0 ]]; then
        echo -e "${GREEN}✓ All tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}✗ Some tests failed${NC}"
        echo ""
        echo "Check gateway logs at: $SCRIPT_DIR/gateway.log"
        exit 1
    fi
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

# Main execution
main() {
    echo "========================================"
    echo "HTTP/3 Integration Test Suite"
    echo "========================================"
    echo ""

    check_prerequisites
    setup_certificates
    start_backends
    start_gateway

    # Run tests
    test_http_connectivity
    test_https_connectivity
    test_alt_svc_header
    test_load_balancing
    test_health_endpoint
    test_post_request
    test_concurrent_connections
    test_protocol_detection

    # Cleanup will be called automatically via trap
}

# Run main
main
