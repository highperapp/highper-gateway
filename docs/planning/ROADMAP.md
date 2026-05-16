# Highper Gateway — Roadmap (v2)

**Owner-of-record:** Highper Gateway maintainer.
**Restart date:** 2026-05-16.
**Status of this document:** Single source of truth for v1.0 GA
planning and tracking, derived from architectural review against the
15 deployment use cases.

---

## 0. Document conventions

This is the **v2 roadmap**. The v1 roadmap and ~20 sibling planning /
status-tracking documents are archived at
[`docs/planning/archive/2026-05-16-v1/`](archive/2026-05-16-v1/) with
a manifest explaining what's there and why
([`ARCHIVE_MANIFEST.md`](archive/2026-05-16-v1/ARCHIVE_MANIFEST.md)).

### 0.1 Why v2

The v1 ROADMAP grew to 3,962 lines with 42 lifecycle entries and an
ad-hoc structure (B-blockers + workstreams + D-flags + M-items)
that became too dense to track at a glance. v2 reorganises around
the **15 deployment use cases** since those are the gateway's
stated product surface — and around a small set of cross-cutting
items that don't bind to any single UC.

### 0.2 Citation rule (carried forward from v1 `CLAUDE.md`)

Every concrete claim in this document cites a source span (file
path with optional `:LINE`) or is marked `(unsourced inference)`.
Verbatim source spans use quotation marks; paraphrases do not.

### 0.3 Tracking conventions

- `[ ]` = pending
- `[~]` = in progress / partially shipped
- `[x]` = complete (link the merging commit when reasonable)
- `[d]` = deferred (with one-line reason)
- `[k]` = killed / out of scope (with one-line reason)

### 0.4 Configuration rule (carried forward from v1 §0.1)

Every operator-tunable value loads from a `HIGHPER_*` environment
variable at startup. Hot paths must not call `std::env::var`. No
compile-time constants for tunables — existing `pub const DEFAULT_*`
literals are bugs and tracked under the body-size centralisation
work in §3.5. The env-var-only Workstream 0.J infrastructure
landed 2026-05-03 (commits `6897310` → `7c00488` across Stages
1 / 2 / 3a / 3b / 3c-1 / 3c-2 / 3c-3) and is documented at
`docs/CONFIG_ENV.md` (currently 10 sections stale — see §3.5).

### 0.5 Core-first / vertical-second scoping (carried forward from v1 §0.2)

The platform extensibility surface ships in v1.0 GA. The first
vertical that *uses* that surface (UC16 AI/LLM Gateway) ships in
v1.x. The standard platform-product pattern (Kubernetes CRDs;
Linux loadable modules). Concretely:

- **In v1.0 GA:** all 16 `runtime_config` sections (including
  `AiRuntimeConfig` with its 25 `HIGHPER_AI_*` env vars), cluster
  bootstrap (Type B = Valkey, Type C = etcd; `PeerDiscovery`
  trait), cross-subsystem validator. `src/ai/` does **not** exist
  yet — UC16 vertical implementation is deferred.
- **Out of v1.0 GA:** `AiProvider` / `AiStateStore` /
  `VectorIndex` trait implementations, virtual keys + budgets,
  exact cache, SSE chunker, MCP passthrough, semantic cache.

For UC16 design history and decisions, see the archived
[`USECASE_16_AI_LLM_GATEWAY.md`](archive/2026-05-16-v1/USECASE_16_AI_LLM_GATEWAY.md).

---

## 1. Project status snapshot (2026-05-16)

- **Repository:** `D:\my-opensource\highper-gateway` (crate at
  `highper-gateway/`). Branch `master` head commit `b018071`
  ("docs: B6 (Workstream 0.E) COMPLETE — closes B6 release
  blocker; Workstream 0.E ✅") at restart time.
- **15 deployment use cases (UC1–UC15) all have code shipping** in
  `highper-gateway/src/`. Module layout: `tcp/`, `proxy/`, `tls/`,
  `gateway/`, `http/`, `websocket/`, `grpc/`, `middleware/`,
  `cache/`, `discovery/`, `webserver/`, plus cross-cutting
  `admin/`, `runtime/`, `runtime_config/`, `plugin/`,
  `observability/`, `state/`, `config/`, `utils/`.
- **Empty stubs** (per pre-v2 audit): `src/ha/` and `src/health/`
  are empty directories. HA shipped via `runtime_config::cluster`
  + the deferred `PeerDiscovery` trait instead; `health/` may be
  retired.
- **Workstream 0.J ✅** — env-driven `RuntimeConfig` scaffold
  shipped across 7 commits. 16 sections at
  `src/runtime_config/sections/` covering cluster, plugin, ai,
  body, shutdown, secrets, http3, tls, ratelimit, circuit_breaker,
  geo, cache, signals, config_watcher, observability, graphql.
- **4 of 14 prior B-blockers closed** during the 2026-05-03 →
  2026-05-06 sprint: B4 (rate-limit safety), B5 (GraphQL
  depth/complexity), B6 (PostgreSQL pool validation), B11
  (bounded channels). **10 still open** — folded into the
  per-UC and cross-cutting sections below.
- **Admin API surface** is ~60–70% complete at
  `src/admin/server.rs` (2,068 lines): JWT auth, route CRUD,
  upstream CRUD, runtime-config endpoints, login + users,
  health/readiness. An orphaned `src/admin/api.rs` (354 lines,
  not in `src/admin/mod.rs:12-23`) is dead code awaiting
  deletion.
- **All 6 owner gates closed** 2026-05-03 (see archived
  [`OWNER_GATES_2026-05-03.md`](archive/2026-05-16-v1/OWNER_GATES_2026-05-03.md)).
  Phase 0 operationally unblocked.

---

## 2. Per-use-case status and remaining work

Each subsection: **current state** (with file:line citations to
prove the feature exists in code) → **remaining work** (specific
items needed for v1.0 GA) → **dependencies** (cross-cutting items
from §3 that this UC inherits).

### 2.1 UC1 — Layer 4 TCP

- **Current state:** working in isolation.
  - Bidirectional zero-copy forwarding via
    `tokio::io::copy_bidirectional` (`src/tcp/proxy.rs:287`).
  - Five LB algorithms (RoundRobin, LeastConn, ConsistentHash,
    IpHash, WeightedRR) in `BackendSelector`
    (`src/tcp/proxy.rs:103-150`).
  - Connection pool with LIFO queue + idle TTL (`src/tcp/pool.rs`).
  - Protocol-aware health checks for TCP / MySQL / Postgres /
    Redis (`src/tcp/health.rs`).
  - Three-state circuit breaker (`src/tcp/circuit_breaker.rs`).
  - SO_REUSEPORT, large backlog (4096/8192), SO_LINGER,
    TCP_FASTOPEN Linux (`src/utils/socket.rs:24-199`,
    `src/tcp/server.rs:67-113`).

- **Remaining for v1.0 GA:**
  - [ ] **UC1.A — wire TCP listener into `Runtime::run`** (was
    B1 in v1; the blocker). `src/runtime/mod.rs` never calls
    `TcpProxyServer::start()`; hybrid UC10 deployments are
    currently HTTP-only. **~7 days** including hot-reload
    propagation to TCP listener. Inherits §3.6 trait extraction
    if `LoadBalancerStrategy` ships first.

- **Dependencies inherited:** §3.6 (Phase 1.4 traits — namely
  `LoadBalancerStrategy` if extracted), §3.2 (graceful shutdown
  coverage — TCP listener needs to drain alongside HTTP/HTTP3).

### 2.2 UC2 — Layer 7 HTTP/1.1 + HTTP/2

- **Current state:** working at scale.
  - hyper 1.5 with http1+http2 (`src/proxy/server.rs:13-14`).
  - Eight LB algorithms (RoundRobin / LeastConn / Random / IpHash
    / ConsistentHash / LeastResponseTime / PowerOfTwo / Maglev /
    Geographic) at `src/proxy/loadbalancer.rs`.
  - Rolling-window response-time selection at `:115-150`.
  - Circuit breakers + retry with backoff (`src/proxy/handler.rs:63-67`,
    `src/proxy/retry.rs`).
  - Compression middleware (gzip / brotli / zstd / deflate) at
    `src/middleware/compression/`.
  - SIMD-accelerated path matching at
    `src/gateway/routing/matcher.rs:11-90`.
  - Hot-reload routing at `src/gateway/routing/hot_reload.rs`.
  - `Alt-Svc` advertising at `src/http/alt_svc.rs`.
  - Body-size limit hot-path consumers wired (B12 partial,
    Stage 3c-2): `src/proxy/handler.rs:646,1150,1702` read
    `runtime_config::current().body.max_request_body`.

- **Remaining for v1.0 GA:**
  - [ ] **UC2.A — Slowloris / header-size hygiene.** Per-request
    read/idle timeouts → 408 on incomplete header read
    (~2 days). Header-block size cap → 431 via
    `HIGHPER_HTTP_MAX_HEADER_BYTES` default 64 KiB (~1 day).
    HTTP/2 GOAWAY graceful drain on shutdown signal (~2 days;
    overlaps with §3.2).
  - [ ] **UC2.B — Body-size centralisation finish** (was B12).
    `BodyRuntimeConfig` shipped 3 of 11 spec'd fields; 8 more
    needed (`HIGHPER_BODY_MAX_WAF`, `_FASTCGI`, `_STATIC`,
    `_LOG`, `_VALIDATE_*`, `_WEBSERVER_REQ_BUFFER_MAX`,
    `_PROXIED`); consumer migration across `src/middleware/`,
    `src/proxy/`, `src/http/`. **~3 days.**

- **Dependencies inherited:** §3.2 (HTTP/2 GOAWAY overlaps with
  spawn-task drain coverage).

### 2.3 UC3 — TLS termination, ACME, mTLS, OCSP, CRL

- **Current state:** mostly working; some silent stubs (security
  risk).
  - rustls TLS 1.2/1.3 with ALPN
    (`src/tls/manager.rs:244-248`); dynamic `CertResolver` for
    SNI-based selection (`:179-318`).
  - TLS passthrough parses ClientHello SNI
    (`src/tls/passthrough.rs:20-186`).
  - Cert hot-reload with validation
    (`src/tls/cert_reloader.rs`, `src/tls/cert_validator.rs`).
  - ACME HTTP-01 challenge with TTL store
    (`src/tls/acme.rs:95-124`, `src/tls/challenge.rs:19-113`).
  - OCSP fetcher with retry/backoff and stale fallback
    (`src/tls/ocsp_fetcher.rs:86-277`).
  - OCSP cache with auto-refresh
    (`src/tls/ocsp_cache.rs:45-165`).
  - CRL fetcher with delta-CRL framework
    (`src/tls/crl_checker.rs`).
  - Three mTLS modes (Required / Optional / OptionalNoCA) at
    `src/tls/client_verifier.rs:214-250`.
  - `HIGHPER_TLS_CERT_WATCHER_CHANNEL_CAPACITY` bounded channel
    (was B11.1 closed 2026-05-03 commit `fef5bb4`).

- **Remaining for v1.0 GA — TLS honesty (was B3 in v1):**
  - [ ] **UC3.A — OCSP truth.** `src/tls/ocsp_fetcher.rs:346-415`
    never builds a real OCSP request body; pull in
    `rust-ocsp` / `webpki-ocsp`; write proper request + verify
    response signature, thisUpdate/nextUpdate, status, nonce.
    **~5 days.**
  - [ ] **UC3.B — OCSP stapling wired.** `src/tls/ocsp_stapler.rs:97-136`
    never attaches cached responses into `CertifiedKey.ocsp` in
    rustls. **~2 days.**
  - [ ] **UC3.C — `needs_renewal()` against notBefore/notAfter**
    (default renew at 30 d remaining). `src/tls/acme.rs:199-204`
    always returns `false` — ACME certs silently never renew.
    **~1 day. Highest user-facing severity in §2.3.**
  - [ ] **UC3.D — CRL extension parsers** via `x509-parser`:
    `extract_crl_number` + `extract_delta_crl_url`. **~2 days.**
  - [ ] **UC3.E — `tokio::fs::read` in TLS load paths**
    (replace blocking `fs::read`). **~½ day.**
  - [ ] **UC3.F — Strip ACME private-key PEM from debug logs.** **~½ day.**
  - [ ] **UC3.G — TLS staging-URL config field** (e.g.
    `acme.directory_url`); example config. **~½ day.**
  - [ ] **UC3.H — Let's Encrypt staging CI integration test.** **~2 days.**

- **Dependencies inherited:** §3.6 `SecretRef::Secrets://`
  resolver — ACME EAB credentials and OCSP private key access
  may want this.

### 2.4 UC4 — API Gateway with rate limiting ✅ (B-tagged work closed)

- **Current state:** ✅ rate-limit safety closed 2026-05-06.
  - Two parallel implementations: `src/middleware/rate_limit.rs`
    + `src/gateway/ratelimit/{token_bucket, sliding_window,
    distributed}.rs`.
  - Sliding window with VecDeque-based timestamps
    (`src/gateway/ratelimit/sliding_window.rs:82-170`).
  - Distributed limiter with Redis Lua atomic token bucket at
    `src/gateway/ratelimit/distributed.rs:150-204`.
  - **XFF trust mode** wired into
    `src/middleware/rate_limit.rs:181-193`
    (`HIGHPER_RATELIMIT_XFF_TRUST=none|first|last`, default
    `none` — was B4.1 closed 2026-05-03 commit `3d5f4dc`).
  - **Distributed limiter Redis fail-mode** + per-key sharding
    at `src/gateway/ratelimit/distributed.rs:51-156`
    (`HIGHPER_RATELIMIT_REDIS_FAIL_MODE` /
    `HIGHPER_RATELIMIT_KEY_SHARDS` — was B4.2 closed 2026-05-06
    commit `abd6457`).
  - API-key / JWT / OAuth2 authentication at
    `src/gateway/auth/{api_key, jwt, oauth2,
    oauth2_providers}.rs`.

- **Remaining for v1.0 GA:**
  - [ ] **UC4.A — Per-route rate-limit DSL/YAML wiring**
    (config → runtime). **~1 day.**
  - [ ] **UC4.B — `RateLimiter` trait extraction.** Define
    `RateLimiter` trait (`check(&self, key: &Key, cost: u64)
    -> Decision`); migrate `TokenBucket` +
    `gateway/ratelimit/{token_bucket, sliding_window,
    distributed}.rs` into trait impls; chosen algorithm via
    `HIGHPER_RATELIMIT_ALGO`. Opens the door for GCRA /
    leaky-bucket in v1.x without parallel code paths.
    **~5 days.**
  - [ ] **UC4.C — CIDR-list XFF trust (deviation cleanup).**
    Add `HIGHPER_PROXY_TRUST_CIDRS` as a mode alongside the
    existing `XffTrustMode = none|first|last`; satisfies
    multi-LB topologies. Backward-compatible. **~1 day.**
  - [ ] **UC4.D — Stable-hash shard for distributed limiter
    (deviation cleanup).** Replace `rand::random::<u32>() % N`
    in `src/gateway/ratelimit/distributed.rs:120` with a stable
    hash of the rate-limit key — restores per-client locality
    for observability. **~½ day.**
  - [ ] **UC4.E — Cookbook entry**
    `examples/configs/scenarios/scenario-04-rate-limit-hot-key.yaml`
    demonstrating the sharding pattern. **~½ day.**
  - [ ] **UC4.F — Empty parallel `src/gateway/rate_limit/`
    directory delete** (housekeeping). **~5 min.**

### 2.5 UC5 — HTTP/3 / QUIC (quiche)

- **Current state:** working at MVP scale; panic risk + buffering
  pending.
  - UDP listener + quiche connection setup, Initial+Retry
    handshake (`src/http/http3_quiche.rs:177-180, 308-398`).
  - HMAC-SHA256 address-validation tokens at `:1060-1150`.
  - BBR congestion control, 0-RTT, 10 MB flow window,
    h3 / h3-29 / h3-28 at `:1025-1050`.
  - Streaming request body via mpsc channels at `:631, 722`.
  - Four-worker proxy task pool at `:199-270`.
  - **Bounded backend req/resp channels** (was B11.4/B11.5
    closed 2026-05-03 commit `9159aa4`):
    `HIGHPER_HTTP3_BACKEND_REQUEST_CHANNEL_CAPACITY` and
    `HIGHPER_HTTP3_BACKEND_RESPONSE_CHANNEL_CAPACITY`.

- **Remaining for v1.0 GA — HTTP/3 panic + buffering (was B8 in v1):**
  - [ ] **UC5.A — Replace receive-loop `unwrap()`** at
    `src/http/http3_quiche.rs:403` with safe drop + counter.
    Any malformed packet currently crashes the worker.
    **~1 day. Highest user-facing severity in §2.5.**
  - [ ] **UC5.B — Stream backend response (remove `body.collect()`)**
    at `src/http/http3_quiche.rs:944`. Currently buffers full
    response in memory before forwarding. **~3 days.**
  - [ ] **UC5.C — Re-enable connection migration** with
    anti-amplification checks (`:1053`). **~2 days.**
  - [ ] **UC5.D — Wire emitting metrics** (currently scaffold
    only at `src/observability/quic_metrics.rs`). **~1 day.**
  - [ ] **UC5.E — B11 metrics emission** (queue depth + drop
    counters at the 2 bounded H3 channel sites). **~½ day.**

- **Dependencies inherited:** §3.2 (B14 spawn-task drain
  coverage — HTTP/3 worker pool spawn at `:215` needs
  `JoinSet` + cancellation).

### 2.6 UC6 — WebSocket load balancer

- **Current state:** working.
  - RFC 6455 upgrade + accept-key derivation at
    `src/websocket/handler.rs:13-50, 191-197`.
  - Sticky sessions via `HPGW_WS_SESSION` cookie at
    `:53-83, 121-128`.
  - Bidirectional copy via `tokio::io::copy_bidirectional` at
    `:134-159`.
  - Connection state machine + per-conn metrics at
    `src/websocket/connection.rs`.
  - Configurable ping/pong with missed-pong threshold at
    `src/websocket/keepalive.rs`.
  - Recovery + circuit breaker with seven error classes at
    `src/websocket/recovery.rs`.
  - Graceful shutdown coordinator with phased timeouts at
    `src/websocket/shutdown.rs`.

- **Remaining for v1.0 GA:**
  - [ ] **UC6.A — Swap `RwLock::*().unwrap()` for `safe_lock!`**
    in `src/websocket/recovery.rs` (use existing poison-recovery
    macro). **~1 day.**

### 2.7 UC7 — gRPC gateway

- **Current state:** working but protocol non-compliant.
  - HTTP/2 + content-type detection, gRPC path validation at
    `src/grpc/detector.rs`.
  - gRPC-timeout RFC parsing at `:110-137`.
  - Sixteen-status-code → HTTP mapping at
    `src/grpc/mod.rs:169-191`.
  - Frame parser (5-byte header + payload) at
    `src/grpc/streaming.rs`.
  - `grpc.health.v1.Health` Check at `src/grpc/health.rs`.
  - LB policies (RoundRobin / LeastRequest / Random /
    PowerOfTwo / ConsistentHash) at `src/grpc/mod.rs:114-125`.

- **Remaining for v1.0 GA — gRPC compliance (was B7 in v1):**
  - [ ] **UC7.A — Per-upstream HTTP/2 client pool** (currently
    fresh `hyper` client per request at
    `src/grpc/handler.rs:54-100`); reuse
    `src/proxy/connection_pool.rs`. **~3 days.**
  - [ ] **UC7.B — Real HTTP/2 trailers** (`http_body::Frame::trailers`)
    for `grpc-status` — currently passed as headers. **~1 day.**
  - [ ] **UC7.C — Configurable per-method call-type registry**
    (proto descriptor optional in MVP). **~3 days.**

### 2.8 UC8 — Database load balancer ✅ (B-tagged work closed)

- **Current state:** ✅ pool validation closed 2026-05-06.
  - L4 protocol detection: MySQL `0x0a`, PostgreSQL
    StartupMessage, Redis RESP at
    `src/tcp/protocol.rs:14-109`; port fallback at `:100-108`.
  - Per-backend pool with idle queue + semaphore limits at
    `src/proxy/database_pool.rs:259-505`.
  - Pool prewarm + cleanup tasks at `:445-479, 414-442`.
  - Protocol-aware validation: MySQL COM_PING at `:157-170`;
    Redis PING/RESP at `:186-198`; **PostgreSQL real
    `SELECT 1` probe** at `:174-218` (was B6 closed 2026-05-06
    commit `5c29eb3`).
  - 5 new unit tests for the PG probe at `:748-782`.

- **Remaining for v1.0 GA (non-blocker; queued):**
  - [ ] **UC8.A — PostgreSQL STARTTLS / SSLRequest
    negotiation.** **~3 days.**
  - [ ] **UC8.B — DB pool failover wired to circuit breaker.** **~2 days.**

### 2.9 UC9 — WAF + DDoS protection + mTLS

- **Current state:** scaffolding shipped; depth unverified.
  - Engine abstraction with four backends declared (Custom,
    Coraza, ModSecurity, AWS) at
    `src/middleware/waf/{engine, custom_engine, coraza_engine,
    modsecurity_engine, aws_engine}.rs`.
  - Engine selector + block-vs-log mode + max-body cap at
    `src/middleware/waf/mod.rs:54-92`.
  - Decision types (Allow / Block / RateLimit / Log) at
    `src/middleware/waf/engine.rs:70-93`.
  - DDoS module: per-IP conn-rate, slowloris idle timeout,
    rate limit + burst, blacklist/whitelist, automatic temp
    bans at `src/middleware/ddos_protection.rs:24-49`.
  - Security audit logger with 10+ event types at
    `src/middleware/security_audit.rs:24-79`.

- **Remaining for v1.0 GA:**
  - [ ] **UC9.A — Custom WAF real regex sets** (SQLi / XSS /
    path-traversal); verify Coraza binding actually compiles +
    runs CRS rules; if not, swap to Rust `coraza-rs` port.
    **~5 days.**
  - [ ] **UC9.B — Body decompression pipeline** (gzip / deflate
    up to `HIGHPER_BODY_MAX_WAF` from §2.2 UC2.B) before WAF
    runs. **~3 days.**
  - [ ] **UC9.C — WAF rule hot-reload** via signal + admin API.
    **~2 days.**
  - [ ] **UC9.D — DDoS geo-block actually consults**
    `src/proxy/geographic.rs` (currently config-only).
    **~1 day.**

### 2.10 UC10 — Hybrid multi-protocol

- **Current state:** partially wired; TCP listener missing.
  - HTTP + HTTP/3 + Admin + Metrics tasks spawned by
    `Runtime::run` sharing `Arc<RwLock<Config>>`, `ProxyState`,
    hot-reload trigger (`src/runtime/mod.rs`).
  - TCP server is **not** wired into `Runtime` (see UC1.A
    above).

- **Remaining for v1.0 GA:**
  - [ ] **UC10.A — Cross-protocol metric correlation tests.**
    **~1 day.** Lands with UC1.A.

### 2.11 UC11 — CDN edge caching

- **Current state:** working with three backends.
  - InMemory backend (DashMap, LRU cleanup, hit/miss counters)
    at `src/cache/backends.rs:18-179`.
  - Disk backend (LRU eviction, zstd compression, atomic
    temp+rename writes, sharded files) at
    `src/cache/disk.rs:117-615`.
  - Redis backend (`ConnectionManager`, SETEX TTL) at
    `src/cache/backends.rs:182-326`.
  - MultiTier (L1 InMemory + L2 distributed; backfill on
    L2 hit) at `:331-423`.

- **Remaining for v1.0 GA (non-blocker; Phase 1.6 hygiene):**
  - [ ] **UC11.A — Replace `DefaultHasher`** in
    `src/cache/disk.rs:209` (collision-attack vector).
    **~½ day.**

### 2.12 UC12 — Microservices discovery

- **Current state:** working with one staleness bug.
  - Consul / etcd backends with mTLS pluggability.
  - Circuit breaker tie-in.

- **Remaining for v1.0 GA — discovery quick-fix:**
  - [ ] **UC12.A — Fix `should_refresh = true` always** at
    `src/discovery/registry.rs:41` — cache last-update; respect
    TTL. **~½ day.**
  - [ ] **UC12.B — Consul ACL token field; mTLS to
    Consul/etcd.** **~2 days.**

### 2.13 UC13 — GraphQL gateway ✅ (B-tagged work closed)

- **Current state:** ✅ depth/complexity closed 2026-05-06.
  - Passthrough + introspection cache at
    `src/gateway/graphql/`.
  - **Depth + complexity analyzer** at
    `src/gateway/graphql/analyzer.rs` + wired into
    `src/gateway/graphql/mod.rs:182, 340-372` (was B5 closed
    2026-05-06 commit `c0bf716`). New `GraphqlRuntimeConfig`
    section (`HIGHPER_GRAPHQL_MAX_DEPTH` default 15,
    `_MAX_COMPLEXITY` default 1000, `_ENFORCE_LIMITS` default
    true). 8 analyzer + 5 section unit tests.
  - Stitcher placeholder at
    `src/gateway/graphql/stitcher.rs:56-57` — federation
    explicitly deferred to v2.x per archived
    [`GRAPHQL_FEDERATION.md`](archive/2026-05-16-v1/GRAPHQL_FEDERATION.md).

- **Remaining for v1.0 GA:** none (federation deferred).
  - Queued for v1.x (Phase 1.6+): per-field `@cost` directive
    weights; per-virtual-key thresholds.

### 2.14 UC14 — Static files + PHP-FPM

- **Current state:** working.
  - PHP-FPM support at `src/webserver/php_fpm.rs` with
    `enable_php_fpm` flag and `PhpFpmConfig` at
    `src/webserver/config.rs:49-84, 141-159`.
  - FastCGI params dictionary at
    `src/webserver/config.rs:141`.
  - Static file serving at `src/webserver/static_files.rs`.
  - MIME-type detection at `src/webserver/mime.rs`.

- **Remaining for v1.0 GA (non-blocker; v1.x catch-up):**
  - [ ] **UC14.A — `X-Accel-Redirect` handling.** **~1 day.**
  - [ ] **UC14.B — `.gz` / `.br` pre-compressed file selection**
    with `Accept-Encoding` negotiation. **~1 day.**

### 2.15 UC15 — Geographic load balancing

- **Current state:** working with two correctness issues.
  - MaxMind + IP2Location adapter at
    `src/proxy/geographic.rs`.

- **Remaining for v1.0 GA — UC15 P0 fixes:**
  - [ ] **UC15.A — Replace blocking `Mutex` on IP2Location
    DB** at `src/proxy/geographic.rs:20` with `RwLock`, or
    rebuild as in-memory read-only tree after load.
    **~½ day.**
  - [ ] **UC15.B — Log poisoned-lock event with structured
    fields then attempt recovery**; remove silent
    `db.lock().ok()?` swallow at `:87`. **~½ day.**
  - [ ] **UC15.C — `GeoProvider` trait extraction.** Define
    trait (lookup by IP → optional location); migrate MaxMind +
    IP2Location into `Arc<dyn GeoProvider>` impls; chosen
    provider via `HIGHPER_GEO_PROVIDER`. **~3 days.**

---

## 3. Cross-cutting workstreams

Items that don't bind to a single UC. Ordered by risk class
(security & correctness → completeness → hygiene → validation
→ documentation).

### 3.1 Admin API completion (was B2 in v1)

- **Current state:** `src/admin/server.rs` (2,068 lines) ships
  ~60–70% of B2's task list:
  - JWT decode + expiry + API-key auth at `:359-394`.
  - Full route CRUD (`GET/POST/PUT/DELETE /api/routes/*`) at
    `:237-251`.
  - Full upstream CRUD + per-server add/remove + load-balancing
    config at `:253-282`.
  - `GET /api/config` (summary form) at `:440-459`.
  - `POST /api/config/reload` (read-only-mode aware) at `:512`.
  - `GET /api/runtime-config` + `/diff` at `:469-509` (was
    Stage 3c-1 commit `20aa599`).
  - Login + user management at `:210-214`.
  - Health, readiness, stats, backends listing.
  - Two additional sub-modules acknowledged:
    `src/admin/config_persistence.rs` (461 lines, route/upstream
    save-to-disk) and `src/admin/dashboard.rs` (631 lines, HTML
    UI).

- **Remaining (~4 days):**
  - [ ] **3.1.A — Expand `get_config`** from summary → full
    `Config` tree with `SecretRef::Debug` redaction. **~1 day.**
  - [ ] **3.1.B — JWT scope/claims check** on decoded
    `JwtClaims`; per-endpoint authorization policy. **~½ day.**
  - [ ] **3.1.C — Verify `POST /api/config/reload`
    atomic-apply semantics** + wire `validate_against_config`
    re-run (see §3.5.B). **~½–1 day.**
  - [ ] **3.1.D — OpenAPI 3.1 spec** generated from handlers.
    **~2 days.**
  - [ ] **3.1.E — Audit log of admin-API actions**
    (Postgres or sled append-only). **~2 days.** Can defer to
    Phase 1.3 if scope-sliced.
  - [ ] **3.1.F — Delete orphan `src/admin/api.rs`** (354 lines,
    not in `src/admin/mod.rs:12-23` module tree). **~½ day.**

### 3.2 Graceful shutdown / spawn-task coverage (was B14 in v1)

- **Current state:** **partial.** Stage 3c-3 (commit `7c00488`)
  shipped a shutdown drain *delay* in `src/runtime/mod.rs`
  (`HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN`, default 10 s). But the
  full per-task tracking spec calls for `JoinSet` +
  `CancellationToken` — neither exists anywhere in the repo
  (verified by grep 2026-05-16).
- **Scope:** repo has **68 `tokio::spawn` sites** total;
  **~14 in-scope long-lived task sites**:
  - `src/runtime/mod.rs:172,179,186,190,207,222,245,266` (8)
  - `src/runtime/signals.rs:240` (1)
  - `src/http/http3_quiche.rs:215` (1)
  - `src/gateway/graphql/cache.rs:42` (1)
  - `src/gateway/graphql/executor.rs:57` (1)
  - 3 non-default-feature backends in `src/runtime/{epoll_backend,
    hybrid_stream, io_uring_shim}.rs`

- **Remaining (~5–7 days):**
  - [ ] **3.2.A — Wrap in-scope spawns in a single
    `JoinSet<()>` + `CancellationToken`** plumbed through the
    `Runtime` struct. The existing
    `HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN` becomes the
    `JoinSet::join_all()` timeout. **~5 days core wiring +
    ~2 days under-load testing** (pkill-TERM during traffic;
    assert no silent truncation).

### 3.3 RSA Marvin mitigation (was B13 in v1)

- **Current state:** `oidc` feature gates the vulnerable `rsa`
  crate (RUSTSEC-2023-0071); feature exists but not in
  default-features per `Cargo.toml:191` (verified 2026-05-02).

- **Remaining (~1 day):**
  - [ ] **3.3.A — CI gate.** New CI job runs
    `cargo tree --no-default-features` and fails when `rsa`
    appears in dep graph.
  - [ ] **3.3.B — `HIGHPER_FEATURE_OIDC=1` runtime probe**
    required to surface OIDC discovery endpoints. Default `0`.
  - [ ] **3.3.C — Write `docs/SECURITY_SCANNING.md`**
    documenting RUSTSEC-2023-0071 + operator guidance (prefer
    `jsonwebtoken` ring-based JWT validation).
  - [ ] **3.3.D — Upstream tracking** for `rsa` constant-time
    fix; when landed, remove `oidc` gate.

### 3.4 Phase 1.5 SAST stack (~3 days)

- **Decided 2026-05-03 owner gate #1** (see archived
  [`OWNER_GATES_2026-05-03.md`](archive/2026-05-16-v1/OWNER_GATES_2026-05-03.md)).
- [ ] **3.4.A — Tier A local SAST.** `cargo-clippy` (already
  on) + `cargo-audit` + `cargo-deny` (already on per
  `deny.toml`) + `cargo-geiger` + Semgrep (Rust ruleset).
  Wire all 5 into CI. **~1.5 days.**
- [ ] **3.4.B — Tier B cloud SAST.** CodeQL via GitHub
  Actions. **~1 day.**
- Tier C (`cargo-vet`, custom `dylint`) deferred to v1.x.

### 3.5 Configuration documentation backfill (was D11 in v1)

- **Current state:** `docs/CONFIG_ENV.md` (228 lines, last
  touched 2026-05-03) documents **6 of 16** `RuntimeConfig`
  sections. **§0.4 rule silently violated since 2026-05-03** —
  10 PRs landed env vars without updating the file.
- **Coverage gap:** ~88 distinct `env_string("…")` calls in
  `src/runtime_config/sections/*.rs` vs. ~54 documented rows
  in `CONFIG_ENV.md` (~34 env vars undocumented).
- **Missing sections:** `http3`, `tls`, `ratelimit`,
  `circuit_breaker`, `geo`, `cache`, `signals`,
  `config_watcher`, `observability` (all Stage 3a), `graphql`
  (B5).

- **Remaining (~2 days):**
  - [ ] **3.5.A — Backfill 10 missing sections in
    `CONFIG_ENV.md`** with one row per env var (default /
    valid range / reload tier). **~1.5 days mechanical.**
  - [ ] **3.5.B — `cargo xtask check-config-env`** consistency
    check that fails CI when a section file references an
    `env_string("…")` not present in `CONFIG_ENV.md`.
    **~½ day.**
  - [ ] **3.5.C — Cross-subsystem validator on reload paths.**
    Today `validate_cross_subsystem` re-runs on SIGHUP
    (`runtime_config/reload.rs:131-141`) but
    `validate_against_config(&RuntimeConfig, &Config)`
    (`runtime_config/loader.rs:85`) does not. Same gap on
    admin POST path at `src/admin/server.rs:512 reload_config`.
    Thread the most-recent `Arc<Config>` into both reload
    paths; call `validate_against_config` before
    `arc_swap.store()`. **~1 day.**
  - [ ] **3.5.D — End-to-end env-loader integration tests
    per section.** Today only `for_test()` defaults-only tests
    exist; add a `tests/runtime_config_e2e.rs` module using
    `serial_test` + `temp_env::with_vars` to mutate real
    `HIGHPER_*` vars and assert validators fire correctly.
    **~2 days.**
  - [ ] **3.5.E — Project-wide §0.4 lint enforcement.**
    Widen `xtask/src/lint_runtime_config.rs:33-36` from 4
    hard-fail paths (`src/plugin/`, `src/cluster/`,
    `src/cache/`, `src/ai/`) to project-wide. ~150 pre-existing
    literals tagged with `// allow:` waivers during widening
    (mechanical sweep). **~2 days.**

### 3.6 Phase 1.4 cross-cutting traits — v1.0 GA scope (non-B-tagged)

Per §0.5 core-first convention, these are **v1.0 GA scope
even though none is a B-blocker**. Slipping any to post-v1.0
breaks the "UC16 inherits not retrofits" guarantee and forces
the v1.x trait extraction effort estimate to blow out 2–3×.

- [ ] **3.6.A — `AuthProvider` trait.** Define + migrate
  `src/gateway/auth/{api_key, jwt, oauth2,
  oauth2_providers}.rs` to `Arc<dyn AuthProvider>` impls.
  Opens UC16 virtual-key auth as a 5th impl in v1.x.
  **~5 days.**
- [ ] **3.6.B — `MetricsBackend` / `LogBackend` traits.**
  Currently prometheus + tracing crates hardwired. Trait
  needed for OTLP exporter (v1.0) and UC16 cost metering
  (v1.x). **~5 days.**
- [ ] **3.6.C — `PeerDiscovery` trait.** Per §11 cluster
  bootstrap. `static` / `k8s_headless` / `dns` / `consul`
  impls. **~5 days.**
- [ ] **3.6.D — `SecretRef::Secrets://` resolver.** Stage 2
  shipped parse path; resolution errors loudly. Concrete
  clients required: Vault (`secrets://vault/<path>`), AWS
  Secrets Manager (`secrets://aws/<arn>`), Kubernetes Secret
  mount (`secrets://k8s/<name>`). UC3 (TLS) ACME EAB
  credentials may want this. **~5 days.**
- [ ] **3.6.E — `LoadBalancerStrategy` trait** (folded from
  v1 §4.4 row 1). Define `select(&self, candidates: &[Backend],
  req: &SelectionContext) -> Option<&Backend>`; migrate 8 HTTP
  algorithms in `src/proxy/loadbalancer.rs:365-461` + 5 TCP in
  `src/tcp/proxy.rs:103-150`. Both TCP and HTTP listeners take
  `Arc<dyn LoadBalancerStrategy>`. Algorithm chosen via
  `HIGHPER_LB_STRATEGY` or per-route DSL. Ships with UC1.A.
  **~10 days** (largest trait extraction; do this first to
  avoid rebase pain).
- [ ] **3.6.F — `CircuitBreaker` trait** (folded from v1 §4.4
  row 3). Extract single trait + state machine; remove
  duplicated logic in `src/proxy/circuit_breaker.rs:75-150`
  and `src/tcp/circuit_breaker.rs:81-200`. Thresholds via
  `HIGHPER_CB_*`. **~7 days.**

Total Phase 1.4 trait effort: **~37 days**. Several parallelize
across hands.

### 3.7 Phase 1.3 documentation (v1.0 GA scope; non-B-tagged)

All five files net-new. Total ~18 engineer-days; can run in
parallel with code work (different skill set).

- [ ] **3.7.A — `docs/MONITORING.md`** (Prometheus + Grafana
  quickstart with importable RED + USE dashboards for every UC).
  **~5 days.**
- [ ] **3.7.B — `docs/TROUBLESHOOTING.md`** (common errors →
  root cause → fix). **~3 days.**
- [ ] **3.7.C — `docs/UPGRADE.md`** (config migration matrix
  beta → v1.0). **~2 days.**
- [ ] **3.7.D — `docs/SECURITY_CLUSTER_BASELINE.md`**
  (per-cluster-type security hardening templates: Valkey AUTH +
  TLS; etcd client/peer mTLS + RBAC; K8s NetworkPolicy YAML;
  VM/BM firewall templates; secrets-rotation playbook).
  **~3 days.**
- [ ] **3.7.E — `docs/INTEGRATION_GUIDE.md`** (6 sections:
  plugin hooks; metrics consumption; audit-log export;
  inference engine integration; configuration sources;
  migrate-from-LiteLLM/Portkey walkthrough for UC16-prep).
  **~5 days.**

### 3.8 Phase 1.3.1 cluster-deployment cookbook cells (v1.0 GA scope)

- [ ] **3.8.A — 9 cookbook cells** under
  `examples/configs/clusters/<type>-<infra>/` (4 cluster types
  × {k8s, vm, baremetal}). Each cell: `README.md` + manifest /
  unit / playbook appropriate for the infrastructure + sanity
  check that `cargo run` loads the example config.
  **~6 days total.**
- [ ] **3.8.B — Decision-flow README** at
  `examples/configs/clusters/README.md` mapping deployment
  context → cell to pick. **~½ day.**

See archived
[`HA_ARCHITECTURE.md`](archive/2026-05-16-v1/HA_ARCHITECTURE.md)
for per-cluster-type design context.

### 3.9 Validation & soak (was B9 + B10 in v1)

Long lead time — start the rig now in parallel with code work;
finishes after P0a/P0b code is stable.

- [ ] **3.9.A — Cloud validation matrix for 9 UCs**
  (UC3 / UC5 / UC6 / UC7 / UC8 / UC10 / UC12 / UC13 / UC14;
  the 9 that failed for environment reasons in the prior
  `VALIDATION_REPORT_2026-01-11.md`, now archived).
  Pre-build container images for backend mocks; CI pipeline;
  cloud test on real VM. **~3 weeks elapsed; ~15 engineer-days.**
- [ ] **3.9.B — 7-day soak at 1M concurrent connections.**
  Memory-leak / fd-leak diff; chaos pass (kill backends,
  inject 5 % packet loss, add 200 ms latency, saturate CPU).
  **~10 engineer-days + 7 days wall-clock.**
- [ ] **3.9.C — Per-cluster-type RPS benchmark** at 3-replica
  HA size; document p50 / p99 latency adders per coordination
  layer (Type 1 / 2 / 3 / 4). **~5 days.**

### 3.10 UC16 platform extensibility — core only

Per §0.5 core-first convention. The platform pieces ship in
v1.0 even though the vertical (`src/ai/`) does not.

- **Already shipped in v1.0:**
  - `AiRuntimeConfig` section with 25 `HIGHPER_AI_*` env vars
    (Stage 2 commit `67bf863`).
  - Cross-subsystem validator rules covering AI / cluster
    interactions (`runtime_config/loader.rs:48-117`).
  - `SecretRef` URI grammar (Stage 2) — `Literal://` shipped;
    `Secrets://` resolver in §3.6.D above.

- **Remaining for v1.0 (cheap insurance, ~1 day):**
  - [ ] **3.10.A — Silent-no-op WARN** for UC16 env vars
    (was R1 in v1). Today `derive_enabled_ucs` in
    `runtime_config/loader.rs:122-160` only detects UC4 + UC11;
    UC16 detection is a TODO at `:143-148`. Add a UC16
    sentinel; at startup, if `HIGHPER_AI_*` is set but the
    sentinel reports UC16 absent, log WARN: "UC16 env vars
    set but module not built into this binary; values
    ignored". Prevents silent-no-op support tickets.
    **~1 day** (sentinel + WARN + serial_test fixture).
  - [ ] **3.10.B — `docs-keeper` cron extension** for UC16
    design-vs-code drift. Existing
    `trig_017YZKK1gLdJNntEAcSqVE7H` runs weekly. Extend to:
    (a) `runtime_config::ai.*` field ↔ `HIGHPER_AI_*` env var
    row in `docs/CONFIG_ENV.md` consistency; (b) trait names
    referenced in the archived UC16 design ↔ selector enum
    variants in `runtime_config`. **~1 day** to configure.

- **Deferred to v1.x** (UC16 vertical implementation, all in
  the archived [`USECASE_16_AI_LLM_GATEWAY.md`](archive/2026-05-16-v1/USECASE_16_AI_LLM_GATEWAY.md)
  design doc):
  - `src/ai/` module: `AiProvider` trait + 4 providers
    (OpenAI / Anthropic / Bedrock / Gemini); `AiStateStore`
    trait + ReDB / RocksDB / ScyllaDB impls; `VectorIndex`
    trait + Qdrant impl.
  - Virtual keys + budgets + spend rollups.
  - Exact + semantic cache.
  - SSE chunker + streaming.
  - Admin endpoints (`/admin/ai/*`).
  - Acceptance suite.

- **Risk mitigations (v1.0):**
  - **§3.7 R3 design freeze** at v1.0 GA tag time (snapshot
    archived UC16 design to `archive/2026-05-16-v1/` already
    done as part of this restart — but a v1.0-GA-specific
    snapshot is created at tag time per §3.11.D below).

---

## 3.11 v1.0 GA tag-day checklist (NEW)

Fires at the v1.0 GA tag commit. Owner: release manager. ~1 day
to draft `docs/planning/GA_CHECKLIST.md`; ~½ day to execute.

- [ ] **3.11.A — Verify all per-UC items in §2 closed**
  (acceptable: `[x]` or `[d]` with explicit reason).
- [ ] **3.11.B — Verify all §3 cross-cutting items closed.**
- [ ] **3.11.C — Snapshot `docs/CONFIG_ENV.md`** to
  `docs/planning/archive/v1.0-GA/CONFIG_ENV_v1.0-GA.md` so
  v1.x can diff.
- [ ] **3.11.D — Snapshot archived UC16 design** to
  `docs/planning/archive/v1.0-GA/USECASE_16_v1.0-GA.md`.
  Resume v1.x UC16 work against this frozen baseline (avoids
  the v1.x trait estimate blow-out).
- [ ] **3.11.E — Re-baseline reconciliation banners** in
  `README.md`, `KNOWN_LIMITATIONS.md`, `CHANGELOG.md`,
  `docs/ARCHITECTURE.md` against then-current docs.
- [ ] **3.11.F — Tag `Cargo.toml` version** + push git tag.
- [ ] **3.11.G — Cut `docs/planning/RELEASE_NOTES_v1.0.md`**
  listing all closed UC and cross-cutting items + new env
  vars + breaking changes.
- [ ] **3.11.H — Trigger `gap-auditor` monthly cron**
  + verify no open items.

---

## 4. v1.0 GA acceptance criteria

v1.0 GA is tagged when **all** of the following are true:

1. **Every UC1–UC15 section in §2 has all open items closed**
   (acceptable: `[x]` shipped or `[d]` deferred with explicit
   reason).
2. **All §3 cross-cutting items closed** (with the same `[x]`
   or `[d]` convention).
3. **§3.9 validation green:** 9 cloud-matrix UCs pass on at
   least one cloud VM image; 7-day soak completes without
   memory/fd leaks; chaos pass produces graceful degradation.
4. **§3.10 UC16 core extensibility shipped** with the §3.10.A
   silent-no-op WARN in place.
5. **§3.6 Phase 1.4 traits all shipped** even though
   non-B-tagged.
6. **§3.7 Phase 1.3 docs all shipped.**
7. **§3.8 9 cookbook cells shipped.**
8. **§3.11 GA tag-day checklist executed.**

---

## 5. Aggregate effort summary

One-engineer pace, no context-switching:

| Section | Engineer-days | Notes |
|---|---:|---|
| §2 Per-UC remaining (15 UCs) | ~58 | UC1.A (7) + UC2 (8) + UC3 (~12) + UC4 (~7) + UC5 (~7) + UC6 (1) + UC7 (~7) + UC8 (~5 non-blocker) + UC9 (~11) + UC10 (1) + UC11 (½) + UC12 (~2.5) + UC13 (0) + UC14 (~2) + UC15 (~4) |
| §3.1 Admin API | ~4 | server.rs ~60–70% done |
| §3.2 Spawn-task drain | ~5–7 | ~14 sites; single JoinSet |
| §3.3 RSA Marvin | ~1 | CI gate + docs |
| §3.4 SAST | ~3 | Tier A + Tier B |
| §3.5 Config docs backfill | ~7 | D11 + cross-subsystem fix + e2e tests + lint widen |
| §3.6 Phase 1.4 traits | ~37 | 6 traits; parallelizable across hands |
| §3.7 Phase 1.3 docs | ~18 | 5 new files; parallel-with-code |
| §3.8 Cluster cookbooks | ~6.5 | 9 cells + decision-flow README |
| §3.9 Validation + soak | ~30 | Long wall-clock; B9 + B10 + RPS bench |
| §3.10 UC16 core | ~2 | WARN + cron extension |
| §3.11 GA tag-day | ~1.5 | Checklist drafting + execution |
| **Total** | **~180** | One-engineer pace |

**Two-engineer pace + parallelization** (different skill sets
for §3.7 docs and §3.8 cookbooks vs. code work): wall-clock
~10–12 calendar weeks to v1.0 GA from 2026-05-16.

This estimate is **higher than v1's 155 days** because it now
explicitly costs every per-UC remaining item rather than
collapsing them under workstream banners. The v1 estimate
under-counted by absorbing UC2.A slowloris work, UC9 WAF body
decompression, UC15.A/B P0 fixes, and UC6.A `safe_lock!` swap
into the workstream tax.

---

## 6. Document lifecycle

This is a fresh v2 document. Future revisions append below
with date + one-line summary.

- **2026-05-16 (first revision, current):** v2 ROADMAP
  created. Restart from architectural review against
  UC1–UC15. v1 ROADMAP (3,962 lines, 42 lifecycle entries)
  and ~20 sibling planning / status-tracking documents
  archived to
  [`docs/planning/archive/2026-05-16-v1/`](archive/2026-05-16-v1/).
  Per-UC organisation replaces the v1 B-blocker /
  workstream / D-flag / M-item structure. UC16 vertical
  formally deferred to v1.x per §0.5; platform
  extensibility shipped in §3.10 stays in v1.0 GA. All v1
  decisions preserved as historical record in the archive
  with an `ARCHIVE_MANIFEST.md` cross-referencing them.

- **Future:** edit in place. Append to Section 6 with each
  substantive revision.

---

*Document author: claude-opus-4-7-1m, 2026-05-16. Working tree:
`master` head commit `b018071` at restart. Per CLAUDE.md rules.*
