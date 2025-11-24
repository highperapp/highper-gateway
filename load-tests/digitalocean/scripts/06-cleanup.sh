#!/bin/bash
# Cleanup all DigitalOcean resources
# WARNING: This will delete all load test infrastructure!

set -e

if [ -f ../config.env ]; then
    source ../config.env
fi

echo "=== DigitalOcean Load Test Cleanup ==="
echo ""
echo "This will delete:"
echo "  - DOKS Cluster: $CLUSTER_NAME"
echo "  - Proxy Droplet: proxy-loadtest"
echo "  - VPC: loadtest-vpc"
echo ""
read -p "Are you sure? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "Aborted."
    exit 1
fi

# 1. Delete DOKS Cluster
echo ""
echo "=== Deleting DOKS Cluster ==="
if doctl kubernetes cluster list --format Name --no-header | grep -q "loadtest-cluster"; then
    doctl kubernetes cluster delete "loadtest-cluster" --force --dangerous
    echo "DOKS cluster deleted"
else
    echo "DOKS cluster not found"
fi

# 2. Delete Proxy Droplet
echo ""
echo "=== Deleting Proxy Droplet ==="
PROXY_ID=$(doctl compute droplet list --format ID,Name --no-header | grep "proxy-loadtest" | awk '{print $1}')
if [ -n "$PROXY_ID" ]; then
    doctl compute droplet delete "$PROXY_ID" --force
    echo "Proxy droplet deleted"
else
    echo "Proxy droplet not found"
fi

# Wait for resources to be deleted before deleting VPC
echo "Waiting 60 seconds for resources to be fully deleted..."
sleep 60

# 3. Delete VPC
echo ""
echo "=== Deleting VPC ==="
VPC_ID=$(doctl vpcs list --format ID,Name --no-header | grep "loadtest-vpc" | awk '{print $1}')
if [ -n "$VPC_ID" ]; then
    doctl vpcs delete "$VPC_ID" --force
    echo "VPC deleted"
else
    echo "VPC not found"
fi

# 4. Clean up local files
rm -f ../config.env ../backend-urls.txt

echo ""
echo "=== Cleanup Complete ==="
