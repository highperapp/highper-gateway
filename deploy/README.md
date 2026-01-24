# Highper Gateway Deployment

This directory contains deployment resources for Highper Gateway across various platforms and environments.

## Directory Structure

```
deploy/
├── terraform/           # Infrastructure as Code
│   ├── aws/            # AWS (EC2, VPC, NLB, ECR)
│   ├── gcp/            # Google Cloud (GCE, VPC, LB)
│   ├── azure/          # Azure (VMSS, VNet, LB)
│   └── bare-metal/     # SSH-based bare metal provisioning
├── ansible/            # Configuration Management
│   ├── roles/          # Ansible role for Highper Gateway
│   ├── playbooks/      # Deployment playbooks
│   └── inventory/      # Inventory templates
├── helm/               # Kubernetes Deployment
│   └── highper-gateway/
├── docker/             # Container Deployment
│   ├── Dockerfile
│   └── docker-compose.yml
├── configs/            # Configuration Templates
│   ├── yaml/           # YAML configs per use case
│   └── dsl/            # HCL/DSL configs
├── systemd/            # Systemd Service Files
└── DEPLOYMENT_STRATEGY.md
```

## Quick Start

### Option 1: Docker

```bash
cd deploy/docker
docker-compose up -d
```

### Option 2: Kubernetes (Helm)

```bash
cd deploy/helm
helm install gateway ./highper-gateway -f values.yaml
```

### Option 3: AWS (Terraform + Ansible)

```bash
# Provision infrastructure
cd deploy/terraform/aws
terraform init
terraform apply -var="ssh_key_name=my-key" -var="use_case=02"

# Configure instances
cd ../../ansible
ansible-playbook -i inventory/hosts.yml playbooks/deploy.yml
```

### Option 4: Bare Metal

```bash
cd deploy/ansible
cp inventory/hosts.yml.example inventory/hosts.yml
# Edit hosts.yml with your servers
ansible-playbook -i inventory/hosts.yml playbooks/deploy.yml
```

## Use Cases

| ID | Use Case | Ports | Config File |
|----|----------|-------|-------------|
| 01 | Layer 4 TCP Proxy | 80, 443, 3306, 5432 | `uc01-tcp-proxy.yaml` |
| 02 | HTTP Load Balancer | 80, 8080 | `uc02-http-lb.yaml` |
| 03 | HTTPS/TLS Termination | 443, 8443 | `uc03-https-tls.yaml` |
| 04 | API Gateway | 8080, 443 | `uc04-api-gateway.yaml` |
| 05 | HTTP/3 QUIC | 443 (UDP) | `uc05-http3-quic.yaml` |
| 06 | WebSocket LB | 80, 443, 8080 | `uc06-websocket.yaml` |
| 07 | gRPC Gateway | 9090, 443 | `uc07-grpc-gateway.yaml` |
| 08 | Database LB | 3306, 5432, 6379 | `uc08-database-lb.yaml` |
| 09 | WAF + mTLS | 443 | `uc09-waf-mtls.yaml` |
| 10 | Hybrid Multi-Protocol | Multiple | `uc10-hybrid.yaml` |
| 11 | CDN Edge | 80, 443 | `uc11-cdn-edge.yaml` |
| 12 | Service Discovery | 8080, 8500 | `uc12-discovery.yaml` |
| 13 | GraphQL Gateway | 4000, 443 | `uc13-graphql.yaml` |
| 14 | Static + FastCGI | 80, 443 | `uc14-static-fcgi.yaml` |
| 15 | Geo Load Balancing | 80, 443 | `uc15-geo-lb.yaml` |

## System Requirements

- **Kernel:** Linux 5.1+ (5.11+ recommended for full io_uring)
- **RAM:** 512MB minimum (2GB+ recommended)
- **CPU:** 2+ cores
- **Packages:** `liburing2` (runtime)

## Security Considerations

### Required Capabilities

```bash
setcap 'cap_net_bind_service,cap_ipc_lock=+ep' /usr/bin/highper-gateway
```

### System Limits

Add to `/etc/security/limits.d/highper-gateway.conf`:
```
highper-gateway soft memlock unlimited
highper-gateway hard memlock unlimited
highper-gateway soft nofile 1048576
highper-gateway hard nofile 1048576
```

## Documentation

### Deployment
- [Deployment Strategy](./DEPLOYMENT_STRATEGY.md) - Comprehensive deployment guide
- [Use Case Configs](./configs/) - Configuration templates (YAML + HCL)
- [Use Case Documentation](./docs/usecases/) - Detailed guides for all 15 use cases

### Testing
- [Local Testing Guide](./docs/LOCAL_TESTING_GUIDE.md) - Local dev, Docker Compose, Rancher Desktop
- [Load Testing Framework](../highper-gateway/tests/load/README.md) - All 15 scenario tests

### Comparison & Roadmap
- [Feature Comparison Matrix](./docs/FEATURE_COMPARISON_MATRIX.md) - vs Nginx, HAProxy, Caddy, KrakenD, Pingora
- [DSL vs Caddy Comparison](./docs/DSL_VS_CADDY_COMPARISON.md) - Configuration format comparison
- [Feature Improvement Roadmap](../docs/FEATURE_IMPROVEMENT_ROADMAP.md) - Planned enhancements
