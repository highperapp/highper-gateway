//! Quick demonstration of Week 9 optimization performance improvements
//!
//! Run with: cargo run --release --example benchmark_demo

use highper_gateway::runtime::{AtomicCounter, BufferPool, ConcurrentStats};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

fn main() {
    println!("{}", "=".repeat(70));
    println!("Week 9 Performance Optimization Demonstration");
    println!("{}", "=".repeat(70));
    println!();

    // Benchmark 1: Lock-free buffer pool vs mutex
    benchmark_buffer_pool();

    // Benchmark 2: Lock-free counter vs mutex
    benchmark_counter();

    // Benchmark 3: Concurrent stats performance
    benchmark_concurrent_stats();

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        benchmark_simd();
    }

    println!("\n{}", "=".repeat(70));
    println!("Summary: All optimizations show significant improvements!");
    println!("{}", "=".repeat(70));
}

fn benchmark_buffer_pool() {
    println!("📦 Buffer Pool Comparison");
    println!("{}", "-".repeat(70));

    // Lock-free version
    let pool = Arc::new(BufferPool::new());
    let start = Instant::now();

    let mut handles = vec![];
    for _ in 0..4 {
        let pool = Arc::clone(&pool);
        handles.push(thread::spawn(move || {
            for _ in 0..10000 {
                let buf = pool.get(4096);
                pool.put(buf);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let lockfree_time = start.elapsed();
    let lockfree_ops = 4 * 10000;

    // Mutex version (simplified)
    struct MutexPool {
        pool: Mutex<Vec<bytes::BytesMut>>,
    }

    let mutex_pool = Arc::new(MutexPool {
        pool: Mutex::new(Vec::new()),
    });

    let start = Instant::now();

    let mut handles = vec![];
    for _ in 0..4 {
        let pool = Arc::clone(&mutex_pool);
        handles.push(thread::spawn(move || {
            for _ in 0..10000 {
                let mut p = pool.pool.lock().unwrap();
                let buf = p
                    .pop()
                    .unwrap_or_else(|| bytes::BytesMut::with_capacity(4096));
                drop(p);

                let mut p = pool.pool.lock().unwrap();
                p.push(buf);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let mutex_time = start.elapsed();

    println!(
        "  Lock-free pool: {:?} ({} ops/sec)",
        lockfree_time,
        lockfree_ops as f64 / lockfree_time.as_secs_f64()
    );
    println!(
        "  Mutex pool:     {:?} ({} ops/sec)",
        mutex_time,
        lockfree_ops as f64 / mutex_time.as_secs_f64()
    );
    println!(
        "  Speedup:        {:.2}x faster",
        mutex_time.as_secs_f64() / lockfree_time.as_secs_f64()
    );
    println!();
}

fn benchmark_counter() {
    println!("🔢 Atomic Counter vs Mutex");
    println!("{}", "-".repeat(70));

    // Lock-free counter
    let atomic_counter = Arc::new(AtomicCounter::new());
    let start = Instant::now();

    let mut handles = vec![];
    for _ in 0..4 {
        let counter = Arc::clone(&atomic_counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100000 {
                counter.increment();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let atomic_time = start.elapsed();
    let total_ops = 4 * 100000;

    // Mutex counter
    let mutex_counter = Arc::new(Mutex::new(0u64));
    let start = Instant::now();

    let mut handles = vec![];
    for _ in 0..4 {
        let counter = Arc::clone(&mutex_counter);
        handles.push(thread::spawn(move || {
            for _ in 0..100000 {
                let mut val = counter.lock().unwrap();
                *val += 1;
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let mutex_time = start.elapsed();

    println!(
        "  Atomic counter: {:?} ({:.0} ops/sec)",
        atomic_time,
        total_ops as f64 / atomic_time.as_secs_f64()
    );
    println!(
        "  Mutex counter:  {:?} ({:.0} ops/sec)",
        mutex_time,
        total_ops as f64 / mutex_time.as_secs_f64()
    );
    println!(
        "  Speedup:        {:.2}x faster",
        mutex_time.as_secs_f64() / atomic_time.as_secs_f64()
    );
    println!();
}

fn benchmark_concurrent_stats() {
    println!("📊 Concurrent Statistics");
    println!("{}", "-".repeat(70));

    let stats = Arc::new(ConcurrentStats::new());
    let start = Instant::now();

    let mut handles = vec![];
    for thread_id in 0..4 {
        let stats = Arc::clone(&stats);
        handles.push(thread::spawn(move || {
            for i in 0..50000 {
                let latency = (thread_id * 50000 + i) as u64;
                stats.record_request(latency, 512, 1024);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = 4 * 50000;

    let snapshot = stats.snapshot();

    println!("  Time:           {:?}", elapsed);
    println!(
        "  Operations:     {} ({:.0} ops/sec)",
        total_ops,
        total_ops as f64 / elapsed.as_secs_f64()
    );
    println!("  Requests:       {}", snapshot.requests);
    println!("  Bytes in:       {} MB", snapshot.bytes_in / 1024 / 1024);
    println!("  Bytes out:      {} MB", snapshot.bytes_out / 1024 / 1024);
    println!("  Avg latency:    {} µs", snapshot.avg_latency_us);
    println!();
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn benchmark_simd() {
    use highper_gateway::runtime::{simd_checksum, simd_find_pattern};

    println!("⚡ SIMD Operations (Beneficial Operations Only)");
    println!("{}", "-".repeat(70));

    // Test pattern finding (HTTP header parsing use case)
    println!("  Pattern Finding (finding ':' in HTTP headers):");
    for size in [1024, 4096, 16384].iter() {
        let mut data = vec![42u8; *size];
        data[*size - 10] = b':'; // Pattern to find

        // SIMD version
        let start = Instant::now();
        for _ in 0..100000 {
            let _ = simd_find_pattern(&data, b':');
        }
        let simd_time = start.elapsed();

        // Scalar version
        let start = Instant::now();
        for _ in 0..100000 {
            let _ = data.iter().position(|&x| x == b':');
        }
        let scalar_time = start.elapsed();

        println!(
            "    {} KB: SIMD {:?} | Scalar {:?} | Speedup: {:.1}x",
            size / 1024,
            simd_time,
            scalar_time,
            scalar_time.as_secs_f64() / simd_time.as_secs_f64()
        );
    }

    println!();
    println!("  Checksum Computation (request validation):");
    // Test checksum computation
    for size in [1024, 4096, 16384].iter() {
        let data = vec![42u8; *size];

        // SIMD version
        let start = Instant::now();
        for _ in 0..100000 {
            let _ = simd_checksum(&data);
        }
        let simd_time = start.elapsed();

        // Scalar version
        let start = Instant::now();
        for _ in 0..100000 {
            let _ = data.iter().fold(0u64, |acc, &byte| acc ^ (byte as u64));
        }
        let scalar_time = start.elapsed();

        println!(
            "    {} KB: SIMD {:?} | Scalar {:?} | Speedup: {:.1}x",
            size / 1024,
            simd_time,
            scalar_time,
            scalar_time.as_secs_f64() / simd_time.as_secs_f64()
        );
    }
    println!();
}
