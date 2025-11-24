//! Benchmarks for Week 9 performance optimizations
//!
//! This benchmark suite measures the performance improvements from:
//! - Lock-free buffer pool vs mutex-based pool
//! - SIMD-accelerated operations vs scalar implementations
//! - Lock-free data structures vs traditional mutex-based structures

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use highper_gateway::runtime::{BufferPool, AtomicCounter, ConcurrentStats, WorkStealingQueue};
use bytes::BytesMut;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
use highper_gateway::runtime::{simd_find_pattern, simd_checksum};

// ============================================================================
// Buffer Pool Benchmarks
// ============================================================================

/// Benchmark lock-free buffer pool allocation
fn bench_buffer_pool_lockfree(c: &mut Criterion) {
    let pool = BufferPool::new();

    c.bench_function("buffer_pool_lockfree_alloc", |b| {
        b.iter(|| {
            let buf = pool.get(black_box(4096));
            pool.put(buf);
        });
    });
}

/// Benchmark lock-free buffer pool under contention
fn bench_buffer_pool_contended(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_contention");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let pool = Arc::new(BufferPool::new());

                b.iter(|| {
                    let mut handles = vec![];

                    for _ in 0..num_threads {
                        let pool = Arc::clone(&pool);
                        handles.push(thread::spawn(move || {
                            for _ in 0..100 {
                                let buf = pool.get(black_box(4096));
                                pool.put(buf);
                            }
                        }));
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark mutex-based buffer pool (for comparison)
fn bench_buffer_pool_mutex(c: &mut Criterion) {
    // Simple mutex-based pool for comparison
    struct MutexPool {
        pool: Mutex<Vec<BytesMut>>,
        size: usize,
    }

    impl MutexPool {
        fn new(size: usize) -> Self {
            Self {
                pool: Mutex::new(Vec::new()),
                size,
            }
        }

        fn get(&self) -> BytesMut {
            let mut pool = self.pool.lock().unwrap();
            pool.pop().unwrap_or_else(|| BytesMut::with_capacity(self.size))
        }

        fn put(&self, mut buf: BytesMut) {
            buf.clear();
            let mut pool = self.pool.lock().unwrap();
            if pool.len() < 100 {
                pool.push(buf);
            }
        }
    }

    let pool = MutexPool::new(4096);

    c.bench_function("buffer_pool_mutex_alloc", |b| {
        b.iter(|| {
            let buf = pool.get();
            pool.put(buf);
        });
    });
}

// ============================================================================
// SIMD Benchmarks
// ============================================================================
// NOTE: simd_memcpy and simd_memcmp benchmarks removed - they were 2-3x SLOWER than stdlib
// See WEEK10_BENCHMARK_RESULTS.md and SIMD_DEPRECATION_NOTICE.md for details

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn bench_simd_find_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_pattern");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let mut data = vec![42u8; *size];
        data[*size - 10] = 255; // Pattern to find

        // SIMD version
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            size,
            |b, _| {
                b.iter(|| {
                    simd_find_pattern(black_box(&data), black_box(255));
                });
            },
        );

        // Scalar version
        group.bench_with_input(
            BenchmarkId::new("scalar", size),
            size,
            |b, _| {
                b.iter(|| {
                    black_box(&data).iter().position(|&x| x == black_box(255))
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn bench_simd_checksum(c: &mut Criterion) {
    let mut group = c.benchmark_group("checksum");

    for size in [64, 256, 1024, 4096, 16384].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        let data = vec![42u8; *size];

        // SIMD version
        group.bench_with_input(
            BenchmarkId::new("simd", size),
            size,
            |b, _| {
                b.iter(|| {
                    simd_checksum(black_box(&data));
                });
            },
        );

        // Scalar version
        group.bench_with_input(
            BenchmarkId::new("scalar", size),
            size,
            |b, _| {
                b.iter(|| {
                    black_box(&data).iter().fold(0u64, |acc, &byte| acc ^ (byte as u64))
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Lock-Free Structure Benchmarks
// ============================================================================

fn bench_atomic_counter(c: &mut Criterion) {
    let counter = AtomicCounter::new();

    c.bench_function("atomic_counter_increment", |b| {
        b.iter(|| {
            counter.increment();
        });
    });

    c.bench_function("atomic_counter_add", |b| {
        b.iter(|| {
            counter.add(black_box(10));
        });
    });
}

fn bench_atomic_counter_vs_mutex(c: &mut Criterion) {
    let mut group = c.benchmark_group("counter_comparison");

    // Atomic counter
    let atomic_counter = Arc::new(AtomicCounter::new());
    group.bench_function("atomic_counter_contended", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..4 {
                let counter = Arc::clone(&atomic_counter);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        counter.increment();
                    }
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Mutex counter
    let mutex_counter = Arc::new(Mutex::new(0u64));
    group.bench_function("mutex_counter_contended", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..4 {
                let counter = Arc::clone(&mutex_counter);
                handles.push(thread::spawn(move || {
                    for _ in 0..100 {
                        let mut val = counter.lock().unwrap();
                        *val += 1;
                    }
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

fn bench_work_stealing_queue(c: &mut Criterion) {
    let queue = WorkStealingQueue::new();

    c.bench_function("work_stealing_queue_push_pop", |b| {
        b.iter(|| {
            queue.push(black_box(42));
            queue.pop();
        });
    });
}

fn bench_concurrent_stats(c: &mut Criterion) {
    let stats = ConcurrentStats::new();

    c.bench_function("concurrent_stats_record", |b| {
        b.iter(|| {
            stats.record_request(black_box(100), black_box(512), black_box(1024));
        });
    });

    c.bench_function("concurrent_stats_snapshot", |b| {
        b.iter(|| {
            stats.snapshot()
        });
    });
}

fn bench_concurrent_stats_contended(c: &mut Criterion) {
    let stats = Arc::new(ConcurrentStats::new());

    c.bench_function("concurrent_stats_multithreaded", |b| {
        b.iter(|| {
            let mut handles = vec![];
            for _ in 0..4 {
                let stats = Arc::clone(&stats);
                handles.push(thread::spawn(move || {
                    for i in 0..100 {
                        stats.record_request(i as u64, 512, 1024);
                    }
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }
        });
    });
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    buffer_pool_benches,
    bench_buffer_pool_lockfree,
    bench_buffer_pool_mutex,
    bench_buffer_pool_contended,
);

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
criterion_group!(
    simd_benches,
    bench_simd_find_pattern,
    bench_simd_checksum,
);

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
criterion_group!(simd_benches,);

criterion_group!(
    lockfree_benches,
    bench_atomic_counter,
    bench_atomic_counter_vs_mutex,
    bench_work_stealing_queue,
    bench_concurrent_stats,
    bench_concurrent_stats_contended,
);

criterion_main!(buffer_pool_benches, simd_benches, lockfree_benches);
