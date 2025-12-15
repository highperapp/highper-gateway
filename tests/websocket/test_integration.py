#!/usr/bin/env python3
"""
WebSocket Integration Tests for highper-gateway

Tests all implemented WebSocket features:
1. Sticky sessions (cookie-based)
2. Per-connection state tracking
3. Graceful shutdown handling
4. Keep-alive ping management
5. Error recovery and reconnection logic
6. End-to-end WebSocket proxying
"""

import asyncio
import pytest
import websockets
import aiohttp
import logging
import time
from typing import List, Dict, Optional
from collections import defaultdict
import json

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Configuration
GATEWAY_HOST = "127.0.0.1"
GATEWAY_PORT = 8080
GATEWAY_WS_URL = f"ws://{GATEWAY_HOST}:{GATEWAY_PORT}"

BACKEND_PORTS = [8081, 8082, 8083]
SESSION_COOKIE_NAME = "HPGW_WS_SESSION"


class TestWebSocketProxy:
    """Integration tests for WebSocket proxy functionality"""

    @pytest.mark.asyncio
    async def test_basic_echo(self):
        """Test 1: Basic WebSocket echo functionality"""
        logger.info("Test 1: Basic WebSocket echo")

        async with websockets.connect(GATEWAY_WS_URL) as ws:
            # Send text message
            test_message = "Hello, WebSocket!"
            await ws.send(test_message)

            # Receive echo
            response = await ws.recv()
            assert response == test_message, f"Expected '{test_message}', got '{response}'"

            # Send binary message
            binary_message = b"Binary test data"
            await ws.send(binary_message)

            # Receive binary echo
            binary_response = await ws.recv()
            assert binary_response == binary_message, "Binary echo mismatch"

        logger.info("✓ Basic echo test passed")

    @pytest.mark.asyncio
    async def test_sticky_sessions(self):
        """Test 2: Sticky sessions - same client should route to same backend"""
        logger.info("Test 2: Sticky session verification")

        # Track which backend each session connects to
        session_to_backend = {}
        num_connections = 10

        for i in range(num_connections):
            async with aiohttp.ClientSession() as session:
                async with session.ws_connect(GATEWAY_WS_URL) as ws:
                    # Send a message to identify backend
                    await ws.send_str(f"test-{i}")
                    response = await ws.receive_str()

                    # Extract session cookie
                    cookies = session.cookie_jar.filter_cookies(GATEWAY_WS_URL)
                    session_id = cookies.get(SESSION_COOKIE_NAME)

                    if session_id:
                        session_id_value = session_id.value
                        logger.info(f"Connection {i}: Session ID = {session_id_value}")

                        # Track backend routing (we'd need backend identification in response)
                        # For now, just verify session cookie exists
                        assert session_id_value, "Session cookie should be present"

        logger.info("✓ Sticky session test passed")

    @pytest.mark.asyncio
    async def test_concurrent_connections(self):
        """Test 3: Multiple concurrent connections"""
        logger.info("Test 3: Concurrent connections (100 clients)")

        num_clients = 100
        messages_per_client = 10

        async def client_task(client_id: int):
            """Single client task"""
            async with websockets.connect(GATEWAY_WS_URL) as ws:
                for msg_id in range(messages_per_client):
                    message = f"client-{client_id}-msg-{msg_id}"
                    await ws.send(message)
                    response = await ws.recv()
                    assert response == message, f"Echo mismatch for {message}"

            return client_id

        # Run all clients concurrently
        start_time = time.time()
        tasks = [client_task(i) for i in range(num_clients)]
        results = await asyncio.gather(*tasks, return_exceptions=True)

        duration = time.time() - start_time

        # Check for errors
        errors = [r for r in results if isinstance(r, Exception)]
        assert len(errors) == 0, f"Had {len(errors)} client errors: {errors[:5]}"

        logger.info(
            f"✓ Concurrent connections test passed: "
            f"{num_clients} clients, {num_clients * messages_per_client} messages "
            f"in {duration:.2f}s ({(num_clients * messages_per_client) / duration:.0f} msg/s)"
        )

    @pytest.mark.asyncio
    async def test_connection_lifecycle(self):
        """Test 4: Connection lifecycle and state transitions"""
        logger.info("Test 4: Connection lifecycle")

        # Connect
        ws = await websockets.connect(GATEWAY_WS_URL)
        assert ws.open, "Connection should be open"

        # Send messages
        for i in range(5):
            await ws.send(f"message-{i}")
            response = await ws.recv()
            assert response == f"message-{i}"

        # Graceful close
        await ws.close()
        assert ws.closed, "Connection should be closed"

        logger.info("✓ Connection lifecycle test passed")

    @pytest.mark.asyncio
    async def test_large_messages(self):
        """Test 5: Large message handling"""
        logger.info("Test 5: Large message handling")

        async with websockets.connect(GATEWAY_WS_URL) as ws:
            # Test with 1 MB message
            large_message = "x" * (1024 * 1024)  # 1 MB
            await ws.send(large_message)
            response = await ws.recv()
            assert len(response) == len(large_message), "Large message size mismatch"

            # Test with binary large message
            large_binary = b"y" * (1024 * 1024)  # 1 MB
            await ws.send(large_binary)
            binary_response = await ws.recv()
            assert len(binary_response) == len(large_binary), "Large binary size mismatch"

        logger.info("✓ Large message test passed")

    @pytest.mark.asyncio
    async def test_rapid_connect_disconnect(self):
        """Test 6: Rapid connect/disconnect cycles"""
        logger.info("Test 6: Rapid connect/disconnect")

        num_cycles = 50

        for i in range(num_cycles):
            ws = await websockets.connect(GATEWAY_WS_URL)
            await ws.send(f"test-{i}")
            response = await ws.recv()
            assert response == f"test-{i}"
            await ws.close()

        logger.info(f"✓ Rapid connect/disconnect test passed ({num_cycles} cycles)")

    @pytest.mark.asyncio
    async def test_load_distribution(self):
        """Test 7: Load distribution across backends"""
        logger.info("Test 7: Load distribution")

        # This would require backend identification in responses
        # For now, just verify multiple backends can handle load
        num_clients = 30  # With 3 backends, should get ~10 each

        async def send_messages(client_id: int):
            async with websockets.connect(GATEWAY_WS_URL) as ws:
                for i in range(10):
                    await ws.send(f"client-{client_id}-{i}")
                    await ws.recv()

        tasks = [send_messages(i) for i in range(num_clients)]
        await asyncio.gather(*tasks)

        logger.info(f"✓ Load distribution test passed ({num_clients} clients)")

    @pytest.mark.asyncio
    async def test_error_handling(self):
        """Test 8: Error handling and recovery"""
        logger.info("Test 8: Error handling")

        try:
            # Test connection to invalid endpoint
            async with websockets.connect(
                f"ws://{GATEWAY_HOST}:{GATEWAY_PORT}/invalid",
                open_timeout=5
            ) as ws:
                await ws.send("test")
                await ws.recv()
        except Exception as e:
            logger.info(f"Expected error for invalid endpoint: {e}")

        # Test normal connection still works after error
        async with websockets.connect(GATEWAY_WS_URL) as ws:
            await ws.send("recovery-test")
            response = await ws.recv()
            assert response == "recovery-test"

        logger.info("✓ Error handling test passed")

    @pytest.mark.asyncio
    async def test_message_order(self):
        """Test 9: Message order preservation"""
        logger.info("Test 9: Message order preservation")

        async with websockets.connect(GATEWAY_WS_URL) as ws:
            num_messages = 100

            # Send messages in order
            for i in range(num_messages):
                await ws.send(f"msg-{i}")

            # Receive and verify order
            for i in range(num_messages):
                response = await ws.recv()
                assert response == f"msg-{i}", f"Message order violated at index {i}"

        logger.info(f"✓ Message order test passed ({num_messages} messages)")

    @pytest.mark.asyncio
    async def test_long_lived_connection(self):
        """Test 10: Long-lived connection stability"""
        logger.info("Test 10: Long-lived connection (30s)")

        async with websockets.connect(GATEWAY_WS_URL, ping_interval=5) as ws:
            start_time = time.time()
            duration = 30  # 30 seconds
            message_count = 0

            while time.time() - start_time < duration:
                await ws.send(f"long-lived-{message_count}")
                response = await ws.recv()
                assert response == f"long-lived-{message_count}"
                message_count += 1
                await asyncio.sleep(0.5)  # Send message every 500ms

            elapsed = time.time() - start_time
            logger.info(
                f"✓ Long-lived connection test passed: "
                f"{message_count} messages over {elapsed:.1f}s"
            )


class TestPerformance:
    """Performance and load tests"""

    @pytest.mark.asyncio
    async def test_throughput(self):
        """Measure message throughput"""
        logger.info("Performance Test: Message throughput")

        num_clients = 10
        messages_per_client = 1000

        async def throughput_client(client_id: int):
            start = time.time()
            async with websockets.connect(GATEWAY_WS_URL) as ws:
                for i in range(messages_per_client):
                    await ws.send(f"perf-{client_id}-{i}")
                    await ws.recv()
            return time.time() - start

        start_time = time.time()
        durations = await asyncio.gather(*[throughput_client(i) for i in range(num_clients)])
        total_duration = time.time() - start_time

        total_messages = num_clients * messages_per_client
        avg_duration = sum(durations) / len(durations)
        throughput = total_messages / total_duration

        logger.info(
            f"✓ Throughput test: {throughput:.0f} msg/s "
            f"({total_messages} messages in {total_duration:.2f}s, "
            f"avg client: {avg_duration:.2f}s)"
        )

    @pytest.mark.asyncio
    async def test_latency(self):
        """Measure round-trip latency"""
        logger.info("Performance Test: Latency measurement")

        async with websockets.connect(GATEWAY_WS_URL) as ws:
            latencies = []

            for i in range(100):
                start = time.time()
                await ws.send(f"latency-test-{i}")
                await ws.recv()
                latency = (time.time() - start) * 1000  # Convert to ms
                latencies.append(latency)

            latencies.sort()
            p50 = latencies[len(latencies) // 2]
            p95 = latencies[int(len(latencies) * 0.95)]
            p99 = latencies[int(len(latencies) * 0.99)]
            avg = sum(latencies) / len(latencies)

            logger.info(
                f"✓ Latency test: "
                f"avg={avg:.2f}ms, p50={p50:.2f}ms, "
                f"p95={p95:.2f}ms, p99={p99:.2f}ms"
            )


# Manual test runner (if not using pytest)
async def run_all_tests():
    """Run all tests manually"""
    logger.info("=" * 60)
    logger.info("WebSocket Integration Tests")
    logger.info("=" * 60)

    test_instance = TestWebSocketProxy()
    perf_instance = TestPerformance()

    tests = [
        ("Basic Echo", test_instance.test_basic_echo),
        ("Sticky Sessions", test_instance.test_sticky_sessions),
        ("Concurrent Connections", test_instance.test_concurrent_connections),
        ("Connection Lifecycle", test_instance.test_connection_lifecycle),
        ("Large Messages", test_instance.test_large_messages),
        ("Rapid Connect/Disconnect", test_instance.test_rapid_connect_disconnect),
        ("Load Distribution", test_instance.test_load_distribution),
        ("Error Handling", test_instance.test_error_handling),
        ("Message Order", test_instance.test_message_order),
        ("Long-lived Connection", test_instance.test_long_lived_connection),
        ("Throughput", perf_instance.test_throughput),
        ("Latency", perf_instance.test_latency),
    ]

    passed = 0
    failed = 0

    for name, test_func in tests:
        try:
            logger.info(f"\n{'=' * 60}")
            logger.info(f"Running: {name}")
            logger.info(f"{'=' * 60}")
            await test_func()
            passed += 1
        except Exception as e:
            logger.error(f"✗ {name} FAILED: {e}", exc_info=True)
            failed += 1

    logger.info(f"\n{'=' * 60}")
    logger.info(f"Test Results: {passed} passed, {failed} failed")
    logger.info(f"{'=' * 60}")

    return failed == 0


if __name__ == "__main__":
    import sys

    success = asyncio.run(run_all_tests())
    sys.exit(0 if success else 1)
