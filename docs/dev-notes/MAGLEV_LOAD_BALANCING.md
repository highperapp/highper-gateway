# Maglev Load Balancing Algorithm Implementation
## Google's Consistent Hashing at Scale

**Priority**: 🟡 HIGH - Add to Phase 2 (Week 4)
**Complexity**: Medium
**Time**: 3-4 days
**Impact**: Production-grade consistent hashing with minimal disruption

---

## OVERVIEW

### What is Maglev?

Maglev is Google's network load balancer algorithm that provides:
- **Consistent hashing** with minimal backend changes on scaling
- **Even distribution** across backends
- **Fast lookup** (O(1) after preprocessing)
- **Minimal disruption** when backends are added/removed

**Paper**: "Maglev: A Fast and Reliable Software Network Load Balancer" (NSDI 2016)

### Why Add Maglev?

**Current State**: We have 7 load balancing algorithms
1. Round Robin
2. Least Connections
3. Random
4. IP Hash
5. Consistent Hash (basic hash ring)
6. Power of Two
7. Geographic

**Gap**: Our "Consistent Hash" is basic. Maglev provides:
- **Better disruption**: Only K/N keys remapped vs ~50% with basic consistent hashing
- **Faster lookup**: O(1) lookup table vs O(log N) binary search
- **More even distribution**: 1% variance vs 5-10% with hash ring
- **Connection draining**: Graceful backend removal

---

## ALGORITHM EXPLANATION

### Core Concept

Maglev creates a **lookup table** of size M (typically 65537, a prime number) where:
- Each entry maps to a backend
- Table is precomputed when backends change
- Lookup is O(1): `backend = table[hash(key) % M]`

### Key Properties

1. **Minimal Disruption**: When a backend is added/removed, only ~1/N connections move
2. **Even Distribution**: All backends get equal load (within 1%)
3. **Fast**: No tree traversal, just array lookup
4. **Deterministic**: Same key always maps to same backend (unless backends change)

### Algorithm Steps

**Preprocessing** (when backends change):
```
1. For each backend i:
   - Generate offset: offset[i] = hash1(backend_i.name) % M
   - Generate skip: skip[i] = hash2(backend_i.name) % (M - 1) + 1

2. Create lookup table of size M (all entries initially -1)

3. Round-robin fill:
   For round = 0 to infinity:
       For each backend i:
           next = (offset[i] + skip[i] * round) % M
           If table[next] == -1:
               table[next] = i
       If table is full, break
```

**Runtime** (per request):
```
key = client_ip  # or connection tuple
hash = hash(key)
index = hash % M
backend_id = table[index]
return backends[backend_id]
```

---

## IMPLEMENTATION PLAN

### File Structure

```
rust-proxy/src/proxy/
├── load_balancer.rs              (MODIFY - add Maglev variant)
├── maglev.rs                     (NEW - 400 lines)
└── maglev_table.rs               (NEW - 200 lines)
```

### Implementation: maglev.rs

```rust
//! Maglev consistent hashing load balancer
//!
//! Based on Google's Maglev paper (NSDI 2016):
//! "Maglev: A Fast and Reliable Software Network Load Balancer"

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use dashmap::DashMap;
use crate::config::Backend;

/// Maglev lookup table size (prime number)
///
/// Google uses 65537 in production. Must be prime for good distribution.
const MAGLEV_TABLE_SIZE: usize = 65537;

/// Maglev load balancer
pub struct MaglevLoadBalancer {
    /// Backend servers
    backends: Vec<Arc<Backend>>,

    /// Lookup table (precomputed)
    lookup_table: Vec<usize>,

    /// Hash function for keys
    hasher: ahash::RandomState,

    /// Metrics
    total_lookups: AtomicU64,
    table_version: AtomicU64,
}

impl MaglevLoadBalancer {
    /// Create a new Maglev load balancer
    pub fn new(backends: Vec<Arc<Backend>>) -> Self {
        let hasher = ahash::RandomState::new();
        let lookup_table = Self::build_lookup_table(&backends, MAGLEV_TABLE_SIZE);

        Self {
            backends,
            lookup_table,
            hasher,
            total_lookups: AtomicU64::new(0),
            table_version: AtomicU64::new(1),
        }
    }

    /// Select backend for a given key (client IP, connection tuple, etc.)
    pub fn select(&self, key: &str) -> Option<Arc<Backend>> {
        self.total_lookups.fetch_add(1, Ordering::Relaxed);

        if self.backends.is_empty() {
            return None;
        }

        // Hash the key
        let hash = self.hasher.hash_one(key);

        // Lookup in table
        let index = (hash as usize) % self.lookup_table.len();
        let backend_id = self.lookup_table[index];

        Some(self.backends[backend_id].clone())
    }

    /// Rebuild lookup table (called when backends change)
    pub fn rebuild(&mut self, backends: Vec<Arc<Backend>>) {
        self.backends = backends;
        self.lookup_table = Self::build_lookup_table(&self.backends, MAGLEV_TABLE_SIZE);
        self.table_version.fetch_add(1, Ordering::Relaxed);

        tracing::info!(
            "Maglev table rebuilt: {} backends, table_size={}, version={}",
            self.backends.len(),
            self.lookup_table.len(),
            self.table_version.load(Ordering::Relaxed)
        );
    }

    /// Build Maglev lookup table
    ///
    /// This is the core algorithm from the Maglev paper.
    fn build_lookup_table(backends: &[Arc<Backend>], table_size: usize) -> Vec<usize> {
        if backends.is_empty() {
            return vec![];
        }

        let n = backends.len();
        let m = table_size;

        // Step 1: Compute offset and skip for each backend
        let mut offsets = Vec::with_capacity(n);
        let mut skips = Vec::with_capacity(n);

        let hasher1 = ahash::RandomState::new();
        let hasher2 = ahash::RandomState::with_seeds(42, 1337, 0, 0);

        for backend in backends {
            let name = format!("{}:{}", backend.url, backend.weight);

            // offset[i] = hash1(name) % M
            let offset = (hasher1.hash_one(&name) as usize) % m;
            offsets.push(offset);

            // skip[i] = hash2(name) % (M - 1) + 1
            // Must be in range [1, M-1] to ensure coprime with M
            let skip = ((hasher2.hash_one(&name) as usize) % (m - 1)) + 1;
            skips.push(skip);
        }

        // Step 2: Initialize lookup table (all entries = None)
        let mut table = vec![None; m];
        let mut next = vec![0usize; n];  // Next position for each backend

        // Step 3: Populate lookup table using round-robin
        let mut filled = 0;

        while filled < m {
            for i in 0..n {
                let mut cursor = (offsets[i] + skips[i] * next[i]) % m;

                // Find next empty slot
                while table[cursor].is_some() {
                    next[i] += 1;
                    cursor = (offsets[i] + skips[i] * next[i]) % m;
                }

                // Assign backend i to this slot
                table[cursor] = Some(i);
                next[i] += 1;
                filled += 1;

                if filled == m {
                    break;
                }
            }
        }

        // Convert Option<usize> to usize (unwrap is safe, table is fully filled)
        table.into_iter().map(|opt| opt.unwrap()).collect()
    }

    /// Get statistics
    pub fn stats(&self) -> MaglevStats {
        let backend_distribution = self.compute_distribution();

        MaglevStats {
            total_lookups: self.total_lookups.load(Ordering::Relaxed),
            table_version: self.table_version.load(Ordering::Relaxed),
            table_size: self.lookup_table.len(),
            num_backends: self.backends.len(),
            backend_distribution,
        }
    }

    /// Compute how many table entries each backend owns
    fn compute_distribution(&self) -> Vec<(String, usize, f64)> {
        let mut counts = vec![0usize; self.backends.len()];

        for &backend_id in &self.lookup_table {
            counts[backend_id] += 1;
        }

        let expected_per_backend = self.lookup_table.len() as f64 / self.backends.len() as f64;

        self.backends.iter()
            .enumerate()
            .map(|(i, backend)| {
                let count = counts[i];
                let percentage = (count as f64 / self.lookup_table.len() as f64) * 100.0;
                (backend.url.clone(), count, percentage)
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct MaglevStats {
    pub total_lookups: u64,
    pub table_version: u64,
    pub table_size: usize,
    pub num_backends: usize,
    pub backend_distribution: Vec<(String, usize, f64)>,  // (name, count, percentage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maglev_basic() {
        let backends = vec![
            Arc::new(Backend::new("http://backend1:8080")),
            Arc::new(Backend::new("http://backend2:8080")),
            Arc::new(Backend::new("http://backend3:8080")),
        ];

        let lb = MaglevLoadBalancer::new(backends);

        // Same key should always map to same backend
        let key = "192.168.1.100";
        let backend1 = lb.select(key).unwrap();
        let backend2 = lb.select(key).unwrap();
        assert_eq!(backend1.url, backend2.url);
    }

    #[test]
    fn test_maglev_distribution() {
        let backends = vec![
            Arc::new(Backend::new("http://backend1:8080")),
            Arc::new(Backend::new("http://backend2:8080")),
            Arc::new(Backend::new("http://backend3:8080")),
            Arc::new(Backend::new("http://backend4:8080")),
        ];

        let lb = MaglevLoadBalancer::new(backends);

        // Count how many times each backend is selected
        let mut counts = vec![0usize; 4];
        let num_keys = 10000;

        for i in 0..num_keys {
            let key = format!("192.168.1.{}", i);
            let backend = lb.select(&key).unwrap();

            // Find backend index
            let idx = lb.backends.iter().position(|b| b.url == backend.url).unwrap();
            counts[idx] += 1;
        }

        // Check distribution is relatively even (within 10%)
        let expected = num_keys / 4;
        for &count in &counts {
            let deviation = ((count as f64 - expected as f64).abs() / expected as f64) * 100.0;
            assert!(deviation < 10.0, "Backend distribution too uneven: {:?}", counts);
        }
    }

    #[test]
    fn test_maglev_minimal_disruption() {
        let backends_v1 = vec![
            Arc::new(Backend::new("http://backend1:8080")),
            Arc::new(Backend::new("http://backend2:8080")),
            Arc::new(Backend::new("http://backend3:8080")),
        ];

        let lb_v1 = MaglevLoadBalancer::new(backends_v1.clone());

        // Map 1000 keys
        let num_keys = 1000;
        let mut mappings_v1 = Vec::new();
        for i in 0..num_keys {
            let key = format!("192.168.1.{}", i);
            let backend = lb_v1.select(&key).unwrap();
            mappings_v1.push(backend.url.clone());
        }

        // Add a 4th backend
        let mut backends_v2 = backends_v1.clone();
        backends_v2.push(Arc::new(Backend::new("http://backend4:8080")));

        let mut lb_v2 = MaglevLoadBalancer::new(backends_v2);

        // Remap the same keys
        let mut changed = 0;
        for i in 0..num_keys {
            let key = format!("192.168.1.{}", i);
            let backend = lb_v2.select(&key).unwrap();
            if backend.url != mappings_v1[i] {
                changed += 1;
            }
        }

        // With Maglev, only ~25% should change (1/N where N=4)
        let change_rate = (changed as f64 / num_keys as f64) * 100.0;
        println!("Change rate when adding backend: {:.1}%", change_rate);

        // Should be close to 25% (allow ±10% variance)
        assert!(change_rate >= 15.0 && change_rate <= 35.0,
                "Change rate {} is outside expected range [15%, 35%]", change_rate);
    }
}
```

### Integration with Load Balancer

```rust
// File: rust-proxy/src/proxy/load_balancer.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceAlgorithm {
    RoundRobin,
    LeastConnections,
    Random,
    IpHash,
    ConsistentHash,
    PowerOfTwo,
    Geographic,
    Maglev,  // NEW
}

impl LoadBalancer {
    pub fn new(algorithm: LoadBalanceAlgorithm, backends: Vec<Arc<Backend>>) -> Self {
        match algorithm {
            LoadBalanceAlgorithm::RoundRobin => { /* ... */ },
            LoadBalanceAlgorithm::LeastConnections => { /* ... */ },
            // ... other algorithms ...
            LoadBalanceAlgorithm::Maglev => {
                Self::Maglev(maglev::MaglevLoadBalancer::new(backends))
            },
        }
    }

    pub fn select(&self, request: &Request<Body>) -> Option<Arc<Backend>> {
        match self {
            Self::Maglev(lb) => {
                // Extract key (client IP or connection tuple)
                let key = extract_client_ip(request);
                lb.select(&key)
            },
            // ... other algorithms ...
        }
    }
}

fn extract_client_ip(request: &Request<Body>) -> String {
    // Check X-Forwarded-For header
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(ips) = forwarded.to_str() {
            if let Some(client_ip) = ips.split(',').next() {
                return client_ip.trim().to_string();
            }
        }
    }

    // Fallback to connection remote address
    request.extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
```

---

## CONFIGURATION

### YAML Configuration

```yaml
upstreams:
  - name: "api_backend"
    servers:
      - url: "http://backend1:8080"
        weight: 100
      - url: "http://backend2:8080"
        weight: 100
      - url: "http://backend3:8080"
        weight: 100
      - url: "http://backend4:8080"
        weight: 100

    load_balancing:
      algorithm: "maglev"  # NEW

      # Optional: Custom table size (must be prime)
      # Default: 65537 (Google's production value)
      maglev_table_size: 65537

      # Optional: Hash key strategy
      # - "client_ip" (default): Use client IP address
      # - "connection_tuple": Use (client_ip, client_port, protocol)
      # - "session_cookie": Use cookie value
      maglev_key: "client_ip"
```

### DSL Configuration (Future)

```
example.com {
    reverse_proxy backend1:8080 backend2:8080 backend3:8080 {
        load_balance maglev
    }
}
```

---

## PERFORMANCE CHARACTERISTICS

### Time Complexity

| Operation | Complexity | Notes |
|-----------|------------|-------|
| **Lookup** | O(1) | Array index, very fast |
| **Table Build** | O(M × N) | M = table size, N = num backends |
| **Backend Add** | O(M × N) | Rebuild table |
| **Backend Remove** | O(M × N) | Rebuild table |

### Space Complexity

- **Lookup Table**: M × sizeof(usize) = 65537 × 8 = ~524 KB
- **Per Backend**: 2 × sizeof(usize) = 16 bytes (offset + skip)
- **Total**: ~524 KB + (16 × N) bytes

### Performance Benchmarks (Expected)

| Metric | Value |
|--------|-------|
| Lookup Time | < 20ns (single array access) |
| Table Build Time | ~1-2ms for 100 backends |
| Memory Overhead | ~524 KB |
| Distribution Variance | < 1% with 100+ backends |

---

## WHEN TO USE MAGLEV

### ✅ USE Maglev When:

1. **Connection Affinity** - Need same client → same backend
2. **Frequent Scaling** - Backends added/removed often
3. **Caching** - Backend caches, want to maximize cache hits
4. **Stateful Services** - Need session stickiness
5. **Large Scale** - Many backends (10+)

### ❌ DON'T Use Maglev When:

1. **Stateless Services** - No session affinity needed (use Round Robin)
2. **Load-Sensitive** - Need real-time load balancing (use Least Connections)
3. **Small Scale** - 2-3 backends (simpler algorithms sufficient)
4. **Frequent Request Distribution** - Need even distribution over short time (use Power of Two)

---

## COMPARISON WITH OTHER ALGORITHMS

| Algorithm | Consistency | Disruption | Lookup | Distribution | Use Case |
|-----------|-------------|------------|--------|--------------|----------|
| **Round Robin** | None | N/A | O(1) | Perfect | Stateless, even load |
| **Least Conn** | None | N/A | O(N) | Load-based | Varying request times |
| **IP Hash** | High | 100% | O(1) | Uneven (5-10%) | Simple stickiness |
| **Consistent Hash** | High | ~50% | O(log N) | Uneven (5-10%) | Caching |
| **Maglev** | High | ~1/N | O(1) | Even (<1%) | Production stickiness |
| **Power of Two** | None | N/A | O(1) | Even | Low latency variance |

**Winner for Sticky Sessions**: Maglev (best combination of consistency + disruption + speed)

---

## TESTING PLAN

### Unit Tests (in maglev.rs)

- [x] Basic selection (same key → same backend)
- [x] Distribution evenness (< 10% variance)
- [x] Minimal disruption (< 35% when adding backend)
- [ ] Weighted backends
- [ ] Backend removal
- [ ] Empty backend list
- [ ] Single backend
- [ ] Large number of backends (100+)

### Integration Tests

- [ ] HTTP requests stick to same backend
- [ ] Backend scaling doesn't disrupt existing sessions
- [ ] Graceful backend removal
- [ ] Health check integration (skip unhealthy backends)
- [ ] Metrics collection

### Load Tests

- [ ] 100K RPS with 10 backends
- [ ] Measure lookup latency (< 100ns target)
- [ ] Memory usage (< 1MB for 100 backends)
- [ ] Add/remove backend during load (disruption < 2%)

### Correctness Tests

- [ ] Same client IP always goes to same backend
- [ ] Distribution is even (Chi-squared test)
- [ ] No backend gets 0 traffic
- [ ] All backends eventually used

---

## IMPLEMENTATION TIMELINE

### Day 1: Core Algorithm (4-5 hours)
- [ ] Implement `MaglevLoadBalancer` struct
- [ ] Implement `build_lookup_table()` function
- [ ] Implement `select()` function
- [ ] Add unit tests

### Day 2: Integration (4-5 hours)
- [ ] Add `Maglev` variant to `LoadBalanceAlgorithm` enum
- [ ] Integrate with `LoadBalancer` enum
- [ ] Add configuration support
- [ ] Wire up to request handler

### Day 3: Testing & Validation (6-8 hours)
- [ ] Write integration tests
- [ ] Run load tests
- [ ] Benchmark vs other algorithms
- [ ] Validate minimal disruption property
- [ ] Check distribution evenness

### Day 4: Documentation & Polish (2-3 hours)
- [ ] Write usage documentation
- [ ] Add configuration examples
- [ ] Add metrics/observability
- [ ] Create migration guide from IP Hash/Consistent Hash

**Total Time**: 3-4 days

---

## METRICS & OBSERVABILITY

### Prometheus Metrics

```rust
// Add to observability/metrics.rs

pub struct MaglevMetrics {
    /// Total number of lookups
    pub lookups_total: IntCounter,

    /// Lookup latency histogram
    pub lookup_duration: Histogram,

    /// Table rebuild count
    pub table_rebuilds_total: IntCounter,

    /// Table rebuild duration
    pub table_rebuild_duration: Histogram,

    /// Current table size
    pub table_size: IntGauge,

    /// Number of backends in table
    pub backends_count: IntGauge,

    /// Distribution per backend (gauge vector)
    pub backend_distribution: GaugeVec,
}
```

### Grafana Dashboard

```json
{
  "title": "Maglev Load Balancer",
  "panels": [
    {
      "title": "Lookup Latency",
      "targets": ["histogram_quantile(0.99, maglev_lookup_duration_bucket)"]
    },
    {
      "title": "Backend Distribution",
      "targets": ["maglev_backend_distribution"]
    },
    {
      "title": "Table Rebuilds",
      "targets": ["rate(maglev_table_rebuilds_total[5m])"]
    }
  ]
}
```

---

## MIGRATION GUIDE

### From IP Hash to Maglev

**Before (IP Hash)**:
```yaml
load_balancing:
  algorithm: "ip_hash"
```

**After (Maglev)**:
```yaml
load_balancing:
  algorithm: "maglev"
  maglev_key: "client_ip"  # Same behavior
```

**Migration Steps**:
1. Deploy Maglev to 10% of traffic
2. Monitor disruption rate (should be < 5%)
3. Gradually increase to 100%
4. Monitor cache hit rates (should improve)

### From Consistent Hash to Maglev

**Benefits**:
- **50% less disruption** when adding backends
- **10x faster lookup** (O(1) vs O(log N))
- **Better distribution** (1% vs 5-10% variance)

**Tradeoff**:
- **More memory** (~524 KB vs ~1 KB)

---

## ADVANCED FEATURES (Future)

### 1. Weighted Maglev

Allow different backend weights:
```yaml
servers:
  - url: "backend1:8080"
    weight: 200  # Gets 2x traffic
  - url: "backend2:8080"
    weight: 100  # Gets 1x traffic
```

**Implementation**: Assign more table entries to higher-weight backends

### 2. Connection Draining

Gracefully remove backend without disrupting existing connections:
```yaml
servers:
  - url: "backend1:8080"
    drain: true  # Stop new connections, keep existing
```

### 3. Custom Hash Functions

Support different hash strategies:
- Session cookie
- JWT token ID
- Custom header value

---

## REFERENCES

1. **Original Paper**: "Maglev: A Fast and Reliable Software Network Load Balancer" (NSDI 2016)
   - https://research.google/pubs/pub44824/

2. **Google Blog**: Maglev in Production
   - https://cloudplatform.googleblog.com/2016/03/Google-shares-software-network-load-balancer-design-powering-GCP-networking.html

3. **Open Source Implementations**:
   - Katran (Facebook): https://github.com/facebookincubator/katran
   - Go implementation: https://github.com/dgryski/go-maglev

---

## SUMMARY

**What**: Google's Maglev consistent hashing algorithm
**Why**: Better than existing consistent hash (less disruption, faster, more even)
**When**: Week 4 (after io_uring, before DSL config)
**Time**: 3-4 days
**Complexity**: Medium
**Value**: High - production-grade load balancing for stateful services

**Key Benefits**:
1. ✅ **Minimal Disruption**: Only 1/N connections move when scaling
2. ✅ **Fast**: O(1) lookup (vs O(log N) for hash ring)
3. ✅ **Even Distribution**: < 1% variance
4. ✅ **Battle-Tested**: Used by Google in production for years

**Add to roadmap**: **Phase 2, Week 4** (after io_uring benchmarking, alongside DSL config start)

---

**Ready to implement Maglev and match Google's load balancing capabilities!** 🚀
