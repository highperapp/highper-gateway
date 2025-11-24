#!/bin/bash
#
# Quick Start Script for Monitoring Setup
# This script helps set up Prometheus and Grafana for the Rust proxy
#

set -euo pipefail

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                                                                ║"
echo "║       Highper Gateway - Monitoring Setup (Prometheus + Grafana)    ║"
echo "║                                                                ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "⚠️  Note: Some steps may require sudo privileges"
fi

echo "This script will help you set up monitoring for the Rust proxy."
echo

# Step 1: Check proxy metrics endpoint
echo "Step 1: Checking if proxy metrics endpoint is accessible..."
if curl -sf http://localhost:9090/metrics > /dev/null 2>&1; then
    echo "✅ Proxy metrics endpoint is accessible"
    echo "   URL: http://localhost:9090/metrics"
else
    echo "❌ Cannot access proxy metrics endpoint"
    echo "   Please ensure:"
    echo "   1. Proxy is running"
    echo "   2. Admin API is enabled on port 9090"
    echo "   3. Configuration has [admin] bind = \"127.0.0.1:9090\""
    echo
    read -p "Press Enter to continue anyway, or Ctrl+C to exit..."
fi
echo

# Step 2: Install Prometheus
echo "Step 2: Prometheus Installation"
echo "   Download from: https://prometheus.io/download/"
echo "   Or install via package manager:"
echo "   Ubuntu: sudo apt-get install prometheus"
echo "   macOS: brew install prometheus"
echo
read -p "Have you installed Prometheus? (y/n): " has_prom
if [ "$has_prom" = "y" ]; then
    echo "✅ Prometheus installed"
else
    echo "⏭️  Skipping Prometheus setup"
fi
echo

# Step 3: Configure Prometheus
echo "Step 3: Prometheus Configuration"
echo "   Configuration file: monitoring/prometheus.yml"
echo "   Alert rules: monitoring/highper_gateway_alerts.yml"
echo
if [ "$has_prom" = "y" ]; then
    read -p "Copy prometheus.yml to Prometheus directory? (y/n): " copy_prom
    if [ "$copy_prom" = "y" ]; then
        read -p "Enter Prometheus directory path: " prom_dir
        if [ -d "$prom_dir" ]; then
            cp prometheus.yml "$prom_dir/"
            cp highper_gateway_alerts.yml "$prom_dir/"
            echo "✅ Configuration files copied"
        else
            echo "❌ Directory not found: $prom_dir"
        fi
    fi
fi
echo

# Step 4: Install Grafana
echo "Step 4: Grafana Installation"
echo "   Download from: https://grafana.com/grafana/download"
echo "   Or install via package manager:"
echo "   Ubuntu: sudo apt-get install grafana"
echo "   macOS: brew install grafana"
echo
read -p "Have you installed Grafana? (y/n): " has_graf
if [ "$has_graf" = "y" ]; then
    echo "✅ Grafana installed"
else
    echo "⏭️  Skipping Grafana setup"
fi
echo

# Step 5: Summary
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                    Setup Summary                               ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo
echo "📄 Configuration Files Created:"
echo "   - monitoring/prometheus.yml          (Prometheus config)"
echo "   - monitoring/highper_gateway_alerts.yml   (Alert rules)"
echo "   - monitoring/grafana-dashboard.json  (Grafana dashboard)"
echo
echo "🚀 Next Steps:"
echo
if [ "$has_prom" = "y" ]; then
    echo "1. Start Prometheus:"
    echo "   cd <prometheus-dir>"
    echo "   ./prometheus --config.file=prometheus.yml"
    echo
    echo "2. Verify scraping:"
    echo "   Open: http://localhost:9090"
    echo "   Go to: Status > Targets"
    echo "   Check: highper-gateway target is UP"
    echo
fi

if [ "$has_graf" = "y" ]; then
    echo "3. Start Grafana:"
    echo "   sudo systemctl start grafana-server"
    echo "   Or: brew services start grafana"
    echo
    echo "4. Configure Grafana:"
    echo "   Open: http://localhost:3000"
    echo "   Login: admin / admin"
    echo "   Add Prometheus data source (http://localhost:9090)"
    echo "   Import dashboard: monitoring/grafana-dashboard.json"
    echo
fi

echo "📖 Full Documentation:"
echo "   PROMETHEUS_GRAFANA_GUIDE.md"
echo
echo "✅ Setup script complete!"
