#!/bin/bash
#
# Run vegeta load test with different rates
#
# Usage:
#   ./run-vegeta-test.sh <scenario> <rate> <duration>
#   ./run-vegeta-test.sh vegeta-http-simple.txt 1000 30s
#   ./run-vegeta-test.sh vegeta-http-post.txt 500 1m
#

set -euo pipefail

SCENARIO=${1:-vegeta-http-simple.txt}
RATE=${2:-1000}
DURATION=${3:-30s}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SCENARIO_FILE="${SCRIPT_DIR}/../scenarios/${SCENARIO}"
RESULTS_DIR="${SCRIPT_DIR}/../results"

mkdir -p "${RESULTS_DIR}"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULT_FILE="${RESULTS_DIR}/vegeta_${SCENARIO%.txt}_${RATE}rps_${TIMESTAMP}.bin"
REPORT_FILE="${RESULTS_DIR}/vegeta_${SCENARIO%.txt}_${RATE}rps_${TIMESTAMP}.txt"

echo "========================================"
echo "Vegeta Load Test"
echo "========================================"
echo "Scenario: ${SCENARIO}"
echo "Rate: ${RATE} requests/second"
echo "Duration: ${DURATION}"
echo "========================================"
echo

# Check if scenario file exists
if [ ! -f "${SCENARIO_FILE}" ]; then
    echo "Error: Scenario file not found: ${SCENARIO_FILE}"
    exit 1
fi

# Run vegeta attack
echo "Starting attack..."
~/.local/bin/vegeta attack \
    -targets="${SCENARIO_FILE}" \
    -rate="${RATE}" \
    -duration="${DURATION}" \
    -timeout=30s \
    -workers=10 \
    > "${RESULT_FILE}"

echo "Attack complete. Generating report..."

# Generate text report
~/.local/bin/vegeta report "${RESULT_FILE}" > "${REPORT_FILE}"

# Display report
echo
cat "${REPORT_FILE}"

# Generate additional reports
echo
echo "Generating detailed reports..."

# Plot latency histogram
~/.local/bin/vegeta report -type='hist[0,5ms,10ms,25ms,50ms,100ms,250ms,500ms,1s,2.5s]' "${RESULT_FILE}" \
    >> "${REPORT_FILE}"

echo
echo "Results saved to:"
echo "  Binary: ${RESULT_FILE}"
echo "  Report: ${REPORT_FILE}"
echo
echo "To generate plots:"
echo "  vegeta plot ${RESULT_FILE} > plot.html"
echo "  vegeta report -type=json ${RESULT_FILE} | jq ."
