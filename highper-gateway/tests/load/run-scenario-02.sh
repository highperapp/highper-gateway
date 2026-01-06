#!/bin/bash
# Wrapper script for Scenario 02 with proper environment setup

set -euo pipefail

# Ensure tools are in PATH
export PATH=~/bin:$PATH

# Ensure we're in the right directory
cd "$(dirname "$0")"

# Export environment for local mode
export LOAD_TEST_MODE=local
export LOAD_TEST_DURATION=30  # Shorter duration for initial test
export LOAD_TEST_WARMUP=5
export LOAD_TEST_RATE_START=1000
export LOAD_TEST_RATE_STEP=2000
export LOAD_TEST_RATE_MAX=10000  # Start with lower target

# Run the scenario
./scenarios/02-http-loadbalancer.sh
