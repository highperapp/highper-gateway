#!/bin/bash
# WebSocket Integration Test Runner
# Automates setup, execution, and teardown of WebSocket tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
GATEWAY_BIN="$PROJECT_ROOT/target/release/highper-gateway"
CONFIG_FILE="$SCRIPT_DIR/test_config.toml"

# Flags
USE_DOCKER=1
BUILD_GATEWAY=1
RUN_TESTS=1
CLEANUP=1

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --no-docker)
            USE_DOCKER=0
            shift
            ;;
        --no-build)
            BUILD_GATEWAY=0
            shift
            ;;
        --no-cleanup)
            CLEANUP=0
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --no-docker     Don't use Docker, run backends manually"
            echo "  --no-build      Skip building the gateway"
            echo "  --no-cleanup    Don't cleanup after tests"
            echo "  --help          Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Function to print colored messages
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Cleanup function
cleanup() {
    if [ $CLEANUP -eq 0 ]; then
        print_warn "Skipping cleanup (--no-cleanup specified)"
        return
    fi

    print_info "Cleaning up..."

    # Stop gateway
    if [ -n "$GATEWAY_PID" ]; then
        print_info "Stopping gateway (PID: $GATEWAY_PID)"
        kill $GATEWAY_PID 2>/dev/null || true
        wait $GATEWAY_PID 2>/dev/null || true
    fi

    # Stop Docker backends
    if [ $USE_DOCKER -eq 1 ]; then
        print_info "Stopping Docker backends"
        cd "$SCRIPT_DIR"
        docker-compose down
    fi

    print_info "Cleanup complete"
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Main execution
main() {
    print_info "WebSocket Integration Test Runner"
    print_info "=================================="

    # Check prerequisites
    print_info "Checking prerequisites..."

    if ! command_exists python3; then
        print_error "python3 is not installed"
        exit 1
    fi

    if [ $USE_DOCKER -eq 1 ] && ! command_exists docker-compose; then
        print_error "docker-compose is not installed (use --no-docker to skip)"
        exit 1
    fi

    # Build gateway if requested
    if [ $BUILD_GATEWAY -eq 1 ]; then
        print_info "Building highper-gateway..."
        cd "$PROJECT_ROOT"
        cargo build --release

        if [ ! -f "$GATEWAY_BIN" ]; then
            print_error "Gateway binary not found: $GATEWAY_BIN"
            exit 1
        fi
    else
        print_warn "Skipping gateway build (--no-build specified)"
        if [ ! -f "$GATEWAY_BIN" ]; then
            print_error "Gateway binary not found: $GATEWAY_BIN"
            print_error "Build it first or remove --no-build flag"
            exit 1
        fi
    fi

    # Install Python dependencies
    print_info "Installing Python dependencies..."
    cd "$SCRIPT_DIR"
    python3 -m pip install -q -r requirements.txt

    # Start backends
    if [ $USE_DOCKER -eq 1 ]; then
        print_info "Starting Docker backends..."
        docker-compose up -d

        # Wait for backends to be ready
        print_info "Waiting for backends to be ready..."
        sleep 5

        # Verify backends are running
        for port in 8081 8082 8083; do
            if ! docker-compose ps | grep -q "ws-backend-$(($port - 8080))"; then
                print_error "Backend on port $port failed to start"
                docker-compose logs
                exit 1
            fi
        done

        print_info "All backends ready"
    else
        print_warn "Not using Docker. Make sure backends are running on ports 8081-8083"
        print_warn "Run: python3 mock_backend.py --port PORT"

        # Check if backends are listening
        for port in 8081 8082 8083; do
            if ! nc -z 127.0.0.1 $port 2>/dev/null; then
                print_error "Backend on port $port is not running"
                exit 1
            fi
        done
    fi

    # Start gateway
    print_info "Starting highper-gateway..."
    "$GATEWAY_BIN" --config "$CONFIG_FILE" > /tmp/gateway-test.log 2>&1 &
    GATEWAY_PID=$!

    print_info "Gateway started (PID: $GATEWAY_PID)"

    # Wait for gateway to be ready
    print_info "Waiting for gateway to be ready..."
    sleep 3

    # Verify gateway is running
    if ! ps -p $GATEWAY_PID > /dev/null; then
        print_error "Gateway failed to start. Check logs:"
        cat /tmp/gateway-test.log
        exit 1
    fi

    # Check if gateway is listening
    if ! nc -z 127.0.0.1 8080 2>/dev/null; then
        print_error "Gateway is not listening on port 8080"
        cat /tmp/gateway-test.log
        exit 1
    fi

    print_info "Gateway ready"

    # Run tests
    print_info "Running integration tests..."
    echo ""
    echo "========================================"
    echo "  WebSocket Integration Tests"
    echo "========================================"
    echo ""

    if python3 test_integration.py; then
        echo ""
        print_info "✅ All tests passed!"
        return 0
    else
        echo ""
        print_error "❌ Some tests failed!"
        return 1
    fi
}

# Run main function
main
exit $?
