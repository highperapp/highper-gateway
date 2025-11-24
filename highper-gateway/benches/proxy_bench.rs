//! Proxy performance benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use highper_gateway::proxy::{LoadBalancer, LoadBalancerAlgorithm, UpstreamServer};
use highper_gateway::gateway::cache::{LocalCache, CacheEntry};
use bytes::Bytes;
use std::time::{Duration, Instant};

// Benchmark load balancer selection algorithms
fn benchmark_loadbalancer_round_robin(c: &mut Criterion) {
    let servers = vec![
        UpstreamServer {
            url: "http://backend1:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend2:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend3:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
    ];

    let lb = LoadBalancer::new(LoadBalancerAlgorithm::RoundRobin, servers);

    c.bench_function("loadbalancer_round_robin", |b| {
        b.iter(|| {
            black_box(lb.select(None, None));
        });
    });
}

fn benchmark_loadbalancer_least_connections(c: &mut Criterion) {
    let servers = vec![
        UpstreamServer {
            url: "http://backend1:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend2:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend3:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
    ];

    let lb = LoadBalancer::new(LoadBalancerAlgorithm::LeastConnections, servers);

    c.bench_function("loadbalancer_least_connections", |b| {
        b.iter(|| {
            black_box(lb.select(None, None));
        });
    });
}

fn benchmark_loadbalancer_ip_hash(c: &mut Criterion) {
    let servers = vec![
        UpstreamServer {
            url: "http://backend1:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend2:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
        UpstreamServer {
            url: "http://backend3:8080".to_string(),
            weight: 1,
            max_conns: Some(100),
            max_fails: 3,
            fail_timeout: Duration::from_secs(10),
        },
    ];

    let lb = LoadBalancer::new(LoadBalancerAlgorithm::IpHash, servers);

    c.bench_function("loadbalancer_ip_hash", |b| {
        b.iter(|| {
            black_box(lb.select(Some("192.168.1.100"), None));
        });
    });
}

// Benchmark cache operations
fn benchmark_cache_operations(c: &mut Criterion) {
    let cache = LocalCache::new(Duration::from_secs(60));

    c.bench_function("cache_set", |b| {
        b.iter(|| {
            let entry = CacheEntry {
                body: Bytes::from_static(b"test_value"),
                status: 200,
                headers: vec![],
                created_at: Instant::now(),
                ttl: Duration::from_secs(60),
            };
            black_box(cache.set("test_key".to_string(), entry))
        });
    });

    // Pre-populate for get benchmark
    let entry = CacheEntry {
        body: Bytes::from_static(b"test_value"),
        status: 200,
        headers: vec![],
        created_at: Instant::now(),
        ttl: Duration::from_secs(60),
    };
    cache.set("test_key".to_string(), entry);

    c.bench_function("cache_get", |b| {
        b.iter(|| {
            black_box(cache.get("test_key"))
        });
    });

    c.bench_function("cache_get_miss", |b| {
        b.iter(|| {
            black_box(cache.get("nonexistent_key"))
        });
    });
}

// Benchmark varying load sizes
fn benchmark_loadbalancer_scale(c: &mut Criterion) {
    let mut group = c.benchmark_group("loadbalancer_scale");

    for size in [10, 50, 100, 500].iter() {
        let servers: Vec<UpstreamServer> = (0..*size)
            .map(|i| UpstreamServer {
                url: format!("http://backend{}:8080", i),
                weight: 1,
                max_conns: Some(100),
                max_fails: 3,
                fail_timeout: Duration::from_secs(10),
            })
            .collect();

        let lb = LoadBalancer::new(LoadBalancerAlgorithm::RoundRobin, servers);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(lb.select(None, None));
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_loadbalancer_round_robin,
    benchmark_loadbalancer_least_connections,
    benchmark_loadbalancer_ip_hash,
    benchmark_cache_operations,
    benchmark_loadbalancer_scale
);
criterion_main!(benches);
