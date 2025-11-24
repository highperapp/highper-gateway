//! Real-time metrics dashboard with WebSocket support
//!
//! Provides:
//! - HTML dashboard UI
//! - WebSocket endpoint for live metric updates
//! - JSON metrics snapshots
//! - Real-time visualization

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Metrics snapshot for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Timestamp of snapshot
    pub timestamp: u64,

    /// HTTP request metrics
    pub http: HttpMetrics,

    /// Upstream metrics
    pub upstream: UpstreamMetrics,

    /// System metrics
    pub system: SystemMetrics,

    /// Connection metrics
    pub connections: ConnectionMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMetrics {
    /// Total requests
    pub total_requests: u64,
    /// Total errors
    pub total_errors: u64,
    /// Requests per second (last minute)
    pub requests_per_second: f64,
    /// Average latency (milliseconds)
    pub avg_latency_ms: f64,
    /// P95 latency (milliseconds)
    pub p95_latency_ms: f64,
    /// P99 latency (milliseconds)
    pub p99_latency_ms: f64,
    /// Request bytes
    pub request_bytes: u64,
    /// Response bytes
    pub response_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamMetrics {
    /// Total upstream requests
    pub total_requests: u64,
    /// Total upstream errors
    pub total_errors: u64,
    /// Average upstream latency (milliseconds)
    pub avg_latency_ms: f64,
    /// Healthy upstreams
    pub healthy_count: usize,
    /// Unhealthy upstreams
    pub unhealthy_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Memory usage (MB)
    pub memory_usage_mb: f64,
    /// Memory usage percentage
    pub memory_usage_percent: f64,
    /// Uptime (seconds)
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    /// Active connections
    pub active: u64,
    /// Total connections
    pub total: u64,
    /// TLS handshakes
    pub tls_handshakes: u64,
    /// TLS errors
    pub tls_errors: u64,
}

impl Default for MetricsSnapshot {
    fn default() -> Self {
        Self {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            http: HttpMetrics {
                total_requests: 0,
                total_errors: 0,
                requests_per_second: 0.0,
                avg_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                p99_latency_ms: 0.0,
                request_bytes: 0,
                response_bytes: 0,
            },
            upstream: UpstreamMetrics {
                total_requests: 0,
                total_errors: 0,
                avg_latency_ms: 0.0,
                healthy_count: 0,
                unhealthy_count: 0,
            },
            system: SystemMetrics {
                cpu_usage_percent: 0.0,
                memory_usage_mb: 0.0,
                memory_usage_percent: 0.0,
                uptime_seconds: 0,
            },
            connections: ConnectionMetrics {
                active: 0,
                total: 0,
                tls_handshakes: 0,
                tls_errors: 0,
            },
        }
    }
}

/// Dashboard broadcaster for WebSocket clients
pub struct DashboardBroadcaster {
    tx: broadcast::Sender<MetricsSnapshot>,
    update_interval: Duration,
}

impl DashboardBroadcaster {
    /// Create a new dashboard broadcaster
    pub fn new(update_interval: Duration) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx,
            update_interval,
        }
    }

    /// Get a receiver for metrics updates
    pub fn subscribe(&self) -> broadcast::Receiver<MetricsSnapshot> {
        self.tx.subscribe()
    }

    /// Start broadcasting metrics updates
    pub async fn start(self: Arc<Self>, metrics: Arc<super::Metrics>) {
        info!("Starting dashboard broadcaster (interval: {:?})", self.update_interval);

        let mut interval = tokio::time::interval(self.update_interval);
        loop {
            interval.tick().await;

            // Collect current metrics snapshot
            let snapshot = Self::collect_snapshot(&metrics).await;

            // Broadcast to all WebSocket clients
            match self.tx.send(snapshot.clone()) {
                Ok(receivers) => {
                    debug!("Broadcasted metrics to {} clients", receivers);
                }
                Err(_) => {
                    debug!("No active dashboard clients");
                }
            }
        }
    }

    /// Collect metrics snapshot from Prometheus metrics
    async fn collect_snapshot(metrics: &Arc<super::Metrics>) -> MetricsSnapshot {
        // Parse Prometheus metrics and extract values
        // This is a simplified version - in production, you'd parse the actual Prometheus output
        let prometheus_text = metrics.render();

        // For now, return a snapshot with sample data
        // In a full implementation, parse prometheus_text to extract real values
        let snapshot = Self::parse_prometheus_metrics(&prometheus_text);

        snapshot
    }

    /// Parse Prometheus metrics text format
    fn parse_prometheus_metrics(prometheus_text: &str) -> MetricsSnapshot {
        let mut snapshot = MetricsSnapshot::default();

        // Parse lines and extract metric values
        for line in prometheus_text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            // Simple parsing: "metric_name{labels} value"
            if let Some((name_labels, value)) = line.split_once(' ') {
                if let Some(metric_name) = name_labels.split('{').next() {
                    if let Ok(val) = value.trim().parse::<f64>() {
                        match metric_name {
                            "http_requests_total" => snapshot.http.total_requests = val as u64,
                            "http_requests_errors_total" => snapshot.http.total_errors = val as u64,
                            "http_requests_bytes_total" => snapshot.http.request_bytes = val as u64,
                            "http_responses_bytes_total" => snapshot.http.response_bytes = val as u64,
                            "http_connections_active" => snapshot.connections.active = val as u64,
                            "http_connections_total" => snapshot.connections.total = val as u64,
                            "upstream_requests_total" => snapshot.upstream.total_requests = val as u64,
                            "upstream_requests_errors_total" => snapshot.upstream.total_errors = val as u64,
                            "tls_handshakes_total" => snapshot.connections.tls_handshakes = val as u64,
                            "tls_handshakes_errors_total" => snapshot.connections.tls_errors = val as u64,
                            _ => {}
                        }
                    }
                }
            }
        }

        // Calculate derived metrics
        if snapshot.http.total_requests > 0 {
            let error_rate = snapshot.http.total_errors as f64 / snapshot.http.total_requests as f64;
            snapshot.http.requests_per_second = snapshot.http.total_requests as f64 / 60.0; // Rough estimate
        }

        snapshot
    }
}

/// Dashboard HTML content
pub const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Highper Gateway - Real-Time Dashboard</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            background: #0f172a;
            color: #e2e8f0;
            padding: 20px;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            padding: 30px;
            border-radius: 12px;
            margin-bottom: 30px;
            box-shadow: 0 10px 40px rgba(102, 126, 234, 0.3);
        }
        .header h1 { font-size: 32px; margin-bottom: 10px; }
        .header p { opacity: 0.9; }
        .status {
            display: inline-block;
            background: #10b981;
            color: white;
            padding: 4px 12px;
            border-radius: 20px;
            font-size: 12px;
            font-weight: 600;
            margin-left: 10px;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 20px;
            margin-bottom: 30px;
        }
        .card {
            background: #1e293b;
            border-radius: 12px;
            padding: 24px;
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
        }
        .card h2 {
            font-size: 14px;
            text-transform: uppercase;
            letter-spacing: 1px;
            color: #94a3b8;
            margin-bottom: 16px;
        }
        .metric {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 0;
            border-bottom: 1px solid #334155;
        }
        .metric:last-child { border-bottom: none; }
        .metric-label { color: #cbd5e1; font-size: 14px; }
        .metric-value {
            font-size: 24px;
            font-weight: 700;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
        }
        .metric-unit { font-size: 14px; color: #64748b; margin-left: 4px; }
        .chart {
            height: 200px;
            background: #0f172a;
            border-radius: 8px;
            margin-top: 16px;
            padding: 16px;
            position: relative;
            overflow: hidden;
        }
        .pulse {
            animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
        }
        @keyframes pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.5; }
        }
        .connection-status {
            position: fixed;
            top: 20px;
            right: 20px;
            padding: 8px 16px;
            background: #10b981;
            color: white;
            border-radius: 20px;
            font-size: 12px;
            font-weight: 600;
            box-shadow: 0 4px 6px rgba(16, 185, 129, 0.4);
        }
        .connection-status.disconnected {
            background: #ef4444;
        }
    </style>
</head>
<body>
    <div class="connection-status" id="wsStatus">● Connected</div>

    <div class="header">
        <h1>🚀 Highper Gateway Dashboard <span class="status pulse">LIVE</span></h1>
        <p>Real-time metrics and monitoring</p>
    </div>

    <div class="grid">
        <div class="card">
            <h2>HTTP Requests</h2>
            <div class="metric">
                <span class="metric-label">Total Requests</span>
                <span class="metric-value" id="totalRequests">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">Requests/sec</span>
                <span class="metric-value" id="reqPerSec">0<span class="metric-unit">req/s</span></span>
            </div>
            <div class="metric">
                <span class="metric-label">Error Rate</span>
                <span class="metric-value" id="errorRate">0<span class="metric-unit">%</span></span>
            </div>
        </div>

        <div class="card">
            <h2>Latency</h2>
            <div class="metric">
                <span class="metric-label">Average</span>
                <span class="metric-value" id="avgLatency">0<span class="metric-unit">ms</span></span>
            </div>
            <div class="metric">
                <span class="metric-label">P95</span>
                <span class="metric-value" id="p95Latency">0<span class="metric-unit">ms</span></span>
            </div>
            <div class="metric">
                <span class="metric-label">P99</span>
                <span class="metric-value" id="p99Latency">0<span class="metric-unit">ms</span></span>
            </div>
        </div>

        <div class="card">
            <h2>Connections</h2>
            <div class="metric">
                <span class="metric-label">Active</span>
                <span class="metric-value" id="activeConn">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">Total</span>
                <span class="metric-value" id="totalConn">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">TLS Handshakes</span>
                <span class="metric-value" id="tlsHandshakes">0</span>
            </div>
        </div>

        <div class="card">
            <h2>Upstream</h2>
            <div class="metric">
                <span class="metric-label">Total Requests</span>
                <span class="metric-value" id="upstreamReq">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">Errors</span>
                <span class="metric-value" id="upstreamErr">0</span>
            </div>
            <div class="metric">
                <span class="metric-label">Avg Latency</span>
                <span class="metric-value" id="upstreamLatency">0<span class="metric-unit">ms</span></span>
            </div>
        </div>
    </div>

    <script>
        // WebSocket connection for real-time updates
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/metrics`;
        let ws;
        let reconnectAttempts = 0;
        const maxReconnectDelay = 30000;

        function connect() {
            ws = new WebSocket(wsUrl);
            const statusEl = document.getElementById('wsStatus');

            ws.onopen = () => {
                console.log('WebSocket connected');
                statusEl.textContent = '● Connected';
                statusEl.className = 'connection-status';
                reconnectAttempts = 0;
            };

            ws.onmessage = (event) => {
                const data = JSON.parse(event.data);
                updateMetrics(data);
            };

            ws.onerror = (error) => {
                console.error('WebSocket error:', error);
            };

            ws.onclose = () => {
                console.log('WebSocket disconnected');
                statusEl.textContent = '● Disconnected';
                statusEl.className = 'connection-status disconnected';

                // Exponential backoff reconnection
                const delay = Math.min(1000 * Math.pow(2, reconnectAttempts), maxReconnectDelay);
                reconnectAttempts++;
                setTimeout(connect, delay);
            };
        }

        function updateMetrics(metrics) {
            // HTTP metrics
            document.getElementById('totalRequests').textContent =
                metrics.http.total_requests.toLocaleString();
            document.getElementById('reqPerSec').innerHTML =
                `${metrics.http.requests_per_second.toFixed(1)}<span class="metric-unit">req/s</span>`;

            const errorRate = metrics.http.total_requests > 0
                ? (metrics.http.total_errors / metrics.http.total_requests * 100).toFixed(2)
                : 0;
            document.getElementById('errorRate').innerHTML =
                `${errorRate}<span class="metric-unit">%</span>`;

            // Latency metrics
            document.getElementById('avgLatency').innerHTML =
                `${metrics.http.avg_latency_ms.toFixed(1)}<span class="metric-unit">ms</span>`;
            document.getElementById('p95Latency').innerHTML =
                `${metrics.http.p95_latency_ms.toFixed(1)}<span class="metric-unit">ms</span>`;
            document.getElementById('p99Latency').innerHTML =
                `${metrics.http.p99_latency_ms.toFixed(1)}<span class="metric-unit">ms</span>`;

            // Connection metrics
            document.getElementById('activeConn').textContent =
                metrics.connections.active.toLocaleString();
            document.getElementById('totalConn').textContent =
                metrics.connections.total.toLocaleString();
            document.getElementById('tlsHandshakes').textContent =
                metrics.connections.tls_handshakes.toLocaleString();

            // Upstream metrics
            document.getElementById('upstreamReq').textContent =
                metrics.upstream.total_requests.toLocaleString();
            document.getElementById('upstreamErr').textContent =
                metrics.upstream.total_errors.toLocaleString();
            document.getElementById('upstreamLatency').innerHTML =
                `${metrics.upstream.avg_latency_ms.toFixed(1)}<span class="metric-unit">ms</span>`;
        }

        // Initial connection
        connect();

        // Update timestamp every second
        setInterval(() => {
            const now = new Date();
            console.log('Dashboard active:', now.toLocaleTimeString());
        }, 60000);
    </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_snapshot_default() {
        let snapshot = MetricsSnapshot::default();
        assert_eq!(snapshot.http.total_requests, 0);
        assert_eq!(snapshot.upstream.total_requests, 0);
        assert!(snapshot.timestamp > 0);
    }

    #[test]
    fn test_parse_prometheus_metrics() {
        let prometheus_text = r#"
# HELP http_requests_total Total number of HTTP requests
# TYPE http_requests_total counter
http_requests_total{method="GET",status="200"} 1234
http_requests_errors_total 56
http_connections_active 42
"#;

        let snapshot = DashboardBroadcaster::parse_prometheus_metrics(prometheus_text);
        assert_eq!(snapshot.http.total_requests, 1234);
        assert_eq!(snapshot.http.total_errors, 56);
        assert_eq!(snapshot.connections.active, 42);
    }

    #[tokio::test]
    async fn test_dashboard_broadcaster() {
        let broadcaster = Arc::new(DashboardBroadcaster::new(Duration::from_secs(1)));
        let mut rx = broadcaster.subscribe();

        // Test that we can subscribe and the channel is working
        // Don't actually start the broadcaster to avoid metrics initialization issues in tests

        // Manually send a test snapshot
        let snapshot = MetricsSnapshot::default();
        broadcaster.tx.send(snapshot).ok();

        // Verify we can receive
        let received = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(received.is_ok());
    }
}
