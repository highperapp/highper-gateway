#!/bin/bash
# OWASP ZAP Security Scan Script for Highper Gateway
#
# This script runs automated security testing using OWASP ZAP
# Run with: ./scripts/security-scan.sh <target-url> [options]
#
# Requirements:
# - OWASP ZAP installed (https://www.zaproxy.org/download/)
# - ZAP CLI (optional): pip install zapcli
# - Or Docker: docker pull ghcr.io/zaproxy/zaproxy:stable

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
TARGET_URL="${1:-http://localhost:8080}"
REPORT_DIR="./security-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="${REPORT_DIR}/zap_report_${TIMESTAMP}"
ZAP_PORT=8090

# Parse options
SCAN_TYPE="baseline"  # baseline, full, api
USE_DOCKER=false
GENERATE_REPORT=true
CHECK_HEADERS=true

usage() {
    cat << EOF
Usage: $0 <target-url> [options]

Options:
    -h, --help              Show this help message
    -t, --type TYPE         Scan type: baseline, full, api (default: baseline)
    -d, --docker            Use Docker image instead of local ZAP
    -p, --port PORT         ZAP proxy port (default: 8090)
    --no-report             Skip report generation
    --no-headers            Skip security headers check

Examples:
    # Basic scan
    $0 http://localhost:8080

    # Full scan with Docker
    $0 http://localhost:8080 --type full --docker

    # API scan
    $0 http://localhost:8080/api --type api

    # Custom port
    $0 https://example.com --port 8888
EOF
    exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -t|--type)
            SCAN_TYPE="$2"
            shift 2
            ;;
        -d|--docker)
            USE_DOCKER=true
            shift
            ;;
        -p|--port)
            ZAP_PORT="$2"
            shift 2
            ;;
        --no-report)
            GENERATE_REPORT=false
            shift
            ;;
        --no-headers)
            CHECK_HEADERS=false
            shift
            ;;
        *)
            if [[ -z "$TARGET_URL" || "$TARGET_URL" == "-"* ]]; then
                TARGET_URL="$1"
            fi
            shift
            ;;
    esac
done

echo -e "${BLUE}╔════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       OWASP ZAP Security Scan - Highper Gateway    ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${YELLOW}Target URL:${NC}    $TARGET_URL"
echo -e "${YELLOW}Scan Type:${NC}     $SCAN_TYPE"
echo -e "${YELLOW}ZAP Port:${NC}      $ZAP_PORT"
echo -e "${YELLOW}Use Docker:${NC}    $USE_DOCKER"
echo -e "${YELLOW}Report Dir:${NC}    $REPORT_DIR"
echo ""

# Create report directory
mkdir -p "$REPORT_DIR"

# Function to check if ZAP is installed
check_zap_installed() {
    if [ "$USE_DOCKER" = true ]; then
        if ! command -v docker &> /dev/null; then
            echo -e "${RED}✗ Docker not found. Please install Docker.${NC}"
            exit 1
        fi
        echo -e "${GREEN}✓ Docker found${NC}"
    else
        if ! command -v zap.sh &> /dev/null && ! command -v zap-cli &> /dev/null; then
            echo -e "${RED}✗ OWASP ZAP not found. Please install ZAP or use --docker flag.${NC}"
            echo -e "${YELLOW}  Install: https://www.zaproxy.org/download/${NC}"
            exit 1
        fi
        echo -e "${GREEN}✓ OWASP ZAP found${NC}"
    fi
}

# Function to check if target is accessible
check_target() {
    echo -e "\n${BLUE}Checking target accessibility...${NC}"
    if curl -s --max-time 5 "$TARGET_URL" > /dev/null; then
        echo -e "${GREEN}✓ Target is accessible${NC}"
    else
        echo -e "${RED}✗ Target is not accessible. Is the proxy running?${NC}"
        exit 1
    fi
}

# Function to check security headers
check_security_headers() {
    if [ "$CHECK_HEADERS" = false ]; then
        return
    fi

    echo -e "\n${BLUE}Checking security headers...${NC}"

    local HEADERS_FILE="${REPORT_DIR}/headers_${TIMESTAMP}.txt"
    curl -sI "$TARGET_URL" > "$HEADERS_FILE"

    # Define expected headers
    declare -A EXPECTED_HEADERS=(
        ["X-Content-Type-Options"]="nosniff"
        ["X-Frame-Options"]="DENY|SAMEORIGIN"
        ["X-XSS-Protection"]="1"
        ["Strict-Transport-Security"]="max-age"
        ["Referrer-Policy"]="."
    )

    local PASS_COUNT=0
    local FAIL_COUNT=0

    echo ""
    for header in "${!EXPECTED_HEADERS[@]}"; do
        if grep -qi "^${header}:" "$HEADERS_FILE"; then
            local value=$(grep -i "^${header}:" "$HEADERS_FILE" | cut -d':' -f2- | tr -d ' \r\n')
            if echo "$value" | grep -qE "${EXPECTED_HEADERS[$header]}"; then
                echo -e "${GREEN}✓${NC} $header: $value"
                ((PASS_COUNT++))
            else
                echo -e "${YELLOW}⚠${NC} $header: $value (unexpected value)"
            fi
        else
            echo -e "${RED}✗${NC} $header: MISSING"
            ((FAIL_COUNT++))
        fi
    done

    echo ""
    if [ $FAIL_COUNT -eq 0 ]; then
        echo -e "${GREEN}✓ All security headers present${NC}"
    else
        echo -e "${YELLOW}⚠ $FAIL_COUNT security headers missing${NC}"
    fi

    echo -e "\nFull headers saved to: $HEADERS_FILE"
}

# Function to run baseline scan
run_baseline_scan() {
    echo -e "\n${BLUE}Running baseline security scan...${NC}"

    if [ "$USE_DOCKER" = true ]; then
        docker run -v "$(pwd)/${REPORT_DIR}:/zap/wrk:rw" \
            -t ghcr.io/zaproxy/zaproxy:stable \
            zap-baseline.py \
            -t "$TARGET_URL" \
            -r "report_${TIMESTAMP}.html" \
            -J "report_${TIMESTAMP}.json" \
            || true
    else
        zap-baseline.py \
            -t "$TARGET_URL" \
            -r "${REPORT_FILE}.html" \
            -J "${REPORT_FILE}.json" \
            || true
    fi
}

# Function to run full scan
run_full_scan() {
    echo -e "\n${BLUE}Running full security scan (this may take a while)...${NC}"

    if [ "$USE_DOCKER" = true ]; then
        docker run -v "$(pwd)/${REPORT_DIR}:/zap/wrk:rw" \
            -t ghcr.io/zaproxy/zaproxy:stable \
            zap-full-scan.py \
            -t "$TARGET_URL" \
            -r "report_${TIMESTAMP}.html" \
            -J "report_${TIMESTAMP}.json" \
            || true
    else
        zap-full-scan.py \
            -t "$TARGET_URL" \
            -r "${REPORT_FILE}.html" \
            -J "${REPORT_FILE}.json" \
            || true
    fi
}

# Function to run API scan
run_api_scan() {
    echo -e "\n${BLUE}Running API security scan...${NC}"

    if [ "$USE_DOCKER" = true ]; then
        docker run -v "$(pwd)/${REPORT_DIR}:/zap/wrk:rw" \
            -t ghcr.io/zaproxy/zaproxy:stable \
            zap-api-scan.py \
            -t "$TARGET_URL" \
            -r "report_${TIMESTAMP}.html" \
            -J "report_${TIMESTAMP}.json" \
            || true
    else
        zap-api-scan.py \
            -t "$TARGET_URL" \
            -r "${REPORT_FILE}.html" \
            -J "${REPORT_FILE}.json" \
            || true
    fi
}

# Function to generate summary report
generate_summary() {
    if [ "$GENERATE_REPORT" = false ]; then
        return
    fi

    echo -e "\n${BLUE}Generating summary report...${NC}"

    local SUMMARY_FILE="${REPORT_DIR}/summary_${TIMESTAMP}.txt"

    cat > "$SUMMARY_FILE" << EOF
OWASP ZAP Security Scan Summary
===============================

Date: $(date)
Target: $TARGET_URL
Scan Type: $SCAN_TYPE

Security Headers Check:
-----------------------
See: ${REPORT_DIR}/headers_${TIMESTAMP}.txt

ZAP Scan Results:
-----------------
HTML Report: ${REPORT_FILE}.html
JSON Report: ${REPORT_FILE}.json

Review the reports for detailed findings.

Recommendations:
----------------
1. Review all HIGH and MEDIUM severity findings
2. Verify security headers are present
3. Test for XSS, SQL injection, and other OWASP Top 10 vulnerabilities
4. Check for information disclosure
5. Validate authentication and authorization controls

For more information:
- OWASP Top 10: https://owasp.org/www-project-top-ten/
- ZAP Documentation: https://www.zaproxy.org/docs/
EOF

    echo -e "${GREEN}✓ Summary saved to: $SUMMARY_FILE${NC}"
}

# Main execution
main() {
    check_zap_installed
    check_target
    check_security_headers

    case $SCAN_TYPE in
        baseline)
            run_baseline_scan
            ;;
        full)
            run_full_scan
            ;;
        api)
            run_api_scan
            ;;
        *)
            echo -e "${RED}✗ Invalid scan type: $SCAN_TYPE${NC}"
            echo -e "${YELLOW}Valid types: baseline, full, api${NC}"
            exit 1
            ;;
    esac

    generate_summary

    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║             Security Scan Complete!                ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${GREEN}Reports saved to:${NC} $REPORT_DIR"
    echo -e "${YELLOW}Review the reports for security findings.${NC}"
    echo ""
}

# Run main
main
