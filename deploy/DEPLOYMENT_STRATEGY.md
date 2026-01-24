# Highper Gateway Deployment Strategy

## Overview

This document provides a comprehensive deployment strategy for Highper Gateway, a high-performance reverse proxy, load balancer, and API gateway built in Rust with io_uring support.

## System Requirements

### Minimum Requirements
- **Kernel:** Linux 5.1+ (5.11+ recommended for full io_uring networking features)
- **CPU:** 2 cores (4+ recommended for production)
- **RAM:** 512MB minimum (2GB+ recommended)
- **Disk:** 100MB for binary + configuration storage

### io_uring Specific Requirements
- `liburing` headers (for source builds)
- `CAP_IPC_LOCK` capability for memory locking
- `CAP_NET_BIND_SERVICE` for privileged ports
- Adequate `memlock` limits (unlimited recommended)

---

## 1. Distribution Channels

### A. Binary Distribution (Production)

| Distribution | Package Format | Tool |
|--------------|----------------|------|
| Debian/Ubuntu | `.deb` | `cargo-deb` |
| RHEL/CentOS/Fedora | `.rpm` | `cargo-generate-rpm` |
| Alpine | `.apk` | Custom APKBUILD |
| Generic Linux | Tarball | Direct download |

### B. Container Images

| Registry | Image Name | Tag Strategy |
|----------|-----------|--------------|
| DockerHub | `highpergateway/highper-gateway` | `latest`, `v1.x.x`, `nightly` |
| GitHub Container Registry | `ghcr.io/highper/gateway` | Same |
| AWS ECR | `<account>.dkr.ecr.<region>.amazonaws.com/highper-gateway` | Same |
| Azure ACR | `<registry>.azurecr.io/highper-gateway` | Same |

### C. Source Distribution

```bash
# From crates.io (future)
cargo install highper-gateway --locked

# From source
git clone https://github.com/highperapp/highper-gateway.git
cd highper-gateway
cargo build --release
```

---

## 2. Infrastructure as Code (IaC) Stack

```
┌─────────────────────────────────────────────────────────────┐
│                    DEPLOYMENT PIPELINE                       │
├─────────────────────────────────────────────────────────────┤
│  Terraform          │  Ansible           │  Helm            │
│  (Provisioning)     │  (Configuration)   │  (Orchestration) │
├─────────────────────┼────────────────────┼──────────────────┤
│  • VMs/Instances    │  • OS Tuning       │  • K8s Deployments│
│  • Networks/VPCs    │  • io_uring setup  │  • ConfigMaps     │
│  • Load Balancers   │  • Binary install  │  • Services       │
│  • Container Regs   │  • systemd units   │  • Ingress        │
│  • K8s Clusters     │  • Config deploy   │  • HPA/VPA        │
└─────────────────────┴────────────────────┴──────────────────┘
```

---

## 3. The 15 Use Case Deployment Matrix

| ID | Use Case | Default Port(s) | Privileged | Scaling |
|----|----------|-----------------|------------|---------|
| 01 | Layer 4 TCP Proxy | 80, 443, 3306, 5432 | Yes | Horizontal |
| 02 | Layer 7 HTTP Load Balancer | 80, 8080 | Optional | Horizontal |
| 03 | HTTPS/TLS Termination | 443, 8443 | Yes | Horizontal |
| 04 | API Gateway + Rate Limiting | 8080, 443 | Optional | Horizontal |
| 05 | HTTP/3 QUIC | 443/UDP | Yes | Horizontal |
| 06 | WebSocket Load Balancer | 80, 443, 8080 | Optional | Horizontal |
| 07 | gRPC Gateway | 9090, 443 | Optional | Horizontal |
| 08 | Database Load Balancer | 3306, 5432, 6379 | Yes | HA Pair |
| 09 | WAF + mTLS | 443 | Yes | Edge/PoP |
| 10 | Hybrid Multi-Protocol | Multiple | Yes | Horizontal |
| 11 | CDN Edge Caching | 80, 443 | Yes | Edge/PoP |
| 12 | Microservices Discovery | 8080, 8500 | No | Sidecar |
| 13 | GraphQL Gateway | 4000, 443 | Optional | Horizontal |
| 14 | Static + PHP-FPM | 80, 443 | Yes | Vertical |
| 15 | Geographic Load Balancing | 80, 443 | Yes | Global |

---

## 4. Configuration Architecture

### Hierarchy
```
/etc/highper-gateway/
├── config.yaml          # Main configuration (YAML)
├── config.hcl           # Alternative (DSL/HCL-like)
├── certs/               # TLS certificates
│   ├── server.crt
│   ├── server.key
│   └── ca.crt
├── rules/               # DSL rule files
│   ├── waf.rules
│   └── routing.rules
└── backends/            # Backend definitions
    └── services.yaml
```

### Port Binding Strategy

| Category | Port Range | Capability Required | Use Cases |
|----------|------------|---------------------|-----------|
| Privileged | 1-1024 | `CAP_NET_BIND_SERVICE` | 01, 03, 05, 08, 09, 11, 14, 15 |
| Standard | 1025-49151 | None | 02, 04, 06, 07, 12, 13 |
| Dynamic | 49152-65535 | None | Internal/Testing |

---

## 5. Deployment Targets

### A. Bare Metal / Virtual Machines

```
┌─────────────────────────────────────────┐
│           Linux Host (5.11+)            │
├─────────────────────────────────────────┤
│  systemd service                        │
│  └── highper-gateway.service            │
├─────────────────────────────────────────┤
│  Binary: /usr/bin/highper-gateway       │
│  Config: /etc/highper-gateway/          │
│  Logs:   /var/log/highper-gateway/      │
│  Data:   /var/lib/highper-gateway/      │
└─────────────────────────────────────────┘
```

### B. Docker / Docker Compose

```
┌─────────────────────────────────────────┐
│         Docker Container                │
├─────────────────────────────────────────┤
│  Base: distroless/cc or alpine:3.19     │
│  Binary: /usr/local/bin/highper-gateway │
│  Config: /etc/highper-gateway/ (mount)  │
│  Ports: Configured per use case         │
└─────────────────────────────────────────┘
```

### C. Kubernetes

```
┌─────────────────────────────────────────┐
│         Kubernetes Cluster              │
├─────────────────────────────────────────┤
│  Deployment/DaemonSet                   │
│  ├── Pod (SecurityContext for io_uring)│
│  │   └── Container: highper-gateway    │
│  ├── ConfigMap: configuration          │
│  ├── Secret: TLS certs                 │
│  ├── Service: ClusterIP/LoadBalancer   │
│  └── Ingress/Gateway API (optional)    │
└─────────────────────────────────────────┘
```

---

## 6. Scaling Strategies

### Single Server (Vertical)
- Maximize io_uring ring size (`entries: 4096+`)
- Enable `SQPOLL` for zero-syscall I/O
- Tune sysctl parameters for high concurrency

### Horizontal Scaling
- Stateless instances behind L4 load balancer
- Shared configuration via ConfigMaps or Consul
- Session affinity when required (WebSocket, gRPC streams)

### Edge/PoP Deployment
- Geographic distribution
- Anycast IP addressing
- Local caching tiers

---

## 7. Security Considerations

### Capability Management
```bash
# Required capabilities for privileged ports + io_uring
setcap 'cap_net_bind_service,cap_ipc_lock=+ep' /usr/bin/highper-gateway
```

### systemd Hardening
```ini
[Service]
# Capabilities
CapabilityBoundingSet=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_IPC_LOCK

# Memory locking for io_uring
LimitMEMLOCK=infinity

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
```

### Kubernetes SecurityContext
```yaml
securityContext:
  capabilities:
    add:
      - NET_BIND_SERVICE
      - IPC_LOCK
  readOnlyRootFilesystem: true
  runAsNonRoot: true
```

---

## 8. Future: OS Repository Strategy

### Phase 1: Direct Distribution
- GitHub Releases (binaries, checksums)
- DockerHub / GHCR (container images)

### Phase 2: Package Repositories
- Cloudsmith or JFrog Artifactory (unified hosting)
- APT repository for Debian/Ubuntu
- YUM/DNF repository for RHEL/Fedora

### Phase 3: Native Integration
- Submit to official distro repositories
- Homebrew formula (for dev environments)
- Nix package

---

## Directory Structure

```
deploy/
├── terraform/           # Infrastructure provisioning
│   ├── aws/
│   ├── azure/
│   ├── gcp/
│   └── bare-metal/
├── ansible/             # Configuration management
│   ├── roles/
│   ├── playbooks/
│   └── inventory/
├── helm/                # Kubernetes orchestration
│   └── highper-gateway/
├── docker/              # Container builds
├── configs/             # Configuration templates
│   ├── yaml/            # YAML configs per use case
│   └── dsl/             # DSL configs per use case
├── scripts/             # Utility scripts
└── docs/                # Use case documentation
    └── usecases/
```

---

## Quick Start

### Option 1: Binary Install (Debian/Ubuntu)
```bash
# Download latest release from GitHub
curl -L https://github.com/highperapp/highper-gateway/releases/latest/download/highper-gateway-linux-x86_64.tar.gz | tar xz
sudo mv highper-gateway /usr/bin/
sudo cp deploy/systemd/highper-gateway.service /etc/systemd/system/
sudo systemctl enable --now highper-gateway
```

### Option 2: Docker
```bash
docker run -d \
  -p 80:80 -p 443:443 \
  -v /path/to/config:/etc/highper-gateway \
  ghcr.io/highperapp/highper-gateway:latest
```

### Option 3: Kubernetes (Helm)
```bash
# Install from local chart (GitHub releases)
git clone https://github.com/highperapp/highper-gateway.git
helm install gateway ./highper-gateway/deploy/helm/highper-gateway -f values.yaml

# Or install from OCI registry (when available)
# helm install gateway oci://ghcr.io/highperapp/charts/highper-gateway -f values.yaml
```

---

## Next Steps

1. Review use case requirements
2. Select deployment target
3. Generate configuration from templates
4. Apply IaC (Terraform → Ansible → Helm)
5. Validate deployment
6. Configure monitoring/observability
