#!/bin/bash
# Run progressive load tests: 100k -> 200k -> 400k -> 600k RPS
# Usage: ./05-progressive-test.sh

set -e

if [ -f ../config.env ]; then
    source ../config.env
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Get pod count
POD_COUNT=$(kubectl get pods -n loadtest -l app=vegeta -o name | wc -l)
echo "Available generators: $POD_COUNT"

# Calculate RPS per generator for each level
# With 16 generators:
# 100k total = 6250 per gen
# 200k total = 12500 per gen
# 400k total = 25000 per gen
# 600k total = 37500 per gen

declare -a LEVELS=(100000 200000 400000 600000)
declare -a RESULTS

echo "=== Progressive Load Test ==="
echo ""

for target_rps in "${LEVELS[@]}"; do
    rps_per_gen=$((target_rps / POD_COUNT))

    echo "=========================================="
    echo "Testing ${target_rps} RPS (${rps_per_gen}/generator)"
    echo "=========================================="

    # Run test
    "$SCRIPT_DIR/04-run-loadtest.sh" "$rps_per_gen" 30

    echo ""
    echo "Cooldown 30 seconds..."
    sleep 30
    echo ""
done

echo ""
echo "=== All Tests Complete ==="
echo "Results in: $SCRIPT_DIR/../results/"
