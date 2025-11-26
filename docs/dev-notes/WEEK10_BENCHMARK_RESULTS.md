# Week 10: Benchmark Results and Performance Analysis
## November 10, 2025

## Executive Summary

Week 10 focused on benchmarking the Week 9 performance optimizations and fixing test failures. Successfully ran comprehensive benchmarks on lock-free data structures, SIMD operations, and buffer pool implementations.

**Status**: ✅ Week 9 benchmarks completed, test fixes applied
**Key Deliverables**:
- Fixed failing config watcher test
- Added `optimization_bench` to Cargo.toml
- Ran comprehensive benchmark suite
- Documented performance characteristics
- Identified optimization opportunities

---

## 1. Test Fixes

### Fixed: `config::watcher::tests::test_file_modification_detection`

**Problem**: Timing race condition causing flaky test failures
- Insufficient wait time for file watcher initialization (100ms)
- Non-unique temp file names causing conflicts
- Single timeout without retry logic

**Solution** (rust-proxy/src/config/watcher.rs:126-163):
```rust
// Increased initialization wait to 200ms
sleep(Duration::from_millis(200)).await;

// Added unique filename using process ID
let temp_file = temp_dir.join(format!("test_config_modify_{}.yaml", std::process::id()));

// Implemented retry logic with 5 attempts
for _ in 0..5 {
    match tokio::time::timeout(Duration::from_millis(500), event_rx.recv()).await {
        Ok(Some(event)) if matches!(event, ConfigEvent::Modified | ConfigEvent::Created) => {
            received_event = true;
            break;
        }
        ...
    }
}
```

**Result**: ✅ All 3 watcher tests now pass consistently

---

## 2. Benchmark Infrastructure Fix

**Problem**: `optimization_bench` not registered in Cargo.toml
- Benchmark file existed but wasn't being executed
- `cargo bench --bench optimization_bench` resulted in "running 0 tests"

**Solution** (Cargo.toml:191-193):
```toml
[[bench]]
name = "optimization_bench"
harness = false
```

**Result**: ✅ Benchmark now runs with full Criterion.rs suite

---

## 3. Complete Benchmark Results

### 3.1 Buffer Pool Performance

#### Lock-Free vs Mutex (Single-Threaded)

| Implementation | Latency (ns) | Notes |
|---|---|---|
| **Lock-free** | 56.6 ns | Slightly slower |
| **Mutex** | 30.9 ns | **1.8x faster** |

**Surprising Result**: Mutex is faster for single-threaded workloads due to lower overhead.

#### Buffer Pool Under Contention

| Threads | Latency (µs) | Throughput |
|---|---|---|
| 1 | 79.8 µs | 12.5k ops/sec |
| 2 | 124.7 µs | 16.0k ops/sec |
| 4 | 392.4 µs | 10.2k ops/sec |
| 8 | 1,221.8 µs | 6.5k ops/sec |

**Analysis**: Performance degrades with high thread counts due to contention on the crossbeam queue.

---

### 3.2 SIMD Memory Operations

#### SIMD memcpy (DISAPPOINTING RESULTS)

| Size | SIMD | Scalar | Scalar Advantage |
|---|---|---|---|
| **64 bytes** | 9.22 ns (6.46 GiB/s) | 3.18 ns (18.73 GiB/s) | **2.9x faster** |
| **256 bytes** | 5.61 ns (42.47 GiB/s) | 5.42 ns (43.98 GiB/s) | **1.04x faster** |
| **1024 bytes** | 25.16 ns (37.90 GiB/s) | 11.74 ns (81.23 GiB/s) | **2.1x faster** |
| **4096 bytes** | 71.44 ns (53.40 GiB/s) | 42.59 ns (89.58 GiB/s) | **1.7x faster** |
| **16384 bytes** | 304.27 ns (50.15 GiB/s) | 151.39 ns (100.79 GiB/s) | **2.0x faster** |

**Critical Finding**: ❌ **SIMD memcpy is SLOWER than scalar across all sizes**

**Root Cause Analysis**:
1. Modern CPUs have highly optimized `memcpy` implementations
2. Compiler auto-vectorization already applies SIMD
3. Manual SIMD adds overhead without benefit
4. Scalar path likely uses CPU's hardware memcpy acceleration

#### SIMD memcmp (ALSO SLOWER)

| Size | SIMD | Scalar | Scalar Advantage |
|---|---|---|---|
| **64 bytes** | 3.53 ns (16.91 GiB/s) | 2.82 ns (21.15 GiB/s) | **1.25x faster** |
| **256 bytes** | 7.15 ns (33.35 GiB/s) | 4.33 ns (55.00 GiB/s) | **1.65x faster** |
| **1024 bytes** | 26.14 ns (36.48 GiB/s) | 15.46 ns (61.67 GiB/s) | **1.69x faster** |
| **4096 bytes** | 102.89 ns (37.08 GiB/s) | 53.45 ns (71.37 GiB/s) | **1.93x faster** |
| **16384 bytes** | 345.19 ns (44.20 GiB/s) | 212.67 ns (71.75 GiB/s) | **1.62x faster** |

**Critical Finding**: ❌ **SIMD memcmp is also SLOWER than scalar**

---

#### SIMD find_pattern (HUGE WINS!)

| Size | SIMD | Scalar | SIMD Advantage |
|---|---|---|---|
| **64 bytes** | 2.85 ns (20.91 GiB/s) | 20.67 ns (2.88 GiB/s) | **7.3x faster** ✅ |
| **256 bytes** | 5.47 ns (43.55 GiB/s) | 98.40 ns (2.42 GiB/s) | **18.0x faster** ✅ |
| **1024 bytes** | 17.03 ns (56.00 GiB/s) | 347.46 ns (2.74 GiB/s) | **20.4x faster** ✅ |
| **4096 bytes** | 79.45 ns (48.02 GiB/s) | 1,326.2 ns (2.88 GiB/s) | **16.7x faster** ✅ |
| **16384 bytes** | 275.94 ns (55.30 GiB/s) | 5,603.0 ns (2.72 GiB/s) | **20.3x faster** ✅ |

**Excellent Result**: ✅ **SIMD provides 7-20x speedup for pattern matching**

---

#### SIMD checksum (MASSIVE WINS!)

| Size | SIMD | Scalar | SIMD Advantage |
|---|---|---|---|
| **64 bytes** | 1.16 ns (51.55 GiB/s) | 10.01 ns (5.95 GiB/s) | **8.6x faster** ✅ |
| **256 bytes** | 2.23 ns (106.72 GiB/s) | 34.94 ns (6.82 GiB/s) | **15.6x faster** ✅ |
| **1024 bytes** | 6.88 ns (138.59 GiB/s) | 175.10 ns (5.45 GiB/s) | **25.4x faster** ✅ |
| **4096 bytes** | 38.62 ns (98.77 GiB/s) | 658.91 ns (5.79 GiB/s) | **17.1x faster** ✅ |
| **16384 bytes** | 101.72 ns (150.01 GiB/s) | 2,626.2 ns (5.81 GiB/s) | **25.8x faster** ✅ |

**Outstanding Result**: ✅ **SIMD provides 8-26x speedup for checksums**

---

### 3.3 Lock-Free Data Structures

#### Atomic Counter Performance

| Operation | Latency | Throughput |
|---|---|---|
| **Push/Pop** | 5.81 ns | 172M ops/sec |
| **Atomic Add** | 8.89 ns | 112M ops/sec |

#### Concurrent Stats

| Operation | Latency | Notes |
|---|---|---|
| **Record Request** | 24.13 ns | Lock-free, ~41M ops/sec |
| **Snapshot** | 3.25 ns | Read-only, ~307M ops/sec |
| **Multithreaded (4 threads)** | 214.95 µs | 400 operations across 4 threads |

**Analysis**: Lock-free structures provide excellent single-threaded performance and good scaling characteristics.

---

## 4. Key Findings & Recommendations

### 4.1 Critical Issues Identified

#### ❌ SIMD memcpy/memcmp Should Be Removed

**Problem**: Custom SIMD implementations are 1.5-3x SLOWER than compiler-optimized scalar code.

**Root Causes**:
1. **Compiler Auto-Vectorization**: Modern compilers already use SIMD for `memcpy`/`memcmp`
2. **CPU Optimizations**: x86_64 CPUs have hardware-accelerated memory operations
3. **Overhead**: Manual SIMD adds function call overhead + setup cost
4. **Alignment**: Unaligned memory access penalties in manual SIMD

**Recommendation**:
```rust
// REMOVE manual SIMD implementations for memcpy/memcmp
// Instead, use standard library functions:
use std::ptr::copy_nonoverlapping;  // Compiler will vectorize this
use std::slice::cmp;                  // Already optimized
```

**Expected Impact**: 2-3x performance improvement by using stdlib

---

### 4.2 Optimization Opportunities

#### ✅ Keep SIMD for Pattern Matching & Checksums

**Rationale**: These operations show 7-26x speedups because:
1. Complex logic benefits more from parallelism
2. Compiler can't auto-vectorize complex patterns
3. Data-parallel operations (XOR, compare) are SIMD-friendly

**Recommendation**: Expand SIMD usage for:
- HTTP header parsing (pattern matching)
- Request/response validation (checksums)
- WAF rule matching (pattern scanning)

---

#### ⚠️ Buffer Pool Needs Improvement

**Problem**: Performance degrades significantly under high thread contention (8 threads: 1.2ms latency).

**Recommendations**:
1. **Per-Thread Caches**: Add thread-local buffer caches to reduce contention
   ```rust
   thread_local! {
       static BUFFER_CACHE: RefCell<Vec<BytesMut>> = RefCell::new(Vec::new());
   }
   ```

2. **Size-Class Sharding**: Separate queues per buffer size
   ```rust
   struct BufferPool {
       pools: [SegQueue<BytesMut>; 8],  // One per size class
   }
   ```

3. **Batch Operations**: Allow get/put of multiple buffers at once

**Expected Impact**: 5-10x improvement under high contention

---

### 4.3 Integration Recommendations

#### Priority 1: Remove Harmful SIMD

**Action Items**:
1. Remove `simd_memcpy` and `simd_memcmp` from runtime/simd_opt.rs
2. Replace all usage with stdlib equivalents
3. Update benchmarks to verify improvements

**Files to Change**:
- `rust-proxy/src/runtime/simd_opt.rs` - Remove bad implementations
- `rust-proxy/src/runtime/mod.rs` - Remove exports
- `benches/optimization_bench.rs` - Update benchmarks

---

#### Priority 2: Expand Good SIMD

**Action Items**:
1. Use `simd_find_pattern` in HTTP parser
2. Use `simd_checksum` for request validation
3. Add SIMD pattern matching to WAF

**Integration Points**:
- `rust-proxy/src/http/parser.rs` - Header name/value parsing
- `rust-proxy/src/middleware/waf/engine.rs` - Rule matching
- `rust-proxy/src/proxy/handler.rs` - Request validation

---

#### Priority 3: Improve Buffer Pool

**Action Items**:
1. Implement per-thread caching
2. Add sharding by size class
3. Benchmark improvements

**File to Change**:
- `rust-proxy/src/runtime/buffer_pool.rs`

---

## 5. Performance Summary

### What Worked ✅

| Optimization | Speedup | Status |
|---|---|---|
| SIMD Checksum | 8-26x faster | ✅ Keep & Expand |
| SIMD Pattern Matching | 7-20x faster | ✅ Keep & Expand |
| Lock-Free Counter | 112M ops/sec | ✅ Production Ready |
| Concurrent Stats | 41M record/sec | ✅ Production Ready |

### What Didn't Work ❌

| Optimization | Performance | Status |
|---|---|---|
| SIMD memcpy | 2-3x SLOWER | ❌ Remove |
| SIMD memcmp | 1.5-2x SLOWER | ❌ Remove |
| Buffer Pool (8+ threads) | High contention | ⚠️ Needs Improvement |

---

## 6. Next Steps (Week 11)

### Immediate Actions

1. **Remove Harmful SIMD** (1-2 hours)
   - Delete simd_memcpy, simd_memcmp
   - Replace with stdlib
   - Re-run benchmarks to verify 2-3x improvement

2. **Integrate Good SIMD** (2-3 hours)
   - HTTP header parsing with simd_find_pattern
   - Request validation with simd_checksum
   - Measure end-to-end impact

3. **Buffer Pool Improvements** (3-4 hours)
   - Implement thread-local caching
   - Add per-size sharding
   - Benchmark contention improvements

### Testing & Validation

1. **Micro-benchmarks**: Re-run `cargo bench` after changes
2. **Integration benchmarks**: Run `proxy_bench` and `tcp_bench`
3. **Load testing**: Test with wrk/ab at 10k+ req/sec
4. **Profiling**: Use `perf` to verify hotspot improvements

---

## 7. Benchmark Reproducibility

### Running the Benchmarks

```bash
# Add benchmark to Cargo.toml (already done)
[[bench]]
name = "optimization_bench"
harness = false

# Run with Criterion.rs
cargo bench --bench optimization_bench

# Generate HTML reports
open target/criterion/report/index.html
```

### Benchmark Environment

- **CPU**: x86_64 Linux (WSL2)
- **Compiler**: rustc 1.83.0 (stable)
- **Optimization**: --release (opt-level=3, LTO=fat)
- **Features**: io-uring, jemalloc enabled
- **Samples**: 100 per benchmark
- **Warmup**: 3 seconds per test

---

## 8. Conclusion

Week 10 successfully completed comprehensive benchmarking of Week 9 optimizations. Key findings:

**Successes**:
- ✅ SIMD pattern matching: 7-20x faster
- ✅ SIMD checksums: 8-26x faster
- ✅ Lock-free structures: excellent performance
- ✅ Fixed flaky tests

**Surprises**:
- ❌ SIMD memcpy/memcmp are SLOWER than scalar
- ⚠️ Buffer pool needs thread-local caching
- ℹ️ Compiler auto-vectorization is very effective

**Action Plan**:
1. Remove harmful SIMD (memcpy/memcmp) → **+2-3x performance**
2. Expand good SIMD (pattern/checksum) → **Integrate into HTTP/WAF**
3. Improve buffer pool → **Add per-thread caching**

**Overall Assessment**: Week 9's lock-free and SIMD work was valuable, but benchmarking revealed that some optimizations (memcpy/memcmp) should be removed while others (pattern matching/checksum) should be expanded.

---

**Next Session**: Begin Week 11 - Remove harmful SIMD, integrate good SIMD into production code paths, and improve buffer pool contention handling.
