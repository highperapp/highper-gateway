#!/bin/bash
# Run progressive load tests from 10k to 800k RPS
set -e

cd "$(dirname "$0")"

echo "=== Progressive Load Testing ==="
echo ""
echo "Test Plan:"
echo "  1. 10k RPS   (1 gen x 10k)"
echo "  2. 20k RPS   (1 gen x 20k)"
echo "  3. 50k RPS   (1 gen x 50k)"
echo "  4. 100k RPS  (2 gen x 50k)"
echo "  5. 300k RPS  (3 gen x 100k)"
echo "  6. 500k RPS  (3 gen x 167k)"
echo "  7. 600k RPS  (3 gen x 200k)"
echo "  8. 800k RPS  (3 gen x 267k)"
echo ""
echo "Each test runs for 60 seconds with 30s cooldown between tests."
echo ""
read -p "Press Enter to start, or Ctrl+C to abort..."

DURATION=60
COOLDOWN=30

run_test() {
    local RPS=$1
    local GENS=$2
    local NAME=$3

    echo ""
    echo "=========================================="
    echo "Test: $NAME - $((RPS * GENS)) RPS"
    echo "=========================================="

    ./04-run-loadtest.sh $RPS $DURATION $GENS

    echo ""
    echo "Cooldown ${COOLDOWN}s..."
    sleep $COOLDOWN
}

# Run tests
run_test 10000 1 "10k RPS"
run_test 20000 1 "20k RPS"
run_test 50000 1 "50k RPS"
run_test 50000 2 "100k RPS"
run_test 100000 3 "300k RPS"
run_test 167000 3 "500k RPS"
run_test 200000 3 "600k RPS"
run_test 267000 3 "800k RPS"

echo ""
echo "=== All Tests Complete ==="
echo ""
echo "Results saved in ../results/"
ls -la ../results/
