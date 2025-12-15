#!/usr/bin/env python3
"""
Mock WebSocket Backend Server for Testing

This server provides a simple WebSocket echo backend for testing
the highper-gateway WebSocket proxy functionality.

Features:
- Echo back received messages
- Support for text and binary frames
- Automatic pong responses to pings
- Configurable port and delays
- Connection tracking and statistics
"""

import asyncio
import argparse
import logging
import signal
import sys
from datetime import datetime
from typing import Set
import websockets
from websockets.server import WebSocketServerProtocol

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger('mock_backend')

# Global connection tracking
active_connections: Set[WebSocketServerProtocol] = set()
message_count = 0
byte_count = 0


async def echo_handler(websocket: WebSocketServerProtocol, path: str):
    """Handle WebSocket connections with echo functionality"""
    global message_count, byte_count

    client_addr = f"{websocket.remote_address[0]}:{websocket.remote_address[1]}"
    logger.info(f"New connection from {client_addr} (path: {path})")

    active_connections.add(websocket)
    connection_start = datetime.now()
    local_message_count = 0
    local_byte_count = 0

    try:
        async for message in websocket:
            message_count += 1
            local_message_count += 1

            if isinstance(message, bytes):
                byte_count += len(message)
                local_byte_count += len(message)
                logger.debug(f"Received binary message ({len(message)} bytes) from {client_addr}")

                # Echo back binary message
                await websocket.send(message)
            else:
                byte_count += len(message.encode('utf-8'))
                local_byte_count += len(message.encode('utf-8'))
                logger.debug(f"Received text message: {message[:50]}... from {client_addr}")

                # Echo back text message
                await websocket.send(message)

    except websockets.exceptions.ConnectionClosed as e:
        logger.info(f"Connection closed by {client_addr}: {e.code} {e.reason}")

    except Exception as e:
        logger.error(f"Error handling connection from {client_addr}: {e}", exc_info=True)

    finally:
        active_connections.discard(websocket)
        duration = (datetime.now() - connection_start).total_seconds()

        logger.info(
            f"Connection from {client_addr} ended "
            f"(duration: {duration:.2f}s, messages: {local_message_count}, "
            f"bytes: {local_byte_count})"
        )


async def stats_reporter():
    """Periodically report server statistics"""
    while True:
        await asyncio.sleep(10)
        logger.info(
            f"Stats: {len(active_connections)} active connections, "
            f"{message_count} total messages, "
            f"{byte_count} total bytes"
        )


async def main(host: str, port: int):
    """Start the WebSocket server"""
    logger.info(f"Starting WebSocket echo server on {host}:{port}")

    # Start the stats reporter
    stats_task = asyncio.create_task(stats_reporter())

    # Start the WebSocket server
    async with websockets.serve(
        echo_handler,
        host,
        port,
        ping_interval=30,
        ping_timeout=10,
        max_size=16 * 1024 * 1024,  # 16 MB max message size
        compression=None,  # Disable compression for testing
    ):
        logger.info(f"Server ready to accept connections on ws://{host}:{port}")

        # Wait forever
        await asyncio.Future()


def signal_handler(sig, frame):
    """Handle shutdown signals"""
    logger.info(f"Received signal {sig}, shutting down...")
    logger.info(
        f"Final stats: {message_count} total messages, "
        f"{byte_count} total bytes processed"
    )
    sys.exit(0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Mock WebSocket Backend Server")
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="Host to bind to (default: 127.0.0.1)"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8081,
        help="Port to bind to (default: 8081)"
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose debug logging"
    )

    args = parser.parse_args()

    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)

    # Register signal handlers
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    try:
        asyncio.run(main(args.host, args.port))
    except KeyboardInterrupt:
        logger.info("Shutting down...")
        logger.info(
            f"Final stats: {message_count} total messages, "
            f"{byte_count} total bytes processed"
        )
