/**
 * k6 Load Test: HTTP POST requests with JSON payload
 *
 * Usage:
 *   k6 run --vus 100 --duration 30s k6-http-post.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const requestDuration = new Trend('request_duration');
const successRate = new Rate('success_rate');

// Test configuration
export const options = {
  stages: [
    { duration: '20s', target: 50 },    // Ramp up to 50 users
    { duration: '1m', target: 50 },     // Stay at 50 users
    { duration: '20s', target: 200 },   // Ramp up to 200 users
    { duration: '1m', target: 200 },    // Stay at 200 users
    { duration: '20s', target: 0 },     // Ramp down
  ],
  thresholds: {
    'http_req_duration': ['p(95)<150', 'p(99)<300'],
    'http_req_failed': ['rate<0.01'],
    'success_rate': ['rate>0.99'],
  },
};

const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';

export default function () {
  const payload = JSON.stringify({
    username: `user_${__VU}_${__ITER}`,
    email: `user${__VU}@example.com`,
    data: {
      timestamp: Date.now(),
      action: 'load_test',
      metadata: {
        vu: __VU,
        iteration: __ITER,
      },
    },
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
      'X-Request-ID': `req_${__VU}_${__ITER}`,
    },
  };

  const response = http.post(`${BASE_URL}/api/data`, payload, params);

  const checkResult = check(response, {
    'status is 200 or 201': (r) => r.status === 200 || r.status === 201,
    'response time < 300ms': (r) => r.timings.duration < 300,
    'has valid JSON response': (r) => {
      try {
        JSON.parse(r.body);
        return true;
      } catch (e) {
        return false;
      }
    },
  });

  requestDuration.add(response.timings.duration);
  successRate.add(checkResult);

  sleep(0.2);
}

export function handleSummary(data) {
  return {
    '../results/k6-http-post.json': JSON.stringify(data),
  };
}
