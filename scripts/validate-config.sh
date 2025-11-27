#!/bin/bash
# Configuration Validation Script
# Validates gateway configuration files before deployment

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
CONFIG_FILE=""
GATEWAY_BINARY="./target/release/highper-gateway"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -c|--config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        -b|--binary)
            GATEWAY_BINARY="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 -c <config-file> [-b <gateway-binary>]"
            echo ""
            echo "Options:"
            echo "  -c, --config      Configuration file to validate (required)"
            echo "  -b, --binary      Path to highper-gateway binary (default: ./target/release/highper-gateway)"
            echo "  -h, --help        Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Validate arguments
if [ -z "$CONFIG_FILE" ]; then
    echo -e "${RED}Error: Configuration file is required${NC}"
    echo "Usage: $0 -c <config-file> [-b <gateway-binary>]"
    exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}Error: Configuration file not found: $CONFIG_FILE${NC}"
    exit 1
fi

if [ ! -f "$GATEWAY_BINARY" ]; then
    echo -e "${RED}Error: Gateway binary not found: $GATEWAY_BINARY${NC}"
    echo -e "${YELLOW}Hint: Run 'cargo build --release' first${NC}"
    exit 1
fi

if [ ! -x "$GATEWAY_BINARY" ]; then
    echo -e "${RED}Error: Gateway binary is not executable: $GATEWAY_BINARY${NC}"
    exit 1
fi

echo "=================================================="
echo "  Configuration Validation"
echo "=================================================="
echo ""
echo "Config file: $CONFIG_FILE"
echo "Binary:      $GATEWAY_BINARY"
echo ""

# Step 1: Check file syntax (basic checks)
echo -e "${YELLOW}[1/4] Checking file syntax...${NC}"
if grep -q "proxy" "$CONFIG_FILE"; then
    echo -e "${GREEN}✓ Found proxy directive${NC}"
else
    echo -e "${RED}✗ No proxy directive found${NC}"
    echo "  Configuration should contain at least one 'proxy' directive"
    exit 1
fi

# Check for common mistakes
if grep -q "0.0.0.0:" "$CONFIG_FILE"; then
    echo -e "${RED}✗ Warning: Found '0.0.0.0:' address${NC}"
    echo "  DSL routing doesn't work with raw IP addresses"
    echo "  Use domain-style addresses like 'api.gateway:8080' or 'localhost:8080'"
    exit 1
fi

# Check for suspicious patterns
if grep -E "^:[0-9]+ " "$CONFIG_FILE" | grep -v "mysql\|postgres\|redis\|tcp" > /dev/null 2>&1; then
    echo -e "${YELLOW}⚠ Warning: Found port-only address for HTTP${NC}"
    echo "  Port-only addresses (:8080) are for TCP protocols only"
    echo "  For HTTP, use 'localhost:8080' or 'api.gateway:8080'"
fi

echo ""

# Step 2: Parse configuration (if validate command exists)
echo -e "${YELLOW}[2/4] Validating configuration structure...${NC}"

# Check if validate command is available
if $GATEWAY_BINARY --help 2>&1 | grep -q "validate"; then
    if $GATEWAY_BINARY validate -c "$CONFIG_FILE" 2>&1; then
        echo -e "${GREEN}✓ Configuration structure is valid${NC}"
    else
        echo -e "${RED}✗ Configuration validation failed${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}⚠ 'validate' command not available, skipping structural validation${NC}"
fi

echo ""

# Step 3: Check for required components
echo -e "${YELLOW}[3/4] Checking configuration completeness...${NC}"

# Check for load balancing algorithm
if grep -q "lb " "$CONFIG_FILE"; then
    LB_ALG=$(grep "lb " "$CONFIG_FILE" | head -1 | awk '{print $2}')
    echo -e "${GREEN}✓ Load balancing algorithm: $LB_ALG${NC}"
else
    echo -e "${YELLOW}⚠ No explicit load balancing algorithm (will use default: round_robin)${NC}"
fi

# Check for health checks
if grep -q "health " "$CONFIG_FILE"; then
    echo -e "${GREEN}✓ Health checks configured${NC}"
else
    echo -e "${YELLOW}⚠ No health checks configured (recommended for production)${NC}"
fi

# Check for logging
if grep -q "log " "$CONFIG_FILE"; then
    LOG_LEVEL=$(grep "log " "$CONFIG_FILE" | head -1 | awk '{print $2}')
    echo -e "${GREEN}✓ Logging level: $LOG_LEVEL${NC}"
else
    echo -e "${YELLOW}⚠ No logging level set (will use default)${NC}"
fi

echo ""

# Step 4: Summary and recommendations
echo -e "${YELLOW}[4/4] Validation summary${NC}"
echo ""
echo -e "${GREEN}✓ Configuration file is valid and ready for use${NC}"
echo ""
echo "Next steps:"
echo "  1. Test locally with mock backends"
echo "  2. Verify load balancing behavior"
echo "  3. Only then deploy to cloud"
echo ""
echo "Local testing example:"
echo "  # Start backend servers"
echo "  python3 -m http.server 8081 &"
echo "  python3 -m http.server 8082 &"
echo "  python3 -m http.server 8083 &"
echo ""
echo "  # Start gateway"
echo "  $GATEWAY_BINARY start -c $CONFIG_FILE"
echo ""
echo "  # Test"
echo "  curl http://localhost:8080/"
echo ""
echo -e "${GREEN}=================================================="
echo "  Validation Successful!"
echo "==================================================${NC}"
