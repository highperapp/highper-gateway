# HTTP/3 with QUIC Support - Status

## Current Status: Deferred ⏸️

HTTP/3 with QUIC support was deferred in early development due to dependency compatibility issues between:
- `h3` - HTTP/3 implementation
- `quinn` - QUIC protocol implementation
- `hyper` - HTTP library
- `tokio` - Async runtime

## Technical Background

### Why HTTP/3 / QUIC?

**Benefits:**
- **Faster Connection Setup**: 0-RTT and 1-RTT handshakes (vs 2-3 RTT for TCP+TLS)
- **No Head-of-Line Blocking**: Independent streams don't block each other
- **Better Mobile Performance**: Connection migration between networks
- **Improved Loss Recovery**: Better congestion control
- **Built-in Encryption**: QUIC mandates TLS 1.3

**Use Cases:**
- Mobile applications with frequent network switches
- High-latency networks
- Concurrent requests (images, API calls)
- Real-time applications (streaming, gaming)

## Compatibility Assessment (2025)

### Current Library Ecosystem

1. **`h3`** (HTTP/3 implementation)
   - Latest: `0.0.6`
   - Status: Early development, API unstable
   - Depends on `quinn` for QUIC

2. **`quinn`** (QUIC implementation)
   - Latest: `0.11.x`
   - Status: Production-ready, actively maintained
   - Based on rustls for TLS

3. **`hyper`** (HTTP library)
   - Current version: `1.5.x`
   - HTTP/3 support: Experimental
   - Requires hyper 1.0+ with specific feature flags

### Compatibility Issues

**Problem**: Version conflicts between libraries
- `h3 0.0.6` requires specific `quinn` version
- `hyper 1.x` HTTP/3 support requires specific `h3` version
- Our current `tokio-rustls 0.26` may conflict with `quinn`'s TLS requirements

## Recommended Approach

### Option 1: Wait for Ecosystem Maturity (Recommended)
**Status**: Best for production stability
- Wait for `h3` to reach 0.1.0+ (stable API)
- Ensure hyper HTTP/3 support stabilizes
- Monitor compatibility between quinn/h3/hyper

**Timeline**: 6-12 months (estimated)

### Option 2: Add Experimental Support
**Status**: Possible but risky
- Add HTTP/3 as optional feature flag
- Document as experimental/unstable
- Maintain separate code path from HTTP/1.1/2

**Risks**:
- API breakage in updates
- Potential security issues in early implementations
- Maintenance burden

### Option 3: Use Alternative Stack
**Status**: Consider for specific use cases
- Use `s2n-quic` (AWS's implementation)
- More stable but different API
- Less ecosystem integration

## Implementation Plan (When Ready)

### Phase 1: Preparation
- [ ] Monitor `h3` changelog for 0.1.0 release
- [ ] Test compatibility with current `hyper` version
- [ ] Check `quinn` + `tokio-rustls` compatibility

### Phase 2: Integration
- [ ] Add HTTP/3 dependencies with feature flag
- [ ] Create QUIC listener alongside TCP listener
- [ ] Implement H3 connection handling
- [ ] Add ALPN negotiation for h3

### Phase 3: Testing
- [ ] Test connection establishment
- [ ] Verify 0-RTT and 1-RTT handshakes
- [ ] Benchmark vs HTTP/2 performance
- [ ] Test connection migration

## Current Alternative

For now, **HTTP/2 over TLS provides most benefits**:
- ✅ Multiplexing (no head-of-line blocking at HTTP layer)
- ✅ Header compression
- ✅ Server push capability
- ✅ Binary protocol
- ✅ TLS 1.3 with 1-RTT

**Missing from HTTP/2:**
- ❌ 0-RTT connection resumption
- ❌ UDP-based (no TCP head-of-line blocking)
- ❌ Connection migration

## Decision

**Defer HTTP/3 implementation until:**
1. `h3` reaches stable 0.1.0+ release
2. Hyper HTTP/3 support stabilizes
3. Clear production use cases emerge

**Current priority**: Focus on HTTP/1.1 and HTTP/2 excellence with features that provide immediate value:
- ✅ TLS 1.3 with ALPN
- ✅ Load balancing
- ✅ Health checks
- ✅ Compression
- ✅ Middleware system

## References

- h3: https://github.com/hyperium/h3
- quinn: https://github.com/quinn-rs/quinn
- HTTP/3 RFC: https://www.rfc-editor.org/rfc/rfc9114.html
- QUIC RFC: https://www.rfc-editor.org/rfc/rfc9000.html

---

**Last Updated**: October 2025
**Next Review**: When h3 0.1.0 is released
