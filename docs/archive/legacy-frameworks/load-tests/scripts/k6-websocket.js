/**
 * k6 Load Test: WebSocket connections
 *
 * Tests WebSocket upgrade and message throughput
 *
 * Usage:
 *   k6 run --vus 100 --duration 1m k6-websocket.js
 */

import ws from 'k6/ws';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const wsConnectDuration = new Trend('ws_connect_duration');
const wsMessageDuration = new Trend('ws_message_duration');
const wsMessagesReceived = new Counter('ws_messages_received');
const wsConnectionSuccess = new Rate('ws_connection_success');

export const options = {
  stages: [
    { duration: '20s', target: 50 },
    { duration: '1m', target: 100 },
    { duration: '20s', target: 200 },
    { duration: '1m', target: 200 },
    { duration: '20s', target: 0 },
  ],
  thresholds: {
    'ws_connect_duration': ['p(95)<200', 'p(99)<500'],
    'ws_message_duration': ['p(95)<50', 'p(99)<100'],
    'ws_connection_success': ['rate>0.99'],
  },
};

const WS_URL = __ENV.WS_URL || 'ws://localhost:8080/ws';

export default function () {
  const url = WS_URL;
  const params = { tags: { name: 'WebSocketTest' } };

  const startTime = Date.now();

  const res = ws.connect(url, params, function (socket) {
    const connectDuration = Date.now() - startTime;
    wsConnectDuration.add(connectDuration);

    socket.on('open', () => {
      wsConnectionSuccess.add(true);

      // Send test messages
      for (let i = 0; i < 10; i++) {
        const msgStart = Date.now();
        const message = JSON.stringify({
          type: 'ping',
          id: i,
          timestamp: msgStart,
          vu: __VU,
        });

        socket.send(message);
      }
    });

    socket.on('message', (data) => {
      const msgDuration = Date.now() - JSON.parse(data).timestamp;
      wsMessageDuration.add(msgDuration);
      wsMessagesReceived.add(1);
    });

    socket.on('error', (e) => {
      console.error(`WebSocket error: ${e}`);
      wsConnectionSuccess.add(false);
    });

    socket.setTimeout(() => {
      socket.close();
    }, 5000);  // Keep connection open for 5 seconds
  });

  check(res, {
    'WebSocket connected': (r) => r && r.status === 101,
  });

  sleep(1);
}

export function handleSummary(data) {
  return {
    '../results/k6-websocket.json': JSON.stringify(data),
  };
}
