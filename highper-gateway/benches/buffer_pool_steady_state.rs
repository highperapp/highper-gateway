//! Steady-state buffer pool benchmark with long-lived threads
//!
//! This benchmark properly measures per-thread caching by using long-lived
//! threads that can benefit from thread-local caches.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use highper_gateway::runtime::BufferPool;
use std::sync::{Arc, Barrier};
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

/// Benchmark buffer pool with long-lived worker threads (steady-state)
fn bench_buffer_pool_steady_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_steady_state");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                // Create long-lived worker threads
                let pool = Arc::new(BufferPool::new());
                let start_barrier = Arc::new(Barrier::new(num_threads + 1));
                let stop_flag = Arc::new(AtomicBool::new(false));
                let mut workers = vec![];

                // Spawn worker threads
                for _ in 0..num_threads {
                    let pool = Arc::clone(&pool);
                    let start_barrier = Arc::clone(&start_barrier);
                    let stop_flag = Arc::clone(&stop_flag);

                    let worker = thread::spawn(move || {
                        // Wait for benchmark to start
                        start_barrier.wait();

                        // Warm up thread-local cache
                        for _ in 0..10 {
                            let buf = pool.get(4096);
                            pool.put(buf);
                        }

                        // Run until stop flag is set
                        while !stop_flag.load(Ordering::Relaxed) {
                            let buf = pool.get(black_box(4096));
                            pool.put(buf);
                        }
                    });

                    workers.push(worker);
                }

                // Benchmark the workers
                b.iter(|| {
                    // Reset stop flag
                    stop_flag.store(false, Ordering::Relaxed);

                    // Start all workers
                    start_barrier.wait();

                    // Let workers run for a bit (100 operations per thread)
                    thread::sleep(std::time::Duration::from_micros(100 * num_threads as u64));

                    // Stop workers
                    stop_flag.store(true, Ordering::Relaxed);
                });

                // Cleanup: join all workers
                drop(start_barrier);
                stop_flag.store(true, Ordering::Relaxed);
                for worker in workers {
                    let _ = worker.join();
                }
            },
        );
    }

    group.finish();
}

/// Simpler benchmark: measure steady-state per-thread get/put
fn bench_buffer_pool_per_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_per_thread");

    for num_threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &num_threads| {
                let pool = Arc::new(BufferPool::new());

                b.iter_custom(|iters| {
                    let barrier = Arc::new(Barrier::new(num_threads + 1));
                    let mut workers = vec![];

                    for _ in 0..num_threads {
                        let pool = Arc::clone(&pool);
                        let barrier = Arc::clone(&barrier);
                        let iters_per_thread = iters / num_threads as u64;

                        let worker = thread::spawn(move || {
                            // Warm up thread-local cache
                            for _ in 0..10 {
                                let buf = pool.get(4096);
                                pool.put(buf);
                            }

                            // Wait for all threads to be ready
                            barrier.wait();

                            // Measure this
                            let start = std::time::Instant::now();
                            for _ in 0..iters_per_thread {
                                let buf = pool.get(black_box(4096));
                                pool.put(buf);
                            }
                            start.elapsed()
                        });

                        workers.push(worker);
                    }

                    // Signal workers to start
                    barrier.wait();

                    // Collect results (take max time)
                    workers.into_iter()
                        .map(|w| w.join().unwrap())
                        .max()
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    steady_state_benches,
    bench_buffer_pool_per_thread,
);

criterion_main!(steady_state_benches);
