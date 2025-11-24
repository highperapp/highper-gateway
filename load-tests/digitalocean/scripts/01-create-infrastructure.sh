#!/bin/bash
# DigitalOcean Infrastructure Setup for Load Testing
# Creates: VPC, Proxy Droplet, DOKS Cluster

set -e

# Configuration
REGION="nyc1"  # Change as needed
VPC_NAME="loadtest-vpc"
PROXY_NAME="proxy-loadtest"
CLUSTER_NAME="loadtest-cluster"

# Droplet sizes
# CPU-Optimized: c-32 (32 vCPU, 64GB) for proxy
# For cost optimization, can use c-16 (16 vCPU, 32GB)
PROXY_SIZE="c-32"

# DOKS node pool
DOKS_NODE_SIZE="c-8"  # 8 vCPU CPU-optimized
DOKS_NODE_COUNT=3

echo "=== DigitalOcean Load Test Infrastructure Setup ==="
echo "Region: $REGION"
echo "Proxy Size: $PROXY_SIZE"
echo "DOKS Nodes: ${DOKS_NODE_COUNT}x $DOKS_NODE_SIZE"

# Check doctl is authenticated
if ! doctl account get &>/dev/null; then
    echo "Error: doctl not authenticated. Run: doctl auth init"
    exit 1
fi

# 1. Create VPC
echo ""
echo "=== Creating VPC ==="
VPC_ID=$(doctl vpcs list --format ID,Name --no-header | grep "$VPC_NAME" | awk '{print $1}')
if [ -z "$VPC_ID" ]; then
    VPC_ID=$(doctl vpcs create \
        --name "$VPC_NAME" \
        --region "$REGION" \
        --ip-range "10.10.0.0/16" \
        --format ID --no-header)
    echo "Created VPC: $VPC_ID"
else
    echo "VPC already exists: $VPC_ID"
fi

# 2. Create Proxy Droplet
echo ""
echo "=== Creating Proxy Droplet ==="
PROXY_ID=$(doctl compute droplet list --format ID,Name --no-header | grep "$PROXY_NAME" | awk '{print $1}')
if [ -z "$PROXY_ID" ]; then
    PROXY_ID=$(doctl compute droplet create "$PROXY_NAME" \
        --region "$REGION" \
        --size "$PROXY_SIZE" \
        --image ubuntu-24-04-x64 \
        --vpc-uuid "$VPC_ID" \
        --ssh-keys "$(doctl compute ssh-key list --format ID --no-header | head -1)" \
        --enable-monitoring \
        --format ID --no-header \
        --wait)
    echo "Created Proxy Droplet: $PROXY_ID"
else
    echo "Proxy Droplet already exists: $PROXY_ID"
fi

# Get Proxy IPs
PROXY_PUBLIC_IP=$(doctl compute droplet get "$PROXY_ID" --format PublicIPv4 --no-header)
PROXY_PRIVATE_IP=$(doctl compute droplet get "$PROXY_ID" --format PrivateIPv4 --no-header)
echo "Proxy Public IP: $PROXY_PUBLIC_IP"
echo "Proxy Private IP: $PROXY_PRIVATE_IP"

# 3. Create DOKS Cluster
echo ""
echo "=== Creating DOKS Cluster ==="
CLUSTER_ID=$(doctl kubernetes cluster list --format ID,Name --no-header | grep "$CLUSTER_NAME" | awk '{print $1}')
if [ -z "$CLUSTER_ID" ]; then
    doctl kubernetes cluster create "$CLUSTER_NAME" \
        --region "$REGION" \
        --vpc-uuid "$VPC_ID" \
        --node-pool "name=loadtest-pool;size=$DOKS_NODE_SIZE;count=$DOKS_NODE_COUNT;auto-scale=false" \
        --wait

    CLUSTER_ID=$(doctl kubernetes cluster list --format ID,Name --no-header | grep "$CLUSTER_NAME" | awk '{print $1}')
    echo "Created DOKS Cluster: $CLUSTER_ID"
else
    echo "DOKS Cluster already exists: $CLUSTER_ID"
fi

# 4. Get kubeconfig
echo ""
echo "=== Configuring kubectl ==="
doctl kubernetes cluster kubeconfig save "$CLUSTER_NAME"
kubectl get nodes

# 5. Output summary
echo ""
echo "=========================================="
echo "Infrastructure Created Successfully!"
echo "=========================================="
echo ""
echo "Proxy Droplet:"
echo "  Public IP:  $PROXY_PUBLIC_IP"
echo "  Private IP: $PROXY_PRIVATE_IP"
echo "  SSH: ssh root@$PROXY_PUBLIC_IP"
echo ""
echo "DOKS Cluster: $CLUSTER_NAME"
echo "  Nodes: $DOKS_NODE_COUNT x $DOKS_NODE_SIZE"
echo ""
echo "Next steps:"
echo "  1. Run: ./02-setup-proxy.sh $PROXY_PUBLIC_IP"
echo "  2. Run: ./03-deploy-doks.sh $PROXY_PRIVATE_IP"
echo "  3. Run: ./04-run-loadtest.sh"
echo ""

# Save config for other scripts
cat > ../config.env << EOF
PROXY_PUBLIC_IP=$PROXY_PUBLIC_IP
PROXY_PRIVATE_IP=$PROXY_PRIVATE_IP
CLUSTER_NAME=$CLUSTER_NAME
VPC_ID=$VPC_ID
REGION=$REGION
EOF

echo "Config saved to config.env"
