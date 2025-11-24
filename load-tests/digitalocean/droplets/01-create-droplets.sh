#!/bin/bash
# Create load generator and backend droplets in the same VPC
set -e

REGION="blr1"
VPC_ID=$(doctl vpcs list --format ID,Name --no-header | grep "highper-gateway-loadtest-vpc" | awk '{print $1}')
SSH_KEYS=$(doctl compute ssh-key list --format ID --no-header | head -1)
PROJECT_ID=$(doctl projects list --format ID,Name --no-header | grep "highper-gateway-test" | awk '{print $1}')

echo "=== Creating Droplets for Load Testing ==="
echo "VPC: $VPC_ID"
echo "Region: $REGION"
echo ""

# Create backend droplets
echo "Creating backend droplets..."
for i in 1 2 3; do
    NAME="backend-$i"
    if ! doctl compute droplet list --format Name --no-header | grep -q "^${NAME}$"; then
        doctl compute droplet create "$NAME" \
            --size c-8 \
            --image ubuntu-22-04-x64 \
            --region "$REGION" \
            --vpc-uuid "$VPC_ID" \
            --ssh-keys "$SSH_KEYS" \
            --wait
        echo "Created $NAME"
    else
        echo "$NAME already exists"
    fi
done

# Create load generator droplets
echo ""
echo "Creating load generator droplets..."
for i in 1 2 3; do
    NAME="loadgen-$i"
    if ! doctl compute droplet list --format Name --no-header | grep -q "^${NAME}$"; then
        doctl compute droplet create "$NAME" \
            --size c-8 \
            --image ubuntu-22-04-x64 \
            --region "$REGION" \
            --vpc-uuid "$VPC_ID" \
            --ssh-keys "$SSH_KEYS" \
            --wait
        echo "Created $NAME"
    else
        echo "$NAME already exists"
    fi
done

# Move all to project
echo ""
echo "Moving droplets to project..."
for NAME in backend-1 backend-2 backend-3 loadgen-1 loadgen-2 loadgen-3; do
    DROPLET_ID=$(doctl compute droplet list --format ID,Name --no-header | grep "$NAME" | awk '{print $1}')
    if [ -n "$DROPLET_ID" ] && [ -n "$PROJECT_ID" ]; then
        doctl projects resources assign "$PROJECT_ID" --resource="do:droplet:$DROPLET_ID" 2>/dev/null || true
    fi
done

# Get IPs
echo ""
echo "=== Droplet IPs ==="
echo ""
echo "Backends:"
for i in 1 2 3; do
    NAME="backend-$i"
    PUBLIC_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $2}')
    PRIVATE_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $3}')
    echo "  $NAME: Public=$PUBLIC_IP, Private=$PRIVATE_IP"
done

echo ""
echo "Load Generators:"
for i in 1 2 3; do
    NAME="loadgen-$i"
    PUBLIC_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $2}')
    PRIVATE_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $3}')
    echo "  $NAME: Public=$PUBLIC_IP, Private=$PRIVATE_IP"
done

echo ""
echo "Proxy:"
PUBLIC_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "proxy-loadtest" | awk '{print $2}')
PRIVATE_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "proxy-loadtest" | awk '{print $3}')
echo "  proxy-loadtest: Public=$PUBLIC_IP, Private=$PRIVATE_IP"

# Save IPs to file
echo ""
echo "Saving IPs to ../droplet-ips.env..."
cat > ../droplet-ips.env << EOF
PROXY_PUBLIC=143.110.184.230
PROXY_PRIVATE=10.20.0.2
EOF

for i in 1 2 3; do
    NAME="backend-$i"
    PUBLIC_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $2}')
    PRIVATE_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $3}')
    echo "BACKEND_${i}_PUBLIC=$PUBLIC_IP" >> ../droplet-ips.env
    echo "BACKEND_${i}_PRIVATE=$PRIVATE_IP" >> ../droplet-ips.env
done

for i in 1 2 3; do
    NAME="loadgen-$i"
    PUBLIC_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $2}')
    PRIVATE_IP=$(doctl compute droplet list --format Name,PublicIPv4,PrivateIPv4 --no-header | grep "^$NAME" | awk '{print $3}')
    echo "LOADGEN_${i}_PUBLIC=$PUBLIC_IP" >> ../droplet-ips.env
    echo "LOADGEN_${i}_PRIVATE=$PRIVATE_IP" >> ../droplet-ips.env
done

echo ""
echo "=== Done ==="
echo "Wait 1-2 minutes for droplets to initialize, then run 02-setup-all.sh"
