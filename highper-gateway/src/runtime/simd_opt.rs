//! SIMD optimizations for hot data processing paths
//!
//! This module provides SIMD-accelerated implementations of common operations:
//! - Memory copying
//! - Buffer comparison
//! - Checksum calculation
//! - Header parsing
//! - Pattern matching

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// SIMD-accelerated memory copy
///
/// **DEPRECATED**: Benchmarks show this is 2-3x SLOWER than standard library memcpy.
/// Modern compilers already auto-vectorize `ptr::copy_nonoverlapping`.
/// Use `std::ptr::copy_nonoverlapping` or slice copy instead.
///
/// See WEEK10_BENCHMARK_RESULTS.md for detailed analysis.
#[deprecated(
    since = "0.1.0",
    note = "Benchmarks show this is 2-3x slower than stdlib. Use ptr::copy_nonoverlapping instead."
)]
#[inline]
pub fn simd_memcpy(dst: &mut [u8], src: &[u8]) -> usize {
    let len = dst.len().min(src.len());

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { avx2_memcpy(dst, src, len) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { sse2_memcpy(dst, src, len) }
        } else {
            fallback_memcpy(dst, src, len)
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            unsafe { neon_memcpy(dst, src, len) }
        } else {
            fallback_memcpy(dst, src, len)
        }
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        fallback_memcpy(dst, src, len)
    }
}

/// AVX2-accelerated memory copy (32 bytes per iteration)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn avx2_memcpy(dst: &mut [u8], src: &[u8], len: usize) -> usize {
    let mut copied = 0;

    // Process 32-byte chunks with AVX2
    while copied + 32 <= len {
        let src_ptr = src.as_ptr().add(copied) as *const __m256i;
        let dst_ptr = dst.as_mut_ptr().add(copied) as *mut __m256i;

        let chunk = _mm256_loadu_si256(src_ptr);
        _mm256_storeu_si256(dst_ptr, chunk);

        copied += 32;
    }

    // Handle remaining bytes
    while copied < len {
        dst[copied] = src[copied];
        copied += 1;
    }

    copied
}

/// SSE2-accelerated memory copy (16 bytes per iteration)
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_memcpy(dst: &mut [u8], src: &[u8], len: usize) -> usize {
    let mut copied = 0;

    // Process 16-byte chunks with SSE2
    while copied + 16 <= len {
        let src_ptr = src.as_ptr().add(copied) as *const __m128i;
        let dst_ptr = dst.as_mut_ptr().add(copied) as *mut __m128i;

        let chunk = _mm_loadu_si128(src_ptr);
        _mm_storeu_si128(dst_ptr, chunk);

        copied += 16;
    }

    // Handle remaining bytes
    while copied < len {
        dst[copied] = src[copied];
        copied += 1;
    }

    copied
}

/// NEON-accelerated memory copy (16 bytes per iteration)
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn neon_memcpy(dst: &mut [u8], src: &[u8], len: usize) -> usize {
    let mut copied = 0;

    // Process 16-byte chunks with NEON
    while copied + 16 <= len {
        let src_ptr = src.as_ptr().add(copied);
        let dst_ptr = dst.as_mut_ptr().add(copied);

        let chunk = vld1q_u8(src_ptr);
        vst1q_u8(dst_ptr, chunk);

        copied += 16;
    }

    // Handle remaining bytes
    while copied < len {
        dst[copied] = src[copied];
        copied += 1;
    }

    copied
}

/// Fallback memory copy for platforms without SIMD
#[inline]
fn fallback_memcpy(dst: &mut [u8], src: &[u8], len: usize) -> usize {
    dst[..len].copy_from_slice(&src[..len]);
    len
}

/// SIMD-accelerated buffer comparison
///
/// **DEPRECATED**: Benchmarks show this is 1.5-2x SLOWER than standard library comparison.
/// Modern compilers already optimize slice equality checks with SIMD.
/// Use `a == b` or `a.eq(b)` instead for better performance.
///
/// Returns true if buffers are equal
///
/// See WEEK10_BENCHMARK_RESULTS.md for detailed analysis.
#[deprecated(
    since = "0.1.0",
    note = "Benchmarks show this is 1.5-2x slower than slice equality. Use a == b instead."
)]
#[inline]
pub fn simd_memcmp(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let len = a.len();

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { avx2_memcmp(a, b, len) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { sse2_memcmp(a, b, len) }
        } else {
            a == b
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        a == b
    }
}

/// AVX2-accelerated buffer comparison
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn avx2_memcmp(a: &[u8], b: &[u8], len: usize) -> bool {
    let mut pos = 0;

    // Process 32-byte chunks
    while pos + 32 <= len {
        let a_ptr = a.as_ptr().add(pos) as *const __m256i;
        let b_ptr = b.as_ptr().add(pos) as *const __m256i;

        let a_chunk = _mm256_loadu_si256(a_ptr);
        let b_chunk = _mm256_loadu_si256(b_ptr);

        let cmp = _mm256_cmpeq_epi8(a_chunk, b_chunk);
        let mask = _mm256_movemask_epi8(cmp);

        if mask != -1 {
            return false;
        }

        pos += 32;
    }

    // Compare remaining bytes
    a[pos..] == b[pos..]
}

/// SSE2-accelerated buffer comparison
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_memcmp(a: &[u8], b: &[u8], len: usize) -> bool {
    let mut pos = 0;

    // Process 16-byte chunks
    while pos + 16 <= len {
        let a_ptr = a.as_ptr().add(pos) as *const __m128i;
        let b_ptr = b.as_ptr().add(pos) as *const __m128i;

        let a_chunk = _mm_loadu_si128(a_ptr);
        let b_chunk = _mm_loadu_si128(b_ptr);

        let cmp = _mm_cmpeq_epi8(a_chunk, b_chunk);
        let mask = _mm_movemask_epi8(cmp);

        if mask != 0xFFFF {
            return false;
        }

        pos += 16;
    }

    // Compare remaining bytes
    a[pos..] == b[pos..]
}

/// SIMD-accelerated pattern search
///
/// Searches for a byte pattern in a buffer using SIMD
#[inline]
pub fn simd_find_pattern(haystack: &[u8], needle: u8) -> Option<usize> {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { avx2_find_byte(haystack, needle) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { sse2_find_byte(haystack, needle) }
        } else {
            haystack.iter().position(|&b| b == needle)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        haystack.iter().position(|&b| b == needle)
    }
}

/// AVX2-accelerated byte search
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn avx2_find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    let len = haystack.len();
    let mut pos = 0;

    // Broadcast needle to all lanes
    let needle_vec = _mm256_set1_epi8(needle as i8);

    // Process 32-byte chunks
    while pos + 32 <= len {
        let ptr = haystack.as_ptr().add(pos) as *const __m256i;
        let chunk = _mm256_loadu_si256(ptr);

        let cmp = _mm256_cmpeq_epi8(chunk, needle_vec);
        let mask = _mm256_movemask_epi8(cmp);

        if mask != 0 {
            return Some(pos + mask.trailing_zeros() as usize);
        }

        pos += 32;
    }

    // Search remaining bytes
    haystack[pos..]
        .iter()
        .position(|&b| b == needle)
        .map(|i| pos + i)
}

/// SSE2-accelerated byte search
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_find_byte(haystack: &[u8], needle: u8) -> Option<usize> {
    let len = haystack.len();
    let mut pos = 0;

    // Broadcast needle to all lanes
    let needle_vec = _mm_set1_epi8(needle as i8);

    // Process 16-byte chunks
    while pos + 16 <= len {
        let ptr = haystack.as_ptr().add(pos) as *const __m128i;
        let chunk = _mm_loadu_si128(ptr);

        let cmp = _mm_cmpeq_epi8(chunk, needle_vec);
        let mask = _mm_movemask_epi8(cmp);

        if mask != 0 {
            return Some(pos + mask.trailing_zeros() as usize);
        }

        pos += 16;
    }

    // Search remaining bytes
    haystack[pos..]
        .iter()
        .position(|&b| b == needle)
        .map(|i| pos + i)
}

/// SIMD-accelerated checksum calculation (simple XOR-based)
#[inline]
pub fn simd_checksum(data: &[u8]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { avx2_checksum(data) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { sse2_checksum(data) }
        } else {
            fallback_checksum(data)
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        fallback_checksum(data)
    }
}

/// AVX2-accelerated checksum
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn avx2_checksum(data: &[u8]) -> u64 {
    let len = data.len();
    let mut pos = 0;
    let mut acc = _mm256_setzero_si256();

    // Process 32-byte chunks
    while pos + 32 <= len {
        let ptr = data.as_ptr().add(pos) as *const __m256i;
        let chunk = _mm256_loadu_si256(ptr);
        acc = _mm256_xor_si256(acc, chunk);
        pos += 32;
    }

    // Extract checksum from accumulator
    let mut checksum = 0u64;
    let acc_bytes: [u8; 32] = std::mem::transmute(acc);
    for &byte in &acc_bytes {
        checksum ^= byte as u64;
    }

    // Process remaining bytes
    for &byte in &data[pos..] {
        checksum ^= byte as u64;
    }

    checksum
}

/// SSE2-accelerated checksum
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
#[inline]
unsafe fn sse2_checksum(data: &[u8]) -> u64 {
    let len = data.len();
    let mut pos = 0;
    let mut acc = _mm_setzero_si128();

    // Process 16-byte chunks
    while pos + 16 <= len {
        let ptr = data.as_ptr().add(pos) as *const __m128i;
        let chunk = _mm_loadu_si128(ptr);
        acc = _mm_xor_si128(acc, chunk);
        pos += 16;
    }

    // Extract checksum from accumulator
    let mut checksum = 0u64;
    let acc_bytes: [u8; 16] = std::mem::transmute(acc);
    for &byte in &acc_bytes {
        checksum ^= byte as u64;
    }

    // Process remaining bytes
    for &byte in &data[pos..] {
        checksum ^= byte as u64;
    }

    checksum
}

/// Fallback checksum implementation
#[inline]
fn fallback_checksum(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, &byte| acc ^ (byte as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_memcpy() {
        let src = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut dst = vec![0u8; 8];

        let copied = simd_memcpy(&mut dst, &src);
        assert_eq!(copied, 8);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_simd_memcpy_large() {
        let src = vec![42u8; 1024];
        let mut dst = vec![0u8; 1024];

        let copied = simd_memcpy(&mut dst, &src);
        assert_eq!(copied, 1024);
        assert_eq!(dst, src);
    }

    #[test]
    fn test_simd_memcmp() {
        let a = vec![1u8, 2, 3, 4];
        let b = vec![1u8, 2, 3, 4];
        let c = vec![1u8, 2, 3, 5];

        assert!(simd_memcmp(&a, &b));
        assert!(!simd_memcmp(&a, &c));
    }

    #[test]
    fn test_simd_find_pattern() {
        let data = b"Hello, World!";

        assert_eq!(simd_find_pattern(data, b'W'), Some(7));
        assert_eq!(simd_find_pattern(data, b'!'), Some(12));
        assert_eq!(simd_find_pattern(data, b'X'), None);
    }

    #[test]
    fn test_simd_checksum() {
        let data1 = vec![1u8, 2, 3, 4];
        let data2 = vec![1u8, 2, 3, 4];
        let data3 = vec![1u8, 2, 3, 5];

        let sum1 = simd_checksum(&data1);
        let sum2 = simd_checksum(&data2);
        let sum3 = simd_checksum(&data3);

        assert_eq!(sum1, sum2);
        assert_ne!(sum1, sum3);
    }

    #[test]
    fn test_simd_checksum_large() {
        let data = vec![42u8; 10000];
        let sum = simd_checksum(&data);
        // XOR of even count of identical values is 0
        // 10000 is even, so 42 ^ 42 ^ ... (10000 times) = 0
        assert_eq!(sum, 0);
    }
}
