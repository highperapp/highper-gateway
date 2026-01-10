#!/bin/bash

# Multi-container Kubernetes Load Test Runner
# Targets: 30k -> 50k -> 100k RPS

set -e

NAMESPACE="loadtest"
CHART_DIR="$(dirname "$0")/loadtest-stack"
RESULTS_DIR="$(dirname "$0")/../results/k8s"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

usage() {
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  setup              - Deploy the load test stack"
    echo "  test <rps>         - Run load test at specified total RPS"
    echo "  results            - Show aggregated results"
    echo "  cleanup            - Remove the load test stack"
    echo "  status             - Show pod status"
    echo ""
    echo "Examples:"
    echo "  $0 setup"
    echo "  $0 test 30000"
    echo "  $0 test 50000"
    echo "  $0 test 100000"
    exit 1
}

setup() {
    echo -e "${GREEN}Setting up load test stack...${NC}"

    # Create namespace if not exists
    kubectl create namespace $NAMESPACE --dry-run=client -o yaml | kubectl apply -f -

    # Build proxy image first
    echo "Building highper-gateway image..."
    cd "$(dirname "$0")/../.."
    docker build -t highper-gateway:loadtest -f deployment/docker/Dockerfile .

    # Install/upgrade helm chart
    echo "Deploying Helm chart..."
    helm upgrade --install loadtest-stack "$CHART_DIR" \
        --namespace $NAMESPACE \
        --wait \
        --timeout 5m

    echo -e "${GREEN}Setup complete!${NC}"
    echo ""
    kubectl get pods -n $NAMESPACE
}

run_test() {
    local TOTAL_RPS=$1
    local DURATION=${2:-60}

    if [ -z "$TOTAL_RPS" ]; then
        echo -e "${RED}Error: RPS not specified${NC}"
        usage
    fi

    # Get vegeta pod names
    VEGETA_PODS=$(kubectl get pods -n $NAMESPACE -l app=vegeta -o jsonpath='{.items[*].metadata.name}')
    POD_COUNT=$(echo $VEGETA_PODS | wc -w)

    if [ "$POD_COUNT" -eq 0 ]; then
        echo -e "${RED}Error: No vegeta pods found. Run setup first.${NC}"
        exit 1
    fi

    # Calculate RPS per pod
    RPS_PER_POD=$((TOTAL_RPS / POD_COUNT))

    echo "=========================================="
    echo "Kubernetes Load Test - ${TOTAL_RPS} RPS"
    echo "=========================================="
    echo ""
    echo "Configuration:"
    echo "  - Total RPS: ${TOTAL_RPS}"
    echo "  - Generator pods: ${POD_COUNT}"
    echo "  - RPS per pod: ${RPS_PER_POD}"
    echo "  - Duration: ${DURATION}s"
    echo ""

    # Create results directory
    mkdir -p "$RESULTS_DIR"
    TIMESTAMP=$(date +%Y%m%d-%H%M%S)
    TEST_DIR="$RESULTS_DIR/${TOTAL_RPS}rps-$TIMESTAMP"
    mkdir -p "$TEST_DIR"

    echo "Starting test in 3 seconds..."
    sleep 3

    # Launch vegeta attack on all pods in parallel
    PIDS=""
    for POD in $VEGETA_PODS; do
        echo "Launching attack on $POD ($RPS_PER_POD RPS)..."
        kubectl exec -n $NAMESPACE $POD -- sh -c "echo 'GET http://proxy:8080/' | vegeta attack -rate=$RPS_PER_POD -duration=${DURATION}s -timeout=10s -keepalive=true -max-connections=100 > /tmp/results.bin" &
        PIDS="$PIDS $!"
    done

    echo ""
    echo "Test running... (${DURATION} seconds)"

    # Wait for all attacks to complete
    for PID in $PIDS; do
        wait $PID
    done

    echo ""
    echo "Collecting results..."

    # Collect results from each pod
    for POD in $VEGETA_PODS; do
        echo "  - $POD"
        kubectl cp $NAMESPACE/$POD:/tmp/results.bin "$TEST_DIR/${POD}.bin"
        kubectl exec -n $NAMESPACE $POD -- vegeta report /tmp/results.bin > "$TEST_DIR/${POD}.txt"
    done

    # Generate aggregated report
    echo ""
    echo "=========================================="
    echo "Results Summary"
    echo "=========================================="

    # Show individual pod results
    for POD in $VEGETA_PODS; do
        echo ""
        echo "--- $POD ---"
        head -20 "$TEST_DIR/${POD}.txt"
    done

    # Calculate totals
    echo ""
    echo "=========================================="
    echo "Aggregate Statistics"
    echo "=========================================="

    TOTAL_REQUESTS=0
    TOTAL_SUCCESS=0

    for POD in $VEGETA_PODS; do
        REQUESTS=$(grep "Requests" "$TEST_DIR/${POD}.txt" | head -1 | awk '{print $3}')
        SUCCESS=$(grep "Success" "$TEST_DIR/${POD}.txt" | awk '{print $3}' | tr -d '%')
        TOTAL_REQUESTS=$((TOTAL_REQUESTS + REQUESTS))
        # Sum success rates for average
    done

    echo "Total Requests: $TOTAL_REQUESTS"
    echo "Expected: $((TOTAL_RPS * DURATION))"
    echo "Actual RPS: $((TOTAL_REQUESTS / DURATION))"
    echo ""
    echo "Results saved to: $TEST_DIR"
}

show_results() {
    echo "Recent test results:"
    echo ""
    ls -lt "$RESULTS_DIR" 2>/dev/null || echo "No results found"
}

cleanup() {
    echo -e "${YELLOW}Removing load test stack...${NC}"
    helm uninstall loadtest-stack -n $NAMESPACE 2>/dev/null || true
    kubectl delete namespace $NAMESPACE 2>/dev/null || true
    echo -e "${GREEN}Cleanup complete!${NC}"
}

status() {
    echo "Load test stack status:"
    echo ""
    kubectl get pods -n $NAMESPACE -o wide
    echo ""
    kubectl top pods -n $NAMESPACE 2>/dev/null || echo "(metrics not available)"
}

# Main
case "${1:-}" in
    setup)
        setup
        ;;
    test)
        run_test "$2" "$3"
        ;;
    results)
        show_results
        ;;
    cleanup)
        cleanup
        ;;
    status)
        status
        ;;
    *)
        usage
        ;;
esac
