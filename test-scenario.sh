#!/bin/bash
# Quick scenario test script

SCENARIO=$1
HOST=${2:-localhost}

echo "Testing Scenario: $SCENARIO"
echo "Host header: $HOST"
echo "================================"

# Test 10 requests
echo "Making 10 requests to test load balancing:"
for i in {1..10}; do
  response=$(curl -s -H "Host: $HOST" http://127.0.0.1:8080/)
  backend=$(echo "$response" | grep -o '"backend":"[^"]*"' | cut -d'"' -f4)
  if [ -n "$backend" ]; then
    echo "  Request $i: $backend"
  else
    echo "  Request $i: ERROR - $(echo $response | head -c 50)"
  fi
done

echo ""
echo "Distribution:"
for i in {1..30}; do
  curl -s -H "Host: $HOST" http://127.0.0.1:8080/ | grep -o '"backend":"[^"]*"'
done | sort | uniq -c
