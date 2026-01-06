#!/usr/bin/env node
/**
 * Node.js API Backend for Highper Gateway Load Testing
 * Supports: REST API, WebSocket, GraphQL scenarios
 */

const http = require('http');
const url = require('url');
const os = require('os');

const PORT = process.env.PORT || 3000;
const HOSTNAME = os.hostname();

// Simple in-memory store for testing
const dataStore = new Map();
let requestCount = 0;

/**
 * Request handler
 */
function handleRequest(req, res) {
  requestCount++;

  const parsedUrl = url.parse(req.url, true);
  const pathname = parsedUrl.pathname;
  const method = req.method;

  // CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, X-Requested-With');

  // Add backend identification
  res.setHeader('X-Backend-Host', HOSTNAME);
  res.setHeader('X-Request-ID', `${HOSTNAME}-${requestCount}`);

  // Handle OPTIONS (CORS preflight)
  if (method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  // Health check
  if (pathname === '/health') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      status: 'healthy',
      service: 'node-api-backend',
      hostname: HOSTNAME,
      uptime: process.uptime(),
      memory: process.memoryUsage(),
      requests: requestCount
    }));
    return;
  }

  // API v1 endpoints
  if (pathname.startsWith('/v1/')) {
    handleApiV1(req, res, pathname, method);
    return;
  }

  // GraphQL endpoint
  if (pathname === '/graphql') {
    handleGraphQL(req, res);
    return;
  }

  // WebSocket upgrade (basic handling)
  if (req.headers.upgrade === 'websocket') {
    res.writeHead(426, { 'Content-Type': 'text/plain' });
    res.end('WebSocket upgrade not supported in this simple server\n');
    return;
  }

  // Default: 404
  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Not found', path: pathname }));
}

/**
 * Handle API v1 endpoints
 */
function handleApiV1(req, res, pathname, method) {
  const path = pathname.replace('/v1/', '');

  // GET /v1/users
  if (path === 'users' && method === 'GET') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      users: Array.from(dataStore.entries()).map(([id, data]) => ({ id, ...data })),
      total: dataStore.size
    }));
    return;
  }

  // POST /v1/users
  if (path === 'users' && method === 'POST') {
    let body = '';
    req.on('data', chunk => { body += chunk.toString(); });
    req.on('end', () => {
      try {
        const data = JSON.parse(body);
        const id = `user-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
        dataStore.set(id, data);

        res.writeHead(201, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ id, ...data }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Invalid JSON' }));
      }
    });
    return;
  }

  // GET /v1/users/:id
  const userMatch = path.match(/^users\/(.+)$/);
  if (userMatch && method === 'GET') {
    const userId = userMatch[1];
    if (dataStore.has(userId)) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ id: userId, ...dataStore.get(userId) }));
    } else {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'User not found' }));
    }
    return;
  }

  // PUT /v1/users/:id
  if (userMatch && method === 'PUT') {
    const userId = userMatch[1];
    let body = '';
    req.on('data', chunk => { body += chunk.toString(); });
    req.on('end', () => {
      try {
        const data = JSON.parse(body);
        dataStore.set(userId, data);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ id: userId, ...data }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Invalid JSON' }));
      }
    });
    return;
  }

  // DELETE /v1/users/:id
  if (userMatch && method === 'DELETE') {
    const userId = userMatch[1];
    if (dataStore.has(userId)) {
      dataStore.delete(userId);
      res.writeHead(204);
      res.end();
    } else {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'User not found' }));
    }
    return;
  }

  // GET /v1/ping
  if (path === 'ping' && method === 'GET') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      message: 'pong',
      timestamp: Date.now(),
      backend: HOSTNAME
    }));
    return;
  }

  // Default: Method not allowed
  res.writeHead(405, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Method not allowed' }));
}

/**
 * Handle GraphQL endpoint
 */
function handleGraphQL(req, res) {
  if (req.method === 'GET') {
    // GraphQL GET request (introspection query via URL param)
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      data: {
        __schema: {
          queryType: { name: 'Query' },
          mutationType: { name: 'Mutation' },
          types: []
        }
      }
    }));
    return;
  }

  if (req.method === 'POST') {
    let body = '';
    req.on('data', chunk => { body += chunk.toString(); });
    req.on('end', () => {
      try {
        const { query, variables } = JSON.parse(body);

        // Simple GraphQL response
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          data: {
            users: Array.from(dataStore.entries()).map(([id, data]) => ({ id, ...data })),
            backend: HOSTNAME,
            timestamp: Date.now()
          }
        }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          errors: [{ message: 'Invalid GraphQL request' }]
        }));
      }
    });
    return;
  }

  res.writeHead(405, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({ error: 'Method not allowed' }));
}

/**
 * Start server
 */
const server = http.createServer(handleRequest);

server.listen(PORT, '0.0.0.0', () => {
  console.log(`[${new Date().toISOString()}] API backend listening on http://0.0.0.0:${PORT}`);
  console.log(`[${new Date().toISOString()}] Hostname: ${HOSTNAME}`);
  console.log(`[${new Date().toISOString()}] Process ID: ${process.pid}`);
  console.log(`[${new Date().toISOString()}] Node version: ${process.version}`);
});

// Graceful shutdown
process.on('SIGTERM', () => {
  console.log(`[${new Date().toISOString()}] SIGTERM received, shutting down gracefully`);
  server.close(() => {
    console.log(`[${new Date().toISOString()}] Server closed`);
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  console.log(`[${new Date().toISOString()}] SIGINT received, shutting down gracefully`);
  server.close(() => {
    console.log(`[${new Date().toISOString()}] Server closed`);
    process.exit(0);
  });
});
