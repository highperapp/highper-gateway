#!/bin/bash
export PATH=$HOME/bin:$PATH

echo "=========================================="
echo "Parallel Vegeta Load Test - 50K req/s"
echo "=========================================="
echo ""
echo "Configuration:"
echo "  - 4 parallel instances"
echo "  - 12,500 req/s each = 50,000 total"
echo "  - Duration: 60 seconds"
echo "  - Keep-alive: enabled"
echo "  - Timeout: 10s"
echo ""
echo "Starting test in 3 seconds..."
sleep 3

# Launch 4 vegeta instances in parallel
echo "Launching instance 1..."
echo "GET http://127.0.0.1:8080/" | vegeta attack -rate=12500 -duration=60s -timeout=10s -keepalive=true -max-connections=50 > /tmp/vegeta-50k-p1.bin 2>&1 &
PID1=$!

echo "Launching instance 2..."
echo "GET http://127.0.0.1:8080/" | vegeta attack -rate=12500 -duration=60s -timeout=10s -keepalive=true -max-connections=50 > /tmp/vegeta-50k-p2.bin 2>&1 &
PID2=$!

echo "Launching instance 3..."
echo "GET http://127.0.0.1:8080/" | vegeta attack -rate=12500 -duration=60s -timeout=10s -keepalive=true -max-connections=50 > /tmp/vegeta-50k-p3.bin 2>&1 &
PID3=$!

echo "Launching instance 4..."
echo "GET http://127.0.0.1:8080/" | vegeta attack -rate=12500 -duration=60s -timeout=10s -keepalive=true -max-connections=50 > /tmp/vegeta-50k-p4.bin 2>&1 &
PID4=$!

echo ""
echo "All instances launched!"
echo "PIDs: $PID1 $PID2 $PID3 $PID4"
echo ""
echo "Test running... (60 seconds)"
echo ""

# Wait for all instances to complete
wait $PID1
echo "Instance 1 complete"
wait $PID2
echo "Instance 2 complete"
wait $PID3
echo "Instance 3 complete"
wait $PID4
echo "Instance 4 complete"

echo ""
echo "=========================================="
echo "All instances completed!"
echo "=========================================="
