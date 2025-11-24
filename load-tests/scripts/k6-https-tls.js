/**
 * k6 Load Test: HTTPS/TLS requests
 *
 * Tests TLS handshake performance and HTTPS throughput
 *
 * Usage:
 *   k6 run --vus 200 --duration 1m k6-https-tls.js
 */

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const tlsHandshakeDuration = new Trend('tls_handshake_duration');
const requestDuration = new Trend('request_duration');
const successRate = new Rate('success_rate');

export const options = {
  stages: [
    { duration: '30s', target: 100 },
    { duration: '1m', target: 200 },
    { duration: '30s', target: 400 },
    { duration: '1m', target: 400 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    'http_req_duration': ['p(95)<200', 'p(99)<500'],
    'tls_handshake_duration': ['p(95)<100', 'p(99)<200'],
    'http_req_failed': ['rate<0.01'],
    'success_rate': ['rate>0.99'],
  },
  insecureSkipTLSVerify: true,  // Skip TLS verification for self-signed certs
};

const BASE_URL = __ENV.TARGET_URL || 'https://localhost:8443';

export default function () {
  const response = http.get(`${BASE_URL}/`);

  const checkResult = check(response, {
    'status is 200': (r) => r.status === 200,
    'TLS version is 1.2 or 1.3': (r) => {
      const tlsVersion = r.tls_version;
      return tlsVersion === 'TLSv1.2' || tlsVersion === 'TLSv1.3';
    },
    'response time < 500ms': (r) => r.timings.duration < 500,
  });

  // Record TLS handshake time
  if (response.timings.tls_handshaking) {
    tlsHandshakeDuration.add(response.timings.tls_handshaking);
  }

  requestDuration.add(response.timings.duration);
  successRate.add(checkResult);

  sleep(0.1);
}

export function handleSummary(data) {
  return {
    '../results/k6-https-tls.json': JSON.stringify(data),
  };
}
