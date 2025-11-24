#!/usr/bin/env node
/**
 * Simple HTTP backend server for load testing
 *
 * This server responds immediately with minimal overhead,
 * suitable for benchmarking the proxy performance.
 */

const http = require('http');
const port = process.env.PORT || 9000;

// Simple response
const responseBody = JSON.stringify({
  status: 'ok',
  timestamp: Date.now(),
  message: 'Test backend server'
});

const server = http.createServer((req, res) => {
  res.writeHead(200, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(responseBody)
  });
  res.end(responseBody);
});

// Handle shutdown gracefully
process.on('SIGTERM', () => {
  console.log('SIGTERM received, shutting down gracefully');
  server.close(() => {
    console.log('Server closed');
    process.exit(0);
  });
});

process.on('SIGINT', () => {
  console.log('SIGINT received, shutting down gracefully');
  server.close(() => {
    console.log('Server closed');
    process.exit(0);
  });
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Backend server listening on http://127.0.0.1:${port}`);
  console.log(`PID: ${process.pid}`);
});
