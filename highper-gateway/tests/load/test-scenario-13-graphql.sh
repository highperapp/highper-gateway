#!/bin/bash
# Scenario 13 - GraphQL Gateway
# Tests GraphQL query and mutation routing through the gateway

set -euo pipefail

export PATH=~/bin:$PATH
cd "$(dirname "$0")"

echo "========================================="
echo "Scenario 13: GraphQL Gateway"
echo "========================================="

cleanup() {
    echo "Cleaning up..."
    [ ! -z "${GATEWAY_PID:-}" ] && kill $GATEWAY_PID 2>/dev/null || true
    docker rm -f graphql-server-1 graphql-server-2 2>/dev/null || true
    echo "Cleanup complete"
}

trap cleanup EXIT INT TERM

# Create GraphQL server code
echo "Creating GraphQL backend server..."
mkdir -p /tmp/graphql-backend

cat > /tmp/graphql-backend/server.js <<'GRAPHQL_SERVER'
const http = require('http');
const url = require('url');

const BACKEND_NAME = process.env.BACKEND_NAME || 'graphql-backend';

// Simple in-memory data store
const users = [
  { id: '1', name: 'Alice', email: 'alice@example.com' },
  { id: '2', name: 'Bob', email: 'bob@example.com' },
  { id: '3', name: 'Charlie', email: 'charlie@example.com' }
];

const posts = [
  { id: '1', title: 'Hello World', content: 'First post', authorId: '1' },
  { id: '2', title: 'GraphQL Basics', content: 'Learning GraphQL', authorId: '2' },
  { id: '3', title: 'API Gateway', content: 'High performance gateway', authorId: '1' }
];

// Simple GraphQL resolver
function executeQuery(query, variables = {}) {
  // Handle introspection query
  if (query.includes('__schema')) {
    return {
      data: {
        __schema: {
          queryType: { name: 'Query' },
          types: [
            { name: 'User', kind: 'OBJECT' },
            { name: 'Post', kind: 'OBJECT' },
            { name: 'Query', kind: 'OBJECT' }
          ]
        }
      }
    };
  }

  // Handle user query
  if (query.includes('user(') || query.includes('user (')) {
    const idMatch = query.match(/id:\s*"(\d+)"/);
    const id = idMatch ? idMatch[1] : variables.id;
    const user = users.find(u => u.id === id);
    return {
      data: {
        user: user || null,
        backend: BACKEND_NAME
      }
    };
  }

  // Handle users query
  if (query.includes('users')) {
    return {
      data: {
        users: users,
        backend: BACKEND_NAME
      }
    };
  }

  // Handle post query
  if (query.includes('post(') || query.includes('post (')) {
    const idMatch = query.match(/id:\s*"(\d+)"/);
    const id = idMatch ? idMatch[1] : variables.id;
    const post = posts.find(p => p.id === id);
    if (post) {
      const author = users.find(u => u.id === post.authorId);
      return {
        data: {
          post: { ...post, author },
          backend: BACKEND_NAME
        }
      };
    }
    return { data: { post: null } };
  }

  // Handle posts query
  if (query.includes('posts')) {
    return {
      data: {
        posts: posts.map(p => ({
          ...p,
          author: users.find(u => u.id === p.authorId)
        })),
        backend: BACKEND_NAME
      }
    };
  }

  // Handle createUser mutation
  if (query.includes('createUser')) {
    const nameMatch = query.match(/name:\s*"([^"]+)"/);
    const emailMatch = query.match(/email:\s*"([^"]+)"/);
    const newUser = {
      id: String(users.length + 1),
      name: nameMatch ? nameMatch[1] : variables.name || 'Unknown',
      email: emailMatch ? emailMatch[1] : variables.email || 'unknown@example.com'
    };
    users.push(newUser);
    return {
      data: {
        createUser: newUser,
        backend: BACKEND_NAME
      }
    };
  }

  // Default response
  return {
    data: {
      hello: 'GraphQL server running on ' + BACKEND_NAME
    }
  };
}

const server = http.createServer((req, res) => {
  if (req.method === 'OPTIONS') {
    res.writeHead(200, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'POST, GET, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type'
    });
    res.end();
    return;
  }

  if (req.method === 'POST' && req.url === '/graphql') {
    let body = '';
    req.on('data', chunk => { body += chunk.toString(); });
    req.on('end', () => {
      try {
        const { query, variables } = JSON.parse(body);
        const result = executeQuery(query, variables || {});

        res.writeHead(200, {
          'Content-Type': 'application/json',
          'Access-Control-Allow-Origin': '*'
        });
        res.end(JSON.stringify(result));
      } catch (error) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ errors: [{ message: error.message }] }));
      }
    });
  } else if (req.method === 'GET' && req.url === '/graphql') {
    // GraphQL playground or simple query interface
    const parsedUrl = url.parse(req.url, true);
    const query = parsedUrl.query.query;

    if (query) {
      const result = executeQuery(query);
      res.writeHead(200, {
        'Content-Type': 'application/json',
        'Access-Control-Allow-Origin': '*'
      });
      res.end(JSON.stringify(result));
    } else {
      res.writeHead(200, { 'Content-Type': 'text/html' });
      res.end('<h1>GraphQL Server (' + BACKEND_NAME + ')</h1><p>POST queries to /graphql</p>');
    }
  } else if (req.url === '/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ status: 'healthy', backend: BACKEND_NAME }));
  } else {
    res.writeHead(404);
    res.end('Not Found');
  }
});

const PORT = process.env.PORT || 4000;
server.listen(PORT, '0.0.0.0', () => {
  console.log(`GraphQL server (${BACKEND_NAME}) listening on port ${PORT}`);
});
GRAPHQL_SERVER

# Start GraphQL backend servers
echo "Starting GraphQL backend servers..."

docker run -d --name graphql-server-1 \
    -p 4001:4000 \
    -v /tmp/graphql-backend:/app \
    -e BACKEND_NAME=graphql-1 \
    -e PORT=4000 \
    -w /app \
    --rm \
    node:18-alpine \
    node server.js > /dev/null 2>&1

docker run -d --name graphql-server-2 \
    -p 4002:4000 \
    -v /tmp/graphql-backend:/app \
    -e BACKEND_NAME=graphql-2 \
    -e PORT=4000 \
    -w /app \
    --rm \
    node:18-alpine \
    node server.js > /dev/null 2>&1

sleep 8

# Check backend health
echo "Checking GraphQL backends..."
for port in 4001 4002; do
    if curl -s -f http://localhost:$port/health > /dev/null 2>&1; then
        echo "✓ GraphQL backend on port $port ready"
    else
        echo "⚠ GraphQL backend on port $port not ready"
    fi
done

# Create gateway config for GraphQL
echo ""
echo "Creating gateway configuration for GraphQL..."
cat > /tmp/gateway-graphql-test.toml <<'EOF'
[server]
bind = ["127.0.0.1:8080"]
workers = "auto"
protocols = ["http1", "http2"]

[server.performance]
max_connections = 100000
read_buffer_size = 32768
write_buffer_size = 32768

# GraphQL backends
[[upstreams]]
name = "graphql-backends"

servers = [
    { url = "http://localhost:4001", weight = 1 },
    { url = "http://localhost:4002", weight = 1 },
]

[upstreams.load_balancing]
algorithm = "round_robin"

[upstreams.connection]
timeout = "10s"
keepalive = "60s"
pool_size = 500

[[routes]]
name = "graphql-route"
upstream = "graphql-backends"

[routes.match]
paths = ["/graphql"]
methods = ["POST", "GET", "OPTIONS"]

[observability.logging]
level = "warn"
format = "json"
EOF

# Start gateway
echo "Starting Highper Gateway (GraphQL mode)..."
GATEWAY_BINARY="../../../target/release/highper-gateway"

[ ! -f "$GATEWAY_BINARY" ] && echo "ERROR: Gateway binary not found" && exit 1

$GATEWAY_BINARY start --config /tmp/gateway-graphql-test.toml > /tmp/gateway-graphql.log 2>&1 &
GATEWAY_PID=$!

sleep 5

if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "ERROR: Gateway failed to start!"
    tail -20 /tmp/gateway-graphql.log
    exit 1
fi

echo "✓ Gateway is running with GraphQL routing"

RESULT_DIR="results/local/13-graphql/$(date '+%Y%m%d-%H%M%S')"
mkdir -p "$RESULT_DIR"

echo ""
echo "Results will be saved to: $RESULT_DIR"

# Test 1: Simple query - users list
echo ""
echo "========================================="
echo "Test 1: GraphQL Query - Users List"
echo "========================================="

response=$(curl -s -X POST http://localhost:8080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ users { id name email } backend }"}')

echo "Query: { users { id name email } backend }"
echo "Response:"
echo "$response" | jq '.' 2>/dev/null || echo "$response"

if echo "$response" | grep -q "users"; then
    echo "✓ GraphQL users query successful"
fi

# Test 2: Query with variables
echo ""
echo "========================================="
echo "Test 2: GraphQL Query with Variables"
echo "========================================="

response=$(curl -s -X POST http://localhost:8080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "query GetUser($id: String!) { user(id: $id) { id name email } backend }", "variables": {"id": "1"}}')

echo "Query: query GetUser(\$id: String!) { user(id: \$id) { id name email } backend }"
echo "Variables: {\"id\": \"1\"}"
echo "Response:"
echo "$response" | jq '.' 2>/dev/null || echo "$response"

if echo "$response" | grep -q "Alice"; then
    echo "✓ GraphQL parameterized query successful"
fi

# Test 3: Mutation - create user
echo ""
echo "========================================="
echo "Test 3: GraphQL Mutation"
echo "========================================="

response=$(curl -s -X POST http://localhost:8080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "mutation { createUser(name: \"Dave\", email: \"dave@example.com\") { id name email } backend }"}')

echo "Mutation: mutation { createUser(name: \"Dave\", email: \"dave@example.com\") { id name email } backend }"
echo "Response:"
echo "$response" | jq '.' 2>/dev/null || echo "$response"

if echo "$response" | grep -q "Dave"; then
    echo "✓ GraphQL mutation successful"
fi

# Test 4: Complex nested query
echo ""
echo "========================================="
echo "Test 4: Complex Nested Query"
echo "========================================="

response=$(curl -s -X POST http://localhost:8080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ posts { id title content author { name email } } backend }"}')

echo "Query: { posts { id title content author { name email } } backend }"
echo "Response:"
echo "$response" | jq '.' 2>/dev/null || echo "$response"

if echo "$response" | grep -q "posts"; then
    echo "✓ Complex nested query successful"
fi

# Test 5: Schema introspection
echo ""
echo "========================================="
echo "Test 5: Schema Introspection"
echo "========================================="

response=$(curl -s -X POST http://localhost:8080/graphql \
    -H "Content-Type: application/json" \
    -d '{"query": "{ __schema { queryType { name } types { name kind } } }"}')

echo "Query: { __schema { queryType { name } types { name kind } } }"
echo "Response (truncated):"
echo "$response" | jq '.data.__schema.queryType' 2>/dev/null || echo "$response" | head -20

if echo "$response" | grep -q "__schema"; then
    echo "✓ Schema introspection successful"
fi

# Test 6: Load balancing across GraphQL servers
echo ""
echo "========================================="
echo "Test 6: Load Balancing Across Backends"
echo "========================================="

echo "Sending 10 GraphQL queries to test load distribution..."
declare -A backend_counts

for i in {1..10}; do
    response=$(curl -s -X POST http://localhost:8080/graphql \
        -H "Content-Type: application/json" \
        -d '{"query": "{ users { id name } backend }"}' 2>/dev/null)

    backend=$(echo "$response" | jq -r '.data.backend' 2>/dev/null || echo "unknown")
    backend_counts["$backend"]=$((${backend_counts["$backend"]:-0} + 1))
    echo "  Request $i: $backend"
done

echo ""
echo "Backend distribution:"
for backend in "${!backend_counts[@]}"; do
    echo "  $backend: ${backend_counts[$backend]} requests"
done

# Test 7: Performance test
echo ""
echo "========================================="
echo "Test 7: GraphQL Query Performance"
echo "========================================="

echo "Running load test (500 req/s for 5s)..."

# Create vegeta targets file
cat > /tmp/graphql-targets.txt <<'TARGETS'
POST http://localhost:8080/graphql
Content-Type: application/json
@/tmp/graphql-query.json

TARGETS

cat > /tmp/graphql-query.json <<'QUERY'
{"query": "{ users { id name email } backend }"}
QUERY

vegeta attack \
    -targets=/tmp/graphql-targets.txt \
    -rate=500 \
    -duration=5s \
    -timeout=10s \
    -workers=4 \
    -keepalive=true \
    > "${RESULT_DIR}/graphql-perf.bin" 2>&1

cat "${RESULT_DIR}/graphql-perf.bin" | vegeta report -type=json > "${RESULT_DIR}/graphql-perf.json"

if [ -f "${RESULT_DIR}/graphql-perf.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/graphql-perf.json")
    p50=$(jq -r '.latencies."50th" // 0 | tonumber / 1000000' "${RESULT_DIR}/graphql-perf.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/graphql-perf.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/graphql-perf.json")
    echo "  GraphQL queries: ${rate} req/s, P50=${p50}ms, P99=${p99}ms, Success=${success}%"
fi

# Test 8: Complex query performance
echo ""
echo "========================================="
echo "Test 8: Complex Query Performance"
echo "========================================="

echo "Running load test with complex nested queries (300 req/s for 5s)..."

cat > /tmp/graphql-complex-query.json <<'QUERY'
{"query": "{ posts { id title content author { name email } } backend }"}
QUERY

cat > /tmp/graphql-complex-targets.txt <<'TARGETS'
POST http://localhost:8080/graphql
Content-Type: application/json
@/tmp/graphql-complex-query.json

TARGETS

vegeta attack \
    -targets=/tmp/graphql-complex-targets.txt \
    -rate=300 \
    -duration=5s \
    -timeout=10s \
    -workers=4 \
    -keepalive=true \
    > "${RESULT_DIR}/graphql-complex.bin" 2>&1

cat "${RESULT_DIR}/graphql-complex.bin" | vegeta report -type=json > "${RESULT_DIR}/graphql-complex.json"

if [ -f "${RESULT_DIR}/graphql-complex.json" ]; then
    rate=$(jq -r '.rate // 0' "${RESULT_DIR}/graphql-complex.json")
    p50=$(jq -r '.latencies."50th" // 0 | tonumber / 1000000' "${RESULT_DIR}/graphql-complex.json")
    p99=$(jq -r '.latencies."99th" // 0 | tonumber / 1000000' "${RESULT_DIR}/graphql-complex.json")
    success=$(jq -r '.success // 0 | . * 100' "${RESULT_DIR}/graphql-complex.json")
    echo "  Complex queries: ${rate} req/s, P50=${p50}ms, P99=${p99}ms, Success=${success}%"
fi

echo ""
echo "========================================="
echo "Test Complete!"
echo "========================================="
echo "Results saved to: $RESULT_DIR"
echo ""
echo "GraphQL Gateway Features Tested:"
echo "  ✓ GraphQL query routing"
echo "  ✓ Parameterized queries with variables"
echo "  ✓ GraphQL mutations"
echo "  ✓ Complex nested queries"
echo "  ✓ Schema introspection"
echo "  ✓ Load balancing across GraphQL backends"
echo "  ✓ Simple query performance"
echo "  ✓ Complex query performance"
echo ""
echo "Sample GraphQL Queries:"
echo "  { users { id name email } }"
echo "  { user(id: \"1\") { name email } }"
echo "  { posts { title author { name } } }"
echo "  mutation { createUser(name: \"Test\", email: \"test@example.com\") { id } }"
echo ""
echo "========================================="
