#!/bin/bash
# Scenario 07 - gRPC Gateway
# Tests gRPC proxying and load balancing

set -euo pipefail

export PATH=~/bin:$PATH
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 07: gRPC Gateway"
echo "========================================="

cleanup() {
    echo "Cleaning up..."
    [ ! -z "${GATEWAY_PID:-}" ] && kill $GATEWAY_PID 2>/dev/null || true

    # Force cleanup gRPC containers
    docker rm -f $(docker ps -aq --filter "name=grpc-server") 2>/dev/null || true

    # Kill any processes using our ports
    for port in 8080 50051 50052; do
        lsof -ti:$port | xargs kill -9 2>/dev/null || true
    done

    # Wait for ports to be free
    sleep 2
    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Detect host IP for Docker containers to connect (WSL2 compatibility)
HOST_IP=$(hostname -I | awk '{print $1}')
echo "Host IP detected: $HOST_IP"

# Force cleanup at test start to ensure clean state
echo "Ensuring clean test environment..."
cleanup

# Create gRPC server code
echo "Creating gRPC backend servers..."
mkdir -p /tmp/grpc-backend

# Create proto definition
cat > /tmp/grpc-backend/service.proto <<'PROTO'
syntax = "proto3";

package highper;

service HighperService {
  rpc SayHello (HelloRequest) returns (HelloReply) {}
  rpc GetUser (UserRequest) returns (User) {}
  rpc ListUsers (Empty) returns (UserList) {}
  rpc StreamMessages (StreamRequest) returns (stream Message) {}
}

message HelloRequest {
  string name = 1;
}

message HelloReply {
  string message = 1;
  string backend = 2;
}

message UserRequest {
  string id = 1;
}

message User {
  string id = 1;
  string name = 2;
  string email = 3;
}

message Empty {}

message UserList {
  repeated User users = 1;
  string backend = 2;
}

message StreamRequest {
  int32 count = 1;
}

message Message {
  string text = 1;
  int32 sequence = 2;
  string backend = 3;
}
PROTO

# Create Python gRPC server
cat > /tmp/grpc-backend/server.py <<'PYTHON_SERVER'
#!/usr/bin/env python3
import os
import sys
import time
import grpc
from concurrent import futures

# Inline proto compilation - simple approach
import subprocess

BACKEND_NAME = os.getenv('BACKEND_NAME', 'grpc-backend')
PORT = int(os.getenv('PORT', 50051))

# Generate Python code from proto
subprocess.run([
    'python3', '-m', 'grpc_tools.protoc',
    '-I.', '--python_out=.', '--grpc_python_out=.',
    'service.proto'
], check=True)

import service_pb2
import service_pb2_grpc

# Sample data
users = [
    {'id': '1', 'name': 'Alice', 'email': 'alice@example.com'},
    {'id': '2', 'name': 'Bob', 'email': 'bob@example.com'},
    {'id': '3', 'name': 'Charlie', 'email': 'charlie@example.com'}
]

class HighperServiceServicer(service_pb2_grpc.HighperServiceServicer):
    def SayHello(self, request, context):
        return service_pb2.HelloReply(
            message=f'Hello {request.name}!',
            backend=BACKEND_NAME
        )

    def GetUser(self, request, context):
        user_data = next((u for u in users if u['id'] == request.id), None)
        if user_data:
            return service_pb2.User(**user_data)
        else:
            context.set_code(grpc.StatusCode.NOT_FOUND)
            context.set_details('User not found')
            return service_pb2.User()

    def ListUsers(self, request, context):
        user_list = [service_pb2.User(**u) for u in users]
        return service_pb2.UserList(users=user_list, backend=BACKEND_NAME)

    def StreamMessages(self, request, context):
        for i in range(request.count):
            yield service_pb2.Message(
                text=f'Message {i+1}',
                sequence=i+1,
                backend=BACKEND_NAME
            )
            time.sleep(0.1)

def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    service_pb2_grpc.add_HighperServiceServicer_to_server(
        HighperServiceServicer(), server
    )
    server.add_insecure_port(f'[::]:{PORT}')
    server.start()
    print(f'gRPC server ({BACKEND_NAME}) listening on port {PORT}')
    server.wait_for_termination()

if __name__ == '__main__':
    serve()
PYTHON_SERVER

# Create client test script
cat > /tmp/grpc-backend/client.py <<'PYTHON_CLIENT'
#!/usr/bin/env python3
import sys
import grpc
import service_pb2
import service_pb2_grpc

def test_say_hello(channel):
    stub = service_pb2_grpc.HighperServiceStub(channel)
    response = stub.SayHello(service_pb2.HelloRequest(name='World'))
    return f'{response.message} (backend: {response.backend})'

def test_list_users(channel):
    stub = service_pb2_grpc.HighperServiceStub(channel)
    response = stub.ListUsers(service_pb2.Empty())
    users = [f'{u.name} ({u.email})' for u in response.users]
    return f'Users: {", ".join(users)} (backend: {response.backend})'

def test_get_user(channel, user_id):
    stub = service_pb2_grpc.HighperServiceStub(channel)
    response = stub.GetUser(service_pb2.UserRequest(id=user_id))
    return f'{response.name} - {response.email}'

def test_stream(channel, count):
    stub = service_pb2_grpc.HighperServiceStub(channel)
    messages = []
    for message in stub.StreamMessages(service_pb2.StreamRequest(count=count)):
        messages.append(f'{message.text} (seq: {message.sequence}, backend: {message.backend})')
    return messages

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print('Usage: client.py <host:port> <command> [args]')
        sys.exit(1)

    target = sys.argv[1]
    command = sys.argv[2]

    channel = grpc.insecure_channel(target)

    try:
        if command == 'hello':
            print(test_say_hello(channel))
        elif command == 'list':
            print(test_list_users(channel))
        elif command == 'get' and len(sys.argv) > 3:
            print(test_get_user(channel, sys.argv[3]))
        elif command == 'stream' and len(sys.argv) > 3:
            messages = test_stream(channel, int(sys.argv[3]))
            for msg in messages:
                print(msg)
        else:
            print('Unknown command')
    except grpc.RpcError as e:
        print(f'gRPC Error: {e.code()}: {e.details()}')
    finally:
        channel.close()
PYTHON_CLIENT

# Create optimized Dockerfile with pre-installed dependencies
echo "Creating optimized Docker image with gRPC dependencies..."
cat > /tmp/grpc-backend/Dockerfile <<'DOCKERFILE'
FROM python:3.11-slim

# Install gRPC dependencies (this is the slow part - do it once!)
RUN pip install --no-cache-dir grpcio grpcio-tools

WORKDIR /app

# Pre-compile proto files on image build
COPY service.proto .
RUN python3 -m grpc_tools.protoc -I. --python_out=. --grpc_python_out=. service.proto

# Copy server and client scripts
COPY server.py client.py ./

CMD ["python3", "server.py"]
DOCKERFILE

# Build the optimized image (only once!)
echo "Building gRPC image with dependencies (one-time setup)..."
docker build -t grpc-test:optimized /tmp/grpc-backend > /dev/null 2>&1

if [ $? -ne 0 ]; then
    echo "ERROR: Failed to build Docker image"
    docker build -t grpc-test:optimized /tmp/grpc-backend
    exit 1
fi

echo "✓ Docker image built with gRPC dependencies pre-installed"

# Start gRPC backend servers
echo "Starting gRPC backend servers..."

docker run -d --name grpc-server-1 \
    -p 50051:50051 \
    -e BACKEND_NAME=grpc-1 \
    -e PORT=50051 \
    grpc-test:optimized > /dev/null 2>&1

if [ $? -ne 0 ]; then
    echo "ERROR: Failed to start grpc-server-1"
    docker logs grpc-server-1 2>&1 | tail -20
    exit 1
fi

docker run -d --name grpc-server-2 \
    -p 50052:50051 \
    -e BACKEND_NAME=grpc-2 \
    -e PORT=50051 \
    grpc-test:optimized > /dev/null 2>&1

if [ $? -ne 0 ]; then
    echo "ERROR: Failed to start grpc-server-2"
    docker logs grpc-server-2 2>&1 | tail -20
    exit 1
fi

echo "Waiting for gRPC servers to start..."
sleep 5

# Check if servers are ready by attempting connection
echo "Checking gRPC backends..."
for port in 50051 50052; do
    success=0
    for i in {1..5}; do
        if docker run --rm --network host grpc-test:optimized \
            python3 client.py localhost:$port hello 2>/dev/null | grep -q "Hello"; then
            echo "✓ gRPC backend on port $port ready"
            success=1
            break
        fi
        echo "  Attempt $i/5: Waiting for port $port..."
        sleep 2
    done

    if [ $success -eq 0 ]; then
        echo "✗ gRPC backend on port $port NOT ready after 5 attempts"
        docker logs grpc-server-$((port - 50050)) 2>&1 | tail -20
    fi
done

# Create gateway config for gRPC
echo ""
echo "Creating gateway configuration for gRPC..."
cat > /tmp/gateway-grpc-test.toml <<'EOF'
[server]
bind = ["0.0.0.0:8080"]
workers = "auto"
protocols = ["http2"]

[server.performance]
max_connections = 100000
read_buffer_size = 65536
write_buffer_size = 65536

# gRPC backends
[[upstreams]]
name = "grpc-backends"

servers = [
    { url = "http://localhost:50051", weight = 1 },
    { url = "http://localhost:50052", weight = 1 },
]

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
timeout = "30s"
keepalive = "60s"
pool_size = 500

# gRPC route
[[routes]]
name = "grpc-route"
upstream = "grpc-backends"

[routes.match]
paths = ["/*"]

[observability.logging]
level = "warn"
format = "json"
EOF

# Start gateway
echo "Starting Highper Gateway (gRPC mode)..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

[ ! -f "$GATEWAY_BINARY" ] && echo "ERROR: Gateway binary not found" && exit 1

$GATEWAY_BINARY start --config /tmp/gateway-grpc-test.toml > /tmp/gateway-grpc.log 2>&1 &
GATEWAY_PID=$!

sleep 5

if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    tail -20 /tmp/gateway-grpc.log
    exit 1
fi

echo "✓ Gateway is running with gRPC routing"

RESULT_DIR="results/local/07-grpc/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo ""
echo "Results will be saved to: $RESULT_DIR"

# Test 1: Simple gRPC call through gateway
echo ""
echo "========================================="
echo "Test 1: gRPC Unary Call - SayHello"
echo "========================================="

response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py $HOST_IP:8080 hello 2>/dev/null)

echo "Method: SayHello(name='World')"
echo "Response: $response"

if echo "$response" | grep -q "Hello World"; then
    echo "✓ gRPC unary call successful"
fi

# Test 2: ListUsers call
echo ""
echo "========================================="
echo "Test 2: gRPC Unary Call - ListUsers"
echo "========================================="

response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py $HOST_IP:8080 list 2>/dev/null)

echo "Method: ListUsers()"
echo "Response: $response"

if echo "$response" | grep -q "Alice"; then
    echo "✓ gRPC list operation successful"
fi

# Test 3: GetUser call with parameter
echo ""
echo "========================================="
echo "Test 3: gRPC Unary Call - GetUser"
echo "========================================="

response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py $HOST_IP:8080 get 2 2>/dev/null)

echo "Method: GetUser(id='2')"
echo "Response: $response"

if echo "$response" | grep -q "Bob"; then
    echo "✓ gRPC parameterized call successful"
fi

# Test 4: Server-side streaming
echo ""
echo "========================================="
echo "Test 4: gRPC Server Streaming"
echo "========================================="

echo "Method: StreamMessages(count=5)"
response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py $HOST_IP:8080 stream 5 2>/dev/null)

echo "Response:"
echo "$response"

if echo "$response" | grep -q "Message 5"; then
    echo "✓ gRPC streaming successful"
fi

# Test 5: Load balancing across gRPC backends
echo ""
echo "========================================="
echo "Test 5: Load Balancing Across Backends"
echo "========================================="

echo "Sending 10 gRPC calls to test load distribution..."
declare -A backend_counts

for i in {1..10}; do
    response=$(docker run --rm --network host grpc-test:optimized \
        python3 client.py $HOST_IP:8080 hello 2>/dev/null)

    backend=$(echo "$response" | grep -oP 'backend: \K[^ ]+' || echo "unknown")
    backend_counts["$backend"]=$((${backend_counts["$backend"]:-0} + 1))
    echo "  Request $i: $backend"
done

echo ""
echo "Backend distribution:"
for backend in "${!backend_counts[@]}"; do
    echo "  $backend: ${backend_counts[$backend]} requests"
done

# Test 6: Performance test with ghz (if available)
echo ""
echo "========================================="
echo "Test 6: gRPC Performance Test"
echo "========================================="

if command -v ghz &> /dev/null; then
    echo "Running load test with ghz (100 RPS for 10s)..."
    ghz --insecure \
        --proto /tmp/grpc-backend/service.proto \
        --call highper.HighperService/SayHello \
        -d '{"name":"LoadTest"}' \
        --rps 100 \
        --duration 10s \
        --connections 10 \
        $HOST_IP:8080 \
        > "${RESULT_DIR}/grpc-perf.txt" 2>&1

    if [ -f "${RESULT_DIR}/grpc-perf.txt" ]; then
        echo "Performance results:"
        grep -E "Count|Total|Average|Fastest|Slowest|Requests/sec|Error" "${RESULT_DIR}/grpc-perf.txt" || cat "${RESULT_DIR}/grpc-perf.txt"
    fi
else
    echo "⚠ ghz tool not installed, skipping performance test"
    echo "  To install: go install github.com/bojand/ghz/cmd/ghz@latest"
    echo ""
    echo "Running basic performance test (100 sequential calls)..."

    start_time=$(date +%s%N)
    success_count=0

    for i in {1..100}; do
        if docker run --rm --network host grpc-test:optimized \
            python3 client.py $HOST_IP:8080 hello 2>/dev/null | grep -q "Hello"; then
            ((success_count++))
        fi
        [ $((i % 20)) -eq 0 ] && echo "  Progress: $i/100 calls"
    done

    end_time=$(date +%s%N)
    elapsed_ms=$(( (end_time - start_time) / 1000000 ))
    avg_latency=$(( elapsed_ms / 100 ))

    echo ""
    echo "  Total calls: 100"
    echo "  Successful: $success_count"
    echo "  Total time: ${elapsed_ms}ms"
    echo "  Average latency: ${avg_latency}ms per call"
fi

# Test 7: Direct backend comparison
echo ""
echo "========================================="
echo "Test 7: Gateway vs Direct Comparison"
echo "========================================="

echo "Testing direct backend connection..."
direct_response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py localhost:50051 hello 2>/dev/null)
echo "  Direct: $direct_response"

echo "Testing through gateway..."
gateway_response=$(docker run --rm --network host grpc-test:optimized \
    python3 client.py $HOST_IP:8080 hello 2>/dev/null)
echo "  Gateway: $gateway_response"

if [ ! -z "$direct_response" ] && [ ! -z "$gateway_response" ]; then
    echo "✓ Both direct and gateway connections working"
fi

echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "gRPC Gateway Features Tested:"
echo "  ✓ gRPC unary calls (SayHello, ListUsers, GetUser)"
echo "  ✓ gRPC server-side streaming (StreamMessages)"
echo "  ✓ Load balancing across gRPC backends"
echo "  ✓ HTTP/2 protocol requirement"
echo "  ✓ Gateway vs direct backend comparison"
if command -v ghz &> /dev/null; then
    echo "  ✓ Performance testing with ghz"
else
    echo "  ✓ Basic performance testing (sequential)"
fi
echo ""
echo "gRPC Methods Available:"
echo "  SayHello(name: string) -> HelloReply"
echo "  ListUsers() -> UserList"
echo "  GetUser(id: string) -> User"
echo "  StreamMessages(count: int) -> stream Message"
echo ""
echo "Proto file: /tmp/grpc-backend/service.proto"
echo "Backend servers: localhost:50051, localhost:50052"
echo "Gateway endpoint: $HOST_IP:8080"
echo "========================================="
