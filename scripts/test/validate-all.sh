#!/bin/bash
# Simple validation script for all scenarios

echo "Validating all scenario configs..."
echo "=================================="

for config in configs/scenarios/scenario-*.proxy; do
  name=$(basename "$config")
  echo ""
  echo "Testing: $name"
  if ./target/release/highper-gateway validate -c "$config" 2>&1 | grep -q "Configuration is valid"; then
    echo "  ✓ PASS"
  else
    echo "  ✗ FAIL"
  fi
done

echo ""
echo "Done!"
