#!/bin/bash
# Run load tests at progressive RPS levels
# Usage: ./04-run-loadtest.sh [RPS_PER_GENERATOR]

set -e

RPS_PER_GEN="${1:-25000}"  # Default 25k per generator
DURATION="${2:-60}"
TIMEOUT="${3:-10s}"
MAX_CONN="${4:-500}"

if [ -f ../config.env ]; then
    source ../config.env
fi

echo "=== Load Test Configuration ==="
echo "RPS per generator: $RPS_PER_GEN"
echo "Duration: ${DURATION}s"
echo "Timeout: $TIMEOUT"
echo "Max connections: $MAX_CONN"

# Get all vegeta pods
PODS=$(kubectl get pods -n loadtest -l app=vegeta -o jsonpath='{.items[*].metadata.name}')
POD_COUNT=$(echo $PODS | wc -w)

TOTAL_RPS=$((RPS_PER_GEN * POD_COUNT))
echo "Generators: $POD_COUNT"
echo "Total target RPS: $TOTAL_RPS"
echo ""

# Verify proxy is reachable
echo "=== Verifying proxy connectivity ==="
FIRST_POD=$(echo $PODS | awk '{print $1}')
if kubectl exec -n loadtest $FIRST_POD -- wget -q -O - "http://$PROXY_PRIVATE_IP:8080/" --timeout=5 2>/dev/null; then
    echo "Proxy is reachable!"
else
    echo "ERROR: Cannot reach proxy at $PROXY_PRIVATE_IP:8080"
    echo "Please ensure proxy is running and config is correct"
    exit 1
fi

echo ""
echo "=== Starting load test ==="
echo "Target: http://$PROXY_PRIVATE_IP:8080/"
echo ""

# Create results directory
RESULTS_DIR="../results/$(date +%Y%m%d-%H%M%S)-${TOTAL_RPS}rps"
mkdir -p "$RESULTS_DIR"

# Run vegeta on all pods in parallel
for pod in $PODS; do
    echo "Starting vegeta on $pod..."
    kubectl exec -n loadtest $pod -- sh -c "echo 'GET http://$PROXY_PRIVATE_IP:8080/' | vegeta attack -rate=$RPS_PER_GEN -duration=${DURATION}s -timeout=$TIMEOUT -max-connections=$MAX_CONN | vegeta encode > /tmp/results.bin && vegeta report < /tmp/results.bin" > "$RESULTS_DIR/$pod.txt" 2>&1 &
done

echo ""
echo "Waiting for tests to complete (${DURATION}s + buffer)..."
wait

echo ""
echo "=== Results ==="
echo ""

# Aggregate results
TOTAL_REQUESTS=0
TOTAL_SUCCESS=0
declare -a LATENCIES_P99

for result_file in "$RESULTS_DIR"/*.txt; do
    pod_name=$(basename "$result_file" .txt)
    echo "--- $pod_name ---"
    cat "$result_file"
    echo ""

    # Extract metrics
    requests=$(grep "Requests" "$result_file" | head -1 | awk '{print $3}' | tr -d ',')
    success=$(grep "Success" "$result_file" | awk '{print $3}' | tr -d '%')

    if [ -n "$requests" ]; then
        TOTAL_REQUESTS=$((TOTAL_REQUESTS + requests))
    fi
done

echo ""
echo "=== Summary ==="
echo "Total Requests: $TOTAL_REQUESTS"
echo "Results saved to: $RESULTS_DIR"
echo ""

# Create summary file
cat > "$RESULTS_DIR/summary.txt" << EOF
Load Test Summary
=================
Date: $(date)
Target RPS: $TOTAL_RPS
Generators: $POD_COUNT
RPS per generator: $RPS_PER_GEN
Duration: ${DURATION}s
Total Requests: $TOTAL_REQUESTS
EOF

echo "Summary saved to $RESULTS_DIR/summary.txt"
