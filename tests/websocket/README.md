# WebSocket Integration Tests

Comprehensive integration tests for highper-gateway WebSocket proxy functionality.

## Overview

This test suite validates all WebSocket features implemented in Phase 2.1:

1. ✅ **Sticky Sessions** - Cookie-based session affinity
2. ✅ **Connection Tracking** - Per-connection state and metrics
3. ✅ **Graceful Shutdown** - Coordinated connection draining
4. ✅ **Keep-Alive** - Ping/pong monitoring
5. ✅ **Error Recovery** - Circuit breaker and retry logic
6. ✅ **End-to-End Proxying** - Full WebSocket protocol support

## Test Architecture

```
┌──────────────┐         ┌─────────────────┐         ┌──────────────┐
│              │         │                 │         │              │
│  Test Client ├────────►│ highper-gateway ├────────►│ WS Backend 1 │
│              │         │                 │         │              │
└──────────────┘         │    (Port 8080)  │         └──────────────┘
                         │                 │
                         │                 │         ┌──────────────┐
                         │                 │         │              │
                         │                 ├────────►│ WS Backend 2 │
                         │                 │         │              │
                         │                 │         └──────────────┘
                         │                 │
                         │                 │         ┌──────────────┐
                         │                 │         │              │
                         │                 ├────────►│ WS Backend 3 │
                         │                 │         │              │
                         └─────────────────┘         └──────────────┘
```

## Quick Start

### Prerequisites

- Python 3.11+
- Docker and Docker Compose (optional, for containerized testing)
- highper-gateway binary built

### Setup

1. **Install Python dependencies**:
```bash
cd tests/websocket
pip install -r requirements.txt
```

2. **Start backend servers** (choose one method):

**Method A: Using Docker Compose** (recommended):
```bash
docker-compose up -d
```

**Method B: Using Python directly**:
```bash
# Terminal 1
python3 mock_backend.py --port 8081

# Terminal 2
python3 mock_backend.py --port 8082

# Terminal 3
python3 mock_backend.py --port 8083
```

3. **Start highper-gateway**:
```bash
# From project root
cargo build --release
./target/release/highper-gateway --config tests/websocket/test_config.toml
```

4. **Run tests**:
```bash
# Using pytest
pytest test_integration.py -v

# Or run manually
python3 test_integration.py
```

## Test Scenarios

### Functional Tests

#### 1. Basic Echo Test
- **Purpose**: Verify basic WebSocket communication
- **Method**: Send text and binary messages, verify echo
- **Success Criteria**: All messages echoed correctly

#### 2. Sticky Sessions Test
- **Purpose**: Verify session affinity across multiple connections
- **Method**: Multiple connections with same session cookie route to same backend
- **Success Criteria**: Session cookies present, affinity maintained

#### 3. Concurrent Connections Test
- **Purpose**: Test handling of multiple simultaneous connections
- **Method**: 100 clients sending 10 messages each concurrently
- **Success Criteria**: All messages delivered correctly, no errors

#### 4. Connection Lifecycle Test
- **Purpose**: Verify proper state transitions
- **Method**: Connect → Send → Receive → Close
- **Success Criteria**: Clean transitions through all states

#### 5. Large Message Test
- **Purpose**: Validate handling of large payloads
- **Method**: Send 1 MB text and binary messages
- **Success Criteria**: Full message delivered without corruption

#### 6. Rapid Connect/Disconnect Test
- **Purpose**: Test connection pool stability
- **Method**: 50 rapid connect/disconnect cycles
- **Success Criteria**: No connection leaks or errors

#### 7. Load Distribution Test
- **Purpose**: Verify load balancing across backends
- **Method**: 30 clients with 10 messages each
- **Success Criteria**: Load distributed evenly

#### 8. Error Handling Test
- **Purpose**: Validate error recovery mechanisms
- **Method**: Invalid requests, verify graceful handling
- **Success Criteria**: Errors handled, normal operation resumes

#### 9. Message Order Test
- **Purpose**: Ensure FIFO message ordering
- **Method**: Send 100 ordered messages, verify sequence
- **Success Criteria**: All messages received in order

#### 10. Long-Lived Connection Test
- **Purpose**: Test connection stability over time
- **Method**: Maintain connection for 30s with periodic messages
- **Success Criteria**: Connection stable, all messages delivered

### Performance Tests

#### 11. Throughput Test
- **Purpose**: Measure maximum message throughput
- **Method**: 10 clients × 1000 messages each
- **Metrics**: Messages per second
- **Target**: > 10,000 msg/s

#### 12. Latency Test
- **Purpose**: Measure round-trip latency
- **Method**: 100 ping-pong messages
- **Metrics**: P50, P95, P99 latency
- **Target**: P99 < 10ms

## Test Results Expected

### Success Criteria

- ✅ All 12 tests pass
- ✅ Throughput > 10,000 msg/s
- ✅ P99 latency < 10ms
- ✅ Zero connection errors under normal load
- ✅ Graceful degradation under failure conditions

### Sample Output

```
==============================================================
WebSocket Integration Tests
==============================================================

==============================================================
Running: Basic Echo
==============================================================
Test 1: Basic WebSocket echo
✓ Basic echo test passed

==============================================================
Running: Sticky Sessions
==============================================================
Test 2: Sticky session verification
Connection 0: Session ID = 01934f2a-4b89-7b1c-a5f3-1234567890ab
Connection 1: Session ID = 01934f2a-4d22-7c3e-b6d4-2345678901bc
...
✓ Sticky session test passed

[... more tests ...]

==============================================================
Test Results: 12 passed, 0 failed
==============================================================
```

## Manual Testing

### Test Basic Connectivity

```bash
# Using websocat
websocat ws://127.0.0.1:8080

# Using wscat
wscat -c ws://127.0.0.1:8080

# Using Python
python3 -c "
import asyncio
import websockets

async def test():
    async with websockets.connect('ws://127.0.0.1:8080') as ws:
        await ws.send('Hello')
        print(await ws.recv())

asyncio.run(test())
"
```

### Monitor Backend Activity

```bash
# View backend logs
docker-compose logs -f ws-backend-1

# Or if running Python directly
# Check terminal output
```

### Monitor Gateway Metrics

```bash
# View Prometheus metrics
curl http://127.0.0.1:9090/metrics | grep websocket

# View connection count
curl http://127.0.0.1:9090/metrics | grep ws_connections_active
```

## Load Testing

For higher load testing (> 100 connections), use the provided Locust configuration:

```bash
# Install Locust
pip install locust

# Run load test (TBD: create locustfile.py)
locust -f locustfile.py --host=ws://127.0.0.1:8080
```

## Troubleshooting

### Backends not starting

```bash
# Check if ports are available
netstat -tuln | grep -E '8081|8082|8083'

# Check Docker status
docker-compose ps

# View logs
docker-compose logs
```

### Gateway not connecting to backends

```bash
# Verify backends are running
curl http://127.0.0.1:8081  # Should fail but confirm port is listening

# Check gateway logs
cat /tmp/highper-gateway.log

# Verify config
cat test_config.toml
```

### Tests timing out

```bash
# Increase timeout in test_integration.py
# Or check system resources
top
netstat -an | grep 808 | wc -l  # Count connections
```

## Advanced Testing

### Scenario 06: Full Load Test

To validate **Scenario 06** from the implementation plan:

```bash
# 1. Start backends (3 servers)
docker-compose up -d

# 2. Start gateway with production config
cargo build --release
./target/release/highper-gateway --config tests/websocket/test_config.toml

# 3. Run progressive load test
python3 load_test_scenario06.py --max-connections 100000

# Target Metrics:
# - 100K concurrent connections
# - Message latency P99 < 10ms
# - 0% connection drops
# - Sticky sessions working
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: WebSocket Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build Gateway
        run: cargo build --release
      - name: Start Backends
        run: |
          cd tests/websocket
          docker-compose up -d
          sleep 5
      - name: Run Tests
        run: |
          cd tests/websocket
          pip install -r requirements.txt
          pytest test_integration.py -v
```

## File Structure

```
tests/websocket/
├── README.md                  # This file
├── requirements.txt           # Python dependencies
├── docker-compose.yml         # Backend orchestration
├── Dockerfile.backend         # Backend container image
├── mock_backend.py           # WebSocket echo server
├── test_config.toml          # Gateway test configuration
├── test_integration.py       # Integration test suite
└── (future)
    ├── locustfile.py         # Load testing configuration
    └── load_test_scenario06.py  # Scenario 06 validation
```

## Next Steps

1. ✅ Test infrastructure created
2. ⏳ Run initial test suite
3. ⏳ Fix any discovered issues
4. ⏳ Add Scenario 06 validation
5. ⏳ Document performance baselines
6. ⏳ Add to CI/CD pipeline

## References

- [Phase 2.1 Implementation](../../PHASE_2.1_WEBSOCKET_COMPLETION_2025-12-13.md)
- [Implementation Plan](~/.claude/plans/serialized-painting-narwhal.md)
- [WebSocket RFC 6455](https://tools.ietf.org/html/rfc6455)
- [Websockets Python Library](https://websockets.readthedocs.io/)

---

**Last Updated**: December 13, 2025
**Status**: Test infrastructure ready, integration testing pending
