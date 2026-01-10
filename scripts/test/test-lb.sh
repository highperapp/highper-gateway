#!/bin/bash
echo "Testing load balancing (9 requests):"
for i in 1 2 3 4 5 6 7 8 9; do
  backend=$(curl -s http://127.0.0.1:8080/ | grep -o 'backend-[0-9]*')
  echo "  Request $i: $backend"
done
