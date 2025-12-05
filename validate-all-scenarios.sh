#!/bin/bash
echo "Validating all scenarios..."
echo
for file in configs/scenarios/scenario-0[5-9]-*.proxy configs/scenarios/scenario-1[0-5]-*.proxy; do
  name=$(basename "$file")
  echo -n "Testing $name: "
  if ./target/release/highper-gateway validate -c "$file" 2>&1 | grep -q "Configuration is valid"; then
    echo "✅ PASSED"
  else
    echo "❌ FAILED"
  fi
done
