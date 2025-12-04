#!/bin/bash
# Run Scenario 03 HTTPS test

echo "Starting gateway in background..."
./target/release/highper-gateway start -c /tmp/scenario-03-test.proxy > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!

# Wait for gateway to start
sleep 5

echo "Testing HTTPS load balancing (9 requests):"
for i in {1..9}; do
    backend=$(curl -sk -H "Host: localhost" https://127.0.0.1:8443/ 2>/dev/null | grep -o 'backend-[0-9]*')
    echo "  Request $i: $backend"
done

echo
echo "Checking metrics:"
curl -s http://127.0.0.1:9090/metrics 2>/dev/null | grep "http_requests_total" | head -5

echo
echo "Stopping gateway..."
kill $GATEWAY_PID 2>/dev/null
sleep 1

echo "✅ Scenario 03 test complete!"
