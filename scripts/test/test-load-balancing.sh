#!/bin/bash
echo "Testing Load Balancing - 30 requests"
echo "====================================="
for i in {1..30}; do
  curl -s http://127.0.0.1:8080/ | grep -o '"backend":"backend-[0-9]*"'
done | sort | uniq -c | awk '{print $2 ": " $1 " requests"}'

echo ""
echo "Sequential requests (should round-robin):"
for i in {1..9}; do
  backend=$(curl -s http://127.0.0.1:8080/ | grep -o '"backend":"backend-[0-9]*"' | cut -d'"' -f4)
  echo "  Request $i: $backend"
done
