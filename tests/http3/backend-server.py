#!/usr/bin/env python3
"""
Simple HTTP backend server for HTTP/3 testing.
Returns different responses on different ports for testing load balancing.
"""

import http.server
import socketserver
import json
import os
import sys
from datetime import datetime

PORT = int(os.getenv('PORT', 9001))
SERVER_ID = os.getenv('SERVER_ID', 'backend-1')

class TestHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        """Handle GET requests"""

        # Health check endpoint
        if self.path == '/health':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            response = {
                'status': 'healthy',
                'server_id': SERVER_ID,
                'port': PORT,
                'timestamp': datetime.now().isoformat()
            }
            self.wfile.write(json.dumps(response).encode())
            return

        # Echo endpoint - returns request info
        if self.path == '/echo':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()

            response = {
                'server_id': SERVER_ID,
                'port': PORT,
                'method': self.command,
                'path': self.path,
                'headers': dict(self.headers),
                'client_address': str(self.client_address),
                'timestamp': datetime.now().isoformat()
            }
            self.wfile.write(json.dumps(response, indent=2).encode())
            return

        # Default response
        self.send_response(200)
        self.send_header('Content-Type', 'text/plain')
        self.send_header('X-Server-ID', SERVER_ID)
        self.send_header('X-Server-Port', str(PORT))
        self.end_headers()

        response = f"Hello from {SERVER_ID} (port {PORT})\n"
        response += f"Request path: {self.path}\n"
        response += f"Timestamp: {datetime.now().isoformat()}\n"

        self.wfile.write(response.encode())

    def do_POST(self):
        """Handle POST requests"""
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length)

        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('X-Server-ID', SERVER_ID)
        self.end_headers()

        response = {
            'server_id': SERVER_ID,
            'port': PORT,
            'method': 'POST',
            'path': self.path,
            'body_length': content_length,
            'body': body.decode('utf-8', errors='replace') if content_length < 1024 else '<truncated>',
            'timestamp': datetime.now().isoformat()
        }

        self.wfile.write(json.dumps(response, indent=2).encode())

    def log_message(self, format, *args):
        """Custom log format"""
        sys.stderr.write(f"[{SERVER_ID}] {format%args}\n")

if __name__ == '__main__':
    with socketserver.TCPServer(("", PORT), TestHandler) as httpd:
        print(f"[{SERVER_ID}] HTTP server listening on port {PORT}")
        print(f"[{SERVER_ID}] Endpoints: /, /health, /echo")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print(f"\n[{SERVER_ID}] Server stopped")
            sys.exit(0)
