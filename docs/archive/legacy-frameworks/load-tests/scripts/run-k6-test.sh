#!/bin/bash
#
# Run k6 load test
#
# Usage:
#   ./run-k6-test.sh <script> [options]
#   ./run-k6-test.sh k6-http-simple.js
#   ./run-k6-test.sh k6-http-post.js --vus 200 --duration 2m
#

set -euo pipefail

SCRIPT=${1:-k6-http-simple.js}
shift || true

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K6_SCRIPT="${SCRIPT_DIR}/${SCRIPT}"
RESULTS_DIR="${SCRIPT_DIR}/../results"

mkdir -p "${RESULTS_DIR}"

echo "========================================"
echo "k6 Load Test"
echo "========================================"
echo "Script: ${SCRIPT}"
echo "Additional options: $@"
echo "========================================"
echo

# Check if script exists
if [ ! -f "${K6_SCRIPT}" ]; then
    echo "Error: k6 script not found: ${K6_SCRIPT}"
    exit 1
fi

# Run k6
~/.local/bin/k6 run "$@" "${K6_SCRIPT}"

echo
echo "Test complete. Results saved to ${RESULTS_DIR}/"
