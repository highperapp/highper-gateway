#!/bin/bash
# Local Testing Script
# Starts mock backends and tests load balancing locally

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
BACKEND_PORTS=(8081 8082 8083)
CONFIG_FILE="${1:-examples/simple-load-balancer.proxy}"
GATEWAY_BINARY="./target/release/highper-gateway"

# Cleanup function
cleanup() {
    echo ""
    echo -e "${YELLOW}Cleaning up...${NC}"

    # Kill backend servers
    for port in "${BACKEND_PORTS[@]}"; do
        pkill -f "http.server $port" 2>/dev/null || true
    done

    # Kill gateway
    pkill -f "highper-gateway" 2>/dev/null || true

    echo -e "${GREEN}Cleanup complete${NC}"
}

# Set up trap
trap cleanup EXIT INT TERM

echo "=================================================="
echo "  Local Load Balancer Testing"
echo "=================================================="
echo ""

# Validate prerequisites
if [ ! -f "$GATEWAY_BINARY" ]; then
    echo -e "${RED}Error: Gateway binary not found${NC}"
    echo "Run: cargo build --release"
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}Error: Config file not found: $CONFIG_FILE${NC}"
    exit 1
fi

# Check if Python is available
if ! command -v python3 &> /dev/null; then
    echo -e "${RED}Error: python3 not found${NC}"
    exit 1
fi

echo -e "${BLUE}Configuration:${NC}"
echo "  Config file: $CONFIG_FILE"
echo "  Gateway:     $GATEWAY_BINARY"
echo "  Backends:    ${BACKEND_PORTS[*]}"
echo ""

# Step 1: Start backend servers
echo -e "${YELLOW}[1/4] Starting backend servers...${NC}"

for i in "${!BACKEND_PORTS[@]}"; do
    port="${BACKEND_PORTS[$i]}"
    backend_num=$((i + 1))

    python3 -c "
from http.server import HTTPServer, BaseHTTPRequestHandler
import socket

hostname = socket.gethostname()

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/health':
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'healthy')
        else:
            self.send_response(200)
            self.send_header('Content-Type', 'text/plain')
            self.end_headers()
            self.wfile.write(f'Response from Backend ${backend_num} (port ${port})\\n'.encode())

    def log_message(self, format, *args):
        pass  # Suppress logs

print(f'Backend ${backend_num} starting on port ${port}...', flush=True)
HTTPServer(('127.0.0.1', ${port}), Handler).serve_forever()
" &

    sleep 0.5
done

echo -e "${GREEN}✓ All backends started${NC}"
echo ""

# Step 2: Validate configuration
echo -e "${YELLOW}[2/4] Validating configuration...${NC}"
./scripts/validate-config.sh -c "$CONFIG_FILE" -b "$GATEWAY_BINARY" | grep "✓" || true
echo ""

# Step 3: Start gateway
echo -e "${YELLOW}[3/4] Starting gateway...${NC}"
$GATEWAY_BINARY start -c "$CONFIG_FILE" > /tmp/gateway-test.log 2>&1 &
GATEWAY_PID=$!

sleep 2

# Check if gateway is running
if ! ps -p $GATEWAY_PID > /dev/null; then
    echo -e "${RED}✗ Gateway failed to start${NC}"
    echo "Logs:"
    cat /tmp/gateway-test.log
    exit 1
fi

echo -e "${GREEN}✓ Gateway started (PID: $GATEWAY_PID)${NC}"
echo ""

# Step 4: Test load balancing
echo -e "${YELLOW}[4/4] Testing load balancing...${NC}"
echo ""

# Wait a bit for health checks
sleep 2

# Test basic connectivity
echo "Testing basic connectivity (10 requests):"
echo ""

for i in {1..10}; do
    response=$(curl -s http://localhost:8080/ 2>&1)
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓${NC} Request $i: $response"
    else
        echo -e "${RED}✗${NC} Request $i: Failed - $response"
    fi
    sleep 0.2
done

echo ""

# Test distribution
echo "Checking load distribution:"
echo ""

declare -A backend_counts
for port in "${BACKEND_PORTS[@]}"; do
    backend_counts[$port]=0
done

for i in {1..30}; do
    response=$(curl -s http://localhost:8080/ 2>&1)

    # Count which backend responded
    for port in "${BACKEND_PORTS[@]}"; do
        if echo "$response" | grep -q "port $port"; then
            backend_counts[$port]=$((backend_counts[$port] + 1))
        fi
    done
done

echo "Distribution (30 requests):"
for port in "${BACKEND_PORTS[@]}"; do
    count=${backend_counts[$port]}
    echo "  Port $port: $count requests ($(echo "scale=1; $count * 100 / 30" | bc)%)"
done

echo ""

# Test health endpoint
echo "Testing health checks:"
for port in "${BACKEND_PORTS[@]}"; do
    health=$(curl -s http://127.0.0.1:$port/health 2>&1)
    if [ "$health" = "healthy" ]; then
        echo -e "${GREEN}✓${NC} Backend on port $port: healthy"
    else
        echo -e "${RED}✗${NC} Backend on port $port: unhealthy"
    fi
done

echo ""
echo -e "${GREEN}=================================================="
echo "  Testing Complete!"
echo "==================================================${NC}"
echo ""
echo "Gateway is running. Press Ctrl+C to stop."
echo ""
echo "Manual testing commands:"
echo "  curl http://localhost:8080/"
echo "  curl http://localhost:8080/health"
echo ""
echo "Load testing:"
echo "  echo 'GET http://localhost:8080/' | vegeta attack -duration=10s -rate=100 | vegeta report"
echo ""

# Keep script running
wait $GATEWAY_PID
