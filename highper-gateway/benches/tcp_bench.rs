//! TCP Proxy Benchmarks
//!
//! Performance benchmarks for TCP proxy operations
//! Target: < 0.5ms P99 overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use highper_gateway::tcp::{
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig},
    pool::{TcpConnectionPool, PoolConfig},
    protocol::{Protocol, MysqlProtocol, PostgresqlProtocol, RedisProtocol},
};
use std::net::SocketAddr;
use std::time::Duration;

/// Benchmark protocol detection
fn bench_protocol_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_detection");

    // MySQL handshake packet
    let mysql_packet = vec![
        0x0a, 0x00, 0x00, 0x00, 0x0a, // Protocol version
        0x35, 0x2e, 0x37, 0x2e, 0x32, 0x39, 0x00, // Version string
    ];

    // PostgreSQL startup packet
    let postgres_packet = vec![
        0x00, 0x00, 0x00, 0x08, // Length
        0x00, 0x03, 0x00, 0x00, // Protocol version 3.0
    ];

    // Redis RESP packet
    let redis_packet = b"*1\r\n$4\r\nPING\r\n".to_vec();

    group.bench_function("detect_from_port", |b| {
        b.iter(|| {
            let proto = highper_gateway::tcp::protocol::ProtocolDetector::detect_from_port(black_box(3306));
            black_box(proto);
        });
    });

    group.finish();
}

/// Benchmark MySQL protocol operations
fn bench_mysql_protocol(c: &mut Criterion) {
    let mut group = c.benchmark_group("mysql_protocol");

    group.bench_function("ping_packet_creation", |b| {
        b.iter(|| {
            let packet = MysqlProtocol::ping_packet();
            black_box(packet);
        });
    });

    let test_packet = vec![0x05, 0x00, 0x00, 0x00, 0x0e, 0x01, 0x02, 0x03, 0x04];

    group.bench_function("packet_length_extraction", |b| {
        b.iter(|| {
            let len = MysqlProtocol::packet_length(black_box(&test_packet));
            black_box(len);
        });
    });

    group.bench_function("is_complete_packet", |b| {
        b.iter(|| {
            let complete = MysqlProtocol::is_complete_packet(black_box(&test_packet));
            black_box(complete);
        });
    });

    let error_packet = vec![0x01, 0x00, 0x00, 0x00, 0xff]; // Error marker

    group.bench_function("is_error_packet", |b| {
        b.iter(|| {
            let is_error = MysqlProtocol::is_error_packet(black_box(&error_packet));
            black_box(is_error);
        });
    });

    group.finish();
}

/// Benchmark PostgreSQL protocol operations
fn bench_postgresql_protocol(c: &mut Criterion) {
    let mut group = c.benchmark_group("postgresql_protocol");

    group.bench_function("simple_query_creation", |b| {
        b.iter(|| {
            let query = PostgresqlProtocol::simple_query(black_box("SELECT 1"));
            black_box(query);
        });
    });

    let test_message = vec![
        b'Q', // Query type
        0x00, 0x00, 0x00, 0x0d, // Length: 13
        b'S', b'E', b'L', b'E', b'C', b'T', b' ', b'1', 0x00, // "SELECT 1" + null
    ];

    group.bench_function("message_length_extraction", |b| {
        b.iter(|| {
            let len = PostgresqlProtocol::message_length(black_box(&test_message));
            black_box(len);
        });
    });

    group.bench_function("is_complete_message", |b| {
        b.iter(|| {
            let complete = PostgresqlProtocol::is_complete_message(black_box(&test_message));
            black_box(complete);
        });
    });

    let error_message = vec![b'E', 0x00, 0x00, 0x00, 0x05, 0x00];

    group.bench_function("is_error_message", |b| {
        b.iter(|| {
            let is_error = PostgresqlProtocol::is_error_message(black_box(&error_message));
            black_box(is_error);
        });
    });

    group.finish();
}

/// Benchmark Redis protocol operations
fn bench_redis_protocol(c: &mut Criterion) {
    let mut group = c.benchmark_group("redis_protocol");

    group.bench_function("ping_command_creation", |b| {
        b.iter(|| {
            let ping = RedisProtocol::ping_command();
            black_box(ping);
        });
    });

    group.bench_function("command_creation_2_args", |b| {
        b.iter(|| {
            let cmd = RedisProtocol::command(black_box(&["GET", "key"]));
            black_box(cmd);
        });
    });

    group.bench_function("command_creation_3_args", |b| {
        b.iter(|| {
            let cmd = RedisProtocol::command(black_box(&["SET", "key", "value"]));
            black_box(cmd);
        });
    });

    let pong_response = b"+PONG\r\n";

    group.bench_function("is_complete_message_simple", |b| {
        b.iter(|| {
            let complete = RedisProtocol::is_complete_message(black_box(pong_response));
            black_box(complete);
        });
    });

    let error_response = b"-ERR unknown command\r\n";

    group.bench_function("is_error_message", |b| {
        b.iter(|| {
            let is_error = RedisProtocol::is_error_message(black_box(error_response));
            black_box(is_error);
        });
    });

    group.finish();
}

/// Benchmark circuit breaker operations
fn bench_circuit_breaker(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker");

    let rt = tokio::runtime::Runtime::new().unwrap();

    let config = CircuitBreakerConfig::default();
    let cb = CircuitBreaker::new("127.0.0.1:8080".parse().unwrap(), config);

    group.bench_function("is_request_allowed_closed", |b| {
        b.iter(|| {
            rt.block_on(async {
                let allowed = cb.is_request_allowed().await;
                black_box(allowed);
            });
        });
    });

    group.bench_function("record_success", |b| {
        b.iter(|| {
            rt.block_on(async {
                cb.record_success().await;
            });
        });
    });

    group.bench_function("record_failure", |b| {
        b.iter(|| {
            rt.block_on(async {
                cb.record_failure().await;
            });
        });
    });

    group.bench_function("get_state", |b| {
        b.iter(|| {
            let state = cb.get_state();
            black_box(state);
        });
    });

    group.bench_function("stats_snapshot", |b| {
        b.iter(|| {
            let stats = cb.stats();
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark connection pool operations (mock, no actual network)
fn bench_connection_pool_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("connection_pool");

    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pool stats operations (no network)
    let pool = TcpConnectionPool::new(
        "127.0.0.1:3306".parse().unwrap(),
        100,
        10,
        Duration::from_secs(3600),
    );

    group.bench_function("stats_snapshot", |b| {
        b.iter(|| {
            let stats = pool.stats();
            black_box(stats);
        });
    });

    group.finish();
}

/// Benchmark atomics and lock-free operations
fn bench_atomics(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};

    let mut group = c.benchmark_group("atomics");

    let counter = AtomicU64::new(0);

    group.bench_function("fetch_add_relaxed", |b| {
        b.iter(|| {
            counter.fetch_add(1, Ordering::Relaxed);
        });
    });

    group.bench_function("load_relaxed", |b| {
        b.iter(|| {
            let val = counter.load(Ordering::Relaxed);
            black_box(val);
        });
    });

    group.bench_function("store_relaxed", |b| {
        b.iter(|| {
            counter.store(black_box(42), Ordering::Relaxed);
        });
    });

    group.bench_function("swap_seqcst", |b| {
        b.iter(|| {
            let old = counter.swap(black_box(100), Ordering::SeqCst);
            black_box(old);
        });
    });

    group.finish();
}

/// Benchmark throughput with varying payload sizes
fn bench_throughput_by_size(c: &mut Criterion) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut group = c.benchmark_group("throughput");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::new("hash_payload", size), size, |b, &size| {
            let payload = vec![0u8; size];
            b.iter(|| {
                let mut hasher = DefaultHasher::new();
                payload.hash(&mut hasher);
                let hash = hasher.finish();
                black_box(hash);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_protocol_detection,
    bench_mysql_protocol,
    bench_postgresql_protocol,
    bench_redis_protocol,
    bench_circuit_breaker,
    bench_connection_pool_ops,
    bench_atomics,
    bench_throughput_by_size,
);

criterion_main!(benches);
