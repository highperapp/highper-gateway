# SIMD Function Deprecation Notice
## Date: November 10, 2025

## Summary

Based on comprehensive benchmark analysis (see `WEEK10_BENCHMARK_RESULTS.md`), we have deprecated two SIMD functions that performed **worse** than their standard library equivalents, while keeping two that showed excellent performance gains.

---

## Deprecated Functions ❌

### `simd_memcpy` - DEPRECATED

**Reason**: 2-3x SLOWER than standard library
**Replacement**: Use `std::ptr::copy_nonoverlapping` or slice copy

```rust
// ❌ OLD (slow)
use highper_gateway::runtime::simd_memcpy;
simd_memcpy(&mut dst, &src);

// ✅ NEW (2-3x faster)
dst.copy_from_slice(&src);
// or
unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), len); }
```

**Benchmark Results**:
| Size | SIMD | Stdlib | Stdlib Advantage |
|---|---|---|---|
| 64 bytes | 9.22 ns | 3.18 ns | **2.9x faster** |
| 1024 bytes | 25.16 ns | 11.74 ns | **2.1x faster** |
| 16KB | 304.27 ns | 151.39 ns | **2.0x faster** |

---

### `simd_memcmp` - DEPRECATED

**Reason**: 1.5-2x SLOWER than standard library
**Replacement**: Use slice equality `a == b`

```rust
// ❌ OLD (slow)
use highper_gateway::runtime::simd_memcmp;
if simd_memcmp(&a, &b) { ... }

// ✅ NEW (1.5-2x faster)
if a == b { ... }
// or explicitly
if a.eq(&b) { ... }
```

**Benchmark Results**:
| Size | SIMD | Stdlib | Stdlib Advantage |
|---|---|---|---|
| 64 bytes | 3.53 ns | 2.82 ns | **1.25x faster** |
| 1024 bytes | 26.14 ns | 15.46 ns | **1.69x faster** |
| 16KB | 345.19 ns | 212.67 ns | **1.62x faster** |

---

## Recommended Functions ✅

### `simd_find_pattern` - KEEP & EXPAND

**Performance**: 7-20x FASTER than scalar search
**Use Cases**: HTTP header parsing, WAF pattern matching

```rust
use highper_gateway::runtime::simd_find_pattern;

// Find byte pattern in haystack
if let Some(pos) = simd_find_pattern(haystack, b':') {
    // Process found position
}
```

**Benchmark Results**:
| Size | SIMD | Scalar | SIMD Advantage |
|---|---|---|---|
| 256 bytes | 5.47 ns | 98.40 ns | **18.0x faster** ✅ |
| 1024 bytes | 17.03 ns | 347.46 ns | **20.4x faster** ✅ |
| 16KB | 275.94 ns | 5,603 ns | **20.3x faster** ✅ |

---

### `simd_checksum` - KEEP & EXPAND

**Performance**: 8-26x FASTER than scalar checksums
**Use Cases**: Request validation, data integrity checks

```rust
use highper_gateway::runtime::simd_checksum;

// Calculate XOR checksum
let checksum = simd_checksum(&data);
```

**Benchmark Results**:
| Size | SIMD | Scalar | SIMD Advantage |
|---|---|---|---|
| 256 bytes | 2.23 ns | 34.94 ns | **15.6x faster** ✅ |
| 1024 bytes | 6.88 ns | 175.10 ns | **25.4x faster** ✅ |
| 16KB | 101.72 ns | 2,626 ns | **25.8x faster** ✅ |

---

## Root Cause Analysis

### Why Manual SIMD Failed for memcpy/memcmp

1. **Compiler Auto-Vectorization**: Modern Rust/LLVM compilers already apply SIMD to `copy_from_slice` and slice equality
2. **Hardware Acceleration**: x86_64 CPUs have specialized `rep movsb` instructions for memory copies
3. **Alignment Overhead**: Manual SIMD requires handling unaligned access, adding overhead
4. **Function Call Cost**: Extra function call overhead without corresponding benefit

### Why Manual SIMD Succeeded for Pattern Matching & Checksums

1. **Complex Logic**: Compilers can't auto-vectorize complex search patterns
2. **Data Parallelism**: XOR operations and byte comparisons are naturally SIMD-friendly
3. **Hot Path Optimization**: These operations appear in critical parsing paths
4. **No Hardware Equivalent**: No specialized CPU instructions for these operations

---

## Migration Guide

### For Library Users

If you were using the deprecated functions (unlikely, as they weren't exported), update your code:

```rust
// Before
use highper_gateway::runtime::{simd_memcpy, simd_memcmp};

// After - these are no longer exported!
// Use stdlib instead (which is faster anyway)
```

### For Contributors

1. **Do NOT** add new usages of `simd_memcpy` or `simd_memcmp`
2. **DO** use `simd_find_pattern` for:
   - HTTP header name/value search
   - URL path parsing
   - WAF pattern matching
3. **DO** use `simd_checksum` for:
   - Request validation
   - Data integrity checks
   - Simple hash computations

---

## Code Changes Made

### 1. Updated `highper-gateway/src/runtime/mod.rs`

```diff
- pub use simd_opt::{simd_memcpy, simd_memcmp, simd_find_pattern, simd_checksum};
+ // Export SIMD optimizations (only the beneficial ones - see WEEK10_BENCHMARK_RESULTS.md)
+ // NOTE: simd_memcpy and simd_memcmp are NOT exported as benchmarks showed they are 2-3x SLOWER
+ pub use simd_opt::{simd_find_pattern, simd_checksum};
```

### 2. Deprecated Functions in `highper-gateway/src/runtime/simd_opt.rs`

Added `#[deprecated]` attributes to `simd_memcpy` and `simd_memcmp` with clear warnings.

---

## Next Steps

### Week 11 Integration (In Progress)

1. ✅ Deprecate harmful SIMD (memcpy/memcmp)
2. 🔄 Integrate `simd_find_pattern` into HTTP parser
3. 🔄 Integrate `simd_checksum` into request validation
4. 🔄 Add SIMD pattern matching to WAF engine

### Expected Performance Impact

**After Full Integration**:
- **HTTP Header Parsing**: 10-15x faster
- **WAF Pattern Matching**: 15-20x faster
- **Request Validation**: 20-25x faster

---

## References

- Full benchmark results: `WEEK10_BENCHMARK_RESULTS.md`
- Benchmark code: `benches/optimization_bench.rs`
- SIMD implementation: `highper-gateway/src/runtime/simd_opt.rs`

---

## Questions?

If you have questions about this deprecation or need help migrating code, please refer to:
1. Benchmark analysis: `WEEK10_BENCHMARK_RESULTS.md`
2. This migration guide
3. Performance documentation: `docs/WEEK9_OPTIMIZATIONS.md` (will be updated)

**TL;DR**: Use stdlib for memcpy/memcmp (it's faster!), use our SIMD for pattern matching and checksums (they're much faster!).
