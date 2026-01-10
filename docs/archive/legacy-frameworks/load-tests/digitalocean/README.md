# DigitalOcean Load Testing for highper-gateway

Target: **600k RPS** on single proxy instance

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    DigitalOcean VPC                 │
│                                                     │
│  ┌─────────────┐     ┌──────────────────────────┐  │
│  │   Proxy     │     │    DOKS Cluster          │  │
│  │  Droplet    │     │                          │  │
│  │             │     │  ┌────────┐ ┌────────┐   │  │
│  │ highper-gateway  │◄────┼──│ vegeta │ │ vegeta │   │  │
│  │  (32 vCPU)  │     │  │  x16   │ │  pods  │   │  │
│  │             │     │  └────────┘ └────────┘   │  │
│  │             │────►│                          │  │
│  │             │     │  ┌────────┐ ┌────────┐   │  │
│  └─────────────┘     │  │backend │ │backend │   │  │
│                      │  │  x30   │ │  pods  │   │  │
│                      │  └────────┘ └────────┘   │  │
│                      └──────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

## Cost Estimate

| Component | Spec | Hourly Cost |
|-----------|------|-------------|
| Proxy Droplet | c-32 (32 vCPU/64GB) | $0.952 |
| DOKS Node 1 | c-8 (8 vCPU) | $0.238 |
| DOKS Node 2 | c-8 (8 vCPU) | $0.238 |
| DOKS Node 3 | c-8 (8 vCPU) | $0.238 |
| **Total** | | **~$1.67/hr** |

For 20 hours: **~$33**

## Prerequisites

1. DigitalOcean account with API token
2. `doctl` CLI installed and authenticated
3. `kubectl` and `helm` installed
4. SSH key added to DigitalOcean

## Quick Start

### 1. Create Infrastructure

```bash
cd scripts
./01-create-infrastructure.sh
```

This creates:
- VPC in NYC1
- Proxy droplet (32 vCPU)
- DOKS cluster (3 nodes)

### 2. Setup Proxy Droplet

```bash
./02-setup-proxy.sh
```

This applies OS tuning for high throughput:
- File descriptor limits (2M)
- TCP buffer tuning
- Connection tracking optimization

### 3. Deploy Proxy Binary

```bash
./02b-deploy-proxy-binary.sh
```

Builds and deploys highper-gateway to the droplet.

### 4. Deploy DOKS Components

```bash
./03-deploy-doks.sh
```

Deploys:
- 30 backend pods (nginx)
- 16 vegeta load generators

### 5. Update Proxy Config

SSH to proxy and update `/root/config.toml` with backend IPs from output.

```bash
ssh root@<PROXY_IP>
# Edit /root/config.toml with backend URLs
systemctl start highper-gateway
systemctl status highper-gateway
```

### 6. Run Load Tests

```bash
# Single test at specific RPS
./04-run-loadtest.sh 25000  # 25k per generator = 400k total

# Progressive tests (100k -> 200k -> 400k -> 600k)
./05-progressive-test.sh
```

### 7. Cleanup

```bash
./06-cleanup.sh
```

**WARNING**: Deletes all resources!

## Test Progression

| Target RPS | Per Generator (16) | Expected Result |
|------------|-------------------|-----------------|
| 100,000 | 6,250 | Baseline, should pass |
| 200,000 | 12,500 | Moderate load |
| 400,000 | 25,000 | High load |
| 600,000 | 37,500 | Target maximum |

## OS Tuning Applied

```bash
# Key settings on proxy droplet
fs.file-max = 2097152
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.ip_local_port_range = 1024 65535
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_fin_timeout = 10
```

## Troubleshooting

### Proxy not responding
```bash
ssh root@<PROXY_IP>
systemctl status highper-gateway
journalctl -u highper-gateway -f
```

### Check backend connectivity
```bash
kubectl exec -n loadtest <vegeta-pod> -- wget -O - http://<PROXY_IP>:8080/
```

### View real-time metrics
```bash
# On proxy droplet
htop
ss -s  # Socket statistics
```

## Results

Results are saved to `results/<timestamp>-<rps>/`:
- Individual pod reports
- Summary file
