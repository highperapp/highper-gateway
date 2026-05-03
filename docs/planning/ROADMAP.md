# Highper Gateway — Roadmap & Tracker

**Owner-of-record:** Highper Gateway maintainer.
**Last updated:** 2026-05-02.
**Status of this document:** Single source of truth for planning and tracking. All future TODOs and progress notes live here.

---

## 0. Document conventions

- This is the **single source of truth** for roadmap and tracking. Edit in place; do not fork.
- Companion documents:
  - `docs/planning/USECASE_16_AI_LLM_GATEWAY.md` — UC16 design **draft, not finalized**. Approach must be discussed and agreed before any UC16 implementation work begins. Scope is fenced — see §5 below.
  - `docs/AUDIT_2026-05-02.md` — original gap audit; cited from this document as the source for many entries.
  - `docs/CONFIG_ENV.md` (NEW, to be created in Phase 0) — authoritative reference for every `HIGHPER_*` environment variable.
- Superseded documents (do not read for current state):
  - `docs/archive/planning/PHASED_RELEASE_PLAN.md` — folded into Section 5 of this document.
  - `docs/ROADMAP.md` — stub stub deleted on 2026-05-02 as part of §0.5 reconciliation.
- Per `CLAUDE.md` rules: every concrete claim in this document is either cited (file path, optionally with `:LINE`) or marked `(unsourced inference)`. Verbatim quotes use quotation marks; paraphrases do not.
- Tracking semantics:
  - `[ ]` = pending work
  - `[~]` = in progress
  - `[x]` = complete (link the merging PR)
  - `[d]` = deferred (with one-line reason)
  - `[k]` = killed / out of scope (with one-line reason)
- When an item is deferred or killed, leave it visible — do not delete history.

### 0.1 Project-wide configuration rule (added 2026-05-02)

**Configuration-by-environment-variable. No hardcoding of operator-tunable values.**

- Every runtime-tunable value (timeout, queue depth, body-size limit, retry count,
  pool size, drain window, feature toggle, cache TTL, etc.) is loaded from a
  `HIGHPER_*`-prefixed environment variable at startup.
- Defaults live in a single `Settings` / `Config` struct loaded once at boot;
  hot paths must not call `std::env::var`.
- No compile-time constants for tunables. Existing `pub const DEFAULT_*` /
  `10 * 1024 * 1024` literals are bugs and are folded into the B12 workstream.
- Every new env var is documented in `docs/CONFIG_ENV.md` (NEW) before the PR
  introducing it can merge.
- Code-review checklist item: literal numeric / `Duration` constant in production
  paths → flag and either remove (load from env) or justify in a comment.

---

## 0.5 Pre-Phase-0 reconciliation (added 2026-05-02 — ✅ landed)

The 2026-05-02 audit surfaced four legacy documents that contradicted the
Section 4.1 release-blocker list. All four were corrected before Phase 0 work
begins so contributors and downstream readers are not misled.

| Issue | Action taken | Verification |
|---|---|---|
| `docs/ROADMAP.md` (1 KB stub from May 2 07:15) listed all 15 UCs as ✅, redirected away from this canonical roadmap | Deleted on 2026-05-02 | File no longer present in `docs/` |
| `KNOWN_LIMITATIONS.md` (project root) summary line "only 1 remaining item: Windows Native Testing" | Prepended a reconciliation banner referencing the 14 release blockers (B1–B14) and pointing to this document as authoritative | First H1 banner in the file |
| `README.md` (project root) implied production-ready / v1.0 | Inserted a v1.0-rc release-status callout above the Performance section, linking to §4.1 of this document | Top of README |
| `CHANGELOG.md` (project root) `[1.0.0] - 2026-01-10` entry framed as "Complete Production Release" | Reconciliation note above the `[Unreleased]` block re-classifies that entry as v1.0-rc and directs readers to this document for current trajectory | Top of CHANGELOG |
| `docs/ARCHITECTURE.md` did not mention the 8 weak interface boundaries (see §4.4) | Prepended a deprecation banner referencing §4.4 and noting `ARCHITECTURE_v2.md` will supersede after refactors land | Top of ARCHITECTURE |

These are documentation-only edits; no source code changed. Phase 0 work begins
from a consistent narrative.

---

## 1. Project snapshot

- **Name.** Highper Gateway. Rust-based reverse-proxy / API gateway, cited at `README.md` and `Cargo.toml`.
- **Working tree.** `D:\my-opensource\highper-gateway` (`highper-gateway/src/` is the crate). Branch `master`, head commit `10752ce` ("refactor: Fix mutable variable and dead assignment warnings") at audit time.
- **Test status (claim).** "763/763 unit tests passing" — `docs/VALIDATION_REPORT_2026-01-11.md:123` (audit confirms quote; result not independently re-run on 2026-05-02).
- **Validation status (claim).** "All failures are environmental/infrastructure issues, NOT code defects" — `docs/VALIDATION_REPORT_2026-01-11.md:115`. Six of 15 scenarios passed local validation; nine failed for environment reasons (cloud/Docker matrix not yet run). Source: `docs/VALIDATION_REPORT_2026-01-11.md:30-39, 83-96`.
- **Stated 15 use cases.** Per `docs/highper-gateway-15-usecases.txt`: UC1 L4 TCP; UC2 L7 HTTP/1.1; UC3 HTTPS+ACME+mTLS+OCSP; UC4 API Gateway with Rate Limiting; UC5 HTTP/3 QUIC (quiche); UC6 WebSocket LB; UC7 gRPC Gateway; UC8 Database LB (MySQL/PostgreSQL/Redis); UC9 WAF + mTLS; UC10 Hybrid Multi-Protocol; UC11 CDN Edge Caching; UC12 Microservices Discovery (Consul/etcd/CB); UC13 GraphQL Gateway; UC14 Static + PHP-FPM; UC15 Geographic LB (MaxMind/IP2Location).
- **Planned 16th.** UC16 AI/LLM Gateway. Greenfield. Design draft only. No code yet.

---

## 2. Existing functionality (UC1–UC15 + cross-cutting)

This section is a condensed, cited inventory of what is in the source tree today. Each `path:LINE` reference can be opened in the working tree to verify. Items here are the **basis** for the gap analysis in Section 4 — features listed here are present; features not listed here are absent (cross-checked against the parallel module audits summarized in `docs/AUDIT_2026-05-02.md`).

### 2.1 UC1 — Layer 4 TCP

- Bidirectional zero-copy forwarding via `tokio::io::copy_bidirectional` — `src/tcp/proxy.rs:287`.
- Five LB algorithms in `BackendSelector` (RoundRobin, LeastConn enum'd, ConsistentHash, IpHash, WeightedRR) — `src/tcp/proxy.rs:103-150`.
- Connection pool with LIFO queue, idle TTL, validation — `src/tcp/pool.rs`.
- Protocol-aware health checks for TCP / MySQL / Postgres / Redis — `src/tcp/health.rs`, `src/tcp/mod.rs:183-198`.
- Three-state circuit breaker — `src/tcp/circuit_breaker.rs`.
- TCP_NODELAY, SO_KEEPALIVE/TCP_KEEPALIVE — `src/tcp/proxy.rs:264-275`.
- SO_REUSEPORT, large backlog (4096/8192), SO_LINGER, TCP_FASTOPEN (Linux) — `src/utils/socket.rs:24-199`, `src/tcp/server.rs:67-113`.
- Graceful-shutdown broadcast — `src/tcp/server.rs:115-166`.

### 2.2 UC2 — Layer 7 HTTP/1.1 + HTTP/2

- hyper 1.5 with http1+http2 — `src/proxy/server.rs:13-14`.
- Eight LB algorithms (RoundRobin, LeastConn, Random, IpHash, ConsistentHash, LeastResponseTime, PowerOfTwo, Maglev, Geographic) — `src/proxy/loadbalancer.rs`.
- Rolling-window response-time selection — `src/proxy/loadbalancer.rs:115-150`.
- Circuit breakers + retry with backoff — `src/proxy/handler.rs:63-67`, `src/proxy/retry.rs`.
- Compression middleware (gzip / brotli / zstd / deflate) — `src/middleware/compression/`.
- SIMD-accelerated path matching — `src/gateway/routing/matcher.rs:11-90`.
- Hot-reload routing — `src/gateway/routing/hot_reload.rs`.
- Body size limits and streaming bodies — `src/http/body_utils.rs`, `src/http/streaming_body.rs`, `src/http/proxy_streaming.rs`.
- `Alt-Svc` advertising — `src/http/alt_svc.rs`.

### 2.3 UC3 — TLS termination, ACME, mTLS, OCSP, CRL

- rustls TLS 1.2/1.3 with ALPN — `src/tls/manager.rs:244-248`.
- Dynamic `CertResolver` for SNI-based selection — `src/tls/manager.rs:179-318`.
- TLS passthrough mode parses ClientHello SNI — `src/tls/passthrough.rs:20-186`.
- Cert hot-reload with validation — `src/tls/cert_reloader.rs`, `src/tls/cert_validator.rs`.
- ACME HTTP-01 challenge with TTL store — `src/tls/acme.rs:95-124`, `src/tls/challenge.rs:19-113`.
- OCSP fetcher with retry/backoff and stale fallback — `src/tls/ocsp_fetcher.rs:86-277`.
- OCSP cache with auto-refresh task — `src/tls/ocsp_cache.rs:45-165`.
- CRL fetcher with delta-CRL framework — `src/tls/crl_checker.rs`.
- CA bundle loader — `src/tls/ca_manager.rs`.
- kTLS framework module (no syscall path yet) — `src/tls/ktls/`.
- Client-cert extraction from rustls stream — `src/tls/acceptor.rs:51-64`.
- Cert info parsing (subject/issuer/serial/fingerprint/validity) — `src/tls/client_cert.rs:44-91`.
- Per-route mTLS policy (subject/issuer/serial/fingerprint) with wildcard DN match — `src/tls/client_verifier.rs:95-211`.
- Three mTLS modes (Required / Optional / OptionalNoCA) — `src/tls/client_verifier.rs:214-250`.
- Header injection (`X-Client-Cert-*`) — `src/tls/client_cert.rs:111-132`.

### 2.4 UC4 — API Gateway with Rate Limiting

- Two parallel implementations: `src/middleware/rate_limit.rs` and `src/gateway/ratelimit/{token_bucket, sliding_window, distributed}.rs`.
- Sliding window with VecDeque-based timestamps — `src/gateway/ratelimit/sliding_window.rs:82-170`.
- Distributed limiter with Redis Lua atomic token bucket — `src/gateway/ratelimit/distributed.rs:150-204`.
- API-key, JWT, OAuth2 authentication — `src/gateway/auth/{api_key, jwt, oauth2, oauth2_providers}.rs`.
- Request transform middleware — `src/middleware/transform.rs`.
- Empty parallel directory `src/gateway/rate_limit/` exists alongside `gateway/ratelimit/` (cleanup item).

### 2.5 UC5 — HTTP/3 / QUIC (quiche)

- UDP listener + quiche connection setup, Initial+Retry handshake — `src/http/http3_quiche.rs:177-180, 308-398`.
- HMAC-SHA256 address-validation tokens — `src/http/http3_quiche.rs:1060-1150`.
- BBR congestion control, 0-RTT, 10MB flow window, h3/h3-29/h3-28 — `src/http/http3_quiche.rs:1025-1050`.
- Streaming request body via mpsc channels — `src/http/http3_quiche.rs:631, 722`.
- Four-worker proxy task pool — `src/http/http3_quiche.rs:199-270`.
- LB + circuit-breaker integration — `src/http/http3_quiche.rs:695-712`.
- `Alt-Svc` header advertising with `ma=` — `src/http/alt_svc.rs`.
- QUIC metrics scaffold — `src/observability/quic_metrics.rs`.
- JSON lifecycle logger — `src/observability/quic_logger.rs`.

### 2.6 UC6 — WebSocket LB

- RFC 6455 upgrade + accept-key derivation — `src/websocket/handler.rs:13-50, 191-197`.
- Sticky sessions via `HPGW_WS_SESSION` cookie (HttpOnly, SameSite=Lax) — `src/websocket/handler.rs:53-83, 121-128`.
- Bidirectional copy via `tokio::io::copy_bidirectional` — `src/websocket/handler.rs:134-159`.
- Connection state machine (Connecting / Connected / Closing / Closed) and per-conn metrics — `src/websocket/connection.rs`.
- Configurable ping/pong with missed-pong threshold — `src/websocket/keepalive.rs`.
- Recovery + circuit breaker with seven error classes — `src/websocket/recovery.rs`.
- Graceful shutdown coordinator with phased timeouts — `src/websocket/shutdown.rs`.
- 16 MB message limit, 10-min idle, 1-hr session — `src/websocket/mod.rs`.

### 2.7 UC7 — gRPC Gateway

- HTTP/2 + content-type detection, gRPC path validation — `src/grpc/detector.rs`.
- gRPC-timeout RFC parsing (H/M/S/m/u/n) — `src/grpc/detector.rs:110-137`.
- Sixteen-status-code → HTTP mapping — `src/grpc/mod.rs:169-191`.
- Frame parser (5-byte header + payload), encode/decode — `src/grpc/streaming.rs`.
- `grpc.health.v1.Health` Check method with proto-level encoding — `src/grpc/health.rs`.
- Streaming-body forwarding (zero-copy) — `src/grpc/handler.rs:94`.
- LB policies enum'd: RoundRobin / LeastRequest / Random / PowerOfTwo / ConsistentHash — `src/grpc/mod.rs:114-125`.
- Per-method metrics scaffold — `src/observability/grpc_metrics.rs`.

### 2.8 UC8 — Database LB

- L4 protocol detection: MySQL `0x0a`, PostgreSQL StartupMessage, Redis RESP — `src/tcp/protocol.rs:14-109`; port fallback (3306/5432/6379) — `src/tcp/protocol.rs:100-108`.
- Per-backend pool with idle queue, semaphore limits, atomic stats — `src/proxy/database_pool.rs:259-505`.
- Pool prewarm + cleanup tasks — `src/proxy/database_pool.rs:445-479, 414-442`.
- TCP keepalive — `src/proxy/database_pool.rs:360-366`.
- Protocol-aware validation: MySQL COM_PING — `src/proxy/database_pool.rs:157-170`; Redis PING/RESP — `src/proxy/database_pool.rs:186-198`.
- Pool stats with reuse ratio — `src/proxy/database_pool.rs:482-504`.

### 2.9 UC9 — WAF + DDoS protection

- Engine abstraction with four backends declared: Custom, Coraza, ModSecurity, AWS — `src/middleware/waf/{engine, custom_engine, coraza_engine, modsecurity_engine, aws_engine}.rs`.
- Engine selector + block-vs-log mode + max-body cap — `src/middleware/waf/mod.rs:54-92`.
- Decision types (Allow / Block / RateLimit / Log) with severity — `src/middleware/waf/engine.rs:70-93`.
- Request context populated with method/path/query/headers/IP/body/UA — `src/middleware/waf/engine.rs:43-68`.
- DDoS module: per-IP conn-rate, slowloris idle timeout, rate limit + burst, blacklist/whitelist, automatic temp bans, geo filter (config only) — `src/middleware/ddos_protection.rs:24-49`.
- Security audit logger with 10+ event types and severity levels — `src/middleware/security_audit.rs:24-79`.
- Note: depth of Coraza/ModSecurity/AWS bindings (vs. stub) is unverified at audit time. (unsourced inference: based on file existence; functional verification pending.)

### 2.10 UC10 — Hybrid multi-protocol

- HTTP + HTTP/3 + Admin + Metrics tasks spawned by `Runtime::run` sharing `Arc<RwLock<Config>>`, `ProxyState`, hot-reload trigger — `src/runtime/mod.rs`.
- TCP server is **not** wired into `Runtime` (gap; see Section 4) — `src/runtime/mod.rs` (no `tcp::` import).

### 2.11 UC11 — CDN Edge Caching

- InMemory backend (DashMap, LRU cleanup, hit/miss counters) — `src/cache/backends.rs:18-179`.
- Disk backend (LRU eviction, zstd compression, atomic temp+rename writes, sharded files) — `src/cache/disk.rs:117-615`.
- Redis backend (`ConnectionManager`, SETEX TTL) — `src/cache/backends.rs:182-326`.
- MultiTier (L1 InMemory + L2 distributed; backfill on L2 hit) — `src/cache/backends.rs:331-423`.
- Tiered (memory hot + disk warm with promotion) — `src/cache/disk.rs:617-731`.
- Cache manager with `get_or_set`, `mget` / `mset`, pattern delete — `src/cache/manager.rs:78-325`.
- Gateway cache wrapper with `CacheEntry` (body + status + headers + TTL) — `src/gateway/cache/mod.rs`.

### 2.12 UC12 — Microservices Discovery

- `ServiceDiscovery` trait with get_instances / healthy filter / register/deregister / update_health / watch — `src/discovery/mod.rs:134-153`.
- Consul HTTP-API client with Passing/Critical/Warning/Unknown mapping, 30 s default polling, in-memory cache — `src/discovery/consul.rs`.
- etcd client with prefix queries (`/services/{name}/`), background poll — `src/discovery/etcd.rs`.
- Static backend with TCP-connect health probes — `src/discovery/static.rs`.
- Registry abstraction with self-register / self-deregister — `src/discovery/registry.rs`.
- Circuit breaker exists at `src/proxy/circuit_breaker.rs`; not auto-tied to discovery health.

### 2.13 UC13 — GraphQL Gateway

- Schema registry per backend, introspection fetcher — `src/gateway/graphql/schema.rs:73-200+`.
- Query parser via `async_graphql_parser` — `src/gateway/graphql/mod.rs:170-182`.
- TTL response cache with DashMap + 60 s expiry sweeper — `src/gateway/graphql/cache.rs:27-105`.
- Cache key = SHA256(query + variables) — `src/gateway/graphql/mod.rs:176-182, 268-272`.
- Federation-mode parallel fan-out with per-backend errors — `src/gateway/graphql/executor.rs:26-100`.
- Batch request handling with `max_batch_size` cap — `src/gateway/graphql/mod.rs:299-316`.

### 2.14 UC14 — Static files + PHP-FPM

- Full FastCGI record protocol (BEGIN_REQUEST, PARAMS length-prefix per spec, STDIN/STDOUT/STDERR, END_REQUEST) — `src/webserver/php_fpm.rs:14-274`.
- Unix and TCP socket support (auto-detect by path prefix) — `src/webserver/php_fpm.rs:82-96, 286-310`.
- Connection pool with 60 s idle expiry — `src/webserver/php_fpm.rs:43-79, 119-133`.
- Path-traversal-safe canonicalization — `src/webserver/static_files.rs:35-61`.
- Index-file lookup (index.html / .htm / .php) — `src/webserver/static_files.rs:63-107`.
- Directory listing (HTML and JSON) — `src/webserver/static_files.rs:109-157`.
- Linux `serve_file_sendfile` with kTLS hookup — `src/webserver/static_files.rs:313+`.
- MIME cache, range-request config, ETag config, per-extension cache-control — `src/webserver/{config, mime}.rs`.

### 2.15 UC15 — Geographic LB

- MaxMind adapter (`maxminddb` crate, City struct, lat/lon) — `src/proxy/geographic.rs:18-66`.
- IP2Location adapter (DB5+, lat/lon as f32→f64) — `src/proxy/geographic.rs:69-126`.
- Haversine distance (validated against NY–London ~5570 km, Sydney–Tokyo ~7800 km) — `src/proxy/geographic.rs:253-267, 274-311`.
- Nearest-server selection by min distance — `src/proxy/geographic.rs:174-235`.

### 2.16 Cross-cutting modules

**Observability.**
- Prometheus exporter — `src/observability/metrics.rs`.
- Structured logging with W3C-compatible request IDs — `src/observability/logging.rs:21-64`.
- Per-protocol metric and logger modules — `src/observability/{tcp_metrics, tls_metrics, quic_metrics, grpc_metrics, graphql_metrics, cache_metrics}.rs`.

**Admin API.**
- ~12 sub-modules under `src/admin/` (api, auth, backends, cache, config_persistence, dashboard, metrics, pool, request_metrics, routes, server, stats, upstreams).
- Header comment "Stub implementation - admin API not yet fully integrated" at `src/admin/api.rs:5`. JWT verify TODO at `src/admin/api.rs:125`. `get_config` placeholder at `src/admin/api.rs:140-149`.

**Plugin system.**
- WASM (wasmtime, fuel limits, epoch interruption) — `src/plugin/wasm.rs:24-28, 53-54, 142-146`.
- FFI dylib path and hot reload — `src/plugin/{ffi, hot_reload}.rs`.

**Config + DSL.**
- Multi-format (YAML/JSON/TOML) loader — `src/config/loader.rs`.
- Pest grammar for Caddy-like DSL — `src/config/dsl.pest`, `src/config/dsl_parser.rs`.
- Env-override, file watcher, reloader — `src/config/{env_override, watcher, reloader}.rs`.

**HA + clustering.**
- `src/ha/` and `src/health/` directories are **empty** (verified: `ls D:/my-opensource/highper-gateway/highper-gateway/src/ha/ src/health/` returns empty). High-availability is design-stubbed.

---

## 3. UC16 status

**Status: design draft; scope fenced 2026-05-02; storage-backend gate open;
no code yet.**

### 3.1 Scope (clarified by owner on 2026-05-02)

UC16 = highper-gateway acting as a **drop-in technical replacement for the
gateway/proxy role** played by LiteLLM and Portkey in AI application
deployments. Nothing more, nothing less.

**In scope** (the proxy's responsibilities):

- Multi-provider routing & translation (OpenAI / Anthropic / Bedrock / Vertex /
  Azure / Cohere / Mistral / xAI / DeepSeek / Groq / Together / Fireworks / …)
- Virtual API keys + per-key budgets, rate limits, usage caps
- Provider fallback & retry orchestration (reuses Phase 0/1 retry + CB)
- Token + cost metering and accounting
- Semantic cache **engine** + exact cache (cache **interface** only;
  embedding-model choice is the operator's, not highper's product)
- SSE / streaming pass-through and aggregation
- Single-node up to multi-node cluster topology
- Cookbook entry under `examples/configs/scenarios/scenario-16-…` with
  AI-app-developer-facing README

**Out of scope** (downstream / external concerns; do **not** absorb):

| Concern | Where it lives instead |
|---|---|
| Guardrails (input/output content filtering, PII redaction, jailbreak detection) | Customer-side service, optionally invoked **via** a highper plugin or upstream call — highper does not ship a guardrail engine |
| vLLM / self-hosted model serving | Customer's inference backend — highper proxies *to* it, does not host it |
| In-memory cache integration as a product feature | Cache *interface* exists in highper (UC11); concrete in-memory product layer is the operator's |
| AI observability platforms (Langfuse, Helicone-style dashboards, Phoenix, Langsmith, Arize) | Customer's observability stack — highper exposes Prometheus + OTLP; the *platform* layer is external |

**Why the fence matters.** Owner stated on 2026-05-02 that AI application
developers must be able to drop highper-gateway into their existing flow
without inheriting an entire ecosystem. Scope creep into guardrails / model
serving / dashboards would dilute the gateway role and conflate ownership.

### 3.2 Open owner gate — storage backend

Storage for virtual keys, budgets, usage records, and (Phase 2.5) sessions is
**NOT decided**. PostgreSQL, ScyllaDB, foundationdb, sled+gossip, Redis-only,
or other are all candidates.

Constraints to weigh:

- **Single-node deployment** must work without external dependencies.
- **Multi-node cluster** must support consistent budget enforcement (no double-spend).
- Strong durability for usage records (no silent loss on crash); eventual
  consistency for budgets is acceptable if we surface the lag.

**Implication for code:** introduce an `AiStateStore` trait in Phase 2.1 with
two ship-in-Phase-2 impls — `sled` (single-node, MVP) + a stub for the
distributed choice. Detailed discussion + decision required before Phase 2.4
(budget enforcement). This gate is item #5 in §6.

### 3.3 Companion design doc

The *historical* draft design lives in
`docs/planning/USECASE_16_AI_LLM_GATEWAY.md`. **Treat that document with
caution:** it predates the 2026-05-02 scope fence and may include items now
explicitly out of scope (guardrails, AI-observability dashboards). When the
two disagree, **this Section 3 is authoritative.** The companion doc will be
revised to match before Phase 2 starts (one of the actions guarded by gate #3
in §6).

UC16 work in Section 5 below (Phases 2–4) is **conditional on owner-finalized
scope + storage decision**. Phase entries are *placeholder estimates* until
those gates clear.

### 3.4 UC13 status — GraphQL Federation deferred for v1.0 (added 2026-05-02)

UC13 (GraphQL Gateway) ships in v1.0 as **passthrough + introspection cache +
depth/complexity enforcement**. Apollo Federation v2 entity resolution and
cross-subgraph query plans are **explicitly deferred to Phase 4.2**.

- v1.0 capabilities and current code are itemized in §2.13 above.
- The deferred design (rationale, what's out of scope, open Phase 4.2
  questions, acceptance criteria) is captured in
  [`docs/planning/GRAPHQL_FEDERATION.md`](GRAPHQL_FEDERATION.md). Treat that
  file as the **frozen** record until Phase 4.2 begins.
- Phase 0.E confirms the deferral; B5 in §4.1 is satisfied by the depth /
  complexity work, not by stitcher completion.
- Operators needing real federation in v1.x can run Apollo Router behind
  highper-gateway. This pairing is documented in the Phase 1.3 deployment
  guide update.

---

## 4. Observations & gaps

This section is the consolidated finding from `docs/AUDIT_2026-05-02.md` and the coverage trace performed on 2026-05-02. The earlier `PHASED_RELEASE_PLAN.md` had ~50 audit items not slotted into any workstream; the coverage trace surfaced them, and Section 5 places them by priority.

### 4.1 Top release blockers (must close before v1.0)

Cited from `docs/AUDIT_2026-05-02.md §1.2`:

| ID | Issue | Source |
|----|-------|--------|
| B1 | UC10 Hybrid not wired — `Runtime::run` never calls `TcpProxyServer::start()` | `src/runtime/mod.rs`, `src/main.rs:220-310` |
| B2 | Admin API stubbed (auth, config, ~20 TODOs) | `src/admin/api.rs:5, 125, 141` |
| B3 | OCSP request body never built; response only length-validated; stapling never attached; ACME `needs_renewal()` always returns `false` | `src/tls/ocsp_fetcher.rs:346-415`, `src/tls/ocsp_stapler.rs:97-136`, `src/tls/acme.rs:199-204` |
| B4 | Distributed rate-limiter fails open on Redis error without alert; X-Forwarded-For trusted unconditionally | `src/gateway/ratelimit/distributed.rs:54-62, 140-147`, `src/middleware/rate_limit.rs:162-182` |
| B5 | GraphQL depth/complexity not enforced; stitcher is acknowledged-stub | `src/gateway/graphql/stitcher.rs:56-57` |
| B6 | PostgreSQL pool validation is `peek()` only — broken connections returned to callers | `src/proxy/database_pool.rs:172-183` |
| B7 | gRPC fresh `hyper` client per request (no pooling); call-type heuristic only; trailers passed as headers | `src/grpc/handler.rs:54-100`, `src/grpc/detector.rs:57-78` |
| B8 | HTTP/3 `unwrap()` in receive loop; backend response fully buffered (`body.collect().await`); connection migration disabled | `src/http/http3_quiche.rs:403, 944, 1053` |
| B9 | Nine of 15 use cases never validated against real cloud backends | `docs/VALIDATION_REPORT_2026-01-11.md:30-39, 83-96` |
| B10 | No 7-day or 30-day soak data at the 1M+ connection target | `docs/TODO.md:112-122` |
| B11 | 8 `mpsc::unbounded_channel()` sites with no backpressure cap → OOM-under-abuse risk (verified 2026-05-02) | `src/http/http3_quiche.rs:186-187`, `src/runtime/signals.rs:196,211,229`, `src/config/watcher.rs:36`, `src/config/reloader.rs:79`, `src/tls/cert_watcher.rs:49` |
| B12 | Hardcoded body-size limits duplicated across 10+ files; no central env-driven config | `src/http/body_utils.rs:12`, `src/middleware/body_access.rs:18`, `src/webserver/security.rs:6,9`, `src/webserver/resource_limits.rs:39`, `src/middleware/request_validation.rs:49,66,81,96`, `src/middleware/streaming_validator.rs:71`, `src/middleware/request_size_limit.rs:27`, `src/config/dsl_ast.rs:617` |
| B13 | Optional `openidconnect` dep transitively pulls vulnerable `rsa` crate (RUSTSEC-2023-0071, Marvin attack); already feature-gated `oidc` but the feature exists and operators can opt in | `Cargo.toml:137-140, 191` |
| B14 | Federation executor + HTTP/3 worker pool spawn tasks via bare `tokio::spawn` with no `JoinHandle` tracked, no graceful drain on shutdown | `src/gateway/graphql/executor.rs:57`, `src/http/http3_quiche.rs:206` |

> **Citation correction (2026-05-02):** the prior coverage trace in
> `reverse-proxy-quick-progress-notes.txt` cited `src/discovery/registry.rs` as
> a fifth `unbounded_channel` site. Re-verification on 2026-05-02 found no such
> call there. The actual count is 8 sites listed above.

### 4.1.1 B11–B14 detailed plan (env-var-only) — added 2026-05-02

All four blockers honor the §0.1 rule: no compile-time constants.

**B11 — Bounded-channel knobs.** Replace each unbounded channel with a bounded
one whose depth and overflow policy come from env. Default `try_send` with a
named overflow policy (`drop_oldest` / `coalesce` / `block`) per call site.

| Site | Env var | Default depth | Overflow policy |
|---|---|---|---|
| `src/http/http3_quiche.rs:186` (req_tx, backend req queue) | `HIGHPER_HTTP3_REQ_QUEUE_DEPTH` | `8192` | `drop_oldest` + `503` to client + counter |
| `src/http/http3_quiche.rs:187` (resp_tx, backend response queue) | `HIGHPER_HTTP3_RESP_QUEUE_DEPTH` | `8192` | `drop_oldest` + log + counter |
| `src/runtime/signals.rs:196` (reload signal queue) | `HIGHPER_SIGNAL_RELOAD_QUEUE_DEPTH` | `16` | `coalesce` (idempotent reload) |
| `src/runtime/signals.rs:211` (shutdown signal queue) | `HIGHPER_SIGNAL_SHUTDOWN_QUEUE_DEPTH` | `16` | `coalesce` |
| `src/runtime/signals.rs:229` (USR1 signal queue) | `HIGHPER_SIGNAL_USER1_QUEUE_DEPTH` | `16` | `coalesce` |
| `src/config/watcher.rs:36` (config-file watch events) | `HIGHPER_CONFIG_WATCH_QUEUE_DEPTH` | `256` | `coalesce` (squash bursts) |
| `src/config/reloader.rs:79` (reload trigger queue) | `HIGHPER_CONFIG_RELOAD_QUEUE_DEPTH` | `16` | `coalesce` |
| `src/tls/cert_watcher.rs:49` (cert-file watch events) | `HIGHPER_TLS_CERT_WATCH_QUEUE_DEPTH` | `256` | `coalesce` |

**B12 — Body-size centralization.** Define `BodySizeLimits` struct, loaded once
at startup from env. Existing per-middleware `max_body_size` fields read defaults
from this central struct; per-route DSL/YAML can still override per-route.

| Knob | Env var | Default |
|---|---|---|
| Incoming HTTP body | `HIGHPER_BODY_MAX_INCOMING` | 10 MiB |
| Proxied body | `HIGHPER_BODY_MAX_PROXIED` | 10 MiB |
| WAF inspection cap | `HIGHPER_BODY_MAX_WAF` | 1 MiB |
| FastCGI / PHP-FPM request body | `HIGHPER_BODY_MAX_FASTCGI` | 10 MiB |
| Static-file response cap | `HIGHPER_BODY_MAX_STATIC` | 100 MiB |
| Streaming validator | `HIGHPER_BODY_MAX_STREAM` | 100 MiB |
| Logging body capture | `HIGHPER_BODY_MAX_LOG` | 1 KiB |
| Request-validation strict preset | `HIGHPER_BODY_MAX_VALIDATE_STRICT` | 1 MiB |
| Request-validation relaxed preset | `HIGHPER_BODY_MAX_VALIDATE_RELAXED` | 100 MiB |
| Request-validation API preset | `HIGHPER_BODY_MAX_VALIDATE_API` | 512 KiB |
| Webserver resource buffer | `HIGHPER_WEBSERVER_REQ_BUFFER_MAX` | 100 MiB |

**B13 — RSA Marvin-attack mitigation.** The `oidc` Cargo feature stays gated
off by default (verified at `Cargo.toml:191`); the work is to make this safer
operationally.

| Step | Detail |
|---|---|
| Default off | `oidc` feature **must not** be in the default feature set; verify in CI |
| CI gate | New CI job: `cargo tree --no-default-features` must not show `rsa` in dep graph |
| Runtime probe | `HIGHPER_FEATURE_OIDC=1` opt-in is required to surface OIDC discovery endpoints; default `0` |
| Documentation | New `docs/SECURITY_SCANNING.md` documents RUSTSEC-2023-0071 and operator guidance: prefer `jsonwebtoken` (ring-based) for JWT validation |
| Upstream tracking | When `rsa` upstream lands the constant-time fix, remove the `oidc` feature gate and re-enable; tracked as a separate one-line task in §12 |

**B14 — Graceful drain for spawned tasks.** Track every long-lived spawn in a
`JoinSet`; broadcast a `CancellationToken` on shutdown; await with timeout.

| Knob | Env var | Default |
|---|---|---|
| Default per-pool drain timeout | `HIGHPER_SHUTDOWN_DRAIN_SECS` | `30` |
| HTTP/3 worker-pool drain (override) | `HIGHPER_SHUTDOWN_HTTP3_DRAIN_SECS` | inherit `30` |
| GraphQL federation executor drain (override) | `HIGHPER_SHUTDOWN_GRAPHQL_DRAIN_SECS` | inherit `30` |
| Force-abort after timeout | `HIGHPER_SHUTDOWN_FORCE_ABORT` | `true` |
| Signal grace period before drain starts | `HIGHPER_SHUTDOWN_GRACE_SECS` | `5` |

Touch sites: `src/gateway/graphql/executor.rs:57` (replace `tokio::spawn` with
`JoinSet::spawn` + cancel-aware loop body); `src/http/http3_quiche.rs:206`
(same pattern; HTTP/3 workers already loop and recv — wrap in `select!` against
the cancel token).

### 4.2 Per-UC P0 / P1 gaps surfaced by coverage trace

(Items below were raised in the audit but were not placed in the prior `PHASED_RELEASE_PLAN.md`. They are placed by phase in Section 5.)

- **UC1.** EWMA peer scoring + outlier ejection; Unix-/abstract-socket upstream; connection draining on reload; `splice(2)` zero-copy; HDR-histogram latency percentiles (currently zeros at `src/tcp/proxy.rs:72-76`).
- **UC2.** PROXY-protocol v2 to upstream; request mirroring/canary; real EWMA + retry budget; Unix-socket upstream in hyper client.
- **UC3.** ACME DNS-01 (Cloudflare/Route53); ACME EAB; session-ticket-key rotation; TLS staging-URL config field; 0-RTT (rustls-dependent).
- **UC4.** GCRA / leaky-bucket; LRU eviction on bucket map; Prometheus rate-limit metrics; remove empty parallel `src/gateway/rate_limit/` directory.
- **UC5.** Auto-inject `Alt-Svc` on every response; GSO/sendmmsg path; GREASE in transport params; ECN/DSCP marking; HTTP/3 datagrams (RFC 9221).
- **UC6.** WS-over-HTTP/3 (RFC 9220); WS frame parser (per-frame metrics, close-code preservation); per-backend connection quota + handshake timeout; `Sec-WebSocket-Extensions` negotiation.
- **UC7.** Configurable per-method call-type registry **(P0; was lost from prior plan)**; gRPC-Web binary+text framing; server reflection v1 passthrough; `grpc-status-details-bin`.
- **UC8.** Read/write split; prepared-statement awareness; COPY-protocol awareness; Redis Cluster slot routing; Sentinel master discovery; sharding/hash-slot routing; query mirroring.
- **UC9.** DDoS geo-block actually consults `src/proxy/geographic.rs`; CRS version pinning; false-positive whitelist per rule per endpoint; SIEM-format (CEF/LEEF) WAF log export.
- **UC10.** Common LB pool shared L4↔L7; shared rate-limit `Arc` between L4 and L7 paths; cross-protocol metric correlation.
- **UC11.** `Vary`-aware cache key (correctness bug); singleflight; RFC 7234 `Cache-Control` parser; tag-based invalidation; conditional revalidation (304/If-None-Match); replace `DefaultHasher` (collision-attack vector at `src/cache/disk.rs:209`); stale-while-revalidate; `X-Cache: HIT|MISS` headers; negative caching policy; max object size at response level.
- **UC12.** Watch-streaming (Consul long-poll, etcd Watch); Kubernetes Endpoints/Service discovery; circuit-breaker tie-in to discovery weight=0 graceful drain.
- **UC13.** GraphQL subscriptions over WS; APQ (Automatic Persisted Queries); schema hot-reload; detailed error-path/extensions per field; query-plan cache.
- **UC14.** X-Accel-Redirect handling; `.gz` / `.br` pre-compressed file selection with `Accept-Encoding` negotiation; conditional revalidation correctness; `fastcgi_status` / `fpm_status` passthrough.
- **UC15.** **(P0)** RwLock around IP2Location DB (replaces blocking Mutex at `src/proxy/geographic.rs:20`); **(P0)** log poisoned-lock event then attempt recovery (replaces silent `db.lock().ok()?` at `src/proxy/geographic.rs:87`); ASN database adapter; country allow/block list; region failover; client-IP extraction integrated with proxy-trust list.

### 4.4 Interface-first architecture audit (added 2026-05-02)

The interface-first architecture audit identified **5 capabilities already
trait-driven** (Cache, Service Discovery, WAF, Plugin, Compression),
**8 retrofit boundaries** (rows 1–8 below — concrete or weakly-bounded
today; refactored during Phases 0–4), and **3 greenfield UC16 traits**
(rows 9–11 — built from scratch in Phase 2.1 / 2.4). Total: 11 trait
extractions in flight. Effort estimates assume one engineer with full context.

| # | Boundary | Current shape | Impact | Refactor effort | Phase placement |
|---|---|---|---|---|---|
| 1 | `LoadBalancerStrategy` | enum + hardcoded `match` (`src/proxy/loadbalancer.rs:365-461`) | Adding a new strategy requires editing core enum + every match arm | 2 wk | **0.A** (Hybrid wiring already touches Runtime/listeners) |
| 2 | `RateLimiter` | only `TokenBucket` (`src/middleware/rate_limit.rs:93-145`); no trait, can't swap GCRA / leaky-bucket / sliding-window cleanly | Phase 4.x adds GCRA — without trait, two parallel impls | 1 wk | **0.C** (rate-limit safety) |
| 3 | `CircuitBreaker` | duplicated state machine in `src/proxy/circuit_breaker.rs:75-150` and `src/tcp/circuit_breaker.rs:81-200`; no integration with discovery health | Bug fix lands twice; risk of drift | 1.5 wk | **0.D** (already touches connection pools) |
| 4 | `AuthProvider` | separate modules per scheme (`src/gateway/auth/{api_key, jwt, oauth2, oauth2_providers}.rs`); UC16 virtual-key scheme will be a 5th | Without trait, UC16 adds yet another sibling module | 1 wk | **1.4** (cross-cutting catch-up; UC16 needs it before Phase 2) |
| 5 | `ConnectionPool<C>` | three parallel impls with ~80 % code overlap (`src/proxy/connection_pool.rs`, `src/tcp/pool.rs`, `src/proxy/database_pool.rs`) | Bug fixes apply 3×; UC8 read/write split + UC16 per-provider pool will diverge further | 2.5 wk | **3.2** (heaviest refactor; risk medium-high; do after v1.0) |
| 6 | `GeoProvider` | concrete struct, MaxMind / IP2Location hardwired (`src/proxy/geographic.rs`) | UC15 P0 fixes already opening this file; trait-extract while we're there | 0.5 wk | **0.I** (UC15 P0 fixes) |
| 7 | `MetricsBackend` / `LogBackend` | hardcoded prometheus + tracing crates; no pluggable backend | OTLP exporter + UC16 cost metering both want a single seam | 1 wk | **1.4** (before OTLP work) |
| 8 | `ConfigSource` | YAML / JSON / TOML / DSL all hardcoded loaders (`src/config/loader.rs`) | GitOps and xDS (Phase 4) want a unified `ConfigSource` | 1 wk | **4.2** (lowest priority; current loader works) |
| 9 | **`AiStateStore` (UC16, greenfield, added 2026-05-02)** | n/a — new with UC16 | Operator selects backend per deployment via `HIGHPER_AI_STATE_BACKEND={redb\|rocksdb\|scylladb}`; trait must be in place from day-one of UC16 work or each impl forks | 1 wk trait + impls (folded into the 5-day Phase 2.1 task) | **2.1** (UC16 design decision #4) |
| 10 | **`AiProvider` (UC16, greenfield, added 2026-05-02)** | n/a — new with UC16 | Built-in providers (OpenAI, Anthropic) register statically; third-party `.so` / `.wasm` plugin loading opens Phase 3 once trait shape soaks (UC16 design decision #3); same evolution path as `Compressor` | 4 days (folded into Phase 2.1 AiProvider task) | **2.1** (UC16 design decision #3) |
| 11 | **`VectorIndex` (UC16, greenfield, added 2026-05-02)** | n/a — new with UC16 | Operator selects vector backend per deployment (`ai-vector-qdrant` / `ai-vector-redis-stack` / `ai-vector-pgvector` / `ai-vector-hnsw` Cargo features). Surface: `search` / `upsert` / `delete` / `delete_by_tag`. Same separate-trait pattern as `AiProvider` and `AiStateStore`. UC16 design decision #8. | 1 wk trait + Qdrant impl (folded into Phase 2.4 semantic-cache task); other 3 impls in Phase 3.1 (1 day each) | **2.4** trait + Qdrant; **3.1** for Redis-Stack / PgVector / HNSW |

**Total effort to reach interface-first:** ~11 person-weeks across all 11
boundaries, distributed across phases so no single phase pays the full cost.
Rows 9–11 are greenfield with UC16 work — cheaper per row than retrofits
but they must ship with the UC16 module skeleton.

**`AiProvider` and `AiStateStore` traits both follow the established
separate-trait-plus-registry pattern** of `Compressor`
(`src/middleware/compression/compressor.rs:70-127`) and `WafEngine`
(`src/middleware/waf/engine.rs:10-40`). They are deliberately *not*
extending the request-pipeline `Plugin` trait at
`src/plugin/trait_def.rs:35-100` (that's headers/body shaped); but they
*do* reuse the existing `src/plugin/` FFI + WASM loader machinery for
out-of-process plugins. See
[`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §3.3.

### 4.5 Cookbook infrastructure — already in repo (added 2026-05-02)

The "configurable + deployable with cookbooks" goal is **largely already met**.
The 2026-05-02 docs walkthrough enumerated existing assets; none of UC1–UC15
is missing a cookbook entry. This section is **maintenance-positioned**: keep
examples loading against the current schema after each Phase 0–4 change.

**Inventory of `examples/` (verified 2026-05-02):**

| UC | Cookbook entries |
|---|---|
| UC1 (L4 TCP) | `configs/scenarios/scenario-01-layer4-tcp.proxy`, `scenario-01-layer4-tcp-http.proxy`, `scenario-01-test.proxy` |
| UC2 (L7 HTTP) | `configs/scenarios/scenario-02-layer7-http.proxy`, `scenario-02-layer7-tls-termination.proxy` |
| UC3 (TLS / mTLS / ACME) | `configs/scenarios/scenario-03-layer7-tls.proxy`, `scenario-03-layer7-tls-simple.proxy`, `certificate-hot-reload.yaml`, `mtls-example.yaml`, `mtls-mutual-authentication.yaml` |
| UC4 (Rate Limiting / API GW) | `configs/scenarios/scenario-04-api-gateway.proxy`, `api-aggregation.yaml` |
| UC5 (HTTP/3) | `configs/scenarios/scenario-05-http3-quic.proxy`, `…yaml`, `http3-config.yaml` |
| UC6 (WebSocket) | `configs/scenarios/scenario-06-websocket.proxy`, `…yaml` |
| UC7 (gRPC) | `configs/scenarios/scenario-07-grpc.proxy`, `…yaml` |
| UC8 (DB LB) | `configs/scenarios/scenario-08-database-lb.proxy`, `…yaml` |
| UC9 (WAF + mTLS) | `configs/scenarios/scenario-09-waf-mtls.proxy`, `…yaml`, `waf-aws.yaml`, `waf-coraza.yaml`, `waf-custom.yaml`, `waf-modsecurity.yaml` |
| UC10 (Hybrid) | `configs/scenarios/scenario-10-hybrid-multiprotocol.proxy` |
| UC11 (CDN Cache) | `configs/scenarios/scenario-11-cdn-caching.yaml`, `scenario-11-cdn-edge-caching.proxy` |
| UC12 (Discovery) | `configs/scenarios/scenario-12-microservices-discovery.proxy`, `scenario-12-microservices.yaml` |
| UC13 (GraphQL) | `configs/scenarios/scenario-13-graphql-gateway.yaml`, `scenario-13-graphql.proxy` |
| UC14 (Static + PHP-FPM) | `configs/scenarios/scenario-14-static-php-fpm.proxy`, `…yaml`, `php-fpm-scenarios.dsl`, `php-fpm-demo/` |
| UC15 (Geographic LB) | `configs/scenarios/scenario-15-geo-routing.proxy`, `scenario-15-geographic-routing.yaml`, `geographic-load-balancing.yaml` |
| Cross-cutting | `simple-load-balancer.proxy`, `cloud-load-balancer.proxy`, `docker-compose-lb.proxy`, `distributed-tracing.yaml`, `hot-reload-example.yaml`, `admin-api/` |
| **UC16** | **none yet — created as Phase 2 deliverable** |

**Inventory of `deploy/` (verified 2026-05-02):** `docker/`, `kubernetes/`,
`helm/`, `systemd/`, `terraform/`, `ansible/`, `monitoring/` (Grafana JSON),
`configs/`, `scripts/`, `docs/usecases/`, plus `DEPLOYMENT_STRATEGY.md` and
`docs/DEPLOYMENT_GUIDE.md` (54 KB user-facing scenario cookbook covering all
15 UCs).

**Cluster-deployment cookbook coverage (added 2026-05-02 — see [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §1 + §7):**

Per the operator-chooses-per-deployment posture, each (cluster type ×
infrastructure) combination needs its own cookbook directory under
`examples/configs/clusters/`. None exist today — all 9 cells (plus the
top-level decision-flow README) are queued in Phase 1.3.1 above.

| Cell | Path | Status |
|---|---|---|
| Type A × K8s | `examples/configs/clusters/type-a-k8s/` | Phase 1.3.1 deliverable |
| Type A × VM | `examples/configs/clusters/type-a-vm/` | Phase 1.3.1 deliverable |
| Type A × Bare Metal | `examples/configs/clusters/type-a-baremetal/` | Phase 1.3.1 deliverable |
| Type B × K8s | `examples/configs/clusters/type-b-k8s/` | Phase 1.3.1 deliverable |
| Type B × VM | `examples/configs/clusters/type-b-vm/` | Phase 1.3.1 deliverable |
| Type B × Bare Metal | `examples/configs/clusters/type-b-baremetal/` | Phase 1.3.1 deliverable |
| Type C × K8s | `examples/configs/clusters/type-c-k8s/` | Phase 1.3.1 deliverable |
| Type C × VM | `examples/configs/clusters/type-c-vm/` | Phase 1.3.1 deliverable |
| Type C × Bare Metal | `examples/configs/clusters/type-c-baremetal/` | Phase 1.3.1 deliverable |
| Top-level decision flow | `examples/configs/clusters/README.md` | Phase 1.3.1 deliverable |

**Maintenance tasks (folded into Phase 1.3):**

- [ ] Schema-load test: write a CI job that loads every file under `examples/`
      with the current `Config` parser; fail PR if any example fails to parse
      after a Phase-0 schema change. **1 day.**
- [ ] Config-env injection test: run a representative subset of cookbook
      examples with all `HIGHPER_*` env vars set (per §0.1) and assert end-to-end
      startup. **2 days.**
- [ ] Add UC16 cookbook entry as part of Phase 2.6: `examples/configs/scenarios/scenario-16-ai-llm-gateway.{proxy,yaml}` +
      `examples/ai-gateway-quickstart.yaml` (multi-provider routing). **(in Phase 2.6).**
- [ ] Once each Phase-3 / Phase-4 N-feature lands, add or extend the matching
      cookbook entry as part of the same PR. **(per-feature; tracked under each N-item.)**

### 4.6 Competitor net-add features (N1–N25) — added 2026-05-02

The 2026-05-02 per-UC competitor study (HAProxy / HAProxy Enterprise / Nginx OSS
/ Nginx Plus / Envoy / Krakend / Pingora / Caddy for UC1–UC15;
LiteLLM / Portkey / Helicone / Kong AI / CF AI / OpenRouter / Envoy AI Gateway
for UC16) identified 25 features peers ship that highper does not. Each is
placed below by phase.

| ID | Feature | Peer reference | Priority | Phase |
|---|---|---|---|---|
| N1 | HAProxy-style stick tables (cross-instance session affinity, counters) | HAProxy | P1 | 3.2 |
| N2 | KrakenD-style response aggregation (declare upstream calls, merge result) | KrakenD | P1 | 4.1 (extend `examples/api-aggregation.yaml`) |
| N3 | OpenAPI 3.1 → routes (declarative routing from spec) | none directly; Krakend has partial | P2 | 4.2 |
| N4 | JSON-schema request validation middleware | Kong, Envoy ext_proc | P1 | 4.1 |
| N5 | gRPC-JSON transcoding | Envoy, Kong | P1 | 4.2 |
| N6 | GraphQL safelist / persisted-query-only mode | Apollo Router | P1 | 3.2 |
| N7 | (reserved — merged into existing UC13 APQ work) | — | — | n/a |
| N8 | GraphQL query-plan cache | Apollo Router | P1 | 4.2 |
| N9 | Nginx try_files DSL primitive for UC14 | Nginx | P1 | 3.2 |
| N10 | SCGI / uWSGI clients (FastCGI siblings) | Nginx | P2 | 4.2 |
| N11 | Varnish-style ESI (Edge-Side Includes) | Varnish, Akamai | P2 | 4.2 |
| N12 | Range-request slicing for UC11 large objects | Nginx slice module, Varnish | P1 | 3.2 |
| N13 | Caddy-style auto-HTTPS UX (ACME by default, on-the-fly) | Caddy | P1 | 4.2 |
| N14 | WebTransport (over HTTP/3 datagrams) | Envoy partial, Caddy plugin | P2 | 4.2 |
| N15 | MASQUE (UDP-over-HTTP/3 proxying) | Cloudflare | P2 | 4.2 |
| N16 | Per-message WS hook (transformation / inspection) | none open-source; Kong has WS plugins | P2 | 4.2 |
| N17 | Listener-filter-chain auto-detect (PROXY-protocol, TLS, HTTP) | Envoy | P1 | 4.2 |
| N18 | Bot management bundle (heuristics + rate by reputation) | HAProxy Enterprise, Cloudflare | P2 | 4.2 |
| N19 | ProxySQL-style query rewrite for DB LB | ProxySQL | P2 | 4.2 |
| N20 | HashiCorp Nomad service discovery | Nomad | P2 | 4.2 |
| N21 | AWS CloudMap / Azure Service Fabric discovery | (proprietary) | P2 | 4.2 |
| N22 | EDNS Client Subnet (ECS) for geo-routing | Cloudflare, geo CDNs | P2 | 4.2 |
| N23 | Helicone-style session/trace object linking N AI requests | Helicone | P1 | **2.5 (NEW for UC16)** |
| N24 | OpenRouter-style daily price-discovery feed | OpenRouter | P2 | 4.1 |
| N25 | xDS-driven UC16 routes (provider list pulled from xDS) | Envoy AI Gateway | P2 | 4.2 |

### 4.3 Cross-cutting gaps

- Observability: no OTLP exporter (Prometheus only); no exemplars; no SLO/SLI definitions; no built-in RED/USE dashboards; QUIC metrics described but never `counter!()`-recorded.
- Admin API: no audit log; no RBAC beyond API-key; no OpenAPI spec.
- Plugin: no WASI Preview 2 / component model; no plugin signing; metadata version hardcoded at `src/plugin/wasm.rs:104`; no per-plugin sandbox isolation; no marketplace.
- Config: no GitOps integration; no Vault / AWS Secrets / K8s Secret references; no template expansion; no profile inheritance.
- HA: empty `src/ha/` and `src/health/` directories; no leader election; no shared-state propagation; no clustering story.
- Code-quality debt: 80 in-source TODOs catalogued at `docs/TODO_COMPREHENSIVE.md:305-313` (admin 16, DSL converter 13, proxy handler/WAF 6); 123 non-critical clippy warnings per `docs/VALIDATION_REPORT_2026-01-11.md:132`; backup file `src/middleware/compression_old.rs.backup` to delete; `deny.toml.backup` to delete.
- Validation: cloud / Docker matrix unrun for the 9 environment-failed scenarios.
- Notes file: `docs/reverse-proxy-quick-progress-notes.txt` (52,400 lines) corroborates formal docs; no new release-blocking issues surfaced in spot-check (audit caveat: file not read in full).

---

## 5. Phased plan

This plan **supersedes** `docs/archive/planning/PHASED_RELEASE_PLAN.md`. Every audit item from Section 4 is placed below. New additions: Phase 0 prerequisite (Rancher Desktop), Phase 1.5 (SBOM + DAST + ZAP), Phase 1.6 (cheap P1 hygiene), and the previously dropped items are folded in.

### Phase 0 prerequisite — Local dev / test environment

**Why first.** Validates that everything else can be built, run, and scanned on the maintainer's machine.

- [ ] Install **Rancher Desktop** as the dev/test environment (provides containerd or dockerd + a local Kubernetes cluster). Document chosen container engine + K8s flavor in a new `docs/DEV_ENVIRONMENT.md`.
- [ ] Wire `cargo build --release` and the existing 763 unit tests in a Rancher-hosted container; verify clean run.
- [ ] Provide a `compose.yml` in repo root for Rancher Desktop's container runtime that brings up: gateway + Postgres + MySQL + Redis + Consul + etcd + NGINX-as-PHP-FPM-backend + a quic-capable NGINX backend + Prometheus + Grafana. (Reused by Phase 1.1 cloud validation matrix.)
- [ ] Document the workflow: `rancher-desktop start` → `docker compose up` → `cargo test --workspace` → SBOM + DAST + ZAP per Phase 1.5.
- [ ] Note: Rancher Desktop choice over Docker Desktop is the maintainer's call; document license rationale in `docs/DEV_ENVIRONMENT.md`. (unsourced inference: assumed driver is licensing.)

### Phase 0 — Release blockers (4–5 weeks)

The blockers map to Section 4.1 items B1–B10. Eight original workstreams plus one new (0.I) plus an addition to 0.D for the previously lost UC7 P0 item.

#### Workstream 0.A — Hybrid wiring + LoadBalancer trait extraction (UC10, B1)

- [ ] Add `Listener` config block (`protocol: tcp|http|http3|grpc`, bind list, TLS optional). All listener bindings configurable via `HIGHPER_LISTENER_*` env override matrix. **2 days.**
- [ ] `Runtime::run` instantiates per-listener server task; share `Arc<RwLock<Config>>`, `ProxyState`, hot-reload bus. **2 days.**
- [ ] Hot-reload propagates to TCP listener (currently HTTP-only). **2 days.**
- [ ] Cross-protocol metric correlation tests. **1 day.**
- [ ] **LoadBalancerStrategy trait extraction (added 2026-05-02):** define `LoadBalancerStrategy` trait covering `select(&self, candidates: &[Backend], req: &SelectionContext) -> Option<&Backend>`; migrate the 8 algorithms in `src/proxy/loadbalancer.rs:365-461` and the 5 in `src/tcp/proxy.rs:103-150` into trait impls; both TCP and HTTP listeners take `Arc<dyn LoadBalancerStrategy>`; algorithm chosen via `HIGHPER_LB_STRATEGY` env var or per-route DSL. **2 weeks.** *(folded from interface-first audit §4.4)*

#### Workstream 0.B — TLS honesty (UC3, B3)

- [ ] Pull in a real OCSP library (`rust-ocsp` / `webpki-ocsp`) — write proper OCSP request and verify response (signature, thisUpdate/nextUpdate, status, nonce). **5 days.**
- [ ] Wire `OcspStapler` cached responses into `CertifiedKey.ocsp` in rustls. **2 days.**
- [ ] Implement `needs_renewal()` against notBefore/notAfter (default renew at 30 d remaining). **1 day.**
- [ ] CRL `extract_crl_number` and `extract_delta_crl_url` via `x509-parser` extensions. **2 days.**
- [ ] Replace `fs::read` with `tokio::fs::read` in TLS load paths. **0.5 day.**
- [ ] Strip ACME private-key PEM from any debug log path. **0.5 day.**
- [ ] Add TLS staging-URL config field (e.g. `acme.directory_url`) and example config. **0.5 day.** *(folded from coverage trace)*
- [ ] CI: integration test against Let's Encrypt staging end-to-end. **2 days.**

#### Workstream 0.C — Rate-limit + WAF safety + RateLimiter trait (UC4, UC9, B4)

- [ ] Proxy-trust-list config (CIDR allowlist for `X-Forwarded-For`); fall back to socket addr outside the list. Configured via `HIGHPER_PROXY_TRUST_CIDRS` (comma-separated). **1.5 days.**
- [ ] Distributed limiter Redis-failure mode: `fail-open | fail-closed | local-fallback` (default `local-fallback`); env var `HIGHPER_RATELIMIT_REDIS_FAIL_MODE`. **2 days.**
- [ ] Per-route limit wiring from config to runtime (DSL + YAML). **1 day.**
- [ ] Custom WAF: real SQLi/XSS/path-traversal regex sets; verify Coraza binding actually compiles + runs CRS rules; if not, swap to Rust `coraza-rs` port. **5 days.**
- [ ] Body decompression pipeline (gzip/deflate up to cap from `HIGHPER_BODY_MAX_WAF`) before WAF runs. **3 days.**
- [ ] WAF rule hot-reload via signal + admin API. **2 days.**
- [ ] **RateLimiter trait extraction (added 2026-05-02):** define `RateLimiter` trait (`check(&self, key: &Key, cost: u64) -> Decision`); migrate `TokenBucket` in `src/middleware/rate_limit.rs:93-145` and `gateway/ratelimit/{token_bucket, sliding_window, distributed}.rs` into trait impls; chosen algorithm via `HIGHPER_RATELIMIT_ALGO`; opens the door for GCRA/leaky-bucket impls in Phase 4 without parallel code paths. **1 week.** *(folded from interface-first audit §4.4)*
- [ ] **Hot-key sharding for distributed rate limiting (added 2026-05-02 — addresses `HA_ARCHITECTURE.md` §1.5.4 F1 Valkey hot-key bottleneck):** add `HIGHPER_RATELIMIT_KEY_SHARDS` env var (default `1` = no sharding); when `>1`, the distributed limiter writes to N sub-keys (`<key>:<shard_id>`) chosen by a stable hash of the request, and reads aggregate by summing all N at decision time. Trade-off: ~0.1 ms extra latency per check vs. eliminating single-shard contention for popular keys. Add a `examples/configs/scenarios/scenario-04-rate-limit-hot-key.yaml` cookbook entry demonstrating the pattern. **2 days.**

#### Workstream 0.D — Protocol fixes (UC5, UC6, UC7, B7, B8, B14)

- [ ] **UC5/H3:** replace receive-loop `unwrap` (`src/http/http3_quiche.rs:403`) with safe drop + counter; re-enable connection migration with anti-amp checks; stream backend response (no `body.collect()`); wire actually-emitting metrics. **6 days.**
- [ ] **UC6/WS:** swap `RwLock::*().unwrap()` calls in `src/websocket/recovery.rs` for the existing `safe_lock!` poison-recovery macro. **1 day.**
- [ ] **UC7/gRPC:** reuse a per-upstream HTTP/2 client pool from `src/proxy/connection_pool.rs`; emit real HTTP/2 trailers (`http_body::Frame::trailers`) for `grpc-status`. **3 days.**
- [ ] **UC7/gRPC (added 2026-05-02):** configurable per-method call-type registry (proto descriptor optional in MVP) — corrects audit P0 lost from prior plan. **3 days.**
- [ ] **B14 graceful drain (added 2026-05-02):** wrap HTTP/3 worker-pool spawn at `src/http/http3_quiche.rs:206` and federation executor spawn at `src/gateway/graphql/executor.rs:57` in `JoinSet` + `CancellationToken`; honor env vars from §4.1.1 (`HIGHPER_SHUTDOWN_*`). Add integration test that pkill-TERMs the process under load and asserts no in-flight request is silently truncated within drain window. **3 days.**
- [ ] **CircuitBreaker unification (added 2026-05-02):** extract a single `CircuitBreaker` trait + state machine; remove duplicated logic in `src/proxy/circuit_breaker.rs:75-150` and `src/tcp/circuit_breaker.rs:81-200`; consumers (HTTP proxy, TCP, gRPC pool) take `Arc<dyn CircuitBreaker>`; thresholds, timeouts, and half-open probe count loaded from `HIGHPER_CB_*` env vars. **1.5 weeks.** *(folded from interface-first audit §4.4)*

#### Workstream 0.E — Application-protocol fixes (UC8, UC13, B5, B6)

- [ ] Real PostgreSQL pool validation (parameter-status / `SELECT 1` probe with timeout). **2 days.**
- [ ] PostgreSQL STARTTLS/SSLRequest negotiation. **3 days.**
- [ ] DB pool failover wired to circuit breaker. **2 days.**
- [ ] GraphQL depth + complexity analyzers wired to parsed AST; reject early; emit metric. **3 days.**
- [x] GraphQL stitcher decision (2026-05-02): **federation deferred to Phase 4.2** per `docs/planning/GRAPHQL_FEDERATION.md`. UC13 ships passthrough + introspection cache + depth/complexity (the Phase 0.E depth/complexity item above satisfies B5). Stitcher placeholder at `src/gateway/graphql/stitcher.rs:56-57` stays as-is for v1.0; no new federation code lands in Phase 0.E.

#### Workstream 0.F — Slowloris + header-size + UC2 hygiene + B11 backpressure (UC2, B11)

- [ ] Per-request read/idle timeouts (config-driven via env, see §0.1); 408 on incomplete header read. **2 days.**
- [ ] Header-block size cap → 431. Knob: `HIGHPER_HTTP_MAX_HEADER_BYTES` (default 64 KiB). **1 day.**
- [ ] HTTP/2 GOAWAY graceful drain on shutdown signal (overlaps with B14 drain). **2 days.**
- [ ] **B11 bounded-channel migration (added 2026-05-02):** replace 8 `mpsc::unbounded_channel()` sites listed in §4.1.1 with bounded `mpsc::channel(depth)` where depth comes from per-site env var; implement overflow policies (`drop_oldest` / `coalesce` / `block`) per the §4.1.1 table; emit `*_queue_dropped_total` and `*_queue_depth_current` metrics. **4 days.**

#### Workstream 0.G — Admin API truth (cross-cutting, B2)

- [ ] Real JWT verify (use `jsonwebtoken` crate or `jose` — pick one); token validation, scope check. **2 days.**
- [ ] `GET /admin/config` returns actual configuration with secrets redacted. **1 day.**
- [ ] `POST /admin/config` (validated, atomic apply, revert on failure). **3 days.**
- [ ] Backend enable/disable, weight changes (live, no restart). **2 days.**
- [ ] OpenAPI 3.1 spec generated from handlers. **2 days.**
- [ ] Audit log of admin-API actions (Postgres or sled append-only). **2 days.**

#### Workstream 0.H — Discovery quick-fix (UC12)

- [ ] Fix `should_refresh = true` always (`src/discovery/registry.rs:41`) — cache last-update; respect TTL. **0.5 day.**
- [ ] Consul ACL token field; mTLS to Consul/etcd. **2 days.**

#### Workstream 0.I — UC15 P0 fixes (UC15) — added 2026-05-02

- [ ] Replace blocking `Mutex` on IP2Location DB at `src/proxy/geographic.rs:20` with `RwLock`, or rebuild as in-memory read-only tree after load. **0.5 day.**
- [ ] Log poisoned-lock event with structured fields then attempt recovery; remove silent `db.lock().ok()?` swallow at `src/proxy/geographic.rs:87`. **0.5 day.**
- [ ] **GeoProvider trait extraction (added 2026-05-02):** define `GeoProvider` trait (lookup by IP → optional location); migrate MaxMind + IP2Location into `Arc<dyn GeoProvider>` impls; chosen provider configured via `HIGHPER_GEO_PROVIDER` env var. **3 days.** *(folded from interface-first audit §4.4)*

#### Workstream 0.J — Central env-driven configuration scaffold (added 2026-05-02 — supports §0.1)

- [ ] Define a top-level `Settings` struct with sub-structs for each subsystem (`Http3`, `Body`, `Shutdown`, `Signals`, `ConfigWatcher`, `Tls`, `RateLimit`, `CircuitBreaker`, `Geo`, `Cache`, etc.). All fields read from `HIGHPER_*` env vars at startup with documented defaults. **3 days.**
- [ ] Build a small "env settings" loader using `figment` or hand-rolled (no proc-macro reflection); validate types and ranges; refuse to start on out-of-range values. **2 days.**
- [ ] Create `docs/CONFIG_ENV.md` (NEW) — exhaustive table of every `HIGHPER_*` var with default, valid range, subsystem owner, and citation to where it's read. **1 day; updated alongside every PR that adds a new var.**
- [ ] CI lint: forbid bare `std::env::var` in `src/**/*.rs` outside `src/config/`; forbid literal `Duration::from_secs(..)` and `* 1024 * 1024` in production code (allowed in tests). **1 day.**
- [ ] **Cluster security env vars (added 2026-05-02 — supports `HA_ARCHITECTURE.md` §7.4):** add the following to the `Settings::cluster` sub-struct with refuse-to-start-on-missing semantics: `HIGHPER_CLUSTER_TYPEB_AUTH` (Valkey AUTH password, file path, or secrets-resolver ref); `HIGHPER_CLUSTER_TYPEB_TLS` (`true`/`false`); `HIGHPER_CLUSTER_TYPEC_CLIENT_CERT`, `_CLIENT_KEY`, `_CA` (file paths or secrets-resolver refs for etcd mTLS); `HIGHPER_CLUSTER_ALLOW_INSECURE` (default `false`; required `true` to start a Type 2/3/4 deployment without AUTH/mTLS — dev escape hatch). Validation: when `_TYPEB_BACKEND ≠ none`, `_TYPEB_AUTH` is required unless `_ALLOW_INSECURE=true`; same shape for Type C cert chain. **1.5 days.**
- [ ] **UC16 virtual-key pepper env var (added 2026-05-02 — supports `USECASE_16_AI_LLM_GATEWAY.md` §7.1.3):** add `HIGHPER_AI_KEY_PEPPER` to the `Settings::ai` sub-struct. 32-byte secret loaded as raw bytes (or hex-decoded) from env var or via the secrets-manager resolver (`HIGHPER_SECRETS_PROVIDER` from Phase 1.4). Used as the HMAC-SHA-256 key for virtual-key hashing. **Refuse to start** when UC16 features are enabled and pepper is empty unless `HIGHPER_CLUSTER_ALLOW_INSECURE=true`. Documented in `docs/CONFIG_ENV.md`; rotation guidance in `docs/SECURITY_CLUSTER_BASELINE.md`. **0.5 day.**
- [ ] **Plugin drain-window env var (added 2026-05-02 — supports `USECASE_16_AI_LLM_GATEWAY.md` §3.3.7):** move the hardcoded 30 s timeout in `src/plugin/manager.rs:252 wait_for_plugin_idle()` to `HIGHPER_PLUGIN_DRAIN_SECS` per §0.1 rule. Default 30 s. Validates ≥ 5 s. **0.5 day.**
- [ ] **UC16 pricing env vars (added 2026-05-02 — supports `USECASE_16_AI_LLM_GATEWAY.md` §5.5):** five env vars on the `Settings::ai.pricing` sub-struct: `HIGHPER_AI_PRICING_FEED_URL` (default LiteLLM upstream, operator can self-host), `HIGHPER_AI_PRICING_FEED_SIGN_KEY` (Ed25519 public key path or sigstore ref; refresh refuses unsigned feed when set), `HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS` (default 604 800 = 7 days; minimum 3600), `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE` (`last_known_good` default / `fail_closed` opt-in), `HIGHPER_AI_ALLOW_FREE_TIER` (default `false`; required `true` to allow requests for models with no pricing entry). Documented in `docs/CONFIG_ENV.md`. **0.5 day.**
- [ ] **UC16 cache + vector backend env vars (added 2026-05-02 — supports `USECASE_16_AI_LLM_GATEWAY.md` §6.0):** `HIGHPER_AI_CACHE_BACKEND` (default `valkey` → uses cluster Type B Valkey; alternatives: `redis` / `memory` / `disk` / `multi-tier` — same set as existing `src/cache/` trait); `HIGHPER_AI_VECTOR_BACKEND` (default `none`; one of `qdrant` / `redis-stack` / `pgvector` / `hnsw` — only required when semantic cache is enabled); `HIGHPER_AI_VECTOR_ADDRS` (host:port comma-list for non-`hnsw` backends); `HIGHPER_AI_VECTOR_AUTH` (auth token / file path / secrets-resolver ref). Refuse-to-start when an `ai_route` block enables `semantic_cache` and the vector backend / addrs are not set. **0.5 day.**
- [ ] **UC16 routing env vars (added 2026-05-02 — supports `USECASE_16_AI_LLM_GATEWAY.md` §3.4):** `HIGHPER_AI_RETRY_BUDGET` (default `3`; max attempts across providers per request; per-virtual-key override via scope's `retry_budget`); `HIGHPER_AI_COOLDOWN_BACKEND` (`auto` default → Valkey when Type B configured, local otherwise; alternatives `valkey` / `local` for forced override); `HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN` (default `50`); `HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX` (default `200`); `HIGHPER_AI_DEFAULT_COOLDOWN_SECS_NO_HEADER` (default `30`; used when provider 429s without `Retry-After`). **0.5 day.**
- [ ] **UC16 streaming env vars (added 2026-05-03 — supports `USECASE_16_AI_LLM_GATEWAY.md` §3.5):** `HIGHPER_PLUGIN_CHUNK_BUDGET_US` (default `500` µs; plugin overshooting budget per chunk is logged + skipped for the rest of the stream — fail-open at chunk level); `HIGHPER_AI_STREAM_BUFFER_DEPTH` (default `64` events per stream); `HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY` (`drop_oldest` default; alternatives `block` / `error`); `HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE` (default `true`; per-virtual-key override via scope); `HIGHPER_AI_DEFAULT_TPM_HARD_STOP` (default `false`; per-virtual-key override via scope). **0.5 day.**

#### Phase 0 deliverables

- All P0 issues from Section 4.1 closed.
- New unit + integration tests covering each fix.
- `docs/CHANGELOG.md` entry per workstream.
- `docs/KNOWN_LIMITATIONS.md` updated to remove fixed items.

#### Phase 0 parallelization

Workstreams **0.A, 0.B, 0.D, 0.G** are largely independent. **0.I** is independent and trivial. **0.C, 0.E, 0.F, 0.H** need to merge after 0.A and other Runtime-touching changes. With one engineer: ~14 weeks of work compressed to ~5 calendar weeks if focus + minimal context-switching.

### Phase 1 — v1.0 GA hardening (5–7 weeks)

#### 1.1 Cloud validation matrix (B9)

- [ ] Pre-build container images for all 15-scenario backend mocks (HTTP/3 with kernel QUIC, WebSocket echo, gRPC server with reflection, MySQL/Postgres/Redis, Consul + etcd, GraphQL test schema with subgraphs, MaxMind+IP2Location DBs, PHP-FPM with sample app). Use Rancher Desktop's container engine. **1 week.**
- [ ] CI pipeline: spin up each scenario, run scenario tests, archive results. **3 days.**
- [ ] Cloud test pass on a real VM (Linux 6.1+ for QUIC, kTLS): record evidence; close env-failure entries from `docs/VALIDATION_REPORT_2026-01-11.md`. **3 days.**
- [ ] Update `docs/VALIDATION_REPORT_<YYYY-MM-DD>.md` with green cells.

#### 1.2 Long-soak validation (B10)

- [ ] 7-day stability test at 1M concurrent connections. **7 days elapsed.**
- [ ] Memory-leak / fd-leak diff using heap profiler at start vs end.
- [ ] 30-day soak as parallel background activity once 7-day is green.
- [ ] Chaos-engineering pass: kill backends, inject 5 % packet loss, add 200 ms latency, saturate CPU, force-close upstreams. Record graceful-degradation behavior. **3 days active work + observation.**
- [ ] **Per-cluster-type RPS benchmark (added 2026-05-02 — supports `HA_ARCHITECTURE.md` §1.5):** measure sustained RPS for Type 1 / 2 / 3 / 4 deployments at 3-replica HA size; document p50 / p99 latency adders per coordination layer; surface Valkey hot-key bottleneck (F1) and etcd write ceiling (F2) with concrete numbers. Replaces the placeholder `(unsourced inference)` numbers in §1.5 with measured ones. **5 days.**

#### 1.3 Operational deliverables

- [ ] Verify `docs/DEPLOYMENT_GUIDE.md` (53 KB, dated Jan 24 — `docs/DEPLOYMENT_GUIDE.md`) still accurate; update for any Phase 0 config schema changes. **2 days.**
- [ ] `docs/MONITORING.md` (NEW) — Prometheus + Grafana quickstart with importable dashboards (RED + USE) for: HTTP, TLS, WAF, cache, discovery, geo, PHP-FPM, gRPC, HTTP/3, WebSocket. **5 days.**
- [ ] `docs/TROUBLESHOOTING.md` (NEW) — common errors → root cause → fix. **3 days.**
- [ ] `docs/UPGRADE.md` (NEW) — config migration matrix from beta → v1.0. **2 days.**
- [ ] Sample systemd unit, container image, Helm chart, compose file (baseline single-node, persona-agnostic). **3 days.**
- [ ] `SECURITY.md` (project root, 3.6 KB, `SECURITY.md`) updated with disclosure policy + bounty link. **0.5 day.**
- [ ] **`docs/SECURITY_CLUSTER_BASELINE.md` (NEW) (added 2026-05-02 — supports `HA_ARCHITECTURE.md` §7.4):** per-type security hardening templates. Sections: Valkey AUTH config + TLS setup; etcd client/peer mTLS config + RBAC role examples; K8s `NetworkPolicy` YAML templates per cluster type; VM/BM firewall-rules templates (iptables / nftables / ufw); secrets-rotation playbook (Valkey password, etcd certs); recovery procedures. **3 days.**

##### 1.3.1 Cluster deployment templates — 3 personas × 3 infrastructures (added 2026-05-02)

Per [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §1 + §7, highper-gateway
facilitates every (cluster type × infrastructure) combination through cookbook
templates. Each cell below is a Phase 1.3 deliverable landing under
`examples/configs/clusters/`. Note: now reorganised by the four-cluster-type
model (Type 1 / 2 / 3 / 4) instead of the earlier 3-persona × 3-infra cells. Cell layout:
`examples/configs/clusters/<type>-<infra>/` with `README.md` + the artifact
appropriate for that infrastructure (manifest / unit + keepalived / playbook).

- [ ] `examples/configs/clusters/type-a-k8s/` — `Deployment` + HPA + Service
      (LoadBalancer or ClusterIP+MetalLB) + pod anti-affinity. README links
      to UC1 / UC2 / UC8 / UC9 / UC10 / UC14 / UC15 cookbook entries.
      Includes a `values-typeA.yaml` for the Helm chart. **2 days.**
- [ ] `examples/configs/clusters/type-a-vm/` — systemd unit per node +
      Keepalived `keepalived.conf` for VIP failover. Cloud-init / Ansible
      playbook for 2-node Active/Standby bringup. **2 days.**
- [ ] `examples/configs/clusters/type-a-baremetal/` — Anycast bootstrap
      script + ExaBGP / FRR config + sample `bird.conf`. README explains
      kernel UDP tuning for UC5 (HTTP/3 prefers BM per `HA_ARCHITECTURE.md` §4 footprint guidance).
      **3 days.**
- [ ] `examples/configs/clusters/type-b-k8s/` — `StatefulSet` for highper +
      Valkey via Bitnami chart (or Valkey operator) + sample sharded-Valkey
      config to isolate UC4 rate-limit traffic from UC16 token quotas
      (per `HA_ARCHITECTURE.md` §2.4 isolation rule). **3 days.**
- [ ] `examples/configs/clusters/type-b-vm/` — Ansible playbook for a
      3-node Valkey cluster + N highper VMs pointing at Valkey via
      `HIGHPER_VALKEY_ADDRS`. Sample `keepalived.conf` for Valkey VIP. **3 days.**
- [ ] `examples/configs/clusters/type-b-baremetal/` — Valkey on dedicated
      bare-metal nodes; sub-ms-latency tuning README; recommended for UC11
      (BM Performance) and UC16 with adjacent self-hosted models. **3 days.**
- [ ] `examples/configs/clusters/type-c-k8s/` — etcd-operator manifests +
      highper `StatefulSet` reading cert / discovery state from etcd.
      Alternative: k3s with embedded etcd. README covers the
      "most-restrictive wins" implication: any deployment running UC3 or
      UC12 lands here regardless of other UCs. **3 days.**
- [ ] `examples/configs/clusters/type-c-vm/` — Ansible playbook for an
      odd-node etcd cluster (3 or 5 VMs) + N highper VMs pointing at etcd.
      Consul-based variant in a sibling subdirectory. **3 days.**
- [ ] `examples/configs/clusters/type-c-baremetal/` — etcd on dedicated
      bare-metal nodes for sub-ms quorum writes. README discourages this
      cell unless UC3 / UC12 latency budget genuinely demands it. **3 days.**
- [ ] **Cross-cell:** `examples/configs/clusters/README.md` (NEW) — table
      mirroring `HA_ARCHITECTURE.md` §3 all-in-one mapping, plus a "pick your cluster type" decision flow
      (which UCs do you enable? → which persona is required? → which infra
      do you have? → click into that cell). **1 day.**
- [ ] **Validation harness:** Phase 1.1 cloud-validation matrix runs each
      of the 9 cells against scenario fixtures so every cell is provably
      bootable end-to-end before v1.0 GA. **2 days CI work; runs as part of
      1.1.**
- [ ] **Single-node opt-in:** every cell's README documents how to fall
      back to a 1-node deployment via `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true`
      (acknowledging "0 % fault tolerance" trade-off). **0.5 day; doc.**
- [ ] **Cloud-VM sub-cells (added 2026-05-02 — supports `HA_ARCHITECTURE.md` §7.5):** each `type-{a,b,c}-vm/` cell needs a `cloud-vm/` sub-folder because **Keepalived does not work on AWS / GCP / Azure** (no L2 multicast). Sub-cell shape: same Ansible playbook for highper / Valkey / etcd VMs, but the front-LB switches from Keepalived to the cloud's L4 LB (NLB / Network LB / Standard LB). Include a `README.md` per sub-cell that lists which provider's LB to provision and the IAM permissions needed. Three sub-cells × three cluster types = nine paths — but the *cloud-LB* portion is mostly identical across cluster types, so factor as a shared module. **2 days.**

#### 1.4 Cross-cutting catch-up

- [ ] **AuthProvider trait extraction (added 2026-05-02):** define `AuthProvider` trait (`authenticate(&self, req: &AuthInput) -> AuthOutcome`); migrate `api_key`, `jwt`, `oauth2`, `oauth2_providers` in `src/gateway/auth/` into trait impls; provider chosen via per-route config + `HIGHPER_AUTH_DEFAULT_PROVIDER`; UC16 virtual-key store registers as a 5th impl in Phase 2. **1 week.** *(folded from interface-first audit §4.4; UC16 prerequisite)*
- [ ] **MetricsBackend / LogBackend traits (added 2026-05-02):** define `MetricsBackend` (counter/gauge/histogram emit) and `LogBackend` (event/span emit) traits; migrate the prometheus + tracing crate calls in `src/observability/{metrics, logging}.rs` behind these traits; OTLP and Prometheus become alternative backends rather than hardcoded calls; chosen via `HIGHPER_METRICS_BACKEND` / `HIGHPER_LOGS_BACKEND`. **1 week.** *(folded from interface-first audit §4.4; OTLP prerequisite)*
- [ ] OTLP gRPC exporter alongside Prometheus (now via the new `MetricsBackend` trait above). **5 days.**
- [ ] W3C Trace Context + B3 propagation end-to-end test. **2 days.**
- [ ] Vault / AWS Secrets Manager / K8s Secret reference resolver in config (resolver chosen via `HIGHPER_SECRETS_PROVIDER`). **5 days.**
- [ ] Cleanups: delete `src/middleware/compression_old.rs.backup`, empty `src/gateway/rate_limit/` directory, `deny.toml.backup`. **0.5 day.**
- [ ] Clippy debt to zero (currently 123 non-critical per `docs/VALIDATION_REPORT_2026-01-11.md:132`). **3 days.**

#### 1.5 Supply-chain + DAST + security scan baseline (NEW — added 2026-05-02)

**Goal.** Establish a clean baseline across SBOM generation, vulnerability scanning, and DAST before any v1.0 announcement. All of the following must be wired into CI and pass with zero unwaived findings before tag.

- [ ] **SBOM (Trivy):** `trivy fs --format cyclonedx-json --output sbom-trivy.cdx.json .` and `trivy image <gateway-image> --format spdx-json --output sbom-trivy.spdx.json` in CI. Publish artifact alongside release. **1 day to wire; baseline triage variable.**
- [ ] **SBOM (syft + Grype):** `syft <gateway-image> -o cyclonedx-json > sbom-syft.cdx.json` and `grype <gateway-image> -o sarif > grype.sarif` in CI. Compare findings against Trivy as a sanity cross-check. **1 day to wire.**
- [ ] **DAST (Dastardly):** integrate Dastardly (Burp's CI-friendly DAST) container against a Rancher-Desktop-hosted gateway instance with deliberately enabled UC4/UC9 features. Fail CI on critical findings. **3 days to wire + tune false-positives.**
- [ ] **DAST (OWASP ZAP):** ZAP Baseline Scan and Full Scan against the same instance; fail on Risk High; report Risk Medium. **3 days to wire.**
- [ ] Triage process: any unwaived finding blocks the v1.0 tag. Document waiver process (record CVE/CWE, justification, expiry) in `docs/SECURITY_SCANNING.md` (NEW).
- [ ] Schedule weekly re-scans post-GA; surface results in `docs/SECURITY_SCANNING_RESULTS_<YYYY-MM-DD>.md`.
- [ ] **B13 RSA Marvin attack (added 2026-05-02):** verify `oidc` feature off by default; new CI job runs `cargo tree --no-default-features` and asserts `rsa` is **not** present in the dep graph; document RUSTSEC-2023-0071 + operator guidance in `docs/SECURITY_SCANNING.md`; track upstream `rsa` constant-time fix as a follow-up to remove the gate altogether. **1 day.**

#### 1.6 Cheap P1 hygiene (NEW — added 2026-05-02)

Quick wins that materially reduce risk for v1.0 and cost <2 days each.

- [ ] Replace `DefaultHasher` (non-cryptographic) at `src/cache/disk.rs:209` with SipHash-1-3 or BLAKE3 — cache-key collision-attack mitigation. **0.5 day.**
- [ ] Auto-inject `Alt-Svc` on every HTTP response when an h3 listener is configured. **0.5 day.**
- [ ] Prometheus rate-limit metrics `rate_limit_hits_total{route,key,algo}` and `rate_limit_redis_errors_total`. **0.5 day.**
- [ ] DDoS geo filter actually consults `src/proxy/geographic.rs` adapters (currently config-only). **1 day.**
- [ ] `Retry-After` precision fix (avoid f64→u64 cast loss). **0.5 day.**
- [ ] Delete the empty parallel `src/gateway/rate_limit/` directory. **0.1 day.** *(may be folded into 1.4 cleanups; tracked here for visibility)*
- [ ] **B12 body-size centralization (added 2026-05-02):** introduce `BodySizeLimits` struct in `src/config/body_limits.rs`, populated from `HIGHPER_BODY_*` env vars per §4.1.1 table. Replace 11 hardcoded `pub const DEFAULT_MAX_BODY_SIZE` / `10 * 1024 * 1024` literals across `body_utils.rs`, `body_access.rs`, `webserver/security.rs`, `webserver/resource_limits.rs`, `request_validation.rs`, `streaming_validator.rs`, `request_size_limit.rs`, `dsl_ast.rs` with reads from `Settings`. Per-route overrides preserved. **3 days.**
- [ ] **DSL grammar parity with HTTP LB algorithms (added 2026-05-02 — addresses `HA_ARCHITECTURE.md` §4.1 footnote):** `src/config/dsl_ast.rs:402-410` exposes 7 of the 9 HTTP algorithms — `Maglev` and `Geographic` are missing. Extend the DSL `LoadBalancingAlgorithm` enum + the `Display` impl + the parser at `src/config/dsl_parser.rs` to accept `maglev` and `geographic` variants; add round-trip parser tests. **1 day.**

#### Phase 1 exit criteria

- All 15 UCs passing in CI **with cloud-grade container fixtures running under Rancher Desktop locally and on cloud CI.**
- 7-day soak green (no memory drift > 5 %, no fd leaks, no panics).
- Phase 1.5 SBOM + DAST + ZAP baseline clean (zero unwaived findings).
- Deployment, monitoring, troubleshooting docs published.
- Public announcement post drafted.

### Phase 2 — UC16 MVP (4–6 weeks) — *conditional on owner-finalized design*

**Status: blocked.** UC16 design (`docs/planning/USECASE_16_AI_LLM_GATEWAY.md`) is a draft with 13 open questions in §12. Owner must answer before any of the work below begins. Phase 2 entries are placeholders.

#### 2.1 Core scaffolding (week 1)

- [ ] Owner-driven decisions: answer the design questions in `USECASE_16_AI_LLM_GATEWAY.md` §12; **decisions #1–#4 already resolved 2026-05-02** (scope fence; OpenAI+Anthropic dual-shape MVP; AiProvider plugin architecture; ReDB/RocksDB/ScyllaDB AiStateStore). Remaining open questions (#4 tenant model, #5 vector index, #6 token-counter posture, #7–#16) committed in design notes. **1 day** (down from 2 — 4 are pre-decided).
- [ ] Create `src/gateway/ai/` module skeleton with files from §3.2 of UC16 doc. **0.5 day.**
- [ ] Canonical `AiRequest` / `AiResponse` types per UC16 doc §4. **2 days.**
- [ ] Inbound shape detect + parse for **both OpenAI and Anthropic** (chat + embeddings + models for OpenAI; messages for Anthropic) — UC16 #2. **3 days** (was 2 — Anthropic-shape inbound moved to MVP).
- [ ] **`AiProvider` trait + plugin-loadable registry (UC16 #3, 2026-05-02):** define `AiProvider` trait at `src/gateway/ai/provider.rs` following the existing `Compressor` (`src/middleware/compression/compressor.rs:70-127`) and `WafEngine` (`src/middleware/waf/engine.rs:10-40`) separate-trait pattern (NOT extending the request-pipeline `Plugin` trait at `src/plugin/trait_def.rs:35-100`). Built-in providers (OpenAI, Anthropic) register statically at startup. Plugin loader paths (FFI dylib via `src/plugin/ffi.rs`; WASM via `src/plugin/wasm.rs`) wire up but **defer third-party plugin loading to Phase 3** to let the trait shape soak — same evolution path Compressor took. Provider list, default model, and per-provider endpoint configured via `HIGHPER_AI_PROVIDERS` / `HIGHPER_AI_<PROVIDER>_*` env vars. Trait shape includes: `name()`, `metadata()`, `estimate_input_tokens()`, `call_streaming()`, optional `health()` and `parse_ratelimit_headers()`. **4 days** (was 3 — plugin loader scaffolding adds 1 day even though third-party loading is deferred).
- [ ] **`AiStateStore` trait + 3 backend impls (UC16 #4, 2026-05-02):** define `AiStateStore` trait at `src/gateway/ai/state_store/mod.rs`. Ship three impls: `redb` (pure-Rust embedded; single-node default), `rocksdb` (mature embedded; alternative), `scylladb` (Cassandra-compatible; multi-node). Operator chooses via `HIGHPER_AI_STATE_BACKEND={redb|rocksdb|scylladb}`. Trait surface: virtual-key CRUD, budget read/decrement, usage record append, audit log append. **5 days** (1 day trait + 1.5 day ReDB + 1 day RocksDB + 1.5 day ScyllaDB). Migration / export tool from ReDB to ScyllaDB is queued for Phase 3 per UC16 §12 #16.

#### 2.2 Translators (week 2)

- [ ] OpenAI ↔ canonical (passthrough). **1 day.**
- [ ] Anthropic Messages translator (incl. tool-use shape difference, system-prompt extraction). **3 days.**
- [ ] Bedrock Converse translator + SigV4 signer (use `aws-sigv4` crate). **5 days.**
- [ ] Gemini `generateContent` / `:streamGenerateContent` translator (incl. SSE-ish streaming). **3 days.**

#### 2.3 Routing + accounting (week 3)

- [ ] **Pricing registry (UC16 #7, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §5.5):** module at `src/gateway/ai/pricing.rs` with two-layer lookup (vendored snapshot + admin overrides applied on top, both held in `Arc<RwLock<HashMap>>`). Boot-load from baked LiteLLM JSON snapshot (build-time include via `include_bytes!`); validate schema; populate registry. Snapshot file vendored at `crates/highper-ai-prices/data/model_prices_and_context_window.json` with a script in `xtask` to refresh from LiteLLM upstream. **1.5 days.**
- [ ] **Pricing weekly signed refresh job (UC16 #7):** internal cron-like task driven by `HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS` (default 604 800 s). Fetches `HIGHPER_AI_PRICING_FEED_URL`, verifies Ed25519 signature against `HIGHPER_AI_PRICING_FEED_SIGN_KEY`, atomically swaps the snapshot layer of the registry. Failure mode `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE` (`last_known_good` default, `fail_closed` opt-in). Emits `ai_pricing_refresh_succeeded_total` / `_failed_total{reason}` metrics. **2 days.**
- [ ] **Pricing admin-override endpoints (UC16 #7):** extend `src/admin/api.rs` with `PATCH /admin/ai/models/{alias}` (override row), `GET /admin/ai/models/{alias}` (effective row), `DELETE /admin/ai/models/{alias}/override` (revert), `GET /admin/ai/pricing/refresh-status`, `POST /admin/ai/pricing/refresh-now`. Overrides stored under `ai/pricing_overrides/global/{alias}` namespace in the configured `AiStateStore`. Every change emits a hash-chain audit event into the `AiStateStore` audit log. Hot-applied via `RwLock` swap — no restart. **2 days.**
- [ ] **Zero-price handling (UC16 #7):** lookup-time check; default reject with 400 + structured error. Honors `HIGHPER_AI_ALLOW_FREE_TIER` for self-hosted / free-tier deployments. Emits `ai_request_no_pricing_total{provider,model}` when allowed. **0.5 day.**
- [ ] Model registry loader (`model_prices_and_context_window.json` from LiteLLM, vendored) — **superseded by the four pricing tasks above 2026-05-02; kept as a checkbox so historical references resolve.** **(0 days; rolled into above)** [d]
- [ ] **Router with layered filtering (UC16 #9, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §3.4):** module at `src/gateway/ai/router.rs`. Five-stage pipeline:
      (A) per-virtual-key `models_allow` filter,
      (B) capability-flags filter (drops providers that can't serve the request shape — vision / function calling / JSON mode / streaming requirements derived from request),
      (C) circuit-breaker filter (reuses `src/proxy/circuit_breaker.rs` per UC16 §3.2),
      (D) rate-limit cooldown filter (parses `x-ratelimit-*` and `Retry-After` from provider responses; cooldown state in Type B Valkey when configured else local `DashMap` — selection via `HIGHPER_AI_COOLDOWN_BACKEND={auto|valkey|local}`),
      (E) priority ordering (operator-declared list at MVP).
      Retry budget default 3 attempts; per-virtual-key override via scope's `retry_budget` field. Backoff honors `Retry-After` when present, exponential with jitter (50-200 ms) otherwise. Exhausted → 503 with structured per-attempt error body. Emits 7 metrics (`ai_route_attempts_total`, `_fallback_taken_total`, `_exhausted_total`, `_cooldown_active`, `_cooldown_remaining_seconds`, `_capability_filter_drops_total`, `_circuit_breaker_open`). **5 days** (was 3 — capability filter + cooldown sharing + structured error body add 2 days).
- [ ] **Cost-aware routing (UC16 #9, Beta — Phase 3.1, 2026-05-02):** alternative ordering mode that sorts the post-filter candidate list by `expected_cost = input_tokens * input_price + max_tokens * output_price`, picks cheapest meeting per-route SLA. Operator opt-in per `ai_route` block with `routing_mode = cost_aware`. **3 days.** *(Phase 3.1)*
- [ ] **Latency-aware routing (UC16 #9, GA — Phase 4.1, 2026-05-02):** alternative ordering mode that sorts by observed p50 / p99 latency per (provider, model). Requires `ai_provider_latency_seconds` histogram from §9.1. Operator opt-in per `ai_route` block with `routing_mode = latency_aware`. **3 days.** *(Phase 4.1)*
- [ ] **Weighted / canary split (UC16 #9, Beta — Phase 3.1, 2026-05-02):** probabilistic pick over the ordered candidate list with operator-declared weights, e.g. `model_alias_map["smart"] = [{provider: "openai/gpt-4o", weight: 90}, {provider: "anthropic/claude-3-7", weight: 10}]`. Per-tenant override. Useful for quality A/B testing. **2 days.** *(Phase 3.1)*
- [ ] **Token counter — bake-in posture (UC16 #6, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §5.1 / §5.2):** `tiktoken-rs` crate for OpenAI `cl100k_base` + `o200k_base`; `tokenizers` (HuggingFace) crate for Anthropic BPE at MVP. Vocabularies baked into the binary; ~6 MB delta for these two providers. Default Cargo feature includes both. New `ai-tokenizers-minimal` Cargo feature ships only OpenAI's two for size-conscious builds. Implementation site: `src/gateway/ai/tokens.rs`. **3 days** (was 2 — adds Anthropic at MVP per UC16 #2). |
- [ ] **Reasoning-token billing rule (UC16 #6, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §5.4):** virtual-key scope gains `count_reasoning_in_output: bool` (default `true`). Token counter records reasoning tokens (Anthropic `thinking`, OpenAI `reasoning_content`, DeepSeek `reasoning_content`) and the accounting layer respects the flag for budget / TPM decrement. Metrics emit `ai_output_tokens_total` with a `kind={prompt|completion|reasoning}` label regardless of billing inclusion. **1 day.**
- [ ] **Phase 3 follow-on — additional provider tokenizers (added 2026-05-02):** when Bedrock + Gemini + the 9 more Phase 3 providers ship, their vocabularies join the bake set: Gemini SentencePiece (~4 MB), Llama-3 BPE (~9 MB), Cohere (~4 MB), DeepSeek + Qwen + others (~7 MB combined). Total binary growth at Phase 3 GA: ~30 MB. Tracked under Phase 3.1 ("9 more providers") rather than as separate work — vocabularies ship alongside their respective `AiProvider` impls.
- [ ] Cost calculator (post-stream + on-cancel). **1 day.**
- [ ] Per-(provider, region) HTTP/2 connection pool wired from `src/proxy/connection_pool.rs`. **2 days.**

#### 2.4 Auth + budgets + cache (week 4)

- [ ] Virtual-key store on the configured `AiStateStore` (UC16 #4) + `sk-hpgw-…` issuance + **HMAC-SHA-256 hash with `HIGHPER_AI_KEY_PEPPER` server pepper** (UC16 #5; replaces the original Argon2id approach — see `USECASE_16_AI_LLM_GATEWAY.md` §7.1.3 for rationale). Constant-time compare. Soft-disable revocation (`enabled=false`) by default; hard-delete is a separate scoped admin action. **3 days.**
- [ ] Per-key budget enforcement (day/month) backed by **Type B Valkey counters + `AiStateStore` durable rollup** (was "Redis counters + sled"; updated 2026-05-02 per UC16 #4). **3 days.**
- [ ] RPM + TPM (token-denominated) buckets — extends `src/gateway/ratelimit/token_bucket.rs` to support dynamic-cost consumption. **3 days.**
- [ ] **Exact cache engine (UC16 #8, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §6.0–§6.1):** module at `src/gateway/ai/cache_exact.rs` with canonical-JSON key (sorted, no float reformat, byte-stable for §6.3 provider prompt-cache compatibility), TTL handling, streaming replay, per-tenant key-space isolation. Backed by the existing `src/cache/` trait — operator picks KV backend (Valkey via existing Redis client / disk / memory / multi-tier) via DSL `cache.backend`. Admin invalidation API: `POST /admin/ai/cache/invalidate` (tag / pattern / per-tenant). **2 days.**
- [ ] **`x-cache` response header + `x-cache-ttl` request header (UC16 #8):** wire response header `HIT | MISS | BYPASS | SEMANTIC` and per-request TTL override. **0.5 day.**
- [ ] **Admin cache-invalidation endpoint (UC16 #8):** `POST /admin/ai/cache/invalidate` with `{tag}` / `{pattern}` / `{tenant_id}` body shapes. Audit event per call into `AiStateStore` audit log. **1 day.**

#### 2.5 Streaming + admin + tests (week 5)

- [ ] **SSE-aware chunker with hook chain (UC16 #10, 2026-05-03 — see `USECASE_16_AI_LLM_GATEWAY.md` §3.5.3):** module at `src/gateway/ai/sse.rs`. Parse provider's SSE / event-stream-binary / NDJSON; re-emit as inbound-shape SSE. Per-chunk hook chain: token counter → cancellation check → plugin chain (with `HIGHPER_PLUGIN_CHUNK_BUDGET_US` budget + fail-open) → cache write buffer → metrics emit. Bounded mpsc between upstream-receive and client-send (`HIGHPER_AI_STREAM_BUFFER_DEPTH`, `HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY`). Emits `ai_stream_buffer_drops_total`, `ai_plugin_chunk_budget_exceeded_total`. **5 days** (was 4 — adds budget enforcement + bounded buffer + 2 metrics).
- [ ] **Cancellation propagation (UC16 #10, 2026-05-03 — see `USECASE_16_AI_LLM_GATEWAY.md` §3.5.1):** detect client SSE close via `select!` between `client_send.send()` and `client_close_signal`. Default `cancel_on_close=true` triggers `AiProvider::cancel(stream_id)` (TCP RST fallback if provider doesn't support cancel). Per-virtual-key override `cancel_on_close=false` switches to drain-and-record (provider keeps generating, gateway discards chunks but waits for `[DONE]` to record full usage). Either way, `ai_request_log` row written with `cancelled=true|false` flag and partial-usage totals. **3 days** (was 2 — adds drain-and-record path + per-key opt-in).
- [ ] **TPM mid-stream policy (UC16 #10, 2026-05-03 — see `USECASE_16_AI_LLM_GATEWAY.md` §3.5.2):** default post-stream warning emits `ai_budget_exceeded_total{kind=tpm_overrun}` + audit event without interrupting stream. Per-virtual-key opt-in `tpm_hard_stop=true` injects SSE error frame (`event: error\ndata: {"code":"tpm_exceeded",...}`) mid-stream and closes upstream when TPM bucket runs out. Both modes record final usage. **2 days.**
- [ ] **Streaming metrics (UC16 #10, 2026-05-03):** `ai_active_streams` gauge, `ai_streaming_cancellations_total{reason}`, `ai_ttft_seconds`, `ai_inter_token_seconds`. Wire into existing `src/observability/metrics.rs` (or via the new `MetricsBackend` trait once Phase 1.4 lands). **1 day.**
- [ ] Admin endpoints (`POST/GET /admin/ai/keys`, `GET /admin/ai/spend`, `POST /admin/ai/budgets`, `GET /admin/ai/models`). **3 days.**
- [ ] Acceptance-test suite covering all 10 criteria from UC16 doc §13 against recorded provider fixtures. **3 days.**

#### 2.6 Polish + docs (week 6)

- [ ] DSL extension for `ai_route` block (per UC16 doc §10) wired to existing pest grammar. **3 days.**
- [ ] `docs/AI_GATEWAY_GUIDE.md` (NEW) — quickstart, recipe book. **3 days.**
- [ ] Prometheus metrics from UC16 doc §9.1 emitting. **2 days.**
- [ ] Migration test: existing OpenAI client (Python `openai==1.x`) talks to gateway against Anthropic upstream — verify zero client-code changes. **2 days.**

#### Phase 2 exit criteria

- All 10 acceptance criteria from UC16 doc §13 pass in CI.
- Four providers (OpenAI, Anthropic, Bedrock, Gemini) supported.
- Owner agrees v1.1 / "AI Gateway" tag is releasable.

### Phase 3 — UC16 Beta + UC1/2/11/12/13/14/15 P1 polish + cross-cutting (6–8 weeks)

#### 3.1 UC16 Beta features

- [ ] **`VectorIndex` trait + Qdrant impl (UC16 #8, 2026-05-02 — see `USECASE_16_AI_LLM_GATEWAY.md` §6.2 + ROADMAP §4.4 row 11):** define trait at `src/gateway/ai/vector_index/mod.rs` (`search` / `upsert` / `delete` / `delete_by_tag`); ship default Qdrant impl behind `ai-vector-qdrant` Cargo feature at `src/gateway/ai/vector_index/qdrant.rs`. Same separate-trait pattern as `AiProvider` and `AiStateStore`. **4 days.**
- [ ] **Semantic cache engine (UC16 #8):** module at `src/gateway/ai/cache_semantic.rs` — embed last user message via configured `AiProvider` embedding model; ANN search via `VectorIndex` trait; threshold default 0.92 cosine (per-tenant override); per-tenant index isolation; emits `x-cache: SEMANTIC` on hit. **3 days.**
- [ ] **Additional `VectorIndex` impls (UC16 #8):** `src/gateway/ai/vector_index/{redis_stack,pgvector,hnsw}.rs` behind Cargo features `ai-vector-redis-stack` / `ai-vector-pgvector` / `ai-vector-hnsw`. **3 days total** (1 day each).
- [ ] Provider prompt-caching passthrough with byte-stable serializer + CI byte-stability test. **3 days.**
- [k] ~~Pre-call guardrails: PII regex set, OpenAI moderation, Bedrock Guardrails, Llama-Guard via configured upstream.~~ **Killed 2026-05-02 per UC16 #1 scope fence — guardrails are operator-side services wired via `src/plugin/` hooks; highper does not ship guardrail logic. See `USECASE_16_AI_LLM_GATEWAY.md` §8.**
- [k] ~~Post-call guardrails (streaming): output PII redact, JSON-schema validate, regex deny.~~ **Killed 2026-05-02 per UC16 #1 scope fence — same as above. JSON-schema validation for non-AI traffic still ships as competitor parity item N4 (Phase 4.1).**
- [ ] **Prompt registry + versioning (UC16 #4 storage):** durable layer in the configured `AiStateStore` (ReDB / RocksDB / ScyllaDB) — was "Postgres" in the original draft; updated 2026-05-02 to use the trait per UC16 #4. **5 days.**
- [ ] **Per-tenant pricing overrides (UC16 #7, 2026-05-02 — `USECASE_16_AI_LLM_GATEWAY.md` §5.5.5):** extends MVP's operator-level overrides with per-tenant rows under `ai/pricing_overrides/tenant/{tid}/{alias}`. Three-level lookup at request time: tenant → global override → snapshot. Admin endpoints `PATCH /admin/ai/tenants/{tid}/models/{alias}` etc. Audit events tagged with tenant. Useful for reseller / multi-tier pricing. **3 days.**
- [ ] MCP passthrough (`/v1/mcp/{server}`). **3 days.**
- [ ] Add providers: xAI, DeepSeek, Mistral, Groq, Together, Fireworks, Cohere, Vertex (non-Anthropic), Azure OpenAI. **7 days.**
- [ ] Embedding batch coalescing (combine N small embeds into one upstream batch within 50 ms window). **3 days.**
- [ ] Vision passthrough (URL-fetch for providers that don't auto-fetch). **3 days.**
- [d] ~~Anthropic-shape inbound (`POST /v1/messages`).~~ **Moved to Phase 2.1 MVP on 2026-05-02 per UC16 design decision #2** (OpenAI + Anthropic dual-shape MVP). Kept as a checkbox so historical references resolve.

#### 3.2 Cross-cutting + UC P1 polish (folded from Section 4.2)

- [ ] WS `permessage-deflate` (RFC 7692). **5 days.**
- [ ] WS-over-h2 (RFC 8441) Extended CONNECT. **5 days.**
- [ ] gRPC retry budget per method + circuit-break per `:authority` + `grpc-encoding` gzip/identity negotiation. **5 days.**
- [ ] **UC1:** EWMA peer scoring + outlier ejection (failure rate or latency Z-score). **4 days.** *(folded from coverage trace)*
- [ ] **UC1:** Unix-socket / abstract-socket upstream support in `BackendSelector`. **1 day.** *(folded from coverage trace)*
- [ ] **UC2:** PROXY-protocol v2 to upstream. **2 days.** *(folded from coverage trace)*
- [ ] **UC2:** Replace rolling-mean with proper EWMA + retry budget (token-bucket on retries per upstream). **3 days.** *(folded from coverage trace)*
- [ ] **UC11:** `Vary`-aware cache key. **2 days.**
- [ ] **UC11:** Singleflight (in-flight map keyed on cache key; followers await). **2 days.**
- [ ] **UC11:** RFC 7234-correct `Cache-Control` parser → directives → behaviour. **5 days.**
- [ ] **UC11:** Tag-based invalidation (PURGE by tag); admin API endpoint. **3 days.** *(folded from coverage trace)*
- [ ] **UC11:** Conditional revalidation (If-Modified-Since / If-None-Match passthrough; on origin 304 update entry timestamp). **3 days.** *(folded from coverage trace)*
- [ ] **UC11:** stale-while-revalidate (RFC 5861). **3 days.** *(folded from coverage trace)*
- [ ] **UC11:** `X-Cache: HIT|MISS` and `X-Cache-Hits` response headers; negative caching policy; max object size at response level. **2 days.** *(folded from coverage trace)*
- [ ] **UC12:** Watch-streaming — Consul long-poll (`?index=`) + etcd `Watch` stream. **5 days.** *(folded from coverage trace)*
- [ ] **UC12:** Kubernetes Endpoints/Service discovery (kube-rs). **5 days.** *(folded from coverage trace)*
- [ ] **UC12:** Tie circuit-breaker open/close to weight=0 in registry to drain gracefully. **2 days.** *(folded from coverage trace)*
- [ ] **UC13:** GraphQL subscriptions over WS (reuse `src/websocket/`). **5 days.** *(folded from coverage trace)*
- [ ] **UC13:** APQ (SHA256 ID → query map; promote on first occurrence). **3 days.** *(folded from coverage trace)*
- [ ] **UC13:** Schema hot-reload (signal or admin API call). **2 days.** *(folded from coverage trace)*
- [ ] **UC14:** X-Accel-Redirect handling on response from PHP-FPM. **2 days.** *(folded from coverage trace)*
- [ ] **UC14:** Pre-compressed file selection (`.gz` / `.br`) with `Accept-Encoding` negotiation. **2 days.** *(folded from coverage trace)*
- [ ] **UC14:** Conditional revalidation correctness (verify implementation; complete if missing). **2 days.** *(folded from coverage trace)*
- [ ] **UC14:** `fastcgi_status` / `fpm_status` passthrough. **1 day.** *(folded from coverage trace)*
- [ ] **UC15:** ASN database adapter (MaxMind ASN GeoLite). **2 days.** *(folded from coverage trace)*
- [ ] **UC15:** Country allow/block list at gateway entry (overlaps with UC9 geo-block fix). **1 day.** *(folded from coverage trace)*
- [ ] **UC15:** Region failover when nearest set is unhealthy. **2 days.** *(folded from coverage trace)*
- [ ] **UC15:** Client-IP extraction integrated with proxy-trust list (shared with rate-limit). **1 day.** *(folded from coverage trace; sister to 0.C)*
- [ ] **UC9:** WS frame parser path additions for per-frame metrics, close-code preservation. **(actually UC6, listed here for proximity to UC6 work above)**. **5 days.**
- [ ] **UC6:** Per-backend conn quota + handshake-timeout (slowloris-WS). **2 days.** *(folded from coverage trace)*
- [ ] HA decision (Raft control plane vs. stateless+Redis); implement chosen path. **2 weeks.**
- [ ] Plugin signing + improved per-plugin isolation. **5 days.**
- [ ] **ConnectionPool<C> unification (added 2026-05-02):** unify the three parallel impls in `src/proxy/connection_pool.rs`, `src/tcp/pool.rs`, `src/proxy/database_pool.rs` behind a single generic `ConnectionPool<C: Connectable>` trait + impl; eliminate ~80 % code duplication; pool sizes / timeouts / idle TTLs all from `HIGHPER_POOL_*` env vars. Risk medium-high — defer to Phase 3 after v1.0. **2.5 weeks.** *(folded from interface-first audit §4.4)*
- [ ] **N1 stick tables (added 2026-05-02 — competitor parity HAProxy):** cross-instance session-affinity table backed by Redis or in-memory primary; configured via `HIGHPER_STICK_TABLE_BACKEND`. **1 week.**
- [ ] **N6 GraphQL safelist / persisted-only mode (added 2026-05-02 — competitor parity Apollo Router):** when `HIGHPER_GRAPHQL_SAFELIST_ONLY=1`, reject any query whose SHA256 is not in the configured safelist. **3 days.**
- [ ] **N9 Nginx try_files DSL primitive (added 2026-05-02 — competitor parity Nginx):** add `try_files` block to DSL grammar, lowering to `src/webserver/static_files.rs` index lookup behavior. **3 days.**
- [ ] **N12 range-request slicing (added 2026-05-02 — competitor parity Nginx slice / Varnish):** UC11 cache stores objects in fixed-size slices (size from `HIGHPER_CACHE_SLICE_BYTES`); range requests served from cached slices with single-flight upstream backfill for missing slices. **5 days.**

### Phase 2.5 — UC16 cross-request infrastructure (NEW — added 2026-05-02)

Inserted between Phase 2 MVP and Phase 3 Beta to land one P1 cross-cutting
UC16 item that doesn't fit cleanly inside the MVP six weeks.

- [ ] **N23 Helicone-style session/trace object (added 2026-05-02 — competitor parity Helicone):** introduce a `Session` concept that links N AI requests under one logical conversation (header `X-Hpgw-Session-Id` or virtual-key default); session events feed metrics + trace exports. Session retention bounded by `HIGHPER_AI_SESSION_TTL_SECS`. Storage backend uses the same `AiStateStore` trait introduced in Phase 2 (see §5 owner gate). **5 days.**

### Phase 4 — UC16 GA + ecosystem + remaining P1/P2 (10–14 weeks)

#### 4.1 UC16 GA + UC4 + UC9 + UC10 polish

- [ ] Eval framework integration (Promptfoo subprocess runner). **5 days.**
- [ ] Full audit log + hash-chain integrity + signed JSONL export. **5 days.**
- [ ] BYOK per tenant + tenant CMEK with KMS / Vault Transit. **2 weeks.**
- [ ] Realtime / WS (OpenAI Realtime + Gemini Live) reusing `src/websocket/`. **2 weeks.**
- [ ] Fine-tuning passthrough + Files API with S3-compatible storage. **2 weeks.**
- [ ] Cost-optimized routing (capability-aware cheapest-meeting-SLA). **5 days.**
- [ ] Async response with callback URL. **5 days.**
- [ ] Idempotency keys. **3 days.**
- [ ] A/B prompt experiments with outcome recording. **5 days.**
- [ ] Remaining providers: Replicate, HF Inference, CF Workers AI, Perplexity, Ollama, vLLM, TGI. **5 days.**
- [ ] Rate-limit-aware LB parsing `x-ratelimit-*` per provider. **3 days.**
- [ ] WASM plugin AI host functions (`ai_get_messages`, `ai_set_messages`, `ai_block`, `ai_record_score`). **5 days.**
- [ ] Hard-stop TPM enforcement option (inject SSE error, close upstream). **3 days.**
- [ ] **UC4:** GCRA / leaky-bucket implementation. **2 days.** *(folded from coverage trace)*
- [ ] **UC4:** LRU eviction on bucket map (max-N). **0.5 day.** *(folded from coverage trace)*
- [ ] **UC9:** CRS version pinning + deployment. **2 days.** *(folded from coverage trace)*
- [ ] **UC9:** False-positive whitelist per rule per endpoint. **3 days.** *(folded from coverage trace)*
- [ ] **UC10:** Common LB pool shared L4↔L7. **2 days.** *(folded from coverage trace)*
- [ ] **UC10:** Shared rate-limit `Arc` between L4 and L7 paths. **2 days.** *(folded from coverage trace)*
- [ ] OTel exemplars + SLO/SLI definitions + RED/USE built-in dashboards refresh. **5 days.** *(folded from coverage trace)*
- [ ] Admin RBAC beyond API-key. **3 days.** *(folded from coverage trace)*
- [ ] **N2 KrakenD-style response aggregation (added 2026-05-02 — competitor parity KrakenD):** declarative aggregation block (call N upstreams, merge JSON by JSONPath); extends existing `examples/api-aggregation.yaml`. **1 week.**
- [ ] **N4 JSON-schema request validation (added 2026-05-02 — competitor parity Kong, Envoy ext_proc):** middleware validates request body against per-route JSON schema, rejects with 400 + machine-readable error pointer. **5 days.**
- [ ] **N24 OpenRouter-style daily price-discovery feed (added 2026-05-02 — competitor parity OpenRouter):** scheduled task fetches a configured price feed (`HIGHPER_AI_PRICE_FEED_URL`) and updates the model registry; cost calculator picks up new prices on next request. Out of scope: highper does NOT host its own price feed, only consumes a configured one. **3 days.**

#### 4.2 Ecosystem (UC1–UC15 → enterprise scope)

- [ ] xDS client (CDS/EDS at minimum, then RDS/LDS/SDS) — Envoy-style dynamic config. **3 weeks.**
- [ ] Kubernetes operator + Ingress controller + CRDs + Helm chart. **3 weeks.**
- [ ] kTLS sendfile complete syscall path. **5 days.**
- [ ] Post-quantum hybrid TLS (X25519+MLKEM) once `rustls` stabilizes. **conditional, ~5 days when available.**
- [ ] ECH (Encrypted Client Hello). **conditional on rustls.**
- [ ] Federated GraphQL (Apollo Federation v2 entity resolution). **3 weeks.** Design + acceptance criteria captured in `docs/planning/GRAPHQL_FEDERATION.md`; that doc's open questions (library vs in-house, schema-registry storage, query-plan cache key, entity batching, subscriptions × federation, observability shape, Apollo-Router compatibility) **must be answered before this item starts.**
- [ ] Redis Cluster slot routing + Sentinel master discovery. **2 weeks.**
- [ ] PgBouncer-style transaction-pooling mode for Postgres. **2 weeks.**
- [ ] JA3/JA4 fingerprinting + WAF integration. **5 days.**
- [ ] SIEM-format (CEF/LEEF) WAF log export. **3 days.**
- [ ] CT-log monitoring for issued certs. **5 days.**
- [ ] GitOps controller (declarative-diff/apply against Git). **2 weeks.**
- [ ] **UC1:** Connection draining on reload. **2 days.** *(folded from coverage trace)*
- [ ] **UC1:** Replace `copy_bidirectional` with `splice(2)` on Linux for L4 paths. **3 days.** *(folded from coverage trace)*
- [ ] **UC1:** Wire real latency percentiles (HDR histogram). **1 day.** *(folded from coverage trace)*
- [ ] **UC2:** Request mirroring (% sample to shadow upstream, response discarded). **3 days.** *(folded from coverage trace)*
- [ ] **UC2:** Unix-socket upstream support in hyper client. **1 day.** *(folded from coverage trace)*
- [ ] **UC3:** ACME DNS-01 with Cloudflare and Route53 providers. **5 days.** *(folded from coverage trace)*
- [ ] **UC3:** ACME EAB (External Account Binding). **2 days.** *(folded from coverage trace)*
- [ ] **UC3:** Session-ticket-key rotation strategy exposed in config. **2 days.** *(folded from coverage trace)*
- [ ] **UC3:** 0-RTT (subject to rustls support). **conditional.** *(folded from coverage trace)*
- [ ] **UC5:** GSO sendmmsg path (Linux). **5 days.** *(folded from coverage trace)*
- [ ] **UC5:** GREASE in transport params. **1 day.** *(folded from coverage trace)*
- [ ] **UC5:** ECN/DSCP marking. **2 days.** *(folded from coverage trace)*
- [ ] **UC5:** HTTP/3 datagrams (RFC 9221) for WebTransport / MASQUE. **2 weeks.** *(folded from coverage trace)*
- [ ] **UC6:** WS-over-HTTP/3 (RFC 9220). **5 days.** *(folded from coverage trace)*
- [ ] **UC6:** `Sec-WebSocket-Extensions` negotiation. **2 days.** *(folded from coverage trace)*
- [ ] **UC7:** gRPC-Web binary + text framing translation. **5 days.** *(folded from coverage trace)*
- [ ] **UC7:** Server reflection v1 passthrough endpoint. **3 days.** *(folded from coverage trace)*
- [ ] **UC7:** `grpc-status-details-bin` (binary error details). **2 days.** *(folded from coverage trace)*
- [ ] **UC8:** Read/write split (requires query-aware L7 layer). **2 weeks.** *(folded from coverage trace)*
- [ ] **UC8:** Prepared-statement awareness. **5 days.** *(folded from coverage trace)*
- [ ] **UC8:** COPY-protocol awareness for Postgres. **3 days.** *(folded from coverage trace)*
- [ ] **UC8:** Sharding / hash-slot routing. **2 weeks.** *(folded from coverage trace)*
- [ ] **UC8:** Query mirroring + statement audit log. **3 days.** *(folded from coverage trace)*
- [ ] **Cross:** WASI Preview 2 / component model migration. **4 weeks.** *(folded from coverage trace)*
- [ ] **Cross:** Plugin marketplace concept (signed registry). **2 weeks.** *(folded from coverage trace)*
- [ ] **Cross:** Config template expansion (Jinja2/Helm-style). **5 days.** *(folded from coverage trace)*
- [ ] **Cross:** Config profile inheritance / environment-specific. **5 days.** *(folded from coverage trace)*
- [ ] **Cross:** 80-TODO codebase sweep per `docs/TODO_COMPREHENSIVE.md:305-313`. **5 days.** *(folded from coverage trace)*
- [ ] **ConfigSource trait extraction (added 2026-05-02):** define `ConfigSource` trait (load / watch / hot-apply); migrate YAML/JSON/TOML/DSL loaders in `src/config/loader.rs` into trait impls; opens the door for GitOps-pull and xDS-push (N25 / 4.2 enterprise scope) as additional impls. Default source via `HIGHPER_CONFIG_SOURCE` env var. **1 week.** *(folded from interface-first audit §4.4; lowest priority)*

#### 4.2.N — Competitor net-add features placed in Phase 4.2 (added 2026-05-02)

- [ ] **N3 OpenAPI 3.1 → routes:** declarative routing generator that ingests an OpenAPI 3.1 document and produces a route table; consumed via `HIGHPER_OPENAPI_PATH` or admin-API upload. **2 weeks.**
- [ ] **N5 gRPC-JSON transcoding:** Envoy-style `google.api.http` annotation honoring; JSON in / gRPC out and vice-versa for non-streaming methods. **2 weeks.**
- [ ] **N8 GraphQL query-plan cache:** cache parsed + planned query AST keyed on persisted-query SHA; admin-API purge endpoint. **5 days.**
- [ ] **N10 SCGI / uWSGI clients:** FastCGI sibling protocols; reuses the `src/webserver/php_fpm.rs` framing patterns. **5 days.**
- [ ] **N11 Varnish-style ESI:** Edge-Side Includes — parse `<esi:include>` in cached responses and substitute. Honors `HIGHPER_CACHE_ESI_MAX_DEPTH`. **2 weeks.**
- [ ] **N13 Caddy-style auto-HTTPS UX:** when `HIGHPER_AUTO_HTTPS=1` and a domain has no TLS config, attempt ACME on first request that arrives for that hostname. Hardened against ACME abuse with `HIGHPER_AUTO_HTTPS_RPS_LIMIT`. **5 days.**
- [ ] **N14 WebTransport (RFC drafts):** over HTTP/3 datagrams (RFC 9221) + CONNECT; reuses `src/websocket/` shutdown coordinator. **2 weeks.**
- [ ] **N15 MASQUE:** UDP-over-HTTP/3 tunneling; out-of-scope for v1.0, in scope for v2 enterprise. **2 weeks.**
- [ ] **N16 Per-message WS hook:** plugin hook fired on each WS frame; reuses WASM plugin sandbox; `HIGHPER_WS_HOOK_BUDGET_US` enforces budget. **5 days.**
- [ ] **N17 Listener-filter-chain auto-detect:** Envoy-style listener that peeks first bytes and routes to `tls` / `http` / `proxy-protocol` chain; reduces config burden for hybrid deployments. Auto-detect window from `HIGHPER_LISTENER_DETECT_BYTES`. **5 days.**
- [ ] **N18 Bot-management bundle:** rate-by-reputation (IP / ASN / fingerprint), JA3/JA4 score, integrates with WAF Decision; `HIGHPER_BOT_MGMT=1` toggle. **2 weeks.**
- [ ] **N19 ProxySQL-style query rewrite:** rule-based query rewriting / hash-based read/write split (extends UC8 read/write split entry). **5 days.**
- [ ] **N20 HashiCorp Nomad service discovery:** new impl of the existing `ServiceDiscovery` trait (`src/discovery/mod.rs:134-153`); chosen via `HIGHPER_DISCOVERY_TYPE=nomad`. **5 days.**
- [ ] **N21 AWS CloudMap / Azure Service Fabric discovery:** further `ServiceDiscovery` impls; chosen via `HIGHPER_DISCOVERY_TYPE=cloudmap|azure_sf`. **1 week.**
- [ ] **N22 EDNS Client Subnet (ECS):** geo-LB extension that honors ECS option in upstream DNS resolution for client-region hints. **3 days.**
- [ ] **N25 xDS-driven UC16 routes:** the existing xDS client (above in 4.2 ecosystem list) gains an "AI provider list" resource type so multi-tenant control planes can push provider/model definitions without editing config. **5 days.** *(piggybacks on the xDS work above)*

---

## 6. Owner gates / open decisions

1. **Now (before Phase 0 start):** confirm Phase 0 priority order — agree all 14 blockers (B1–B14) are in scope, or strike specific items with rationale. Confirm Rancher Desktop choice and Phase 1.5 tool stack (Trivy + syft+Grype + Dastardly + ZAP).
2. **End of Phase 1:** does v1.0 launch with UC13 (GraphQL) marked "passthrough only, federation deferred"? Or block on shipping real federation in Phase 1? Recommendation: defer to v1.1.
3. **Before Phase 2 start (UC16 scope gate):** confirm the §3.1 in-scope/out-of-scope fence; revise `docs/planning/USECASE_16_AI_LLM_GATEWAY.md` to remove out-of-scope items (guardrails, AI observability product, in-memory cache product, vLLM); answer the remaining design questions that survive the scope fence. **Phase 2 cannot begin without this.**
4. **End of Phase 3 (HA architecture gate, revised 2026-05-02):** decision recorded — the four-cluster-type model is the design (Type 1 Stateless / Type 2 +Valkey / Type 3 +etcd / Type 4 +Valkey+etcd); see [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md). Three open sub-decisions remain: **(i)** which Type B backend the cookbooks default to and which CI exercises (Valkey vs Redis — same protocol, mostly cookbook + CI choice); **(ii)** which Type C backend ships first as a code path (etcd already coded, Consul already coded, raft-rs not yet — recommendation: etcd default, raft-rs Phase 4.2); **(iii)** whether highper drives peer discovery for clustering or delegates to the chosen infrastructure (K8s headless service / Consul / etc.). Affects Phase 4 enterprise tier feasibility and the UC16 storage gate (#5 below). Per the HA research's "most-restrictive HA logic wins" rule, a deployment that enables UC3 or UC12 must include the etcd layer — that's a runtime validation enforced by §11.2, not a decision.
5. **DECIDED 2026-05-02 (UC16 design decision #4):** UC16 storage-backend gate resolved. **Three impls of the `AiStateStore` trait ship**: **ReDB** (pure-Rust embedded, single-node default — zero external deps, matches highper's tooling stack); **RocksDB** (mature embedded, single-node alternative); **ScyllaDB** (Cassandra-compatible, multi-node). Operator chooses per deployment via `HIGHPER_AI_STATE_BACKEND` env var. Single-node deployments accept 0% storage-layer fault tolerance (same semantics as `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE`). Multi-node prod uses ScyllaDB which provides its own replication. PostgreSQL is no longer a candidate (heavier than ReDB, not horizontally scalable like ScyllaDB; can be reconsidered as a 4th impl in Phase 4 if operator demand surfaces). New follow-on question (UC16 §12 #16): single-node → multi-node migration path — export tool, dual-write, or fresh-start. Recommended: export tool in Phase 3.
6. **Before any v1.0 GA tag (added 2026-05-02):** confirm §0.5 reconciliation banners are still consistent with then-current code; KNOWN_LIMITATIONS / README / CHANGELOG / ARCHITECTURE may need refresh again at tag time.
7. **Multi-region commitment (added 2026-05-02 — `HA_ARCHITECTURE.md` §6.5):** decide *when* multi-region ships as a single-button deployment (recommended: not in v1.0; Phase 4.x ecosystem alongside xDS / K8s operator). Until then, single-region-multi-AZ is the v1.0 default and multi-region patterns are documented but not productized. Decision affects UC15 footprint expectations and UC16 cross-region budget enforcement (gate #5 follow-on).

---

## 7. Risks & dependencies

| Risk | Phase | Mitigation |
|---|---|---|
| Phase 0 reveals a deeper architectural issue (e.g., runtime model can't be retrofitted for true UC10) | 0 | Dedicate a 2-week spike before continuing; document outcome in `ARCHITECTURE_v2.md`. |
| 7-day soak surfaces memory leak | 1 | Phase 1 must not be skipped; budget 1 week for diagnosis; don't ship v1.0 until green. |
| OCSP library choice creates a new dep with thin maintenance | 0 | Vet `webpki-ocsp` or vendor a small wrapper; ASN.1 is the boundary, not a moving target. |
| Rancher Desktop dev-env quirks differ from cloud target | 0 prereq + 1.1 | Always validate on cloud VM in addition to Rancher Desktop; document divergences. |
| Dastardly/ZAP false positives slow CI | 1.5 | Document waiver process up front; tune over first 3 runs before failing CI hard. |
| UC16 owner picks "Anthropic-shape inbound at MVP" — doubles scope | 2 | Fence it: keep MVP at OpenAI-shape only; Anthropic in Beta. |
| Provider API change (Anthropic adds new content-block type) breaks translator | 2+ | CI tests against recorded provider fixtures + `unknown_field` capture so unknown bits are forwarded verbatim. |
| HA decision deferred forever | 3 | Make decision in Phase 2 week 6 latest; document irreversibility. |

---

## 8. Sub-agents (planned)

The owner has indicated openness to sub-agents in `.claude/agents/`. The following are proposed; drafts to be written and committed by the owner after review.

| Agent | Scope | When to invoke |
|---|---|---|
| `release-blocker` | Phase 0 single-workstream fixes (B1–B14) | per workstream PR |
| `validation-runner` | Phase 1.1 cloud validation matrix + Phase 1.2 soak monitoring | Phase 1, then on schedule |
| `ai-translator` | Per-provider shape translator (UC16 §3.2) | Phase 2 weeks 2–3 |
| `security-reviewer` | PR-time review of TLS / WAF / auth / TLS-Marvin / drain changes; SBOM/DAST/ZAP triage | every relevant PR |
| `docs-keeper` | Keep `docs/*` consistent with code (incl. ROADMAP checkboxes, §0.5 reconciliation banners, `KNOWN_LIMITATIONS.md` blocker list) | weekly + post-merge hook |
| `gap-auditor` | Re-run a coverage trace like 2026-05-02's; re-verify B1–B14 path:LINE citations against current code; flag drift | monthly |

**Note on removal (2026-05-02):** the previously-listed `ai-guardrail-builder`
agent was removed because the UC16 scope fence in §3.1 explicitly excludes
guardrails (input/output filtering, PII redaction, jailbreak detection).
Those concerns live in customer-side services, not in highper-gateway.

---

## 9. Tracking conventions

- **Per item:** when you start, change `[ ]` → `[~]`. When merged, change `[~]` → `[x]` and append `(PR #NNN)`. If deferred, `[d]` with one-line reason. If killed, `[k]` with one-line reason.
- **Per phase:** when all checkboxes resolve, mark the phase header complete with the date and tag.
- **New TODOs added later:** append to the appropriate phase. If a new finding is a release blocker, add to Phase 0 (and to Section 4.1) and surface in the next standup-equivalent.
- **PR linkage:** every closed item must link to the merging PR.
- **Audit refresh:** the `gap-auditor` agent (Section 8) should re-run monthly to catch drift.

---

## 11. HA architecture — pointer + cluster-bootstrap config shape (revised 2026-05-02)

**Authoritative reference:** [`docs/planning/HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md).
That document contains the full cluster-type definitions, per-UC profiles
(group letter, supported types, LB algorithms, footprint, scaling pattern),
single-node→multi-node conversion paths, front-LB patterns (VIP / Anycast /
cloud-LB / K8s ingress / kernel-UDP-steering), per-infrastructure deployment
notes, and the all-in-one mapping table.

This section keeps only the items that drive **roadmap tracking**:

1. The four cluster types as one-line summaries (§11.1).
2. The cluster-bootstrap env-var configuration shape — Phase 0.J deliverable (§11.2).
3. Open owner-side decisions still tracked here (§11.3).

For everything else, follow the link.

### 11.1 Four cluster types — one-liners

| Type | Components | UC groups served |
|---|---|---|
| **Type 1 — Stateless** | N highper-gateway replicas | Group A only (7 UCs) |
| **Type 2 — Stateless + Valkey** | N highper + Valkey/Redis cluster | Group A + Group B (14 UCs) |
| **Type 3 — Stateless + etcd** | N highper + etcd / Consul / Raft cluster | Group A + Group C (9 UCs) |
| **Type 4 — Stateless + Valkey + etcd** | All three components | All 16 UCs |

UC group letters and the per-UC mapping live in `HA_ARCHITECTURE.md` §3 (cited
from research lines 11–27). The cluster type follows mechanically from the union
of group letters in the operator's enabled-UC selection.

### 11.2 Cluster-bootstrap configuration shape (revised 2026-05-02)

Operators declare their deployment posture at startup via env vars (§0.1 rule
applies — no hardcoding). The design is **two independent on/off flags** for
the coordination layers (Type B and Type C), not a single "cluster type" enum
— this matches the four-type model in §11.1 and avoids the misleading "C is a
superset of B" framing that the earlier enum design implied.

| Knob | Env var | Values | Default |
|---|---|---|---|
| Infrastructure | `HIGHPER_CLUSTER_INFRA` | `k8s` / `vm` / `baremetal` / `single` | `single` (no peer assumed) |
| Type B backend | `HIGHPER_CLUSTER_TYPEB_BACKEND` | `valkey` / `redis` / `none` | `none` |
| Type B addresses | `HIGHPER_CLUSTER_TYPEB_ADDRS` | comma-separated `host:port` | unset |
| Type C backend | `HIGHPER_CLUSTER_TYPEC_BACKEND` | `etcd` / `consul` / `raft` / `none` | `none` |
| Type C addresses | `HIGHPER_CLUSTER_TYPEC_ADDRS` | comma-separated `host:port` | unset |
| Peer discovery mode | `HIGHPER_CLUSTER_PEER_DISCOVERY` | `static` / `k8s_headless` / `consul` / `dns` / `none` | `none` |
| Peer list (when static) | `HIGHPER_CLUSTER_PEERS` | comma-separated `host:port` | unset |
| Single-node "0 % FT" override | `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE` | `true` / `false` | `false` |

The effective cluster type follows from the two backend flags: both `none` →
Type 1; only B set → Type 2; only C set → Type 3; both set → Type 4. The
operator does not explicitly declare "type"; it is implied.

**Validation at startup (refuses to boot on failure).** Walks enabled UCs
and computes the union of required group letters. Then asserts:

1. If any enabled UC is **Group B** (UC4, UC6, UC7, UC10 mixed-A+B, UC11,
   UC13, UC16), then `HIGHPER_CLUSTER_TYPEB_BACKEND` must be set (not `none`)
   and `HIGHPER_CLUSTER_TYPEB_ADDRS` must be non-empty — unless
   `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true` is set with the operator
   accepting "0 % fault tolerance on Valkey/Redis" per the research file
   line 29 footnote.
2. If any enabled UC is **Group C** (UC3, UC12), then
   `HIGHPER_CLUSTER_TYPEC_BACKEND` must be set (not `none`) and
   `HIGHPER_CLUSTER_TYPEC_ADDRS` must be non-empty — same single-node opt-in
   exception applies.
3. If both flags are `none` and any Group B *or* Group C UC is enabled,
   `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true` must be explicit (no silent
   fallback to a no-coordination mode that would corrupt rate limits / cert
   ordering).
4. Infrastructure-specific sanity: `HIGHPER_CLUSTER_PEER_DISCOVERY=k8s_headless`
   requires `HIGHPER_CLUSTER_INFRA=k8s`. `consul` requires Consul addrs to
   be reachable. `static` requires a non-empty `HIGHPER_CLUSTER_PEERS` list.
5. Group B and Group C are **independent** — a Type 4 deployment must
   satisfy both rule 1 and rule 2 (i.e., both stores must be configured).
   The earlier "C is a superset of B" claim was incorrect; replaced here.

**Implementation site (Phase 0.J — already in workstream):** the central
`Settings` struct gains a `Cluster` sub-struct holding these fields; the
loader runs the validation above before any listener starts. Code consumers
(rate-limit distributed mode, cache distributed tier, ACME store, service
discovery) read `Settings::cluster` rather than calling `std::env::var`.

### 11.3 Open decisions still tracked here

These remain open and are escalated to §6 owner gate #4. Detail and
recommendations are in `HA_ARCHITECTURE.md` §9.

- **Type B backend default** — Valkey vs Redis; mostly a cookbook + CI choice.
- **Type C backend default** — etcd vs Consul vs `raft-rs`-embedded.
- **Peer-discovery responsibility** — highper drives via a `PeerDiscovery`
  trait, or strict delegate-to-infra.
- **UC16 storage backend** (durable layer for virtual keys / budgets / usage /
  sessions) — open at §6 gate #5; hot-path counters lean Valkey per the
  research line 27 footprint.

### 11.4 Superseded sub-sections (moved to HA_ARCHITECTURE.md)

The earlier §11.1 (Three personas), §11.2 (Per-UC mapping), §11.3 (Decision
hierarchy), §11.4 (Implications), §11.5 (Deployment matrix — 3 personas × 3
infrastructures), §11.6 (Cluster-bootstrap configuration shape — earlier
single-enum design), §11.7 (Decisions recorded + still open) were moved to
`docs/planning/HA_ARCHITECTURE.md` on 2026-05-02 as part of the four-cluster-type
restructuring. They contained:

- The three-persona definitions cited from research lines 4–6 → now in
  `HA_ARCHITECTURE.md` §3 (under the four-type model — Type A merged into
  "Stateless"; Type B and C kept as coordination layers).
- The per-UC research mapping table (research lines 10–27) → now in
  `HA_ARCHITECTURE.md` §3.
- The decision hierarchy ("most-restrictive HA logic wins" + Valkey-shard
  isolation, research lines 36–40) → now in `HA_ARCHITECTURE.md` §2.4 and §8.
- The 3 × 3 deployment matrix → replaced by the four-cluster-type model
  in `HA_ARCHITECTURE.md` §1 + §2.x; the per-infrastructure breakdowns are
  in `HA_ARCHITECTURE.md` §7.
- The earlier single-enum `HIGHPER_CLUSTER_TYPE` configuration shape →
  replaced by the two-flag design in §11.2 above.
- The "decisions recorded" framing → replaced by §11.3 above (open
  decisions only) and `HA_ARCHITECTURE.md` §9.

The 9 validation errors flagged by the 2026-05-02 cross-check are all
addressed in `HA_ARCHITECTURE.md`'s rewrite plus the §11.2 two-flag design
above. See §12 lifecycle entry "fifth revision" for the full list.

---

## 12. Document lifecycle

- **2026-05-02 (initial):** Created. Folded `docs/PHASED_RELEASE_PLAN.md` (now archived at `docs/archive/planning/PHASED_RELEASE_PLAN.md`) plus the coverage-trace findings plus new Phase 0 prereq + Phase 1.5 + Phase 1.6.
- **2026-05-02 (revision):** Three-pass validation update.
  - Pass 1: added §0.1 env-var-only configuration rule, §0.5 reconciliation log
    (legacy doc fix-ups), B11–B14 release blockers (with verified path:LINE
    citations), §4.1.1 detailed env-var plan for B11–B14, new workstreams
    0.J (env-driven Settings scaffold) and additions to 0.D (B14 drain), 0.F
    (B11 backpressure), 1.5 (B13 RSA), 1.6 (B12 body-size centralization).
  - Pass 2: added §4.4 interface-first audit (8 weak boundaries with refactor
    effort + path:LINE); folded trait extractions into 0.A (LoadBalancerStrategy),
    0.C (RateLimiter), 0.D (CircuitBreaker), 0.I (GeoProvider), 1.4
    (AuthProvider + Metrics/LogBackend), 3.2 (ConnectionPool unify), 4.2
    (ConfigSource); added `AiProvider` trait acceptance criterion to Phase 2.
  - Pass 3: added §4.5 cookbook coverage matrix (per-UC inventory of `examples/`
    and `deploy/`); added §4.6 N1–N25 competitor net-add features table;
    folded N1/N6/N9/N12 into 3.2; created Phase 2.5 with N23 (Helicone-style
    sessions); added N2/N4/N24 to 4.1; added N3/N5/N8/N10/N11/N13/N14/N15/N16/
    N17/N18/N19/N20/N21/N22/N25 to 4.2 sub-section. Rewrote §3 to fence UC16
    scope per owner statement (LiteLLM/Portkey *gateway role only*; explicit
    out-of-scope items: guardrails, vLLM, AI observability product, in-memory
    cache product). Added §6 gate #5 (UC16 storage backend — PostgreSQL vs
    alternative — open) and #6 (re-confirm §0.5 banners at tag time).
- **2026-05-02 (third revision):**
  - Inserted §11 HA architecture — three cluster personas (Type A / B / C),
    per-UC mapping, decision hierarchy ("most-restrictive wins" + Valkey-shard
    isolation), implications for the roadmap. Source:
    `docs/planning/research-on-HA-architecture-highper-gateway-16-deployment-usecases.txt`.
  - Renumbered Document lifecycle from §10 → §12 to preserve order after §11
    insertion. Updated all in-document references.
  - Rewrote §6 owner gate #4 from binary "Raft vs Redis" to the three-persona
    decision (default persona + Type B backend + Type C backend).
  - Created `docs/planning/GRAPHQL_FEDERATION.md` capturing the v1.0
    deferral of Apollo Federation v2 entity resolution (UC13). Linked from
    §3.4 (UC13 status), §5 Phase 0.E, §5 Phase 4.2.
  - Pruned §8 sub-agents: removed `ai-guardrail-builder` (UC16 §3.1 fence
    excludes guardrails).
  - Added §13 Current status snapshot — what's done in 2026-05-02
    documentation work, what's pending (Phase 0–4 work hasn't begun;
    everything to date is documentation).
- **2026-05-02 (fourth revision):**
  - Replaced "default persona" framing with operator-chooses-per-deployment
    posture per owner direction. Highper-gateway facilitates every
    (cluster type × infrastructure) combination through configuration shape +
    cookbook templates.
  - Added §11.5 deployment matrix — 3 personas × 3 infrastructures with
    HA technique per cell, supported UCs, and infrastructure preferences
    cited from §11.2 footprint column.
  - Added §11.6 cluster-bootstrap configuration shape — 9 `HIGHPER_CLUSTER_*`
    env vars (type, infra, Type B / C backends and addrs, peer discovery,
    single-node opt-in) honoring §0.1 env-var-only rule. 5 startup
    validations (most-restrictive-wins refusal-to-boot when contradicted).
  - Renumbered §11.6 / §11.7 to preserve flow: §11.5 (matrix) → §11.6
    (config shape) → §11.7 (decisions recorded + still open).
  - Rewrote §6 owner gate #4 — removed default-persona question; three
    open sub-decisions remain (Type B backend default, Type C backend
    default, peer-discovery responsibility).
  - Added Phase 1.3.1 cluster-deployment templates — 9 cookbook directories
    queued under `examples/configs/clusters/` (one per cell) + top-level
    decision-flow README + per-cell CI validation harness. Total ~25 days
    of new Phase 1 work.
  - Extended §4.5 cookbook coverage with cluster-deployment dimension.
  - §13 status snapshot: added rows for HA decision, config shape, Phase 1.3.1
    queue, scheduled routines, gitignore update, and the 2-commit baseline.
  - Routines scheduled outside the doc: `gap-auditor` monthly,
    `docs-keeper` weekly.
- **2026-05-02 (fifth revision):**
  - Created [`docs/planning/HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) as the
    authoritative HA reference. Adopts the four-cluster-type model
    (Type 1 Stateless / Type 2 +Valkey / Type 3 +etcd / Type 4 +Valkey+etcd)
    derived directly from research lines 11–27 grouped by coordination need.
  - HA_ARCHITECTURE.md sections: overview / per-type details / all-in-one
    UC×type mapping / per-UC profiles with LB algorithms (grep-verified) /
    single-node→multi-node conversion paths / front-LB patterns
    (VIP / Anycast / cloud-LB / K8s ingress / kernel UDP steering) /
    per-infra deployment notes / decision flow / open decisions.
  - LB algorithm citations grep-verified: TCP `src/tcp/mod.rs:203-224` (7
    algorithms), HTTP `src/config/schema.rs:450-460` + dispatch at
    `src/proxy/loadbalancer.rs:366-392` (9 algorithms), gRPC
    `src/grpc/mod.rs:114-125` (5 algorithms), DSL exposure
    `src/config/dsl_ast.rs:402-410` (7 algorithms).
  - Slimmed ROADMAP §11 from ~250 lines to ~110 lines: §11.1 four
    cluster-type one-liners, §11.2 cluster-bootstrap env-var config shape
    (Phase 0.J tracking — kept inline because it ships as code), §11.3
    open decisions, §11.4 superseded-sub-sections record.
  - Replaced the earlier single-enum `HIGHPER_CLUSTER_TYPE` design with
    a two-flag design (`HIGHPER_CLUSTER_TYPEB_BACKEND` +
    `HIGHPER_CLUSTER_TYPEC_BACKEND`) — cleaner, models the four cluster
    types as the on/off combinations of the two coordination-layer flags.
  - Fixed all 9 validation errors from the 2026-05-02 cross-check report
    (operator-chooses-per-deployment / "C-superset-of-B" framing /
    StatefulSet-for-highper / env-var-name inconsistency / UC10 missing
    from Type B trigger / UC11 in Type A row / etc.) — they're either
    addressed in the new HA_ARCHITECTURE.md or made moot by the
    two-flag design.
  - Updated §6 gate #4 to point at HA_ARCHITECTURE.md; updated §13
    status snapshot rows accordingly. External §11.x cross-references
    in §4.5 and Phase 1.3.1 redirected to HA_ARCHITECTURE.md sections.
- **2026-05-02 (sixth revision):** 360° architectural review of HA
  surfaced 12 concerns (F1–F12) with severity grading. P0 + P1 fixes applied:
  - HA `§1.5` per-type performance envelope (F1 Valkey hot-key, F2 etcd
    write ceiling, F7 RPS targets), `§3.5` UC16 storage as 5th component
    (F6), `§6.5` multi-region (F3), `§7.4` cluster security baseline (F10).
  - HA `§4.1` per-replica vs cluster-consistent LB algorithms (F5),
    `§5.1` cluster-aware config reload (F8), `§7.5` per-cloud-provider
    front-LB matrix (F9), `§6` failover-timing column (F4).
  - HA `§4.1` DSL/schema-mismatch footnote (F11), `§7.1` kTLS / K8s
    privilege caveat (F12).
  - ROADMAP §6 owner gate #7 added (multi-region commitment timing).
  - ROADMAP Phase 1.2 gains a per-type RPS benchmark task that replaces
    the placeholder `(unsourced inference)` numbers in HA `§1.5`.
  - §13 status snapshot extended with 6 new rows for the review fold-in.
- **2026-05-02 (seventh revision):** post-review HA consistency pass
  applying R1 + R2 + R5 + R8 (in HA_ARCHITECTURE.md) plus R3 + R4 + R6 + R7
  (here in ROADMAP) per the review report. Net: 4 new ROADMAP tasks land —
  Phase 0.J cluster-security env vars (R2, 1.5 days), Phase 0.C
  hot-key-sharding cookbook (R6, 2 days), Phase 1.3 SECURITY_CLUSTER_BASELINE
  doc (R3, 3 days), Phase 1.3.1 cloud-VM sub-cells (R4, 2 days), Phase 1.6
  DSL Maglev + Geographic exposure (R7, 1 day). HA_ARCHITECTURE.md gets
  three small clarifications (stale §11.7 ref → §11.3; "Full HA at N=1" →
  "No HA at N=1"; §7.4 hardening overstatement softened). No new sections
  added in either doc — this is purely a consistency-fix pass.
- **2026-05-02 (eighth revision):** UC16 design decisions #1–#4
  resolved and folded into `USECASE_16_AI_LLM_GATEWAY.md` (now 588+ lines
  after pruning + new §3.3 + storage updates).
  - **#1 Scope-fence pruning:** §8 Guardrails section deleted (replaced
    with external-integration note); guardrail module entries removed from
    §3.2; `guardrails {}` DSL block replaced with `plugin {…}` references;
    semantic-cache embedding-model clarified as operator's choice; AI
    observability platforms confirmed external.
  - **#2 OpenAI + Anthropic dual-shape MVP:** Anthropic-shape inbound
    moved from Beta to MVP per owner direction (Anthropic API key already
    held). Phase 2.1 inbound-parse task grew from 2→3 days.
  - **#3 AiProvider plugin architecture:** new §3.3 in UC16 doc documenting
    separate trait + existing FFI / WASM loader infrastructure reuse.
    Phase 2.1 AiProvider task updated to add plugin-scaffolding day; trait
    shape stabilises during MVP, third-party `.so` / `.wasm` plugin
    loading opens in Phase 3.
  - **#4 AiStateStore trait + ReDB / RocksDB / ScyllaDB:** §6 owner gate
    #5 marked DECIDED. PostgreSQL / FoundationDB / sled / Redis-AOF
    dropped from candidate list. New Phase 2.1 task "AiStateStore trait
    + 3 backend impls" (5 days). Single-node → multi-node migration via
    export tool queued for Phase 3 (UC16 §12 #16). HA_ARCHITECTURE.md §3.5
    cookbook table rewritten.
  - §13 status snapshot: 2 new rows (UC16 decisions; storage gate closure).
  - Memory `uc16_scope.md` updated with the four decisions.
- **2026-05-02 (ninth revision):** UC16 topic #5 fold-in (tenant
  model + key hashing) + plugin lifecycle clarifications + interface-first
  audit extended.
  - **UC16 design decision #5 (tenant + hash):** USECASE_16 §12 #4 marked
    DECIDED. Flat keys + tags MVP (hierarchy at Beta). HMAC-SHA-256 +
    server pepper replaces Argon2id; saves ~10 000× per-request validation
    cost vs Argon2id while providing equivalent security against the
    threat model (256-bit random keys). USECASE_16 §7.1 rewritten with
    three sub-sections.
  - **Plugin lifecycle (hot-load):** new USECASE_16 §3.3.7 documenting
    capability matrix. Reuses existing `src/plugin/hot_reload.rs` notify
    watcher + 30 s drain in `manager.rs:252`. Built-in provider config
    updates (key rotation, model alias, RPM tuning) hot-reload via
    Phase 0 config reloader; third-party WASM/FFI plugin code updates
    hot-load in Phase 3 Beta when third-party loading opens.
  - **§4.4 interface-first audit extended from 8 to 10 boundaries** —
    `AiStateStore` (row 9, greenfield, Phase 2.1) and `AiProvider`
    (row 10, greenfield, Phase 2.1) added. Total project trait-extraction
    effort: 9.5 → 10.5 person-weeks.
  - **Phase 0.J** gains `HIGHPER_AI_KEY_PEPPER` (0.5 day) and
    `HIGHPER_PLUGIN_DRAIN_SECS` (0.5 day).
  - **Phase 2.4** virtual-key task updated to use HMAC + pepper +
    `AiStateStore` instead of sled+Argon2id.
  - **§13 status snapshot** to be extended next iteration; this revision
    adds 4 changed lines and 1 new sub-section (§3.3.7) to USECASE_16.
- **2026-05-02 (tenth revision):** UC16 topic #6 fold-in
  (tokenization posture).
  - **UC16 design decision #6 (tokenization):** USECASE_16 §12 #6 and
    #7 marked DECIDED. Bake all tokenizer vocabularies (~30 MB binary
    growth at Phase 3 GA); lazy-fetch and hybrid both dropped to support
    air-gap / regulated deployments and avoid first-request latency.
    Library split: `tiktoken-rs` for OpenAI `cl100k_base` + `o200k_base`;
    HuggingFace `tokenizers` for Anthropic BPE at MVP, plus
    Gemini SentencePiece / Llama-3 BPE / Cohere / DeepSeek / Qwen at
    Phase 3.
  - **Reasoning tokens** count as output by default (matches provider
    pricing); per-virtual-key `count_reasoning_in_output: bool` opt-out
    flag for operators absorbing reasoning costs as quality-of-service
    investment. Metrics still emit reasoning-token counts unconditionally
    with a `kind=reasoning` label.
  - **Cargo feature flags:** default ships all baked vocabularies;
    `ai-tokenizers-minimal` ships OpenAI-only (~4 MB) for size-conscious
    builds; `ai-tokenizers-only=…` for custom subsets.
  - **Phase 2.3 token-counter task** updated to ship OpenAI + Anthropic
    at MVP (was OpenAI-only at 2 days; now 3 days). New 1-day task
    "Reasoning-token billing rule" added. Phase 3 follow-on note for
    the additional provider vocabularies (~24 MB delta) — bundled with
    the respective `AiProvider` impls.
  - **§13 status snapshot** gains 1 row recording decision #6.
  - Memory `uc16_scope.md` updated with the tokenization rule.
- **2026-05-02 (eleventh revision):** UC16 topic #7 fold-in
  (cost / pricing source) + Phase 3.1 scope-fence cleanup.
  - **UC16 design decision #7:** USECASE_16 §12 #12 marked DECIDED.
    Vendored LiteLLM `model_prices_and_context_window.json` snapshot
    baked into binary + weekly signed refresh from
    `HIGHPER_AI_PRICING_FEED_URL` (default LiteLLM upstream, operator
    can self-host) + admin override at runtime via
    `PATCH /admin/ai/models/{alias}` (operator-level at MVP, per-tenant
    in Phase 3).
  - **USECASE_16 §5.5 fully rewritten** with six sub-sections:
    5.5.1 source / 5.5.2 admin override mechanics / 5.5.3 refresh
    failure handling (`last_known_good` default, `fail_closed` opt-in)
    / 5.5.4 zero-price handling / 5.5.5 per-tenant overrides as Phase 3
    follow-on / 5.5.6 schema fields. Added 5 new admin endpoints to
    §2.4.
  - **Phase 0.J** gains 5 pricing env vars
    (`HIGHPER_AI_PRICING_FEED_URL`, `_FEED_SIGN_KEY`,
    `_REFRESH_INTERVAL_SECS`, `_REFRESH_FAIL_MODE`,
    `HIGHPER_AI_ALLOW_FREE_TIER`) — 0.5 day.
  - **Phase 2.3** gains 4 new pricing tasks (registry + refresh job +
    admin endpoints + zero-price handling) totalling ~6 days; original
    "Model registry loader" 1-day task deferred (rolled into the four
    new tasks).
  - **Phase 3.1 cleanup** done at the same time:
    - Pre-call + post-call guardrail items killed (`[k]`) per UC16 #1
      scope fence — these were leftovers from before the fence.
    - Anthropic-shape inbound deferred (`[d]`) — moved to Phase 2.1
      MVP per UC16 #2.
    - Prompt-registry storage updated from "Postgres" to
      `AiStateStore` (UC16 #4 trait).
    - New 3-day **per-tenant pricing override** task added to
      Phase 3.1 (UC16 #7 follow-on).
  - **§13 status snapshot** gains 3 rows (decision #7; Phase 3.1
    cleanup; per-tenant overrides queued).
  - Memory `uc16_scope.md` updated with the pricing-source rule.
- **2026-05-02 (twelfth revision):** UC16 topic #8 fold-in
  (cache architecture).
  - **UC16 design decision #8:** USECASE_16 §12 #5 marked DECIDED.
    Engine-plus-pluggable posture confirmed: highper ships the cache
    *engine* (canonical hashing, lookup / write-back, TTL, tag-based
    + pattern invalidation, streaming replay, metrics, per-tenant key
    isolation); operator picks KV backend via existing `src/cache/`
    trait; new `VectorIndex` trait covers semantic-cache vector
    backend (Qdrant / Redis-Stack / PgVector / HNSW behind Cargo
    features); embedding model is operator's `AiProvider` choice.
    Mirrors `AiProvider` plugin and `AiStateStore` trait architectural
    patterns. Same configure-don't-code operator effort as LiteLLM
    with no backend lock-in.
  - **§6 fully rewritten** with six sub-sections: 6.0 engine-plus-
    pluggable posture / 6.1 exact cache (MVP) / 6.2 semantic cache
    (Beta) with `VectorIndex` trait / 6.3 provider prompt-cache
    passthrough / **6.4 explicit boundary table — what's NOT in scope
    for cache** (mirrors §8 guardrails section) / 6.5 operator effort
    summary.
  - **§3.2 module map** gains four new entries: `provider.rs`,
    `vector_index/{mod,qdrant,redis_stack,pgvector,hnsw}.rs`.
  - **§4.4 interface-first audit extended from 10 to 11 boundaries** —
    `VectorIndex` (row 11, greenfield UC16 trait, Phase 2.4 trait +
    Qdrant impl, Phase 3.1 for the other 3 impls). Total project
    trait-extraction effort: 10.5 → 11 person-weeks.
  - **Phase 0.J** gains 4 cache + vector backend env vars
    (`HIGHPER_AI_CACHE_BACKEND`, `HIGHPER_AI_VECTOR_BACKEND`,
    `HIGHPER_AI_VECTOR_ADDRS`, `HIGHPER_AI_VECTOR_AUTH`) — 0.5 day.
  - **Phase 2.4** gains 3 new exact-cache tasks (engine, headers,
    invalidation API) totalling ~3.5 days; replaces old 2-day
    "Exact cache" line.
  - **Phase 3.1** gains 3 new tasks: `VectorIndex` trait + Qdrant
    impl (4 days), semantic-cache engine (3 days), additional
    `VectorIndex` impls (3 days). Replaces old single 5-day
    "Semantic cache" line.
  - **Phase 2.4 budget enforcement task** updated: was "Redis
    counters + sled durable rollup"; now "Type B Valkey counters +
    `AiStateStore` durable rollup" per UC16 #4.
  - **§13 status snapshot** gains 2 rows (decision #8; §4.4
    extension).
  - Memory `uc16_scope.md` updated with engine-plus-pluggable rule
    and the no-bundled-backends-or-embedding-models guidance.
- **2026-05-02 (thirteenth revision):** UC16 topic #9 fold-in
  (routing strategies and fallback).
  - **UC16 design decision #9:** USECASE_16 §12 entry #17 added and
    marked DECIDED. Five-stage layered routing pipeline at MVP:
    per-virtual-key allow-list filter → capability-flags filter →
    circuit-breaker filter → rate-limit cooldown filter → priority
    ordering. Retry budget default 3 attempts across providers with
    per-virtual-key override. Cooldown state in Type B Valkey when
    configured (cluster-wide consistency), local `DashMap` otherwise;
    selection automatic via `HIGHPER_AI_COOLDOWN_BACKEND=auto`.
    Structured 503 with per-attempt error body on exhaustion. Cost-aware
    routing queued for Phase 3.1 Beta; latency-aware for Phase 4.1 GA;
    session affinity ties into N23 Helicone-style sessions Phase 2.5;
    weighted/canary split Phase 3.1.
  - **USECASE_16 §3.4 added** with six sub-sections: 3.4.1 layered
    candidate-list construction / 3.4.2 retryable-vs-non-retryable
    error classification / 3.4.3 retry budget + backoff / 3.4.4
    structured failure response / 3.4.5 cooldown-state location /
    3.4.6 router metrics (7 metrics: attempts, fallback, exhaustion,
    cooldown, capability filter, circuit breaker).
  - **§3.1 [Route resolve] step** updated to reference §3.4.
  - **Phase 0.J** gains 5 routing env vars
    (`HIGHPER_AI_RETRY_BUDGET`, `HIGHPER_AI_COOLDOWN_BACKEND`,
    `_DEFAULT_BACKOFF_MS_MIN/_MAX`, `_DEFAULT_COOLDOWN_SECS_NO_HEADER`)
    — 0.5 day.
  - **Phase 2.3 router task** expanded from 3 → 5 days; original
    "priority-list + rate-limit-aware skip" replaced with the full
    five-stage pipeline.
  - **Phase 3.1** gains 2 new tasks: cost-aware routing (3 days),
    weighted/canary split (2 days).
  - **Phase 4.1** gains 1 new task: latency-aware routing (3 days).
  - **§13 status snapshot** gains 1 row (decision #9).
  - Memory `uc16_scope.md` updated with the layered-routing rule and
    the cooldown-shared-via-Valkey-when-available guidance.
- **2026-05-03 (fourteenth revision, current):** UC16 topic #10 fold-in
  (streaming and cancellation semantics).
  - **UC16 design decision #10:** USECASE_16 §12 #8 and #9 marked
    DECIDED. Cancel-upstream-on-client-close as default; per-virtual-key
    `cancel_on_close=false` opt-in for drain-and-record (training-data
    collection / audit-completeness use cases). TPM mid-stream defaults
    to post-stream warning (smooth UX); per-virtual-key
    `tpm_hard_stop=true` opt-in injects SSE error frame mid-stream
    (cost-sensitive batch jobs).
  - **USECASE_16 §3.5 added** with five sub-sections:
    3.5.1 cancellation on client disconnect (cancel default, drain
    opt-in) / 3.5.2 TPM hard-stop vs post-stream warning /
    3.5.3 chunk hooks + buffer + plugin budget (with `_BUDGET_US`
    + `_BUFFER_DEPTH` + `_BUFFER_OVERFLOW_POLICY` env vars; fail-open
    on plugin overrun) / 3.5.4 streaming metrics (4 new metrics) /
    3.5.5 provider-side prompt-cache passthrough during streaming.
  - **§3.1 [SSE stream chunker] step** updated to reference §3.5.
  - **§7.1.1 virtual-key scope** gains 4 fields: `retry_budget`
    (decision #9), `count_reasoning_in_output` (decision #6),
    `cancel_on_close` (decision #10), `tpm_hard_stop` (decision #10).
  - **§5.4 streaming-TPM enforcement row** gets a back-reference to §3.5.2.
  - **Phase 0.J** gains 5 streaming env vars
    (`HIGHPER_PLUGIN_CHUNK_BUDGET_US`, `HIGHPER_AI_STREAM_BUFFER_DEPTH`,
    `HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY`,
    `HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE`,
    `HIGHPER_AI_DEFAULT_TPM_HARD_STOP`) — 0.5 day.
  - **Phase 2.5 streaming tasks** expanded:
    - SSE chunker: 4 → 5 days (adds budget enforcement + bounded buffer + 2 metrics).
    - Cancellation propagation: 2 → 3 days (adds drain-and-record path + per-key opt-in).
    - New 2-day TPM mid-stream policy task added.
    - New 1-day streaming metrics task added.
    - Net: Phase 2.5 streaming work grows ~6 → ~11 days.
  - **§13 status snapshot** gains 1 row (decision #10).
  - Memory `uc16_scope.md` updated with the cancel-on-close-default,
    tpm-warn-default, plugin-chunk-budget, and stream-buffer rules.
- **Future:** edit in place. Append to Section 12 with each substantive revision (date + one-line summary).

---

## 13. Current status snapshot (2026-05-02)

**One-line summary.** All work to date in this 2026-05-02 cycle is **documentation
and planning**. No source code has changed. Phase 0 has not started.

### 13.1 What's done — 2026-05-02 cycle

| Deliverable | Where it lives | Verification |
|---|---|---|
| Pre-Phase-0 reconciliation banners on legacy docs | `KNOWN_LIMITATIONS.md`, `README.md`, `CHANGELOG.md`, `docs/ARCHITECTURE.md` | §0.5 of this document |
| Stale `docs/ROADMAP.md` stub deleted | (no longer in repo) | §0.5 of this document |
| 14 release blockers tabulated with verified path:LINE citations | §4.1 (B1–B10 from earlier audit, B11–B14 added 2026-05-02) | §4.1 |
| B11–B14 detailed env-var-only design | §4.1.1 | env-var tables for each blocker |
| Project-wide env-var-only configuration rule | §0.1 + new Workstream 0.J | §0.1 |
| Interface-first architecture audit (8 weak boundaries) | §4.4 | trait extractions folded into 0.A / 0.C / 0.D / 0.I / 1.4 / 3.2 / 4.2 |
| `AiProvider` trait acceptance criterion (UC16) | §5 Phase 2.1 | follows existing `Compressor` / `WafEngine` registry pattern |
| Cookbook coverage matrix (per-UC inventory) | §4.5 | every UC1–UC15 has at least one entry; UC16 entry queued for Phase 2.6 |
| Competitor net-add features (N1–N25) tabulated | §4.6 | each placed by phase + priority |
| New Phase 2.5 (UC16 cross-request infrastructure) | §5 Phase 2.5 | introduced for N23 (Helicone-style sessions) |
| UC16 scope fence (LiteLLM/Portkey gateway role only) | §3.1 | guardrails / vLLM / AI observability / in-memory cache product all OUT of scope |
| UC16 storage-backend gate (open) | §3.2 + §6 #5 | `AiStateStore` trait kept pluggable until decision lands |
| HA architecture cluster personas (Type A / B / C) | §11 | sourced from `research-on-HA-architecture-…txt` |
| §6 gate #4 rewritten from binary to three-persona decision | §6 #4 | per HA research |
| GraphQL Federation explicit deferral for v1.0 | `docs/planning/GRAPHQL_FEDERATION.md` (NEW) + §3.4 ref + §5 Phase 0.E ref + §5 Phase 4.2 ref | UC13 ships passthrough+depth/complexity only |
| Sub-agents list pruned | §8 | `ai-guardrail-builder` removed; 6 remain |
| Persistent memory updated | `~/.claude/projects/.../memory/{config_env_vars_only.md, uc16_scope.md}` | survives across sessions |
| Two recurring routines scheduled | claude.ai routines | `gap-auditor` (monthly, 06:07 IST 1st) `trig_012cxCxcDsxugaqdJXB6syj2`; `docs-keeper` (weekly, 06:13 IST Sun) `trig_017YZKK1gLdJNntEAcSqVE7H` |
| HA architecture extracted into separate doc | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) | 4 cluster types (Type 1 Stateless / 2 +Valkey / 3 +etcd / 4 +V+E); per-UC profiles with LB algorithms cited; per-infra deployment notes |
| Cluster-bootstrap configuration shape defined | §11.2 | two-flag design (`HIGHPER_CLUSTER_TYPEB_BACKEND`, `HIGHPER_CLUSTER_TYPEC_BACKEND`) + 5 startup validations; replaces earlier single-enum design and fixes 9 validation errors |
| LB algorithms verified per UC | `src/tcp/mod.rs:203-224` (7), `src/config/schema.rs:450-460` (9), `src/grpc/mod.rs:114-125` (5), `src/config/dsl_ast.rs:402-410` (7) | grep-verified 2026-05-02; tabulated in `HA_ARCHITECTURE.md` §4 |
| 360° architectural review of HA — 12 concerns surfaced (F1–F12) | review report (this conversation) | 4 P0 fixes + 4 P1 fixes + 2 P2 fixes applied to HA_ARCHITECTURE.md; ROADMAP gained gate #7 (multi-region) and a Phase 1.2 RPS-benchmark task |
| HA §1.5 per-type performance envelope landed | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §1.5 | Type 1/2/3/4 RPS ceilings, latency adders per layer, Valkey hot-key bottleneck (F1), etcd write ceiling (F2) all called out |
| UC16 storage as 5th component documented | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §3.5 | hot-path counters in Valkey vs durable state separately; cookbook table for sled / Postgres / Scylla / FDB / Redis-AOF |
| Multi-region patterns documented | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §6.5 | per-type behaviour, 5 recommended patterns, UC16 multi-region considerations; ships single-region in v1.0 |
| Cluster security baseline documented | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §7.4 | per-type required controls (Valkey AUTH, etcd mTLS, network isolation); `docs/SECURITY_CLUSTER_BASELINE.md` queued in Phase 1.3 |
| Per-cloud-provider front-LB matrix documented | [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §7.5 | AWS / GCP / Azure don't support Keepalived (no L2); cloud-LB is the cloud default |
| UC16 design decisions #1–#4 recorded 2026-05-02 | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §1 / §2.1–§2.2 / §3.3 / §7 / §12 | (1) scope-fence pruned (no guardrails / vLLM / AI-observability product / in-memory cache product); (2) OpenAI + Anthropic dual-shape MVP; (3) AiProvider plugin architecture (separate trait, plugin-loadable via existing FFI/WASM, third-party loading opens Phase 3); (4) AiStateStore trait + ReDB / RocksDB / ScyllaDB impls |
| UC16 storage gate (#5) closed | ROADMAP §6 #5 | ReDB / RocksDB / ScyllaDB selected; Postgres + FDB + sled dropped |
| UC16 design decision #5 recorded 2026-05-02 (tenant + key hashing) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §7.1 / §12 #4 | flat keys+tags MVP; **HMAC-SHA-256 + `HIGHPER_AI_KEY_PEPPER` server pepper** replaces Argon2id (saves ~10 000× per-request validation cost); soft-disable revocation default |
| Plugin hot-load capability matrix documented for AI providers | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §3.3.7 | reuses `src/plugin/hot_reload.rs` + `manager.rs:252` 30 s drain; built-in providers update via Phase 0 config reload; third-party WASM/FFI plugin code hot-loads from Phase 3 Beta |
| §4.4 interface-first audit extended from 8 to 10 boundaries | ROADMAP §4.4 | rows 9 (`AiStateStore`) and 10 (`AiProvider`) added — both greenfield, both Phase 2.1; total trait-extraction effort 9.5 → 10.5 person-weeks |
| UC16 design decision #6 recorded 2026-05-02 (tokenization posture) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §5.1–§5.4 / §12 #6, #7 | bake all vocabularies (~30 MB binary at Phase 3 GA); `tiktoken-rs` for OpenAI + `tokenizers` (HuggingFace) for everything else; reasoning tokens count as output by default with per-virtual-key `count_reasoning_in_output: bool` opt-out; air-gap / regulated deployments fully supported |
| UC16 design decision #7 recorded 2026-05-02 (cost / pricing source) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §5.5 (six sub-sections) / §12 #12 | vendored LiteLLM snapshot baked into binary + weekly signed refresh + admin override at runtime via `PATCH /admin/ai/models/{alias}`; refresh failure default `last_known_good`; zero-price default-reject; per-tenant overrides queued for Phase 3 |
| Phase 3.1 scope-fence cleanup 2026-05-02 | ROADMAP Phase 3.1 | killed pre-/post-call guardrail items per UC16 #1 scope fence; moved Anthropic-shape inbound to Phase 2.1 MVP per UC16 #2; updated prompt-registry storage to `AiStateStore` per UC16 #4 |
| UC16 design decision #8 recorded 2026-05-02 (cache architecture) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §6.0–§6.5 / §12 #5 | engine-plus-pluggable: highper ships canonical hashing / lookup / write-back / TTL / tag invalidation / streaming replay / metrics; KV backend via existing `src/cache/` trait, vector backend via new `VectorIndex` trait, embedding model via `AiProvider` registry — all operator-chosen per deployment. New §6.4 explicit "what's NOT in scope" boundary table mirrors §8 (guardrails). |
| §4.4 interface-first audit extended from 10 to 11 boundaries | ROADMAP §4.4 | row 11 (`VectorIndex`) added — greenfield UC16 trait, Phase 2.4 trait + Qdrant impl, Phase 3.1 for Redis-Stack / PgVector / HNSW; total trait extractions 10.5 → 11 person-weeks |
| UC16 design decision #9 recorded 2026-05-02 (routing strategies) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §3.4 / §12 #17 | layered MVP (priority + rate-limit-aware + health-aware + capability-aware stacked); 3-attempt retry budget default with per-virtual-key override; cooldown state in Type B Valkey when configured else local `DashMap`; structured 503 with per-attempt details on exhaustion; cost-aware (Beta) / latency-aware (GA) / weighted-canary (Beta) queued for later phases |
| UC16 design decision #10 recorded 2026-05-03 (streaming + cancellation) | [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §3.5 / §12 #8, #9 | cancel-upstream-on-client-close default with per-virtual-key drain-and-record opt-in; TPM mid-stream defaults to post-stream warning with per-key hard-stop opt-in; per-chunk plugin budget (default 500 µs, fail-open on overrun); bounded stream buffer (default 64 events, drop-oldest on overflow); 4 new streaming-specific metrics |
| Phase 1.3.1 cluster-deployment templates queued | §5 Phase 1.3.1 | 9 cells (3 personas × 3 infrastructures) under `examples/configs/clusters/` + decision-flow README + CI validation harness |
| `.gitignore` excludes private session notes | `.gitignore` | `docs/reverse-proxy-quick-progress-notes.txt` added |
| Initial commit landed (2 commits) | git log | `535721a` script relocation, `309cc8f` docs reconciliation + ROADMAP refresh |

### 13.2 What's pending — by priority

**P0 — Phase 0 release blockers (14 items, must close before any v1.0 GA tag):**

- B1 UC10 hybrid wiring — Workstream 0.A
- B2 Admin API truth — Workstream 0.G
- B3 OCSP / ACME / TLS honesty — Workstream 0.B
- B4 Distributed rate-limit safety + X-Forwarded-For trust — Workstream 0.C
- B5 GraphQL depth/complexity enforcement — Workstream 0.E (federation explicitly deferred per `GRAPHQL_FEDERATION.md`)
- B6 PostgreSQL pool real validation — Workstream 0.E
- B7 gRPC pooling + trailers + call-type registry — Workstream 0.D
- B8 HTTP/3 unwrap + buffering + migration — Workstream 0.D
- B9 cloud validation matrix for 9 UCs — Phase 1.1
- B10 7-/30-day soak — Phase 1.2
- B11 8 unbounded_channel sites bounded with env vars — Workstream 0.F
- B12 body-size centralization — Phase 1.6
- B13 RSA Marvin attack mitigation (`oidc` feature off by default + CI gate) — Phase 1.5
- B14 graceful drain for spawned tasks — Workstream 0.D

**P0 — Open owner gates (must clear to unblock subsequent phases):**

| Gate | Blocks | Status |
|---|---|---|
| #1 Phase 0 priority confirmation + Rancher Desktop + Phase 1.5 tools | Phase 0 start | open — pending owner sign-off |
| #2 v1.0 GA includes / excludes UC13 federation? | v1.0 tag | **answered 2026-05-02 — defer; passthrough only**; see `GRAPHQL_FEDERATION.md` |
| #3 UC16 scope + design-doc revision | Phase 2 start | open — §3.1 fence written; doc revision pending |
| #4 HA architecture (default persona + Type B + Type C backends) | Phase 4 enterprise scope | open — three personas defined per §11; defaults TBD |
| #5 UC16 storage backend | — | **DECIDED 2026-05-02 (UC16 #4)** — `AiStateStore` trait + ReDB / RocksDB / ScyllaDB impls; per-deployment via `HIGHPER_AI_STATE_BACKEND` |
| #6 Re-confirm §0.5 banners at GA tag time | v1.0 tag | open — re-runs at tag |

**P1 — Cheap hygiene (Phase 1.6, < 2 days each):** `DefaultHasher` swap;
`Alt-Svc` auto-inject; rate-limit metrics; DDoS geo-block wiring;
`Retry-After` precision; empty `gateway/rate_limit/` dir delete;
`compression_old.rs.backup` delete; `deny.toml.backup` delete.

**P1 — Per-UC catch-up (folded into Phase 3.2):** UC1 EWMA + Unix sockets +
HDR histogram; UC2 PROXY-protocol v2 + canary mirror; UC11 RFC 7234 +
single-flight + range-slicing; UC12 watch-streaming + K8s discovery; UC13
subscriptions + APQ + schema hot-reload; UC14 try_files + pre-compressed +
fastcgi_status; UC15 ASN + region failover.

**P1 — Cross-cutting v1.0:** OTLP via `MetricsBackend` trait; W3C trace context;
Vault / AWS Secrets / K8s Secret resolver; clippy-zero; SBOM (Trivy + syft +
Grype); DAST (Dastardly + ZAP); cookbook schema-load CI test; UC16 cookbook entry;
**9 cluster-deployment cookbook cells** under `examples/configs/clusters/`
(Phase 1.3.1) + cell-level validation in Phase 1.1.

**P1 — UC16 MVP (Phase 2):** `AiProvider` trait + 4 providers (OpenAI / Anthropic /
Bedrock / Gemini); virtual keys + budgets; exact cache; SSE chunker; admin
endpoints; acceptance suite — gated on §6 #3 + #5.

**P1 — UC16 Beta (Phase 3.1):** semantic cache; provider prompt-caching
passthrough; 9 more providers; embedding batch coalescing; vision passthrough;
Anthropic-shape inbound; `Session` object (N23, Phase 2.5).

**P2 — Ecosystem (Phase 4.2):** xDS, K8s operator, kTLS, post-quantum TLS,
ECH, federated GraphQL (Apollo v2 — see `GRAPHQL_FEDERATION.md` §6 acceptance
criteria), Redis Cluster slot routing, PgBouncer-style transaction pooling,
JA3/JA4, CT-log monitoring, GitOps controller, all 18 N4.2.N items
(N3 / N5 / N8 / N10 / N11 / N13–N22 / N25), `ConfigSource` trait, config
template expansion, profile inheritance, 80-TODO sweep, plugin marketplace.

### 13.3 Next concrete actions (waiting on the owner)

1. **Sign off on §6 gate #1** so Phase 0 can begin.
2. **Approve scheduled `gap-auditor`** sub-agent (monthly coverage trace +
   citation re-verification + reconciliation banner consistency check).
3. **Approve scheduled `docs-keeper`** sub-agent (weekly: ROADMAP checkbox
   state vs `KNOWN_LIMITATIONS.md` / `README.md` / `CHANGELOG.md` /
   `ARCHITECTURE.md`; flag drift).
4. **Pick a default Type B backend (Valkey vs Redis)** so Phase 0.J's
   `Settings` scaffold can ship its default for that subsystem.
5. **Pick a default Type C backend (etcd vs raft-rs vs Consul)** so the
   Phase 0.A Listener config can wire to it without speculation.

### 13.4 Lines of evidence

- Total ROADMAP.md size: ~1180 lines (was 678 at start of this 2026-05-02
  cycle; +75 % growth).
- 14 release blockers, each with verified `path:LINE` citation.
- 8 weak interface boundaries, each placed in a phase.
- 25 competitor net-add features, each placed in a phase.
- 16 use cases with confirmed cookbook coverage (15 ✅, UC16 queued).
- 3 cluster personas with per-UC mapping.
- 6 owner gates, 1 newly answered (UC13 federation deferral), 5 open.

---

---

*Document author: claude-opus-4-7-1m, 2026-05-02. Working tree: `master @ 10752ce`. Per CLAUDE.md rules.*
