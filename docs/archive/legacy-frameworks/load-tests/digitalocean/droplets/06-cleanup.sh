#!/bin/bash
# Cleanup load test droplets (keeps proxy)
set -e

echo "=== Cleanup Load Test Droplets ==="
echo ""
echo "This will delete:"
echo "  - backend-1, backend-2, backend-3"
echo "  - loadgen-1, loadgen-2, loadgen-3"
echo ""
echo "NOTE: proxy-loadtest will NOT be deleted"
echo ""
read -p "Are you sure? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "Aborted."
    exit 1
fi

# Delete backend droplets
for i in 1 2 3; do
    NAME="backend-$i"
    ID=$(doctl compute droplet list --format ID,Name --no-header | grep "^[0-9]* *$NAME$" | awk '{print $1}')
    if [ -n "$ID" ]; then
        echo "Deleting $NAME ($ID)..."
        doctl compute droplet delete "$ID" --force
    fi
done

# Delete load generator droplets
for i in 1 2 3; do
    NAME="loadgen-$i"
    ID=$(doctl compute droplet list --format ID,Name --no-header | grep "^[0-9]* *$NAME$" | awk '{print $1}')
    if [ -n "$ID" ]; then
        echo "Deleting $NAME ($ID)..."
        doctl compute droplet delete "$ID" --force
    fi
done

# Clean up local files
rm -f ../droplet-ips.env

echo ""
echo "=== Cleanup Complete ==="
