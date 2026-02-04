# Kubernetes Deployment

This directory contains Kubernetes manifests for deploying the Rust reverse proxy.

## Files

- **deployment.yaml** - Main deployment with 3 replicas
- **service.yaml** - LoadBalancer and ClusterIP services
- **configmap.yaml** - Configuration file
- **hpa.yaml** - Horizontal Pod Autoscaler (3-20 replicas)
- **serviceaccount.yaml** - Service account with RBAC
- **pdb.yaml** - Pod Disruption Budget (maintains 2 available pods)
- **ingress.yaml** - Ingress with TLS
- **servicemonitor.yaml** - Prometheus ServiceMonitor

## Quick Start

### 1. Build and Push Image

```bash
# Build the image
docker build -f deployment/docker/Dockerfile -t highper-gateway:v1.0.0 .

# Tag for your registry
docker tag highper-gateway:v1.0.0 your-registry.com/highper-gateway:v1.0.0

# Push to registry
docker push your-registry.com/highper-gateway:v1.0.0

# Update deployment.yaml with your image
```

### 2. Create Namespace (Optional)

```bash
kubectl create namespace highper-gateway
```

### 3. Create TLS Secret

```bash
# Using existing certificates
kubectl create secret tls highper-gateway-tls \
  --cert=/path/to/tls.crt \
  --key=/path/to/tls.key \
  -n default

# Or use cert-manager (recommended)
# See ingress.yaml for cert-manager annotations
```

### 4. Deploy

```bash
# Apply all manifests
kubectl apply -f deployment/kubernetes/

# Or apply individually in order
kubectl apply -f deployment/kubernetes/serviceaccount.yaml
kubectl apply -f deployment/kubernetes/configmap.yaml
kubectl apply -f deployment/kubernetes/deployment.yaml
kubectl apply -f deployment/kubernetes/service.yaml
kubectl apply -f deployment/kubernetes/hpa.yaml
kubectl apply -f deployment/kubernetes/pdb.yaml
kubectl apply -f deployment/kubernetes/ingress.yaml
kubectl apply -f deployment/kubernetes/servicemonitor.yaml  # If using Prometheus Operator
```

### 5. Verify Deployment

```bash
# Check deployment status
kubectl get deployment highper-gateway
kubectl rollout status deployment/highper-gateway

# Check pods
kubectl get pods -l app=highper-gateway

# Check services
kubectl get svc highper-gateway

# Check HPA
kubectl get hpa highper-gateway-hpa

# Check logs
kubectl logs -l app=highper-gateway -f
```

## Configuration

### Update ConfigMap

Edit `configmap.yaml` and apply:

```bash
kubectl apply -f deployment/kubernetes/configmap.yaml

# Rolling restart to pick up new config
kubectl rollout restart deployment/highper-gateway
```

### Update Image

```bash
# Set new image
kubectl set image deployment/highper-gateway highper-gateway=highper-gateway:v1.1.0

# Or update deployment.yaml and apply
kubectl apply -f deployment/kubernetes/deployment.yaml

# Monitor rollout
kubectl rollout status deployment/highper-gateway
```

## Scaling

### Manual Scaling

```bash
# Scale to 5 replicas
kubectl scale deployment highper-gateway --replicas=5

# Check status
kubectl get pods -l app=highper-gateway
```

### Auto-scaling (HPA)

The HPA is configured to scale between 3-20 replicas based on:
- CPU utilization (target: 70%)
- Memory utilization (target: 80%)

```bash
# Check HPA status
kubectl get hpa highper-gateway-hpa

# Describe HPA for details
kubectl describe hpa highper-gateway-hpa

# Adjust HPA
kubectl edit hpa highper-gateway-hpa
```

## Monitoring

### Prometheus Integration

If using Prometheus Operator:

```bash
# Apply ServiceMonitor
kubectl apply -f deployment/kubernetes/servicemonitor.yaml

# Verify
kubectl get servicemonitor highper-gateway
```

### Access Metrics

```bash
# Port-forward to metrics endpoint
kubectl port-forward svc/highper-gateway-metrics 9090:9090

# Access metrics
curl http://localhost:9090/metrics
```

### Grafana Dashboards

1. Import dashboard from `/monitoring/grafana-dashboard.json`
2. Configure Prometheus data source
3. Set namespace filter to `default` (or your namespace)

## High Availability

### Topology Spread

The deployment includes topology spread constraints to distribute pods across:
- Nodes (hard constraint)
- Availability zones (soft constraint)

### Pod Disruption Budget

The PDB ensures at least 2 pods are available during:
- Node drains
- Cluster upgrades
- Voluntary disruptions

```bash
# Check PDB status
kubectl get pdb highper-gateway-pdb
```

### Anti-affinity

Pod anti-affinity rules prevent scheduling multiple pods on the same node when possible.

## Security

### Security Context

Pods run with:
- Non-root user (UID 1000)
- Read-only root filesystem
- Dropped all capabilities (except NET_BIND_SERVICE)
- Seccomp profile

### RBAC

The service account has minimal permissions:
- Read ConfigMaps
- Read Secrets

### Network Policies (Optional)

Create network policies to restrict traffic:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: highper-gateway-netpol
spec:
  podSelector:
    matchLabels:
      app: highper-gateway
  policyTypes:
    - Ingress
    - Egress
  ingress:
    - from:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 8080
        - protocol: TCP
          port: 8443
  egress:
    - to:
        - namespaceSelector: {}
      ports:
        - protocol: TCP
          port: 8080
```

## Troubleshooting

### Pods Not Starting

```bash
# Check pod status
kubectl get pods -l app=highper-gateway

# Describe pod
kubectl describe pod <pod-name>

# Check logs
kubectl logs <pod-name>

# Check events
kubectl get events --sort-by='.lastTimestamp'
```

### Configuration Issues

```bash
# Validate config
kubectl exec <pod-name> -- /usr/local/bin/highper-gateway validate --config /etc/highper-gateway/config.toml

# Check ConfigMap
kubectl get configmap highper-gateway-config -o yaml
```

### Service Not Accessible

```bash
# Check service endpoints
kubectl get endpoints highper-gateway

# Check pod readiness
kubectl get pods -l app=highper-gateway

# Test from another pod
kubectl run -it --rm debug --image=curlimages/curl --restart=Never -- \
  curl http://highper-gateway:8080/health
```

### HPA Not Scaling

```bash
# Check metrics-server
kubectl get deployment metrics-server -n kube-system

# Check HPA conditions
kubectl describe hpa highper-gateway-hpa

# Check pod metrics
kubectl top pods -l app=highper-gateway
```

## Rolling Updates

### Zero-Downtime Updates

The deployment is configured for zero-downtime updates:

```yaml
strategy:
  type: RollingUpdate
  rollingUpdate:
    maxSurge: 1
    maxUnavailable: 0
```

### Rollback

```bash
# Check rollout history
kubectl rollout history deployment/highper-gateway

# Rollback to previous version
kubectl rollout undo deployment/highper-gateway

# Rollback to specific revision
kubectl rollout undo deployment/highper-gateway --to-revision=2
```

## Production Checklist

- [ ] Update image registry in deployment.yaml
- [ ] Configure proper resource requests/limits
- [ ] Set up TLS certificates
- [ ] Configure Ingress with your domain
- [ ] Adjust HPA min/max replicas for your load
- [ ] Review and adjust security contexts
- [ ] Set up Prometheus monitoring
- [ ] Configure alerting rules
- [ ] Test rolling updates
- [ ] Test pod disruptions
- [ ] Document your configuration
- [ ] Set up log aggregation
- [ ] Configure backup for ConfigMaps/Secrets

## Advanced Topics

### Blue-Green Deployment

```bash
# Deploy green version
kubectl apply -f deployment-green.yaml

# Update service selector
kubectl patch service highper-gateway -p '{"spec":{"selector":{"version":"v2"}}}'

# Rollback if needed
kubectl patch service highper-gateway -p '{"spec":{"selector":{"version":"v1"}}}'
```

### Canary Deployment

Use a service mesh like Istio or Linkerd for traffic splitting.

### Multi-Region Deployment

Deploy to multiple clusters and use a global load balancer (e.g., AWS Global Accelerator, Google Cloud Load Balancing).

## Resources

- [Kubernetes Best Practices](https://kubernetes.io/docs/concepts/configuration/overview/)
- [Production Best Practices](https://learnk8s.io/production-best-practices)
- [Security Best Practices](https://kubernetes.io/docs/concepts/security/pod-security-standards/)
