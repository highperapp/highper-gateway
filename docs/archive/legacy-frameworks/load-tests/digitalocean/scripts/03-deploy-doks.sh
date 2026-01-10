#!/bin/bash
# Deploy backends and load generators to DOKS
# Usage: ./03-deploy-doks.sh <PROXY_PRIVATE_IP>

set -e

PROXY_PRIVATE_IP="${1:-}"
if [ -z "$PROXY_PRIVATE_IP" ]; then
    if [ -f ../config.env ]; then
        source ../config.env
    fi
fi

if [ -z "$PROXY_PRIVATE_IP" ]; then
    echo "Usage: $0 <PROXY_PRIVATE_IP>"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HELM_DIR="$SCRIPT_DIR/../helm"

echo "=== Deploying to DOKS ==="
echo "Proxy Private IP: $PROXY_PRIVATE_IP"

# Verify kubectl is configured
kubectl get nodes

# Create namespace
kubectl create namespace loadtest --dry-run=client -o yaml | kubectl apply -f -

# Build and push backend image to DO Container Registry
# For simplicity, we'll use a pre-built image or build on DOKS nodes

# Deploy using Helm
echo ""
echo "=== Deploying Helm chart ==="

helm upgrade --install loadtest-do "$HELM_DIR/loadtest-do" \
    --namespace loadtest \
    --set proxy.privateIP="$PROXY_PRIVATE_IP" \
    --set backend.replicas=30 \
    --set vegeta.replicas=16 \
    --wait --timeout=5m

echo ""
echo "=== Waiting for pods to be ready ==="
kubectl wait --for=condition=ready pod -l app=backend -n loadtest --timeout=300s
kubectl wait --for=condition=ready pod -l app=vegeta -n loadtest --timeout=300s

# Get backend service IP
BACKEND_SERVICE_IP=$(kubectl get svc backend -n loadtest -o jsonpath='{.spec.clusterIP}')
echo ""
echo "Backend Service ClusterIP: $BACKEND_SERVICE_IP"

# For the proxy to reach backends, we need the NodePort or to use host networking
# Let's get the node IPs and backend NodePort
BACKEND_NODEPORT=$(kubectl get svc backend -n loadtest -o jsonpath='{.spec.ports[0].nodePort}')
NODE_IPS=$(kubectl get nodes -o jsonpath='{.items[*].status.addresses[?(@.type=="InternalIP")].address}')

echo "Backend NodePort: $BACKEND_NODEPORT"
echo "Node IPs: $NODE_IPS"

# Create backend URL list for proxy config
BACKEND_URLS=""
for ip in $NODE_IPS; do
    BACKEND_URLS="${BACKEND_URLS}    { url = \"http://${ip}:${BACKEND_NODEPORT}\", weight = 1, max_conns = 5000 },\n"
done

echo ""
echo "=== Update proxy config ==="
echo "SSH to proxy and update /root/config.toml with these backend servers:"
echo ""
echo "servers = ["
echo -e "$BACKEND_URLS]"
echo ""

# Save for convenience
cat > ../backend-urls.txt << EOF
# Backend servers for proxy config
servers = [
$(echo -e "$BACKEND_URLS")]
EOF

echo "Backend URLs saved to backend-urls.txt"
echo ""
echo "=== Deployment Summary ==="
kubectl get pods -n loadtest
echo ""
echo "Next: Update proxy config and run ./04-run-loadtest.sh"
