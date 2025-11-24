#!/bin/bash
#
# Admin API Examples - Common Operations
#
# This script demonstrates common Admin API operations with curl commands.
# Assumes the Admin API is running on localhost:9090 with API key authentication.

# Configuration
ADMIN_URL="http://localhost:9090"
API_KEY="your-secret-api-key"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Admin API Examples ===${NC}\n"

# ============================================================================
# Health Checks
# ============================================================================

echo -e "${GREEN}1. Health Check${NC}"
curl -s "$ADMIN_URL/health" | jq .
echo -e "\n"

echo -e "${GREEN}2. Readiness Check${NC}"
curl -s "$ADMIN_URL/ready" | jq .
echo -e "\n"

# ============================================================================
# Configuration Management
# ============================================================================

echo -e "${GREEN}3. View Current Configuration${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/config" | jq '.upstreams[0]'
echo -e "\n"

echo -e "${GREEN}4. Reload Configuration${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/config/reload" | jq .
echo -e "\n"

# ============================================================================
# Backend Control
# ============================================================================

echo -e "${GREEN}5. List All Backends${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends" | jq .
echo -e "\n"

echo -e "${GREEN}6. Get Specific Backend Details${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends/api_backend_0" | jq .
echo -e "\n"

echo -e "${GREEN}7. Disable Backend for Maintenance${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Scheduled maintenance",
    "drain_timeout_seconds": 30
  }' \
  "$ADMIN_URL/api/backends/api_backend_0/disable" | jq .
echo -e "\n"

echo -e "${GREEN}8. Drain Backend Connections${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "drain_timeout_seconds": 60
  }' \
  "$ADMIN_URL/api/backends/api_backend_0/drain" | jq .
echo -e "\n"

echo -e "${GREEN}9. Enable Backend After Maintenance${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Maintenance completed"
  }' \
  "$ADMIN_URL/api/backends/api_backend_0/enable" | jq .
echo -e "\n"

echo -e "${GREEN}10. Force Health Check${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends/api_backend_0/health-check" | jq .
echo -e "\n"

# ============================================================================
# Cache Management
# ============================================================================

echo -e "${GREEN}11. Get Cache Statistics${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/cache/stats" | jq .
echo -e "\n"

echo -e "${GREEN}12. List Cache Keys${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/cache/keys" | jq .
echo -e "\n"

echo -e "${GREEN}13. Clear All Cache${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "clear_local": true,
    "clear_distributed": true
  }' \
  "$ADMIN_URL/api/cache/clear" | jq .
echo -e "\n"

echo -e "${GREEN}14. Clear Cache by Pattern${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "pattern": "/api/users/*",
    "clear_local": true,
    "clear_distributed": false
  }' \
  "$ADMIN_URL/api/cache/clear" | jq .
echo -e "\n"

echo -e "${GREEN}15. Invalidate Specific Cache Keys${NC}"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "keys": ["GET:/api/users", "GET:/api/products/123"],
    "invalidate_local": true,
    "invalidate_distributed": true
  }' \
  "$ADMIN_URL/api/cache/invalidate" | jq .
echo -e "\n"

# ============================================================================
# Enhanced Metrics
# ============================================================================

echo -e "${GREEN}16. Get Per-Route Metrics${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/metrics/routes" | jq .
echo -e "\n"

echo -e "${GREEN}17. Get Per-Backend Metrics${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/metrics/backends" | jq .
echo -e "\n"

echo -e "${GREEN}18. Get Health Check History${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/metrics/health" | jq '.history[0:3]'
echo -e "\n"

echo -e "${GREEN}19. Export Prometheus Metrics${NC}"
curl -s "$ADMIN_URL/metrics"
echo -e "\n"

# ============================================================================
# Real-Time Statistics
# ============================================================================

echo -e "${GREEN}20. Get Real-Time Statistics${NC}"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/stats" | jq '{
    timestamp: .timestamp,
    requests: .requests,
    connections: .connections,
    cache: .cache
  }'
echo -e "\n"

# ============================================================================
# Common Workflows
# ============================================================================

echo -e "${BLUE}=== Common Workflows ===${NC}\n"

# Workflow 1: Backend Maintenance
echo -e "${GREEN}Workflow 1: Take Backend Offline for Maintenance${NC}"
echo "1. Check backend status"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends/api_backend_0" | jq '{id, url, health_status, enabled}'

echo -e "\n2. Drain connections (60s timeout)"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"drain_timeout_seconds": 60}' \
  "$ADMIN_URL/api/backends/api_backend_0/drain" | jq .

echo -e "\n3. Disable backend"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Scheduled maintenance"}' \
  "$ADMIN_URL/api/backends/api_backend_0/disable" | jq .

echo -e "\n--- Perform maintenance here ---"

echo -e "\n4. Enable backend"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Maintenance completed"}' \
  "$ADMIN_URL/api/backends/api_backend_0/enable" | jq .

echo -e "\n5. Force health check"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends/api_backend_0/health-check" | jq .

echo -e "\n6. Verify backend is healthy"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends/api_backend_0" | jq '{id, url, health_status, enabled}'
echo -e "\n"

# Workflow 2: Cache Invalidation After Deployment
echo -e "${GREEN}Workflow 2: Invalidate Cache After Deployment${NC}"
echo "1. Get current cache stats"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/cache/stats" | jq '{total_entries, local, distributed}'

echo -e "\n2. Clear cache for specific API routes"
curl -s -X POST -H "X-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"pattern": "/api/v2/*", "clear_local": true, "clear_distributed": true}' \
  "$ADMIN_URL/api/cache/clear" | jq .

echo -e "\n3. Verify cache cleared"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/cache/stats" | jq '{total_entries, local, distributed}'
echo -e "\n"

# Workflow 3: Monitor Backend Health
echo -e "${GREEN}Workflow 3: Monitor Backend Health${NC}"
echo "1. List all backends with health status"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/backends" | jq '.backends[] | {id, url, health_status, active_connections}'

echo -e "\n2. Get health check history"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/metrics/health" | jq '.history[0:5]'

echo -e "\n3. Get per-backend metrics"
curl -s -H "X-API-Key: $API_KEY" \
  "$ADMIN_URL/api/metrics/backends" | jq '.backends[] | {
    backend_id,
    url,
    health_status,
    total_requests,
    requests_per_second,
    avg_response_time_ms,
    success_rate
  }'
echo -e "\n"

echo -e "${BLUE}=== Examples Complete ===${NC}"
