# Week 1 Implementation: Quick Wins (Buffer Pool + Socket Optimizations)
## Expected Improvement: 15-20% latency reduction, 20% memory allocation reduction

**Status**: Starting Implementation
**Timeline**: 2-3 days
**Target Completion**: Day 3

---

## Changes to Implement

### 1. Global BufferPool Integration ⚡ HIGH IMPACT

#### Step 1.1: Create Global Buffer Pool (src/runtime/buffer_pool.rs)

**Add**:
```rust
use once_cell::sync::Lazy;

// Global buffer pool singleton
pub static GLOBAL_BUFFER_POOL: Lazy<BufferPool> = Lazy::new(|| {
    BufferPool::new()
});
```

**Why**: Single global pool avoids allocation overhead, better cache locality

---

#### Step 1.2: Integrate into Handler (src/proxy/handler.rs)

**Current Code** (handler.rs:345-362):
```rust
match response.into_body().collect().await {
    Ok(collected) => {
        let body_bytes = collected.to_bytes();  // ❌ Allocates new Vec

        let mut resp = Response::builder()
            .status(status);

        // Copy headers
        for (key, value) in headers.iter() {
            resp = resp.header(key, value);
        }

        let duration = start.elapsed().as_secs_f64();
        record_request(method.as_str(), status.as_u16(), duration);
        record_upstream_request(&route.upstream, status.as_u16(), duration);

        Ok(resp.body(Full::new(body_bytes))?)
    }
```

**New Code with BufferPool**:
```rust
use crate::runtime::GLOBAL_BUFFER_POOL;
use http_body_util::BodyExt;

// Get a buffer from the pool
let mut buffer = GLOBAL_BUFFER_POOL.get(16384); // 16KB default

// Stream the response body into our pooled buffer
let mut body = response.into_body();
while let Some(frame) = body.frame().await {
    match frame {
        Ok(frame) => {
            if let Some(chunk) = frame.data_ref() {
                buffer.extend_from_slice(chunk);
            }
        }
        Err(e) => {
            // Return buffer to pool on error
            GLOBAL_BUFFER_POOL.put(buffer);
            return Err(e.into());
        }
    }
}

// Convert buffer to Bytes (zero-copy view)
let body_bytes = buffer.freeze();

let mut resp = Response::builder()
    .status(status);

// Copy headers
for (key, value) in headers.iter() {
    resp = resp.header(key, value);
}

let duration = start.elapsed().as_secs_f64();
record_request(method.as_str(), status.as_u16(), duration);
record_upstream_request(&route.upstream, status.as_u16(), duration);

Ok(resp.body(Full::new(body_bytes))?)
```

**Benefits**:
- ✅ Reuses buffers instead of allocating
- ✅ Reduces GC pressure
- ✅ Better cache locality
- ✅ Predictable performance (no allocation spikes)

**Expected Gain**: 10-15% latency reduction

---

### 2. Enable jemalloc Globally

#### Step 2.1: Verify jemalloc Feature

**Check** `Cargo.toml`:
```toml
[features]
default = ["jemalloc"]  # ✅ Already enabled by default
jemalloc = ["tikv-jemallocator"]
```

#### Step 2.2: Add Global Allocator (src/lib.rs or src/main.rs)

**Add to top of lib.rs**:
```rust
#[cfg(feature = "jemalloc")]
use tikv_jemallocator::Jemalloc;

#[cfg(feature = "jemalloc")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

**Verify**:
```bash
cargo build --release
ldd target/release/rust-proxy | grep jemalloc
# Should show: libjemalloc.so.2 => /usr/lib/...
```

**Benefits**:
- ✅ 5-10% allocation performance improvement
- ✅ Better memory fragmentation handling
- ✅ Lower peak memory usage

---

### 3. Critical Socket Optimizations

#### Step 3.1: Add Socket Utilities (src/utils/socket.rs)

**Current Code** (client.rs:42):
```rust
connector.set_reuse_address(true); // ✅ Already enabled
```

**Add More Optimizations**:
```rust
use socket2::{Socket, Domain, Type, Protocol};
use std::net::SocketAddr;

pub fn create_optimized_socket(addr: &SocketAddr) -> Result<Socket> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;

    // Critical optimizations
    socket.set_reuse_address(true)?;     // ✅ Allow immediate port reuse
    socket.set_reuse_port(true)?;        // ⭐ NEW: Multiple processes/threads on same port
    socket.set_nodelay(true)?;           // ✅ Already set, but ensure it's here
    socket.set_keepalive(true)?;         // Keep connections alive

    // Linux-specific optimizations
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = socket.as_raw_fd();

        // TCP_QUICKACK - Reduce ACK delay
        unsafe {
            let optval: libc::c_int = 1;
            libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_QUICKACK,
                &optval as *const _ as *const libc::c_void,
                std::mem::size_of_val(&optval) as libc::socklen_t,
            );
        }

        // TCP_FASTOPEN - Reduce connection handshake latency
        unsafe {
            let optval: libc::c_int = 5; // Queue size
            libc::setsockopt(
                fd,
                libc::IPPROTO_TCP,
                libc::TCP_FASTOPEN,
                &optval as *const _ as *const libc::c_void,
                std::mem::size_of_val(&optval) as libc::socklen_t,
            );
        }
    }

    Ok(socket)
}
```

#### Step 3.2: Apply to Server (src/proxy/mod.rs or src/main.rs)

**Usage**:
```rust
use crate::utils::socket::create_optimized_socket;

let socket = create_optimized_socket(&bind_addr)?;
socket.bind(&bind_addr.into())?;
socket.listen(1024)?; // Backlog of 1024

let std_listener: std::net::TcpListener = socket.into();
std_listener.set_nonblocking(true)?;
let listener = tokio::net::TcpListener::from_std(std_listener)?;
```

**Benefits**:
- ✅ SO_REUSEPORT: Better load distribution across threads
- ✅ TCP_QUICKACK: Reduced ACK delay
- ✅ TCP_FASTOPEN: Faster connection establishment
- ✅ Higher concurrency with less TIME_WAIT issues

**Expected Gain**: 5-8% latency reduction, better scalability

---

### 4. Kernel Tuning Script

#### Create `scripts/kernel_tuning.sh`:

```bash
#!/bin/bash
# Kernel tuning for high-performance proxy

set -e

echo "Applying kernel optimizations for rust-proxy..."

# TCP TIME_WAIT optimization
sudo sysctl -w net.ipv4.tcp_tw_reuse=1
sudo sysctl -w net.ipv4.tcp_fin_timeout=10

# Increase TIME_WAIT buckets
sudo sysctl -w net.ipv4.tcp_max_tw_buckets=400000

# Expand ephemeral port range
sudo sysctl -w net.ipv4.ip_local_port_range="10000 65535"

# Socket backlog
sudo sysctl -w net.core.somaxconn=65535
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=8192

# File descriptors
sudo sysctl -w fs.file-max=2097152
sudo sysctl -w fs.nr_open=2097152

# Connection tracking
sudo sysctl -w net.netfilter.nf_conntrack_max=1048576

# TCP buffer sizes (for high throughput)
sudo sysctl -w net.core.rmem_max=134217728    # 128MB
sudo sysctl -w net.core.wmem_max=134217728    # 128MB
sudo sysctl -w net.ipv4.tcp_rmem="4096 87380 134217728"
sudo sysctl -w net.ipv4.tcp_wmem="4096 65536 134217728"

# Enable TCP Fast Open
sudo sysctl -w net.ipv4.tcp_fastopen=3

# BBR congestion control (if available)
if sudo sysctl net.ipv4.tcp_available_congestion_control | grep -q bbr; then
    sudo sysctl -w net.ipv4.tcp_congestion_control=bbr
    sudo sysctl -w net.core.default_qdisc=fq
    echo "✓ BBR congestion control enabled"
fi

echo "✓ Kernel optimizations applied"
echo ""
echo "To make these permanent, add them to /etc/sysctl.conf"
```

**Usage**:
```bash
chmod +x scripts/kernel_tuning.sh
sudo ./scripts/kernel_tuning.sh
```

---

## Implementation Steps

### Day 1: BufferPool Integration (4-6 hours)

1. ✅ Add `once_cell` dependency
2. ✅ Add `GLOBAL_BUFFER_POOL` to `buffer_pool.rs`
3. ✅ Update `handler.rs` to use BufferPool
4. ✅ Run tests to ensure no regressions
5. ✅ Benchmark: measure allocation overhead reduction

**Commands**:
```bash
cd /home/infy/reverse_proxy/rust-proxy

# Add dependency
cargo add once_cell

# Build
cargo build --release

# Test
cargo test --all

# Benchmark (simple)
wrk -t4 -c100 -d30s http://localhost:8080/
```

---

### Day 2: Socket Optimizations (3-4 hours)

1. ✅ Update `socket.rs` with optimized socket creation
2. ✅ Apply to server listener
3. ✅ Test connection behavior
4. ✅ Create kernel tuning script
5. ✅ Apply kernel tuning
6. ✅ Benchmark: measure latency improvement

**Commands**:
```bash
# Apply kernel tuning
sudo ./scripts/kernel_tuning.sh

# Test
cargo test --all

# Benchmark
wrk -t8 -c200 -d30s http://localhost:8080/
```

---

### Day 3: Validation & Benchmarking (2-3 hours)

1. ✅ Run full test suite
2. ✅ Performance benchmarking:
   - Before vs After throughput
   - Before vs After latency (p50, p90, p99)
   - Memory allocation profiling
3. ✅ Document improvements
4. ✅ Commit changes

**Benchmarking Commands**:
```bash
# HTTP/1.1 benchmark
wrk -t8 -c200 -d60s http://localhost:8080/

# HTTP/2 benchmark
h2load -t8 -c200 -n100000 http://localhost:8080/

# Memory profiling
valgrind --tool=massif ./target/release/rust-proxy config/config.yaml
```

---

## Expected Results (Week 1)

### Before Week 1:
- **Throughput**: 150K RPS
- **Latency p50**: 1.2ms
- **Latency p99**: 5.0ms
- **Memory/conn**: 12KB
- **Allocation overhead**: High (every request allocates)

### After Week 1 (Target):
- **Throughput**: 175K-180K RPS (+17-20%)
- **Latency p50**: 1.0ms (-17%)
- **Latency p99**: 4.0ms (-20%)
- **Memory/conn**: 10KB (-17%)
- **Allocation overhead**: Low (buffer reuse)

### Key Improvements:
1. ✅ BufferPool reduces heap allocations by 80%
2. ✅ Socket optimizations reduce connection overhead
3. ✅ jemalloc improves allocation performance
4. ✅ Kernel tuning reduces TIME_WAIT issues

---

## Files to Modify

1. **`rust-proxy/Cargo.toml`**
   - Add: `once_cell = "1.19"`

2. **`rust-proxy/src/lib.rs`** (or `main.rs`)
   - Add: `#[global_allocator]` for jemalloc

3. **`rust-proxy/src/runtime/buffer_pool.rs`**
   - Add: `GLOBAL_BUFFER_POOL` singleton

4. **`rust-proxy/src/proxy/handler.rs`**
   - Modify: Lines 345-375 (use BufferPool)

5. **`rust-proxy/src/utils/socket.rs`**
   - Add: `create_optimized_socket()` function

6. **`scripts/kernel_tuning.sh`** (NEW)
   - Create: Kernel optimization script

---

## Testing Strategy

### Unit Tests:
```bash
# Test BufferPool
cargo test buffer_pool

# Test handler
cargo test handler

# All tests
cargo test --all
```

### Integration Tests:
```bash
# Start proxy
./target/release/rust-proxy config/config.yaml

# Test with wrk
wrk -t4 -c100 -d30s http://localhost:8080/

# Test with h2load (HTTP/2)
h2load -t4 -c100 -n10000 http://localhost:8080/
```

### Memory Profiling:
```bash
# Check for leaks
valgrind --leak-check=full ./target/release/rust-proxy config/config.yaml

# Memory usage over time
valgrind --tool=massif ./target/release/rust-proxy config/config.yaml
```

---

## Risk Mitigation

### Risk 1: BufferPool Contention
**Mitigation**: Use lock-free pools per thread if contention is high

### Risk 2: Buffer Size Mismatch
**Mitigation**: Dynamic size class selection based on actual response sizes

### Risk 3: Kernel Settings Compatibility
**Mitigation**: Check kernel version before applying settings

---

## Success Criteria

- ✅ All 265 tests pass
- ✅ 15-20% latency improvement measured
- ✅ 20% reduction in heap allocations
- ✅ No memory leaks (valgrind clean)
- ✅ No performance regressions

---

## Next: Week 2 Preview

After Week 1 completion:
- **Week 2**: io_uring integration (30-40% additional improvement)
- **Week 3**: Zero-copy splice() (20-30% additional improvement)
- **Week 4**: Lock-free + SIMD (10-15% additional improvement)

**Cumulative Target**: 150K → 500K+ RPS (3.3x improvement)
