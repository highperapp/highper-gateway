# Command Reference - Quick Guide

## 🚀 **Build & Run**

### Build
```bash
cd /home/infy/reverse_proxy/highper-gateway
cargo build --release
```

### Run
```bash
cd /home/infy/reverse_proxy
./target/release/highper-gateway --config config/test-minimal.yaml
```

### Check Version
```bash
./target/release/highper-gateway --version
```

---

## 🧪 **Testing Commands**

### Test TLS Passthrough
```bash
# Test SNI extraction
openssl s_client -connect localhost:9443 -servername test.example.com

# Test with specific cipher
openssl s_client -connect localhost:9443 -servername test.example.com -cipher 'HIGH'

# Quick connection test
echo "Q" | openssl s_client -connect localhost:9443 -servername test.example.com 2>&1 | grep "CONNECTED"
```

### Test gRPC Detection
```bash
# Simulate gRPC request with curl
curl -v --http2 \
  -H "Content-Type: application/grpc" \
  -X POST http://localhost:8080/test.Service/Method

# With grpcurl (if available)
grpcurl -plaintext localhost:8080 grpc.health.v1.Health/Check

# Test service list
grpcurl -plaintext localhost:8080 list
```

### Test WebSocket
```bash
# Test upgrade with curl
curl -i \
  -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  -H "Sec-WebSocket-Version: 13" \
  http://localhost:8080/ws

# With wscat (if available)
wscat -c ws://localhost:8080/ws

# Test secure WebSocket
wscat -c wss://localhost:8443/ws --no-check
```

### Test Standard HTTP
```bash
# Simple GET
curl http://localhost:8080/

# With headers
curl -H "User-Agent: test" http://localhost:8080/api/test

# HTTPS (ignoring self-signed cert)
curl -k https://localhost:8443/

# HTTP/2
curl --http2 https://localhost:8443/
```

---

## 📊 **Monitoring**

### Check Ports
```bash
# List all listening ports
sudo ss -tlnp | grep highper-gateway

# Check specific port
sudo lsof -i :8080
sudo lsof -i :8443
sudo lsof -i :9443
```

### View Logs
```bash
# Tail logs (if logging to file)
tail -f /var/log/highper-gateway.log

# With systemd
journalctl -u highper-gateway -f

# Filter for errors
journalctl -u highper-gateway | grep ERROR
```

### Prometheus Metrics
```bash
# Get all metrics
curl http://localhost:9090/metrics

# Filter specific metrics
curl http://localhost:9090/metrics | grep proxy_

# With formatting
curl -s http://localhost:9090/metrics | grep -E "^(proxy_|http_)"
```

### Health Checks
```bash
# Health endpoint
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/ready

# With details
curl -v http://localhost:8080/health
```

---

## 🔧 **Configuration**

### Validate Configuration
```bash
# Check syntax (will fail on missing backends, but validates YAML)
./target/release/highper-gateway --config config/test-minimal.yaml --check

# Dry run (starts then exits)
timeout 5 ./target/release/highper-gateway --config config/test-minimal.yaml || true
```

### Reload Configuration
```bash
# Send HUP signal
kill -HUP $(pgrep highper-gateway)

# Or restart service
systemctl restart highper-gateway
```

---

## 🐛 **Debugging**

### Verbose Logging
```bash
# Set log level
RUST_LOG=debug ./target/release/highper-gateway --config config/test-minimal.yaml

# Specific module
RUST_LOG=highper_gateway::proxy=trace ./target/release/highper-gateway --config config/test-minimal.yaml

# Multiple modules
RUST_LOG=highper_gateway::tls=debug,highper_gateway::websocket=trace ./target/release/highper-gateway --config config/test-minimal.yaml
```

### Check for Errors
```bash
# Compile and show errors
cargo build 2>&1 | grep error

# Run and capture stderr
./target/release/highper-gateway --config config/test-minimal.yaml 2>&1 | tee error.log

# Check for panics
grep -i "panic\|abort\|fatal" error.log
```

### Network Debugging
```bash
# Capture traffic to passthrough port
sudo tcpdump -i lo -n port 9443 -w passthrough.pcap

# Monitor connections
watch -n 1 'ss -tan | grep -E ":(8080|8443|9443)"'

# Check TLS handshake
openssl s_client -connect localhost:8443 -showcerts
```

---

## 🏗️ **Development**

### Quick Development Cycle
```bash
# Build and run
cargo run -- --config ../config/test-minimal.yaml

# Build with all warnings
cargo build --release 2>&1 | grep warning

# Fix warnings automatically
cargo fix --allow-dirty

# Format code
cargo fmt

# Run tests
cargo test

# Run specific test
cargo test test_name

# Check without building
cargo check
```

### Clean Build
```bash
# Clean all artifacts
cargo clean

# Rebuild from scratch
cargo clean && cargo build --release

# Check disk usage
du -sh target/
```

---

## 📦 **Deployment**

### Build for Production
```bash
# Optimized release build
cargo build --release --locked

# Strip symbols (smaller binary)
strip target/release/highper-gateway

# Check binary size
ls -lh target/release/highper-gateway

# Verify it works
./target/release/highper-gateway --version
```

### Install as System Service
```bash
# Copy binary
sudo cp target/release/highper-gateway /usr/local/bin/

# Copy config
sudo mkdir -p /etc/highper-gateway
sudo cp config/integrated-example.yaml /etc/highper-gateway/config.yaml

# Create systemd service
sudo tee /etc/systemd/system/highper-gateway.service << 'EOF'
[Unit]
Description=Rust Reverse Proxy
After=network.target

[Service]
Type=simple
User=highper-gateway
Group=highper-gateway
ExecStart=/usr/local/bin/highper-gateway --config /etc/highper-gateway/config.yaml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# Create user
sudo useradd -r -s /bin/false highper-gateway

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable highper-gateway
sudo systemctl start highper-gateway

# Check status
sudo systemctl status highper-gateway
```

---

## 🔄 **Common Operations**

### Start/Stop/Restart
```bash
# Foreground (Ctrl+C to stop)
./target/release/highper-gateway --config config/test-minimal.yaml

# Background
./target/release/highper-gateway --config config/test-minimal.yaml &

# With nohup
nohup ./target/release/highper-gateway --config config/test-minimal.yaml > proxy.log 2>&1 &

# Stop
killall highper-gateway

# Graceful restart
kill -HUP $(pgrep highper-gateway)

# Force stop
kill -9 $(pgrep highper-gateway)
```

### View Running Process
```bash
# Process info
ps aux | grep highper-gateway

# With details
ps -fp $(pgrep highper-gateway)

# Memory usage
ps -o pid,vsz,rss,comm -p $(pgrep highper-gateway)

# Open files
lsof -p $(pgrep highper-gateway) | head -20
```

---

## 📈 **Performance Testing**

### Load Testing
```bash
# With wrk
wrk -t4 -c100 -d30s http://localhost:8080/

# With ApacheBench
ab -n 10000 -c 100 http://localhost:8080/

# With hey
hey -n 10000 -c 100 http://localhost:8080/

# TLS load test
wrk -t4 -c100 -d30s https://localhost:8443/
```

### Stress Testing
```bash
# Many concurrent connections
for i in {1..1000}; do
  curl http://localhost:8080/ &
done
wait

# WebSocket flood
for i in {1..100}; do
  wscat -c ws://localhost:8080/ws &
done

# Check limits
ulimit -n  # Max open files
sysctl net.core.somaxconn  # Max connections
```

---

## 🔍 **Troubleshooting**

### Port Already in Use
```bash
# Find what's using port
sudo lsof -i :8080

# Kill process
sudo kill -9 $(sudo lsof -t -i:8080)

# Use different port in config
# Edit config.yaml and change bind addresses
```

### Certificate Issues
```bash
# Check cert validity
openssl x509 -in certs/cert.pem -text -noout

# Check cert dates
openssl x509 -in certs/cert.pem -dates -noout

# Generate new self-signed cert
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout certs/key.pem \
  -out certs/cert.pem \
  -days 365 \
  -subj "/CN=localhost"
```

### Connection Issues
```bash
# Test connectivity
nc -zv localhost 8080

# Test TLS
openssl s_client -connect localhost:8443 -debug

# Check firewall
sudo iptables -L -n | grep -E "8080|8443|9443"

# Test from outside
curl http://$(hostname -I | awk '{print $1}'):8080/
```

---

## 📝 **Quick Reference**

| Command | Purpose |
|---------|---------|
| `cargo build --release` | Build optimized binary |
| `./target/release/highper-gateway --config <file>` | Run proxy |
| `curl http://localhost:8080/health` | Check health |
| `curl http://localhost:9090/metrics` | Get metrics |
| `openssl s_client -connect localhost:9443 -servername <sni>` | Test TLS passthrough |
| `curl --http2 -H "Content-Type: application/grpc" localhost:8080/<path>` | Test gRPC |
| `curl -H "Upgrade: websocket" localhost:8080/ws` | Test WebSocket |
| `RUST_LOG=debug ./target/release/highper-gateway` | Debug mode |
| `kill -HUP $(pgrep highper-gateway)` | Reload config |
| `ss -tlnp \| grep highper-gateway` | Check ports |

---

**For more details, see:**
- `TESTING_GUIDE.md` - Comprehensive testing procedures
- `FINAL_INTEGRATION_REPORT.md` - Complete feature documentation
- `QUICK_START_INTEGRATION.md` - Getting started guide
