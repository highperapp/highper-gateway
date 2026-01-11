#!/usr/bin/env python3
"""Simple HTTP server for testing the reverse proxy"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import sys

class TestHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        # Send response
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()

        response = {
            'message': 'Hello from test backend!',
            'path': self.path,
            'method': 'GET',
            'server': 'test_backend'
        }

        self.wfile.write(json.dumps(response).encode())

    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_length)

        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()

        response = {
            'message': 'POST received',
            'path': self.path,
            'method': 'POST',
            'body_length': content_length,
            'server': 'test_backend'
        }

        self.wfile.write(json.dumps(response).encode())

    def log_message(self, format, *args):
        sys.stderr.write(f"[Backend] {format % args}\n")

if __name__ == '__main__':
    port = 3000
    server = HTTPServer(('localhost', port), TestHandler)
    print(f"Test backend server running on http://localhost:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down test backend...")
        server.shutdown()
