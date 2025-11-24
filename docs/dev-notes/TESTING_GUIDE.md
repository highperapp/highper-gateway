# Testing Guide: TLS Passthrough, gRPC & WebSocket

## ✅ **Pre-flight Check**

### 1. Verify Compilation
```bash
cd /home/infy/reverse_proxy/highper-gateway
cargo build --release
```

**Expected:** No errors, only warnings about unused imports.

### 2. Check Configuration
```bash
ls -la ../config/test-minimal.yaml
ls -la ../certs/cert.pem
ls -la ../certs/key.pem
```

**Expected:** All files exist.

---

## 🧪 **Test 1: Basic Startup**

### Start the Proxy
```bash
cd /home/infy/reverse_proxy/highper-gateway
./target/release/highper-gateway --config ../config/test-minimal.yaml
```

### Expected Log Output
```
INFO Initializing runtime with 2 workers
INFO Starting Highper Gateway server
INFO HTTP listening on 0.0.0.0:8080
INFO TLS initialized successfully
INFO HTTPS listening on 0.0.0.0:8443
INFO Enabled protocols: HTTP/1.1=true, HTTP/2=true
INFO Starting TLS passthrough server
INFO TLS passthrough listening on 0.0.0.0:9443
INFO Registered upstream: test_backend
```

### Verification
- [ ] Server starts without errors
- [ ] Three ports bound: 8080 (HTTP), 8443 (HTTPS), 9443 (Passthrough)
- [ ] TLS passthrough server starts
- [ ] No panics or errors

**If successful, press Ctrl+C to stop.**

---

## 🔒 **Test 2: TLS Passthrough - SNI Detection**

### What We're Testing
- TLS ClientHello reading
- SNI extraction
- Route matching
- Connection handling (will fail to backend, but that's OK)

### Test Command
```bash
# In another terminal while proxy is running:
openssl s_client -connect localhost:9443 -servername test.example.com
```

### Expected Proxy Logs
```
DEBUG TLS passthrough connection from 127.0.0.1:xxxxx
INFO TLS passthrough: SNI=test.example.com from 127.0.0.1:xxxxx
```

Then you'll see:
```
ERROR Failed to connect to backend 127.0.0.1:10443: Connection refused
```

**This is EXPECTED** - we don't have a backend on port 10443. The important part is:
- ✅ SNI was extracted correctly: `test.example.com`
- ✅ Route was matched
- ✅ Proxy attempted to connect to backend

### Test Wildcard Matching

Update config to add wildcard route:
```yaml
tls:
  passthrough:
    routes:
      - server_name: "*.example.com"
        upstream: "127.0.0.1:10444"
```

Test:
```bash
openssl s_client -connect localhost:9443 -servername api.example.com
```

**Expected log:**
```
INFO TLS passthrough: SNI=api.example.com from 127.0.0.1:xxxxx
```

### Test Default Backend

Test with unmatched SNI:
```bash
openssl s_client -connect localhost:9443 -servername unknown.com
```

**Expected log:**
```
WARN No backend found for SNI: unknown.com
```

### ✅ Success Criteria
- [ ] SNI extracted correctly from ClientHello
- [ ] Correct route matched based on server_name
- [ ] Wildcard patterns work (*.example.com)
- [ ] Logs show connection attempts to correct backend
- [ ] Unknown SNI handled gracefully

---

## 🔄 **Test 3: gRPC Detection**

### What We're Testing
- HTTP/2 protocol detection
- gRPC content-type detection
- Path validation
- Metadata extraction

### Test Command (using curl as HTTP/2 client)
```bash
# Test gRPC-like request
curl -v --http2 \
  -H "Content-Type: application/grpc" \
  http://localhost:8080/grpc.health.v1.Health/Check
```

### Expected Proxy Logs
```
DEBUG Received POST request for /grpc.health.v1.Health/Check
DEBUG Detected gRPC request
INFO gRPC request: /grpc.health.v1.Health/Check (service: Some("grpc.health.v1.Health"))
DEBUG Proxying gRPC request as HTTP/2
```

### Test Invalid gRPC Request (Missing Content-Type)
```bash
curl -v --http2 http://localhost:8080/grpc.health.v1.Health/Check
```

**Expected:** No gRPC detection log (treated as normal HTTP/2)

### ✅ Success Criteria
- [ ] gRPC requests detected correctly
- [ ] HTTP/2 + content-type check working
- [ ] Service/method parsed from path
- [ ] Non-gRPC HTTP/2 requests not falsely detected
- [ ] Logs show "Proxying gRPC request as HTTP/2"

---

## 🔌 **Test 4: WebSocket Detection**

### What We're Testing
- WebSocket upgrade header detection
- Handshake validation
- Current behavior (returns 501)

### Test Command
```bash
# Using curl to simulate WebSocket upgrade
curl -v \
  -H "Upgrade: websocket" \
  -H "Connection: Upgrade" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  -H "Sec-WebSocket-Version: 13" \
  http://localhost:8080/ws
```

### Expected Proxy Logs
```
DEBUG Received GET request for /ws
DEBUG Detected WebSocket upgrade request
INFO WebSocket upgrade detected for path: /ws
```

### Expected Response
```
HTTP/1.1 501 Not Implemented
Content-Type: text/plain

WebSocket support is being integrated
```

### ✅ Success Criteria
- [ ] WebSocket upgrade headers detected
- [ ] Validation working (all required headers checked)
- [ ] Returns 501 with informative message
- [ ] Logs show detection
- [ ] No crashes or panics

---

## 🏗️ **Test 5: Multi-Protocol Stress Test**

### Scenario
Send multiple request types simultaneously to verify:
- Concurrent handling
- No protocol confusion
- Correct routing for each type

### Commands (run in separate terminals)

**Terminal 1: Regular HTTP**
```bash
while true; do curl http://localhost:8080/ -s > /dev/null; sleep 1; done
```

**Terminal 2: HTTPS**
```bash
while true; do curl -k https://localhost:8443/ -s > /dev/null; sleep 1; done
```

**Terminal 3: gRPC-like**
```bash
while true; do
  curl -H "Content-Type: application/grpc" --http2 \
    http://localhost:8080/test.Service/Method -s > /dev/null
  sleep 1
done
```

**Terminal 4: WebSocket attempts**
```bash
while true; do
  curl -H "Upgrade: websocket" \
       -H "Connection: Upgrade" \
       -H "Sec-WebSocket-Key: test" \
       -H "Sec-WebSocket-Version: 13" \
       http://localhost:8080/ws -s > /dev/null
  sleep 1
done
```

**Terminal 5: TLS Passthrough**
```bash
while true; do
  echo "Q" | openssl s_client -connect localhost:9443 \
    -servername test.example.com 2>&1 | grep -q "CONNECTED"
  sleep 1
done
```

### Monitor Proxy Logs
```bash
# Look for:
# - Mixed request types being handled
# - No errors or panics
# - Correct detection for each type
# - No protocol confusion
```

### ✅ Success Criteria
- [ ] All protocols handled concurrently
- [ ] No crashes or panics
- [ ] Correct detection for each request type
- [ ] No cross-contamination of protocols
- [ ] Performance stable under load

---

## 📊 **Test Results Summary**

### Fill in after testing:

| Test | Status | Notes |
|------|--------|-------|
| Basic Startup | [ ] Pass / [ ] Fail | |
| TLS Passthrough SNI | [ ] Pass / [ ] Fail | |
| TLS Passthrough Wildcard | [ ] Pass / [ ] Fail | |
| gRPC Detection | [ ] Pass / [ ] Fail | |
| WebSocket Detection | [ ] Pass / [ ] Fail | |
| Multi-Protocol | [ ] Pass / [ ] Fail | |

---

## 🐛 **Common Issues & Solutions**

### "Address already in use"
```
ERROR Failed to bind to 0.0.0.0:8080: Address already in use
```

**Solution:**
```bash
# Find what's using the port
sudo lsof -i :8080
# Kill it or use different port
```

### "No such file or directory: certs/cert.pem"
```
ERROR Failed to load certificate
```

**Solution:**
```bash
# Generate self-signed cert
cd /home/infy/reverse_proxy
mkdir -p certs
openssl req -x509 -newkey rsa:4096 -nodes \
  -keyout certs/key.pem \
  -out certs/cert.pem \
  -days 365 \
  -subj "/CN=localhost"
```

### "Failed to connect to backend"
**This is EXPECTED for TLS passthrough tests** - we're testing detection/routing, not actual proxying.

### Proxy won't stop (Ctrl+C doesn't work)
```bash
# Find process
ps aux | grep highper-gateway
# Kill it
kill -9 <PID>
```

---

## 🎯 **Next Steps After Testing**

### If TLS Passthrough Tests Pass ✅
1. Set up real backend on port 10443
2. Test actual end-to-end encrypted proxying
3. Measure latency overhead

### If gRPC Tests Pass ✅
1. Set up real gRPC server
2. Test with grpcurl
3. Test all streaming types (unary, server, client, bidirectional)
4. Verify metadata preservation

### If WebSocket Tests Pass ✅ (Detection)
1. Implement upgrade mechanism (2-3 hours)
2. Test with wscat
3. Test bidirectional messaging
4. Test with wss:// (secure WebSocket)

---

## 📝 **Test Log Template**

```
=== Test Session ===
Date: YYYY-MM-DD
Version: v0.1.0
Tester:

## Test 1: Basic Startup
Status: [ ] Pass / [ ] Fail
Notes:


## Test 2: TLS Passthrough
Status: [ ] Pass / [ ] Fail
SNI Extracted:
Route Matched:
Notes:


## Test 3: gRPC Detection
Status: [ ] Pass / [ ] Fail
Detected: [ ] Yes / [ ] No
Service Parsed:
Notes:


## Test 4: WebSocket Detection
Status: [ ] Pass / [ ] Fail
Detected: [ ] Yes / [ ] No
Response:
Notes:


## Test 5: Multi-Protocol
Status: [ ] Pass / [ ] Fail
Concurrent Requests:
Errors:
Notes:


## Overall Assessment
[ ] Ready for production
[ ] Needs fixes
[ ] Ready for next phase

Issues Found:
1.
2.

Recommendations:
1.
2.
```

---

**Start with Test 1 and work through sequentially. Document all findings!**
