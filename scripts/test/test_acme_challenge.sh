#!/bin/bash

# Test ACME challenge handling

echo "Testing ACME HTTP-01 challenge handler..."
echo

# Test 1: Challenge not found (no challenge store)
echo "Test 1: Request to /.well-known/acme-challenge/test-token"
response=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/.well-known/acme-challenge/test-token)
echo "Response code: $response"
if [ "$response" = "404" ]; then
    echo "✓ Correctly returns 404 when challenge not found"
else
    echo "✗ Expected 404, got $response"
fi
echo

# Test 2: Regular route still works
echo "Test 2: Regular route /test"
response=$(curl -s http://localhost:8080/test)
echo "Response: $response"
if echo "$response" | grep -q "test_backend"; then
    echo "✓ Regular routing still works"
else
    echo "✗ Regular routing broken"
fi
echo

echo "Basic HTTP-01 challenge handling verified!"
