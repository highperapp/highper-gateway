#!/bin/bash
# Run coordinated load test across multiple generators
set -e

if [ -f ../droplet-ips.env ]; then
    source ../droplet-ips.env
else
    echo "Error: ../droplet-ips.env not found"
    exit 1
fi

RPS_PER_GEN=${1:-10000}
DURATION=${2:-60}
NUM_GENERATORS=${3:-1}

TOTAL_RPS=$((RPS_PER_GEN * NUM_GENERATORS))
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_DIR="../results/${TIMESTAMP}-${TOTAL_RPS}rps"
mkdir -p "$RESULTS_DIR"

echo "=== Load Test Configuration ==="
echo "Target: $TOTAL_RPS RPS ($RPS_PER_GEN per generator x $NUM_GENERATORS generators)"
echo "Duration: ${DURATION}s"
echo "Proxy: $PROXY_PRIVATE:8080"
echo "Results: $RESULTS_DIR"
echo ""

# Verify connectivity first
echo "=== Verifying Connectivity ==="
for i in $(seq 1 $NUM_GENERATORS); do
    eval "IP=\$LOADGEN_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo -n "loadgen-$i -> proxy: "
        ssh root@$IP "curl -s -o /dev/null -w '%{http_code}' http://$PROXY_PRIVATE:8080/" || echo "FAILED"
        echo ""
    fi
done

echo ""
echo "=== Starting Load Test ==="
echo "Press Ctrl+C to abort"
echo ""

# Start load generators in parallel
PIDS=""
for i in $(seq 1 $NUM_GENERATORS); do
    eval "IP=\$LOADGEN_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo "Starting loadgen-$i at $RPS_PER_GEN RPS..."
        ssh root@$IP "echo 'GET http://$PROXY_PRIVATE:8080/' | vegeta attack -rate=$RPS_PER_GEN -duration=${DURATION}s -workers=200 -max-workers=500 | vegeta encode > /tmp/results.bin" &
        PIDS="$PIDS $!"
    fi
done

# Wait for all generators to complete
echo ""
echo "Waiting for tests to complete (${DURATION}s)..."
for PID in $PIDS; do
    wait $PID
done

echo ""
echo "=== Collecting Results ==="

# Collect and analyze results
TOTAL_REQS=0
TOTAL_SUCCESS=0
ALL_LATENCIES=""

for i in $(seq 1 $NUM_GENERATORS); do
    eval "IP=\$LOADGEN_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        echo ""
        echo "--- loadgen-$i Results ---"

        # Get report
        REPORT=$(ssh root@$IP "vegeta report /tmp/results.bin")
        echo "$REPORT"
        echo "$REPORT" > "$RESULTS_DIR/loadgen-$i-report.txt"

        # Get JSON for aggregation
        ssh root@$IP "vegeta report -type=json /tmp/results.bin" > "$RESULTS_DIR/loadgen-$i.json"

        # Extract metrics
        REQS=$(echo "$REPORT" | grep "Requests" | awk '{print $2}' | tr -d ',')
        SUCCESS=$(echo "$REPORT" | grep "Success" | awk '{print $2}' | tr -d '%')

        TOTAL_REQS=$((TOTAL_REQS + REQS))
        # Calculate success count
        SUCCESS_COUNT=$(echo "$REQS * $SUCCESS / 100" | bc)
        TOTAL_SUCCESS=$((TOTAL_SUCCESS + SUCCESS_COUNT))
    fi
done

# Calculate aggregate stats
if [ $TOTAL_REQS -gt 0 ]; then
    AVG_SUCCESS=$(echo "scale=2; $TOTAL_SUCCESS * 100 / $TOTAL_REQS" | bc)
else
    AVG_SUCCESS=0
fi

echo ""
echo "=== Aggregate Results ==="
echo "Total Requests: $TOTAL_REQS"
echo "Success Rate: ${AVG_SUCCESS}%"
echo "Target RPS: $TOTAL_RPS"
echo "Actual RPS: $(echo "$TOTAL_REQS / $DURATION" | bc)"

# Save summary
cat > "$RESULTS_DIR/summary.txt" << EOF
Load Test Summary
=================
Timestamp: $TIMESTAMP
Target RPS: $TOTAL_RPS
Duration: ${DURATION}s
Generators: $NUM_GENERATORS

Results:
--------
Total Requests: $TOTAL_REQS
Actual RPS: $(echo "$TOTAL_REQS / $DURATION" | bc)
Success Rate: ${AVG_SUCCESS}%
EOF

echo ""
echo "Results saved to $RESULTS_DIR/"
