# Build Dependencies Setup

**Issue**: Building highper-gateway failed due to missing `cmake` dependency
**Required for**: HTTP/3 (quiche library) support
**Date**: 2025-12-03

---

## Problem

The `quiche` library (Cloudflare's HTTP/3 implementation) requires `cmake` to build its BoringSSL dependency. The build failed with:

```
failed to execute command: No such file or directory (os error 2)
is `cmake` not installed?
```

## Current Status

✅ **Installed**:
- Rust toolchain (1.91.1)
- build-essential (gcc, g++, make)
- pkg-config

❌ **Missing**:
- cmake

---

## Solutions

### Option 1: Install via apt (Recommended)

**Steps**:
1. Open PowerShell or Windows Terminal as Administrator
2. Run WSL:
   ```powershell
   wsl
   ```
3. Install cmake:
   ```bash
   sudo apt update
   sudo apt install -y cmake
   ```
4. Verify installation:
   ```bash
   cmake --version
   ```
5. Return to project and build:
   ```bash
   cd /mnt/e/my-opensource/highper-gateway
   cargo build --release --manifest-path highper-gateway/Cargo.toml
   ```

**Time**: 2-3 minutes

---

### Option 2: Download cmake Binary (No sudo)

If you cannot use sudo, download cmake binary:

**Steps**:
1. Download cmake:
   ```bash
   cd /tmp
   wget https://github.com/Kitware/CMake/releases/download/v3.27.9/cmake-3.27.9-linux-x86_64.tar.gz
   tar xzf cmake-3.27.9-linux-x86_64.tar.gz
   ```

2. Add to PATH for this session:
   ```bash
   export PATH="/tmp/cmake-3.27.9-linux-x86_64/bin:$PATH"
   cmake --version
   ```

3. Build highper-gateway:
   ```bash
   cd /mnt/e/my-opensource/highper-gateway
   cargo build --release --manifest-path highper-gateway/Cargo.toml
   ```

**Time**: 5-10 minutes (including download)

**Note**: You'll need to export PATH every time you open a new terminal.

---

### Option 3: Make cmake Optional (Code Change)

Make quiche/HTTP/3 optional so we can test 14 out of 15 scenarios without it.

**Steps**:

1. Edit `highper-gateway/Cargo.toml`:
   ```toml
   # Before (line 34):
   quiche = "0.24"

   # After:
   quiche = { version = "0.24", optional = true }
   ```

2. Add feature flag in `[features]` section:
   ```toml
   [features]
   default = ["jemalloc"]
   http3 = ["quiche"]  # Add this line
   ```

3. Build without HTTP/3:
   ```bash
   cargo build --release --manifest-path highper-gateway/Cargo.toml --no-default-features --features jemalloc
   ```

4. Code changes needed:
   - Conditional compilation in HTTP/3 modules
   - Feature gates around quiche usage

**Time**: 15-30 minutes (code changes + testing)

**Impact**:
- ✅ Can test scenarios 01-04, 06-15 (14 scenarios)
- ❌ Cannot test scenario 05 (HTTP/3 QUIC)

---

## Recommendation

**Use Option 1** (install via apt with sudo) - it's the quickest and most reliable solution.

If you want to proceed with Option 2 or 3, I can assist with the implementation.

---

## What's Next After Installing cmake

Once cmake is installed, the build process will:

1. **Compile dependencies** (5-15 minutes):
   - quiche with BoringSSL (requires cmake)
   - All other Rust dependencies

2. **Compile highper-gateway** (2-5 minutes):
   - Main gateway binary
   - With all optimizations enabled

3. **Binary location**:
   ```
   highper-gateway/target/release/highper-gateway
   ```

4. **Verify build**:
   ```bash
   ./highper-gateway/target/release/highper-gateway --version
   ```

5. **Continue with testing plan**:
   - Create backend mock servers
   - Create test runner script
   - Test scenario 01 (Layer 4 TCP)

---

## Additional Dependencies (Already Installed)

✅ All these are already present in your WSL environment:
- gcc, g++, make (build-essential)
- pkg-config
- Rust toolchain

❌ Only cmake is missing.

---

## Build Time Estimates

| Component | First Build | Incremental |
|-----------|-------------|-------------|
| Dependencies | 10-20 min | 1-2 min |
| Gateway | 2-5 min | 30-60 sec |
| **Total** | **12-25 min** | **2-3 min** |

**Note**: First build takes longer due to compiling all dependencies. Subsequent builds are much faster.

---

## Next Steps

1. **Choose solution** (recommend Option 1)
2. **Install cmake**
3. **Retry build**: `cargo build --release --manifest-path highper-gateway/Cargo.toml`
4. **Verify binary**: `./highper-gateway/target/release/highper-gateway --version`
5. **Continue with Phase 1**: Create backend servers and test runner

---

Let me know which option you'd like to proceed with!
