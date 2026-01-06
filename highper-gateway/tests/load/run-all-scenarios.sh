#!/bin/bash
# Master Test Runner - Runs all 15 scenario tests
# Usage: ./run-all-scenarios.sh [scenario_numbers...]
# Example: ./run-all-scenarios.sh 1 2 3  (runs only scenarios 1, 2, 3)
# Example: ./run-all-scenarios.sh       (runs all scenarios)

set -euo pipefail

cd "$(dirname "$0")"

# Color output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse arguments
SCENARIOS_TO_RUN=("$@")
if [ ${#SCENARIOS_TO_RUN[@]} -eq 0 ]; then
    # Run all scenarios
    SCENARIOS_TO_RUN=($(seq 1 15))
fi

echo "========================================="
echo "Highper Gateway - Load Test Suite"
echo "========================================="
echo "Running scenarios: ${SCENARIOS_TO_RUN[*]}"
echo ""

# Create main results directory
TIMESTAMP=$(date '+%Y%m%d-%H%M%S')
MAIN_RESULT_DIR="results/local/all-scenarios-${TIMESTAMP}"
mkdir -p "$MAIN_RESULT_DIR"

# Track results
declare -A RESULTS
declare -A DURATIONS

# Scenario metadata
declare -A SCENARIO_NAMES
SCENARIO_NAMES[1]="TCP Proxy (Native)"
SCENARIO_NAMES[2]="HTTP Load Balancer"
SCENARIO_NAMES[3]="HTTPS/TLS Termination"
SCENARIO_NAMES[4]="API Gateway Rate Limiting"
SCENARIO_NAMES[5]="HTTP/3 QUIC"
SCENARIO_NAMES[6]="WebSocket Load Balancer"
SCENARIO_NAMES[7]="gRPC Gateway"
SCENARIO_NAMES[8]="Database Load Balancer"
SCENARIO_NAMES[9]="WAF + mTLS Security"
SCENARIO_NAMES[10]="Hybrid Multi-Protocol"
SCENARIO_NAMES[11]="CDN Edge Caching"
SCENARIO_NAMES[12]="Microservices Discovery"
SCENARIO_NAMES[13]="GraphQL Gateway"
SCENARIO_NAMES[14]="Static + PHP-FPM"
SCENARIO_NAMES[15]="Geographic Load Balancing"

# Run each scenario
for scenario_num in "${SCENARIOS_TO_RUN[@]}"; do
    scenario_num=$(printf "%02d" $scenario_num)  # Zero-pad
    script_name="test-scenario-${scenario_num}*.sh"

    # Find matching script
    script=$(ls test-scenario-${scenario_num}*.sh 2>/dev/null | head -1)

    if [ -z "$script" ]; then
        echo -e "${YELLOW}⚠ Scenario ${scenario_num} not found, skipping...${NC}"
        RESULTS[$scenario_num]="SKIP"
        continue
    fi

    scenario_name="${SCENARIO_NAMES[$((10#$scenario_num))]:-Unknown}"

    echo ""
    echo "========================================="
    echo -e "${YELLOW}Running Scenario ${scenario_num}: ${scenario_name}${NC}"
    echo "========================================="

    start_time=$(date +%s)

    if bash "$script" > "${MAIN_RESULT_DIR}/scenario-${scenario_num}.log" 2>&1; then
        end_time=$(date +%s)
        duration=$((end_time - start_time))

        echo -e "${GREEN}✓ Scenario ${scenario_num} PASSED (${duration}s)${NC}"
        RESULTS[$scenario_num]="PASS"
        DURATIONS[$scenario_num]=$duration
    else
        end_time=$(date +%s)
        duration=$((end_time - start_time))

        echo -e "${RED}✗ Scenario ${scenario_num} FAILED (${duration}s)${NC}"
        echo "  Check log: ${MAIN_RESULT_DIR}/scenario-${scenario_num}.log"
        RESULTS[$scenario_num]="FAIL"
        DURATIONS[$scenario_num]=$duration
    fi
done

# Generate summary report
echo ""
echo "========================================="
echo "Test Suite Complete!"
echo "========================================="
echo ""
echo "Summary Report:"
echo ""
printf "%-5s | %-35s | %-10s | %-10s\n" "ID" "Scenario" "Result" "Duration"
printf "%-5s-+-%-35s-+-%-10s-+-%-10s\n" "-----" "-----------------------------------" "----------" "----------"

total_pass=0
total_fail=0
total_skip=0

for scenario_num in "${SCENARIOS_TO_RUN[@]}"; do
    scenario_num=$(printf "%02d" $scenario_num)
    scenario_name="${SCENARIO_NAMES[$((10#$scenario_num))]:-Unknown}"
    result="${RESULTS[$scenario_num]:-UNKNOWN}"
    duration="${DURATIONS[$scenario_num]:-0}"

    # Color code result
    case $result in
        PASS)
            result_colored="${GREEN}✓ PASS${NC}"
            ((total_pass++))
            ;;
        FAIL)
            result_colored="${RED}✗ FAIL${NC}"
            ((total_fail++))
            ;;
        SKIP)
            result_colored="${YELLOW}⚠ SKIP${NC}"
            ((total_skip++))
            ;;
        *)
            result_colored="? UNKNOWN"
            ;;
    esac

    printf "%-5s | %-35s | %-10b | %8ss\n" "$scenario_num" "$scenario_name" "$result_colored" "$duration"
done

echo ""
echo "Total: ${total_pass} passed, ${total_fail} failed, ${total_skip} skipped"
echo ""
echo "Full logs available in: $MAIN_RESULT_DIR"
echo "========================================="

# Exit with error if any tests failed
if [ $total_fail -gt 0 ]; then
    exit 1
fi

exit 0
