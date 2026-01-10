#!/bin/bash
# Setup OS tuning on all droplets
set -e

if [ -f ../droplet-ips.env ]; then
    source ../droplet-ips.env
else
    echo "Error: ../droplet-ips.env not found. Run 01-create-droplets.sh first."
    exit 1
fi

SYSCTL_CONF='
fs.file-max = 2097152
fs.nr_open = 2097152
net.core.somaxconn = 65535
net.core.netdev_max_backlog = 65535
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.ip_local_port_range = 1024 65535
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 10
net.ipv4.tcp_keepalive_time = 300
net.ipv4.tcp_keepalive_probes = 5
net.ipv4.tcp_keepalive_intvl = 15
net.ipv4.tcp_rmem = 4096 87380 16777216
net.ipv4.tcp_wmem = 4096 65536 16777216
net.ipv4.tcp_max_tw_buckets = 2000000
net.nf_conntrack_max = 2097152
'

LIMITS_CONF='
* soft nofile 1048576
* hard nofile 1048576
root soft nofile 1048576
root hard nofile 1048576
'

setup_droplet() {
    local IP=$1
    local NAME=$2
    local TYPE=$3

    echo "Setting up $NAME ($IP)..."

    ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 root@$IP << EOF
        # Sysctl tuning
        cat > /etc/sysctl.d/99-loadtest.conf << 'SYSCTL'
$SYSCTL_CONF
SYSCTL
        sysctl --system

        # File descriptor limits
        cat > /etc/security/limits.d/99-loadtest.conf << 'LIMITS'
$LIMITS_CONF
LIMITS

        # Systemd limits
        mkdir -p /etc/systemd/system.conf.d
        cat > /etc/systemd/system.conf.d/limits.conf << 'SYSTEMD'
[Manager]
DefaultLimitNOFILE=1048576
SYSTEMD

        echo "OS tuning applied to $NAME"
EOF

    # Install type-specific tools
    if [ "$TYPE" == "loadgen" ]; then
        echo "Installing vegeta on $NAME..."
        ssh root@$IP << 'EOF'
            if ! command -v vegeta &> /dev/null; then
                wget -q https://github.com/tsenart/vegeta/releases/download/v12.11.1/vegeta_12.11.1_linux_amd64.tar.gz
                tar xzf vegeta_12.11.1_linux_amd64.tar.gz
                mv vegeta /usr/local/bin/
                rm vegeta_12.11.1_linux_amd64.tar.gz
            fi
            vegeta --version
EOF
    fi

    echo "$NAME setup complete"
    echo ""
}

echo "=== Setting Up All Droplets ==="
echo ""

# Setup backends
for i in 1 2 3; do
    eval "IP=\$BACKEND_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        setup_droplet "$IP" "backend-$i" "backend"
    fi
done

# Setup load generators
for i in 1 2 3; do
    eval "IP=\$LOADGEN_${i}_PUBLIC"
    if [ -n "$IP" ]; then
        setup_droplet "$IP" "loadgen-$i" "loadgen"
    fi
done

# Setup proxy (already done but ensure consistency)
setup_droplet "$PROXY_PUBLIC" "proxy-loadtest" "proxy"

echo "=== All Droplets Configured ==="
