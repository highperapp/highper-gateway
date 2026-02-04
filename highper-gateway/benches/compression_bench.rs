//! Compression middleware performance benchmarks
//!
//! Benchmarks for Stage 0 and Stage 1 compression system

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use highper_gateway::middleware::compression::*;

// Benchmark individual compressor performance
fn benchmark_gzip_compress(c: &mut Criterion) {
    let compressor = GzipCompressor::new();
    let config = CompressorConfig::default();

    // Test with different data sizes
    let mut group = c.benchmark_group("gzip_compress");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        let data = vec![b'x'; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&data, &config).unwrap());
            });
        });
    }

    group.finish();
}

fn benchmark_brotli_compress(c: &mut Criterion) {
    let compressor = BrotliCompressor::new();
    let config = CompressorConfig::default();

    let mut group = c.benchmark_group("brotli_compress");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        let data = vec![b'x'; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&data, &config).unwrap());
            });
        });
    }

    group.finish();
}

fn benchmark_zstd_compress(c: &mut Criterion) {
    let compressor = ZstdCompressor::new();
    let config = CompressorConfig::default();

    let mut group = c.benchmark_group("zstd_compress");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        let data = vec![b'x'; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&data, &config).unwrap());
            });
        });
    }

    group.finish();
}

fn benchmark_deflate_compress(c: &mut Criterion) {
    let compressor = DeflateCompressor::new();
    let config = CompressorConfig::default();

    let mut group = c.benchmark_group("deflate_compress");

    for size in [1024, 4096, 16384, 65536, 262144].iter() {
        let data = vec![b'x'; *size];
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&data, &config).unwrap());
            });
        });
    }

    group.finish();
}

// Benchmark compressor registry operations
fn benchmark_registry_operations(c: &mut Criterion) {
    init_compression();

    c.bench_function("registry_get", |b| {
        b.iter(|| {
            black_box(GLOBAL_COMPRESSOR_REGISTRY.get("gzip"));
        });
    });

    c.bench_function("registry_list", |b| {
        b.iter(|| {
            black_box(GLOBAL_COMPRESSOR_REGISTRY.list());
        });
    });

    c.bench_function("registry_list_detailed", |b| {
        b.iter(|| {
            black_box(GLOBAL_COMPRESSOR_REGISTRY.list_detailed());
        });
    });
}

// Benchmark content negotiation
fn benchmark_content_negotiation(c: &mut Criterion) {
    c.bench_function("parse_accept_encoding_simple", |b| {
        b.iter(|| {
            black_box(parse_accept_encoding("gzip, deflate, br"));
        });
    });

    c.bench_function("parse_accept_encoding_with_quality", |b| {
        b.iter(|| {
            black_box(parse_accept_encoding(
                "gzip;q=1.0, deflate;q=0.8, br;q=0.9, zstd;q=0.85",
            ));
        });
    });

    c.bench_function("select_compressor", |b| {
        let prefs = &["br", "zstd", "gzip", "deflate"];
        b.iter(|| {
            black_box(select_compressor("gzip, deflate, br;q=0.9", prefs));
        });
    });

    c.bench_function("is_compressible", |b| {
        b.iter(|| {
            black_box(is_compressible("text/html; charset=utf-8"));
        });
    });
}

// Benchmark compression quality vs speed trade-off
fn benchmark_compression_levels(c: &mut Criterion) {
    let compressor = GzipCompressor::new();
    let data = vec![b'x'; 65536]; // 64KB

    let mut group = c.benchmark_group("gzip_compression_levels");
    group.throughput(Throughput::Bytes(65536));

    for level in [1, 3, 6, 9].iter() {
        let config = CompressorConfig {
            level: *level,
            min_size: 1024,
            buffer_size: 8192,
            streaming: false,
        };

        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, _| {
            b.iter(|| {
                black_box(compressor.compress(&data, &config).unwrap());
            });
        });
    }

    group.finish();
}

// Benchmark comparison: all algorithms on same data
fn benchmark_algorithm_comparison(c: &mut Criterion) {
    let gzip = GzipCompressor::new();
    let brotli = BrotliCompressor::new();
    let zstd = ZstdCompressor::new();
    let deflate = DeflateCompressor::new();
    let config = CompressorConfig::default();

    // Realistic HTML data (more compressible than random data)
    let data = b"<!DOCTYPE html><html><head><title>Test</title></head><body>".repeat(1000);

    let mut group = c.benchmark_group("algorithm_comparison");
    group.throughput(Throughput::Bytes(data.len() as u64));

    group.bench_function("gzip", |b| {
        b.iter(|| {
            black_box(gzip.compress(&data, &config).unwrap());
        });
    });

    group.bench_function("brotli", |b| {
        b.iter(|| {
            black_box(brotli.compress(&data, &config).unwrap());
        });
    });

    group.bench_function("zstd", |b| {
        b.iter(|| {
            black_box(zstd.compress(&data, &config).unwrap());
        });
    });

    group.bench_function("deflate", |b| {
        b.iter(|| {
            black_box(deflate.compress(&data, &config).unwrap());
        });
    });

    group.finish();
}

// Benchmark statistics tracking overhead
fn benchmark_stats_overhead(c: &mut Criterion) {
    let compressor = GzipCompressor::new();
    let config = CompressorConfig::default();
    let data = vec![b'x'; 4096];

    c.bench_function("compress_with_stats", |b| {
        b.iter(|| {
            let result = compressor.compress(&data, &config).unwrap();
            black_box(compressor.stats());
            black_box(result);
        });
    });
}

criterion_group!(
    benches,
    benchmark_gzip_compress,
    benchmark_brotli_compress,
    benchmark_zstd_compress,
    benchmark_deflate_compress,
    benchmark_registry_operations,
    benchmark_content_negotiation,
    benchmark_compression_levels,
    benchmark_algorithm_comparison,
    benchmark_stats_overhead
);
criterion_main!(benches);
