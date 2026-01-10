/**
 * k6 Load Test: Simple HTTP GET requests
 *
 * Usage:
 *   k6 run --vus 100 --duration 30s k6-http-simple.js
 *   k6 run --vus 500 --duration 2m k6-http-simple.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const requestDuration = new Trend('request_duration');
const successRate = new Rate('success_rate');
const requestCount = new Counter('request_count');

// Test configuration
export const options = {
  stages: [
    { duration: '30s', target: 100 },   // Ramp up to 100 users
    { duration: '1m', target: 100 },    // Stay at 100 users
    { duration: '30s', target: 500 },   // Ramp up to 500 users
    { duration: '1m', target: 500 },    // Stay at 500 users
    { duration: '30s', target: 0 },     // Ramp down to 0 users
  ],
  thresholds: {
    'http_req_duration': ['p(95)<100', 'p(99)<200'],  // 95% < 100ms, 99% < 200ms
    'http_req_failed': ['rate<0.01'],                 // Error rate < 1%
    'success_rate': ['rate>0.99'],                    // Success rate > 99%
  },
};

// Test target
const BASE_URL = __ENV.TARGET_URL || 'http://localhost:8080';

export default function () {
  const response = http.get(`${BASE_URL}/`);

  // Check response
  const checkResult = check(response, {
    'status is 200': (r) => r.status === 200,
    'response time < 200ms': (r) => r.timings.duration < 200,
  });

  // Record metrics
  requestDuration.add(response.timings.duration);
  successRate.add(checkResult);
  requestCount.add(1);

  // Think time
  sleep(0.1);
}

export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
    '../results/k6-http-simple.json': JSON.stringify(data),
  };
}

function textSummary(data, options) {
  const indent = options.indent || '';
  const enableColors = options.enableColors || false;

  let summary = `
${indent}Test Results:
${indent}=============
${indent}Total Requests: ${data.metrics.http_reqs.values.count}
${indent}Request Rate: ${data.metrics.http_reqs.values.rate.toFixed(2)} req/s
${indent}
${indent}Response Times:
${indent}  min: ${data.metrics.http_req_duration.values.min.toFixed(2)}ms
${indent}  avg: ${data.metrics.http_req_duration.values.avg.toFixed(2)}ms
${indent}  p50: ${data.metrics.http_req_duration.values['p(50)'].toFixed(2)}ms
${indent}  p95: ${data.metrics.http_req_duration.values['p(95)'].toFixed(2)}ms
${indent}  p99: ${data.metrics.http_req_duration.values['p(99)'].toFixed(2)}ms
${indent}  max: ${data.metrics.http_req_duration.values.max.toFixed(2)}ms
${indent}
${indent}Success Rate: ${(data.metrics.success_rate.values.rate * 100).toFixed(2)}%
${indent}Failed Requests: ${data.metrics.http_req_failed.values.passes || 0}
`;

  return summary;
}
