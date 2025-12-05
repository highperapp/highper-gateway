#!/usr/bin/env python3
"""
Simple HTTP backend server for local testing
Responds with JSON including backend identification
"""
import sys
import socket
import json
from http.server import HTTPServer, BaseHTTPRequestHandler
from datetime import datetime

class BackendHandler(BaseHTTPRequestHandler):
    # Request counter
    request_count = 0

    def do_GET(self):
        BackendHandler.request_count += 1

        backend_id = f"backend-{self.server.server_port}"
        response_data = {
            "backend": backend_id,
            "port": self.server.server_port,
            "status": "ok",
            "request_count": BackendHandler.request_count,
            "timestamp": datetime.now().isoformat(),
            "path": self.path,
            "method": "GET"
        }

        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Server', backend_id)
        self.end_headers()
        self.wfile.write(json.dumps(response_data).encode() + b'\n')

    def do_POST(self):
        content_length = int(self.headers.get('Content-Length', 0))
        post_data = self.rfile.read(content_length) if content_length > 0 else b''

        BackendHandler.request_count += 1

        backend_id = f"backend-{self.server.server_port}"
        response_data = {
            "backend": backend_id,
            "port": self.server.server_port,
            "status": "ok",
            "request_count": BackendHandler.request_count,
            "timestamp": datetime.now().isoformat(),
            "path": self.path,
            "method": "POST",
            "received_bytes": len(post_data)
        }

        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Server', backend_id)
        self.end_headers()
        self.wfile.write(json.dumps(response_data).encode() + b'\n')

    def log_message(self, format, *args):
        # Suppress default logging for performance
        pass

if __name__ == '__main__':
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8081

    try:
        server = HTTPServer(('127.0.0.1', port), BackendHandler)
        print(f"✓ Backend server started on 127.0.0.1:{port}")
        print(f"  Process PID: {socket.gethostname()}")
        sys.stdout.flush()
        server.serve_forever()
    except KeyboardInterrupt:
        print(f"\n✓ Backend on port {port} shutting down...")
        server.shutdown()
    except OSError as e:
        if e.errno == 98:  # Address already in use
            print(f"✗ Error: Port {port} is already in use")
            sys.exit(1)
        else:
            raise
