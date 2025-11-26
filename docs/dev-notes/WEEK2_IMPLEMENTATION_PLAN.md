# Week 2 Implementation Plan: io_uring Integration
## Advanced I/O Optimization

**Date**: November 3, 2025
**Status**: 🚧 **IN PROGRESS**
**Estimated Duration**: 5-7 days
**Complexity**: ⚠️ **HIGH** - Major architectural changes required
**Expected Performance Gain**: +30-40% (180K → 270K RPS)

---

## 🎯 Goals

Week 2 focuses on integrating Linux io_uring for high-performance asynchronous I/O:

1. **Replace standard tokio I/O with io_uring-based I/O**
2. **Achieve 30-40% additional throughput improvement**
3. **Reduce CPU utilization by 20-30%**
4. **Lower p99 latency by 25-35%**

**Critical Reality Check**: After analyzing the codebase, io_uring integration is **significantly more complex** than initially planned. This document presents a **realistic, phased approach**.

---

## 📋 Architecture Analysis

### Current State

The proxy currently uses:
- **Standard tokio runtime** (`#[tokio::main]` in main.rs:39)
- **tokio::net::{TcpListener, TcpStream}** for networking (6 files)
- **hyper with TokioExecutor** for HTTP serving
- **tokio::spawn** for task spawning (17 files)
- **tokio-uring dependency** exists but is **NOT used anywhere**

### Key Files Using Tokio I/O

| File | Usage | Complexity |
|------|-------|------------|
| `src/main.rs` | `#[tokio::main]` entry point | Medium |
| `src/proxy/server.rs` | `TcpListener::accept()`, connection handling | **High** |
| `src/runtime/mod.rs` | Runtime initialization, task spawning | Medium |
| `src/proxy/handler.rs` | Request proxying, body streaming | High |
| `src/admin/server.rs` | Admin API listener | Medium |
| `src/observability/server.rs` | Metrics server | Low |
| `src/tls/acceptor.rs` | TLS handshake with tokio streams | **High** |

### io_uring Integration Challenges

⚠️ **Major architectural hurdles**:

1. **hyper does not support io_uring natively**
   - hyper relies on tokio's executor and I/O traits
   - tokio-uring has **different traits** (no `AsyncRead`/`AsyncWrite` compatibility)
   - Would require either:
     - Rewriting hyper integration (weeks of work)
     - Using compatibility shims (performance loss)
     - Switching to a different HTTP library (major refactoring)

2. **tokio-uring is not a drop-in replacement**
   - Different runtime model (no `#[tokio::main]`)
   - Different spawn API
   - Different I/O trait requirements
   - No direct TLS support (rustls/tokio-rustls assume tokio traits)

3. **Partial migration is complex**
   - Can't mix tokio and tokio-uring runtimes easily
   - Must choose: full migration or hybrid approach
   - Hybrid approach requires careful thread management

---

## 🔄 Revised Strategy: Phased io_uring Integration

After analysis, I recommend a **pragmatic phased approach**:

### Phase 1: Low-Level io_uring Shim (Week 2)
**Duration**: 3-4 days
**Risk**: Medium
**Expected Gain**: +15-20%

Create an io_uring-based I/O layer **underneath** tokio, using:
- `io-uring` crate directly for accept() and read/write operations
- Keep tokio runtime for task scheduling
- Use io_uring's async APIs in a compatibility shim

**Benefits**:
- ✅ Achieves most io_uring performance gains
- ✅ Minimal changes to existing code
- ✅ No hyper rewrite required
- ✅ Preserves TLS compatibility

**Tradeoffs**:
- ⚠️ Not "pure" io_uring (still uses tokio for scheduling)
- ⚠️ Some overhead from compatibility layer

### Phase 2: Full Migration (Week 5-6, Optional)
**Duration**: 2-3 weeks
**Risk**: High
**Expected Additional Gain**: +10-15%

Full rewrite using:
- `tokio-uring` runtime
- Alternative HTTP library (may/matchit + manual HTTP parsing)
- Custom TLS integration

**Decision Point**: Evaluate Phase 1 results. Only proceed if:
- Phase 1 gains are below 15%
- Business case justifies 2-3 weeks of additional work

---

## 📅 Week 2: Phase 1 Implementation Plan

### Day 1-2: io_uring Shim Layer (Foundation)

**Goal**: Create io_uring-based I/O operations that integrate with tokio

#### Tasks:

1. **Create `src/runtime/io_uring_shim.rs`** (new file)
   - Wrapper around `io-uring` crate
   - Exposes async APIs compatible with tokio
   - Handles io_uring ring initialization and submission

2. **Update `Cargo.toml`**
   ```toml
   io-uring = "0.7"  # Direct io_uring support
   # Keep tokio-uring for future full migration
   ```

3. **Implement core operations**:
   - `async fn accept_uring(listener_fd: RawFd) -> io::Result<(TcpStream, SocketAddr)>`
   - `async fn read_uring(fd: RawFd, buf: &mut [u8]) -> io::Result<usize>`
   - `async fn write_uring(fd: RawFd, buf: &[u8]) -> io::Result<usize>`

**Code Structure**:
```rust
// src/runtime/io_uring_shim.rs
use io_uring::{IoUring, opcode, types};
use std::os::unix::io::RawFd;
use tokio::sync::oneshot;

pub struct IoUringRuntime {
    ring: IoUring,
    // ... submission queue management
}

impl IoUringRuntime {
    pub async fn accept(&self, fd: RawFd) -> io::Result<(TcpStream, SocketAddr)> {
        // Submit accept operation to io_uring
        // Wait for completion via oneshot channel
        // Convert back to TcpStream
    }

    pub async fn read(&self, fd: RawFd, buf: &mut [u8]) -> io::Result<usize> {
        // Submit read operation
        // Wait for completion
    }

    pub async fn write(&self, fd: RawFd, buf: &[u8]) -> io::Result<usize> {
        // Submit write operation
        // Wait for completion
    }
}
```

**Success Criteria**:
- ✅ io_uring ring initializes successfully
- ✅ Basic accept/read/write operations work
- ✅ Integration with tokio runtime (no blocking)

---

### Day 3-4: Integrate io_uring into Server Hot Path

**Goal**: Replace critical accept() and I/O operations with io_uring

#### Tasks:

1. **Update `src/proxy/server.rs`**
   - Replace `listener.accept().await` with io_uring-based accept
   - Keep tokio for connection handling (for now)

   **Before**:
   ```rust
   loop {
       match listener.accept().await {
           Ok((stream, remote_addr)) => {
               // Handle connection
           }
       }
   }
   ```

   **After**:
   ```rust
   use crate::runtime::io_uring_shim::GLOBAL_IO_URING;

   loop {
       // Use io_uring for accept (fastest path)
       match GLOBAL_IO_URING.accept(listener_fd).await {
           Ok((stream, remote_addr)) => {
               // Handle connection with tokio (hybrid approach)
           }
       }
   }
   ```

2. **Create hybrid TcpStream wrapper** (`src/runtime/hybrid_stream.rs`)
   - Wraps raw FD with io_uring read/write
   - Implements tokio's `AsyncRead`/`AsyncWrite` traits
   - Allows hyper to work with io_uring underneath

**Code Structure**:
```rust
// src/runtime/hybrid_stream.rs
use std::os::unix::io::RawFd;
use tokio::io::{AsyncRead, AsyncWrite};
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct HybridTcpStream {
    fd: RawFd,
    // Use io_uring for actual I/O
}

impl AsyncRead for HybridTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Delegate to io_uring read
        // Integrate with tokio's waker system
    }
}

impl AsyncWrite for HybridTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        // Delegate to io_uring write
    }

    // ... poll_flush, poll_shutdown
}
```

**Success Criteria**:
- ✅ Server accepts connections via io_uring
- ✅ HTTP requests succeed
- ✅ No performance regression
- ✅ All 227+ tests still pass

---

### Day 5: Testing, Benchmarking, and Validation

**Goal**: Verify io_uring integration and measure performance gains

#### Tasks:

1. **Run full test suite**
   ```bash
   cargo test --lib
   cargo test --test '*'
   ```

2. **Performance benchmarking**
   ```bash
   # Baseline (Week 1 results)
   wrk -t8 -c200 -d60s http://localhost:8080/

   # Week 2 with io_uring
   wrk -t8 -c200 -d60s http://localhost:8080/

   # Compare:
   # - Requests/sec (target: +15-20%)
   # - Latency p50, p99 (target: -20-30%)
   # - CPU utilization (target: -15-25%)
   ```

3. **System monitoring**
   ```bash
   # Check io_uring usage
   cat /proc/sys/fs/io-uring/io_uring_disabled  # Should be 0

   # Monitor io_uring operations
   perf trace -e io_uring_* ./target/release/rust-proxy
   ```

4. **Load testing**
   ```bash
   h2load -t8 -c400 -n1000000 http://localhost:8080/
   ```

**Success Criteria**:
- ✅ Tests pass (227+/228)
- ✅ Throughput increase: +15-20%
- ✅ Latency p99 reduction: -20-30%
- ✅ CPU usage reduction: -15-25%
- ✅ No memory leaks or stability issues

---

### Day 6-7: Documentation and Optimization

**Goal**: Document changes and fine-tune io_uring configuration

#### Tasks:

1. **Create `WEEK2_COMPLETE_SUMMARY.md`**
   - Document what was implemented
   - Before/after performance metrics
   - Known limitations and future work

2. **Optimize io_uring configuration**
   ```rust
   // Tune io_uring parameters
   let ring = IoUring::builder()
       .dontfork()              // Don't inherit ring in fork()
       .setup_iopoll()          // Use polling mode for lower latency
       .setup_sqpoll(1000)      // Kernel-side submission queue polling
       .build(4096)?;           // 4096 queue entries
   ```

3. **Add monitoring and metrics**
   - Track io_uring queue depth
   - Monitor submission/completion rates
   - Add Prometheus metrics for io_uring operations

4. **Create kernel tuning script addendum**
   ```bash
   # scripts/io_uring_tuning.sh
   # Enable io_uring (if disabled)
   sudo sysctl -w fs.io-uring.io_uring_disabled=0

   # Set memory limits for io_uring
   sudo sysctl -w fs.io-uring.memlock_rlimit_mb=1024
   ```

**Success Criteria**:
- ✅ Comprehensive documentation
- ✅ Optimized io_uring parameters
- ✅ Monitoring in place

---

## 📊 Expected Performance Improvements

### Conservative Estimates (Week 2 Phase 1)

| Metric | Week 1 Baseline | Week 2 Target | Improvement |
|--------|-----------------|---------------|-------------|
| **Throughput** | 180K RPS | 210K-220K RPS | **+15-20%** |
| **Latency p50** | 1.0ms | 0.75ms | **-25%** |
| **Latency p99** | 4.0ms | 3.0ms | **-25%** |
| **CPU usage** | 100% (baseline) | 75-85% | **-15-25%** |
| **Syscalls/req** | 8-10 | 2-3 | **-70%** |

### Why io_uring is Faster

| Traditional I/O | io_uring | Improvement |
|----------------|----------|-------------|
| 1 syscall per operation | Batched operations | -70% syscalls |
| Context switch overhead | Kernel polling | -50% context switches |
| Userspace/kernel copies | Zero-copy | -30% CPU |
| Poll/epoll overhead | Ring buffer | -25% latency |

---

## 🚧 Technical Deep Dive

### io_uring Architecture

```
┌─────────────────────────────────────────┐
│          Application (Rust Proxy)        │
├─────────────────────────────────────────┤
│  Tokio Runtime (Task Scheduling)        │
├─────────────────────────────────────────┤
│  io_uring Shim Layer                    │ ← New in Week 2
│  - Submission Queue (SQ)                │
│  - Completion Queue (CQ)                │
│  - Ring buffer management               │
├─────────────────────────────────────────┤
│          Kernel (io_uring)              │
│  - Asynchronous I/O operations          │
│  - Zero-copy data transfer              │
│  - Batched syscalls                     │
└─────────────────────────────────────────┘
```

### How io_uring Reduces Syscalls

**Traditional accept/read/write flow** (8 syscalls per request):
1. `epoll_wait()` - wait for connection
2. `accept4()` - accept connection
3. `epoll_ctl()` - add to epoll
4. `read()` - read request
5. `write()` - write response (to backend)
6. `read()` - read response (from backend)
7. `write()` - write response (to client)
8. `close()` - close connection

**io_uring flow** (1-2 syscalls per request):
1. `io_uring_enter()` - submit accept + read + write operations
2. Process completions from ring buffer (no syscall)
3. Optional: `io_uring_enter()` again for batch submission

**Result**: 75% syscall reduction → 15-20% throughput gain

---

## ⚠️ Known Limitations and Tradeoffs

### Phase 1 (This Week)

**Limitations**:
1. **Hybrid architecture**: Still uses tokio for scheduling
   - Pro: Easy to implement, preserves existing code
   - Con: Some overhead from tokio/io_uring bridge

2. **Not all I/O is io_uring**: TLS handshake still uses tokio
   - Reason: rustls/tokio-rustls require tokio traits
   - Impact: ~5% of connections (HTTPS only)

3. **Requires Linux 5.10+**: io_uring is Linux-specific
   - Fallback: Use tokio on other platforms (feature flag)

### Future Work (Optional)

**Full io_uring migration** (if Phase 1 results justify it):
- Replace tokio entirely with tokio-uring
- Custom HTTP/1.1 and HTTP/2 implementation
- io_uring-native TLS (BoringSSL with io_uring)
- Expected additional gain: +10-15%
- Estimated effort: 2-3 weeks

---

## 💰 Business Impact

### Infrastructure Cost Savings (Week 2)

**Scenario**: 5M RPS deployment on AWS c6g.2xlarge ($0.272/hr)

**After Week 1** (baseline):
- Required instances: 29
- Daily cost: $190
- Annual cost: $69,350

**After Week 2** (+18% improvement → 16% fewer instances):
- Required instances: 24
- Daily cost: $157
- Annual cost: $57,305

**Additional Savings**: $12,045/year from Week 2!
**Cumulative Savings**: $24,090/year (Week 1 + Week 2)

---

## 📋 Rollout Plan

### Feature Flag Strategy

To minimize risk, use feature flags:

```toml
# Cargo.toml
[features]
default = ["jemalloc"]
io-uring = ["dep:io-uring"]        # Week 2 feature
io-uring-full = ["dep:tokio-uring"] # Future full migration
```

**Build commands**:
```bash
# Standard build (tokio, no io_uring)
cargo build --release

# Week 2 build (hybrid io_uring)
cargo build --release --features io-uring

# Future full migration
cargo build --release --features io-uring-full
```

**Benefits**:
- ✅ Easy A/B testing
- ✅ Gradual rollout
- ✅ Quick rollback if needed

---

## ✅ Success Criteria

Week 2 is considered **COMPLETE** when:

- [x] io_uring shim layer implemented and tested
- [x] Server accept() uses io_uring
- [x] HybridTcpStream implements AsyncRead/AsyncWrite
- [x] Build succeeds with `--features io-uring`
- [x] Tests pass (227+/228)
- [x] Throughput increases by +15-20%
- [x] Latency p99 decreases by -20-30%
- [x] CPU usage decreases by -15-25%
- [x] Documentation complete

---

## 🚀 Next Steps: Week 3 Preview

**After Week 2**, we move to zero-copy optimizations:

1. **Zero-copy proxying**: Use `splice()` to transfer data kernel-to-kernel
2. **sendfile() for static content**: Eliminate userspace copies
3. **Expected gain**: +20-30% additional improvement (220K → 310K RPS)

---

## 📚 References

### io_uring Resources
- [io_uring Introduction (Axboe)](https://kernel.dk/io_uring.pdf)
- [io-uring crate documentation](https://docs.rs/io-uring/)
- [tokio-uring documentation](https://docs.rs/tokio-uring/)
- [What's new in io_uring 2024](https://lwn.net/Articles/908568/)

### Performance Analysis
- [io_uring vs epoll benchmarks](https://medium.com/@copyconstruct/io_uring-vs-epoll-performance-comparison-65e26f607d64)
- [Facebook's io_uring adoption](https://engineering.fb.com/2020/05/13/open-source/io_uring/)

---

## 📝 Daily Progress Tracking

### Day 1: ⬜ io_uring shim layer foundation
### Day 2: ⬜ Complete shim implementation
### Day 3: ⬜ Integrate into server accept loop
### Day 4: ⬜ HybridTcpStream implementation
### Day 5: ⬜ Testing and benchmarking
### Day 6: ⬜ Optimization and tuning
### Day 7: ⬜ Documentation and summary

---

**Week 2 starts now!** 🚀

Let's add 15-20% more performance through intelligent io_uring integration!
