//! Built-in Status Dashboard
//!
//! Provides a real-time web UI for monitoring the gateway:
//! - Traffic statistics
//! - Backend health status
//! - Route information
//! - Cache statistics
//! - Connection pool metrics

use bytes::Bytes;
use http_body_util::Full;
use hyper::{header, Response, StatusCode};

/// Dashboard HTML content (embedded)
const DASHBOARD_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Highper Gateway Dashboard</title>
    <style>
        :root {
            --bg-primary: #0f172a;
            --bg-secondary: #1e293b;
            --bg-card: #334155;
            --text-primary: #f1f5f9;
            --text-secondary: #94a3b8;
            --accent: #3b82f6;
            --success: #22c55e;
            --warning: #f59e0b;
            --danger: #ef4444;
            --border: #475569;
        }
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg-primary);
            color: var(--text-primary);
            min-height: 100vh;
        }
        .header {
            background: var(--bg-secondary);
            padding: 1rem 2rem;
            border-bottom: 1px solid var(--border);
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .header h1 {
            font-size: 1.5rem;
            display: flex;
            align-items: center;
            gap: 0.5rem;
        }
        .header h1::before {
            content: '';
            display: inline-block;
            width: 32px;
            height: 32px;
            background: linear-gradient(135deg, var(--accent), #8b5cf6);
            border-radius: 8px;
        }
        .status-badge {
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.875rem;
            font-weight: 500;
        }
        .status-healthy { background: var(--success); color: #000; }
        .status-degraded { background: var(--warning); color: #000; }
        .status-unhealthy { background: var(--danger); color: #fff; }
        .container {
            max-width: 1400px;
            margin: 0 auto;
            padding: 1.5rem;
        }
        .grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
            gap: 1rem;
            margin-bottom: 1.5rem;
        }
        .card {
            background: var(--bg-secondary);
            border-radius: 12px;
            padding: 1.25rem;
            border: 1px solid var(--border);
        }
        .card-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .card-title {
            font-size: 0.875rem;
            color: var(--text-secondary);
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }
        .card-value {
            font-size: 2rem;
            font-weight: 700;
            color: var(--text-primary);
        }
        .card-value.small { font-size: 1.5rem; }
        .card-subtitle {
            font-size: 0.875rem;
            color: var(--text-secondary);
            margin-top: 0.25rem;
        }
        .table-card {
            grid-column: span 2;
        }
        @media (max-width: 768px) {
            .table-card { grid-column: span 1; }
        }
        table {
            width: 100%;
            border-collapse: collapse;
        }
        th, td {
            text-align: left;
            padding: 0.75rem;
            border-bottom: 1px solid var(--border);
        }
        th {
            color: var(--text-secondary);
            font-weight: 500;
            font-size: 0.875rem;
        }
        td { font-size: 0.875rem; }
        .health-dot {
            display: inline-block;
            width: 10px;
            height: 10px;
            border-radius: 50%;
            margin-right: 0.5rem;
        }
        .health-dot.healthy { background: var(--success); }
        .health-dot.unhealthy { background: var(--danger); }
        .health-dot.unknown { background: var(--text-secondary); }
        .progress-bar {
            background: var(--bg-card);
            border-radius: 4px;
            height: 8px;
            overflow: hidden;
        }
        .progress-bar-fill {
            height: 100%;
            background: var(--accent);
            transition: width 0.3s ease;
        }
        .stats-row {
            display: flex;
            justify-content: space-between;
            padding: 0.5rem 0;
            border-bottom: 1px solid var(--border);
        }
        .stats-row:last-child { border-bottom: none; }
        .stats-label { color: var(--text-secondary); }
        .stats-value { font-weight: 600; }
        .refresh-indicator {
            display: flex;
            align-items: center;
            gap: 0.5rem;
            font-size: 0.875rem;
            color: var(--text-secondary);
        }
        .spinner {
            width: 16px;
            height: 16px;
            border: 2px solid var(--border);
            border-top-color: var(--accent);
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        @keyframes spin { to { transform: rotate(360deg); } }
        .error-message {
            background: rgba(239, 68, 68, 0.1);
            border: 1px solid var(--danger);
            color: var(--danger);
            padding: 1rem;
            border-radius: 8px;
            margin-bottom: 1rem;
        }
        .tabs {
            display: flex;
            gap: 0.5rem;
            margin-bottom: 1rem;
            border-bottom: 1px solid var(--border);
            padding-bottom: 0.5rem;
        }
        .tab {
            padding: 0.5rem 1rem;
            border-radius: 6px;
            cursor: pointer;
            color: var(--text-secondary);
            transition: all 0.2s;
        }
        .tab:hover { background: var(--bg-card); }
        .tab.active {
            background: var(--accent);
            color: #fff;
        }
        .chart-container {
            height: 200px;
            display: flex;
            align-items: flex-end;
            gap: 2px;
            padding-top: 1rem;
        }
        .chart-bar {
            flex: 1;
            background: var(--accent);
            border-radius: 2px 2px 0 0;
            min-height: 4px;
            transition: height 0.3s ease;
        }
    </style>
</head>
<body>
    <header class="header">
        <h1>Highper Gateway</h1>
        <div style="display: flex; align-items: center; gap: 1rem;">
            <div class="refresh-indicator">
                <div class="spinner" id="spinner" style="display: none;"></div>
                <span id="lastUpdate">Connecting...</span>
            </div>
            <span class="status-badge status-healthy" id="statusBadge">Healthy</span>
        </div>
    </header>

    <div class="container">
        <div id="error" class="error-message" style="display: none;"></div>

        <!-- Overview Stats -->
        <div class="grid">
            <div class="card">
                <div class="card-header">
                    <span class="card-title">Requests/sec</span>
                </div>
                <div class="card-value" id="rps">--</div>
                <div class="card-subtitle" id="totalRequests">Total: --</div>
            </div>
            <div class="card">
                <div class="card-header">
                    <span class="card-title">Active Connections</span>
                </div>
                <div class="card-value" id="connections">--</div>
                <div class="card-subtitle" id="maxConnections">Max: --</div>
            </div>
            <div class="card">
                <div class="card-header">
                    <span class="card-title">Avg Response Time</span>
                </div>
                <div class="card-value" id="avgLatency">--</div>
                <div class="card-subtitle" id="p99Latency">P99: --</div>
            </div>
            <div class="card">
                <div class="card-header">
                    <span class="card-title">Error Rate</span>
                </div>
                <div class="card-value" id="errorRate">--</div>
                <div class="card-subtitle" id="errors5xx">5xx errors: --</div>
            </div>
        </div>

        <!-- Tabs -->
        <div class="tabs">
            <div class="tab active" data-tab="backends">Backends</div>
            <div class="tab" data-tab="routes">Routes</div>
            <div class="tab" data-tab="cache">Cache</div>
            <div class="tab" data-tab="system">System</div>
        </div>

        <!-- Backends Tab -->
        <div id="tab-backends" class="tab-content">
            <div class="grid">
                <div class="card table-card">
                    <div class="card-header">
                        <span class="card-title">Backend Servers</span>
                    </div>
                    <table>
                        <thead>
                            <tr>
                                <th>Server</th>
                                <th>Status</th>
                                <th>Connections</th>
                                <th>Requests</th>
                                <th>Avg Latency</th>
                            </tr>
                        </thead>
                        <tbody id="backendsTable">
                            <tr><td colspan="5" style="text-align: center; color: var(--text-secondary);">Loading...</td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>

        <!-- Routes Tab -->
        <div id="tab-routes" class="tab-content" style="display: none;">
            <div class="grid">
                <div class="card table-card">
                    <div class="card-header">
                        <span class="card-title">Route Configuration</span>
                    </div>
                    <table>
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Match</th>
                                <th>Upstream</th>
                                <th>Requests</th>
                            </tr>
                        </thead>
                        <tbody id="routesTable">
                            <tr><td colspan="4" style="text-align: center; color: var(--text-secondary);">Loading...</td></tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>

        <!-- Cache Tab -->
        <div id="tab-cache" class="tab-content" style="display: none;">
            <div class="grid">
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">Cache Hit Rate</span>
                    </div>
                    <div class="card-value" id="cacheHitRate">--</div>
                    <div class="progress-bar" style="margin-top: 0.5rem;">
                        <div class="progress-bar-fill" id="cacheHitRateBar" style="width: 0%;"></div>
                    </div>
                </div>
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">Cache Entries</span>
                    </div>
                    <div class="card-value" id="cacheEntries">--</div>
                    <div class="card-subtitle" id="cacheSize">Size: --</div>
                </div>
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">Cache Stats</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Hits</span>
                        <span class="stats-value" id="cacheHits">--</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Misses</span>
                        <span class="stats-value" id="cacheMisses">--</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Backend</span>
                        <span class="stats-value" id="cacheBackend">--</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- System Tab -->
        <div id="tab-system" class="tab-content" style="display: none;">
            <div class="grid">
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">System Info</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Version</span>
                        <span class="stats-value" id="sysVersion">--</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Uptime</span>
                        <span class="stats-value" id="sysUptime">--</span>
                    </div>
                    <div class="stats-row">
                        <span class="stats-label">Workers</span>
                        <span class="stats-value" id="sysWorkers">--</span>
                    </div>
                </div>
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">Memory Usage</span>
                    </div>
                    <div class="card-value small" id="memUsage">--</div>
                    <div class="progress-bar" style="margin-top: 0.5rem;">
                        <div class="progress-bar-fill" id="memUsageBar" style="width: 0%;"></div>
                    </div>
                    <div class="card-subtitle" id="memDetails">--</div>
                </div>
                <div class="card">
                    <div class="card-header">
                        <span class="card-title">CPU Usage</span>
                    </div>
                    <div class="card-value small" id="cpuUsage">--</div>
                    <div class="progress-bar" style="margin-top: 0.5rem;">
                        <div class="progress-bar-fill" id="cpuUsageBar" style="width: 0%;"></div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Request History Chart -->
        <div class="card" style="margin-top: 1rem;">
            <div class="card-header">
                <span class="card-title">Request Rate (last 60 seconds)</span>
            </div>
            <div class="chart-container" id="requestChart"></div>
        </div>
    </div>

    <script>
        // State
        let requestHistory = new Array(60).fill(0);
        let lastRequestCount = 0;
        let refreshInterval = 2000;

        // Tab switching
        document.querySelectorAll('.tab').forEach(tab => {
            tab.addEventListener('click', () => {
                document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
                document.querySelectorAll('.tab-content').forEach(c => c.style.display = 'none');
                tab.classList.add('active');
                document.getElementById('tab-' + tab.dataset.tab).style.display = 'block';
            });
        });

        // Format numbers
        function formatNumber(n) {
            if (n >= 1e9) return (n / 1e9).toFixed(1) + 'B';
            if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
            if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
            return n.toString();
        }

        function formatBytes(bytes) {
            if (bytes >= 1e9) return (bytes / 1e9).toFixed(2) + ' GB';
            if (bytes >= 1e6) return (bytes / 1e6).toFixed(2) + ' MB';
            if (bytes >= 1e3) return (bytes / 1e3).toFixed(2) + ' KB';
            return bytes + ' B';
        }

        function formatDuration(ms) {
            if (ms >= 1000) return (ms / 1000).toFixed(2) + 's';
            return ms.toFixed(1) + 'ms';
        }

        function formatUptime(seconds) {
            const days = Math.floor(seconds / 86400);
            const hours = Math.floor((seconds % 86400) / 3600);
            const mins = Math.floor((seconds % 3600) / 60);
            if (days > 0) return days + 'd ' + hours + 'h';
            if (hours > 0) return hours + 'h ' + mins + 'm';
            return mins + 'm';
        }

        // Update chart
        function updateChart() {
            const container = document.getElementById('requestChart');
            const max = Math.max(...requestHistory, 1);
            container.innerHTML = requestHistory.map(val =>
                `<div class="chart-bar" style="height: ${(val / max) * 100}%"></div>`
            ).join('');
        }

        // Fetch and update data
        async function fetchData() {
            const spinner = document.getElementById('spinner');
            const errorEl = document.getElementById('error');
            spinner.style.display = 'block';

            try {
                // Fetch stats
                const statsRes = await fetch('/api/stats');
                if (!statsRes.ok) throw new Error('Failed to fetch stats');
                const stats = await statsRes.json();

                // Update overview
                const currentRequests = stats.requests?.total || 0;
                const rps = Math.round((currentRequests - lastRequestCount) / (refreshInterval / 1000));
                lastRequestCount = currentRequests;
                requestHistory.push(Math.max(0, rps));
                requestHistory.shift();

                document.getElementById('rps').textContent = formatNumber(rps);
                document.getElementById('totalRequests').textContent = 'Total: ' + formatNumber(currentRequests);
                document.getElementById('connections').textContent = formatNumber(stats.connections?.active || 0);
                document.getElementById('maxConnections').textContent = 'Max: ' + formatNumber(stats.connections?.max || 0);
                document.getElementById('avgLatency').textContent = formatDuration(stats.latency?.avg || 0);
                document.getElementById('p99Latency').textContent = 'P99: ' + formatDuration(stats.latency?.p99 || 0);

                const errorRate = stats.requests?.total > 0
                    ? ((stats.requests?.errors || 0) / stats.requests.total * 100).toFixed(2) + '%'
                    : '0%';
                document.getElementById('errorRate').textContent = errorRate;
                document.getElementById('errors5xx').textContent = '5xx errors: ' + formatNumber(stats.requests?.errors_5xx || 0);

                // System info
                document.getElementById('sysVersion').textContent = stats.version || '--';
                document.getElementById('sysUptime').textContent = formatUptime(stats.uptime_seconds || 0);
                document.getElementById('sysWorkers').textContent = stats.workers || '--';

                updateChart();

                // Fetch backends
                const backendsRes = await fetch('/api/backends');
                if (backendsRes.ok) {
                    const backends = await backendsRes.json();
                    const tbody = document.getElementById('backendsTable');
                    if (Array.isArray(backends) && backends.length > 0) {
                        tbody.innerHTML = backends.map(b => `
                            <tr>
                                <td>${b.url || b.address || '--'}</td>
                                <td>
                                    <span class="health-dot ${b.healthy ? 'healthy' : 'unhealthy'}"></span>
                                    ${b.healthy ? 'Healthy' : 'Unhealthy'}
                                </td>
                                <td>${b.connections || 0}</td>
                                <td>${formatNumber(b.requests || 0)}</td>
                                <td>${formatDuration(b.avg_latency || 0)}</td>
                            </tr>
                        `).join('');
                    } else {
                        tbody.innerHTML = '<tr><td colspan="5" style="text-align: center; color: var(--text-secondary);">No backends configured</td></tr>';
                    }
                }

                // Fetch routes
                const routesRes = await fetch('/api/routes');
                if (routesRes.ok) {
                    const routes = await routesRes.json();
                    const tbody = document.getElementById('routesTable');
                    if (Array.isArray(routes) && routes.length > 0) {
                        tbody.innerHTML = routes.map(r => `
                            <tr>
                                <td>${r.name || '--'}</td>
                                <td>${r.match_criteria?.paths?.join(', ') || r.match?.paths?.join(', ') || '*'}</td>
                                <td>${r.upstream || '--'}</td>
                                <td>${formatNumber(r.requests || 0)}</td>
                            </tr>
                        `).join('');
                    } else {
                        tbody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: var(--text-secondary);">No routes configured</td></tr>';
                    }
                }

                // Fetch cache stats
                const cacheRes = await fetch('/api/cache/stats');
                if (cacheRes.ok) {
                    const cache = await cacheRes.json();
                    const hitRate = cache.hit_rate || 0;
                    document.getElementById('cacheHitRate').textContent = (hitRate * 100).toFixed(1) + '%';
                    document.getElementById('cacheHitRateBar').style.width = (hitRate * 100) + '%';
                    document.getElementById('cacheEntries').textContent = formatNumber(cache.total_entries || 0);
                    document.getElementById('cacheSize').textContent = 'Active: ' + formatNumber(cache.active_entries || 0);
                    document.getElementById('cacheHits').textContent = formatNumber(cache.hits || 0);
                    document.getElementById('cacheMisses').textContent = formatNumber(cache.misses || 0);
                    document.getElementById('cacheBackend').textContent = cache.backend_info || '--';
                }

                // Update status
                const statusBadge = document.getElementById('statusBadge');
                const errorCount = stats.requests?.errors || 0;
                const totalCount = stats.requests?.total || 1;
                const errRate = errorCount / totalCount;

                if (errRate > 0.1) {
                    statusBadge.className = 'status-badge status-unhealthy';
                    statusBadge.textContent = 'Unhealthy';
                } else if (errRate > 0.01) {
                    statusBadge.className = 'status-badge status-degraded';
                    statusBadge.textContent = 'Degraded';
                } else {
                    statusBadge.className = 'status-badge status-healthy';
                    statusBadge.textContent = 'Healthy';
                }

                document.getElementById('lastUpdate').textContent = 'Updated ' + new Date().toLocaleTimeString();
                errorEl.style.display = 'none';

            } catch (err) {
                errorEl.textContent = 'Failed to fetch data: ' + err.message;
                errorEl.style.display = 'block';
            } finally {
                spinner.style.display = 'none';
            }
        }

        // Initial fetch and refresh
        fetchData();
        setInterval(fetchData, refreshInterval);
    </script>
</body>
</html>
"#;

/// Serve the dashboard HTML
pub fn serve_dashboard() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Full::new(Bytes::from(DASHBOARD_HTML)))
        .unwrap()
}

/// Serve dashboard static assets (CSS, JS, etc.)
pub fn serve_asset(path: &str) -> Response<Full<Bytes>> {
    // For now, everything is embedded in the HTML
    // This function can be extended to serve additional assets
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(Bytes::from("Not found")))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serve_dashboard() {
        let response = serve_dashboard();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::CONTENT_TYPE).is_some());
    }
}
