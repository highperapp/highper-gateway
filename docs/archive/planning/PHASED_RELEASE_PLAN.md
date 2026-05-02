# Highper Gateway — Phased Release Plan

**Date:** 2026-05-02.
**Companion documents:** `docs/AUDIT_2026-05-02.md` (gap details), `docs/USECASE_16_AI_LLM_GATEWAY.md` (UC16 design).

This plan sequences the work surfaced by the audit. Phase 0 is **non-negotiable** before any v1.0 release announcement; subsequent phases can re-order but should not parallelize beyond what's noted. All effort estimates assume one focused engineer; double or halve based on team size.

---

## Phase summary

| Phase | Theme | Calendar weeks | Outcome |
|------:|-------|---------------:|---------|
| **0** | Release blockers — fix what we already claim | 4–5 | All 15 UCs honestly defensible |
| **1** | v1.0 GA — ops, docs, soak, validation matrix | 4–6 | Public release |
| **2** | UC16 MVP — AI/LLM Gateway initial cut | 4–6 | "v1.1: AI Gateway" tag |
| **3** | UC16 Beta + cross-cutting catch-up | 4–6 | "v1.2" |
| **4** | UC16 GA + ecosystem (xDS, K8s op) | 8–12 | "v2.0" |

Total: roughly **6–9 months** from today (2026-05-02) to a "v2.0" with full UC1–UC16 + ecosystem integrations. Phase 0+1 alone (a defensible v1.0) is **8–11 weeks**.

---

## Phase 0 — Release blockers (4–5 weeks)

**Goal.** Bring claimed capabilities to truth. After Phase 0, every "✅" in the use-case list is honestly defensible. **Nothing else ships before this is done.**

The blockers map to `docs/AUDIT_2026-05-02.md §1.2` items B1–B10.

### Workstream 0.A — Hybrid wiring (UC10, B1)

- [ ] Add `Listener` config block (`protocol: tcp|http|http3|grpc`, bind list, TLS optional). **2 days.**
- [ ] `Runtime::run` instantiates per-listener server task; share `Arc<RwLock<Config>>`, `ProxyState`, hot-reload bus. **2 days.**
- [ ] Hot-reload propagates to TCP listener (currently HTTP-only). **2 days.**
- [ ] Cross-protocol metric correlation tests. **1 day.**

**Total: ~1.5 weeks.** Owner: 1 engineer.

### Workstream 0.B — TLS honesty (UC3, B3)

- [ ] Pull in a real OCSP library (`rust-ocsp` / `webpki-ocsp`) — write proper OCSP request and verify response (signature, thisUpdate/nextUpdate, status, nonce). **5 days.**
- [ ] Wire `OcspStapler` cached responses into `CertifiedKey.ocsp` in rustls. **2 days.**
- [ ] Implement `needs_renewal()` against notBefore/notAfter (default renew at 30d remaining). **1 day.**
- [ ] CRL `extract_crl_number` and `extract_delta_crl_url` via `x509-parser` extensions. **2 days.**
- [ ] Replace `fs::read` with `tokio::fs::read` in TLS load paths. **0.5 day.**
- [ ] Strip ACME private-key PEM from any debug log path. **0.5 day.**
- [ ] CI: integration test against Let's Encrypt staging end-to-end. **2 days.**

**Total: ~2.5 weeks.** Owner: 1 engineer.

### Workstream 0.C — Rate-limit + WAF safety (UC4, UC9, B4)

- [ ] Proxy-trust-list config (CIDR allowlist for `X-Forwarded-For`); fall back to socket addr outside the list. **1.5 days.**
- [ ] Distributed limiter Redis-failure mode: `fail-open | fail-closed | local-fallback` (default `local-fallback`). **2 days.**
- [ ] Per-route limit wiring from config to runtime (DSL + YAML). **1 day.**
- [ ] Custom WAF: real SQLi/XSS/path-traversal regex sets; verify Coraza binding actually compiles + runs CRS rules; if not, swap to Rust `coraza-rs` port. **5 days.**
- [ ] Body decompression pipeline (gzip/deflate up to cap) before WAF runs. **3 days.**
- [ ] WAF rule hot-reload via signal + admin API. **2 days.**

**Total: ~3 weeks.** Owner: 1 engineer.

### Workstream 0.D — Protocol fixes (UC5, UC6, UC7, B7, B8)

- [ ] **UC5/H3:** replace receive-loop `unwrap` (`http3_quiche.rs:403`) with safe drop + counter; re-enable connection migration with anti-amp checks; stream backend response (no `body.collect()`); wire actually-emitting metrics. **6 days.**
- [ ] **UC6/WS:** swap `RwLock::*().unwrap()` calls in `recovery.rs` for the existing `safe_lock!` poison-recovery macro. **1 day.**
- [ ] **UC7/gRPC:** reuse a per-upstream HTTP/2 client pool from `connection_pool.rs`; emit real HTTP/2 trailers (`http_body::Frame::trailers`) for `grpc-status`. **3 days.**

**Total: ~2 weeks.** Owner: 1 engineer.

### Workstream 0.E — Application-protocol fixes (UC8, UC13, B5, B6)

- [ ] Real PostgreSQL pool validation (parameter-status / `SELECT 1` probe with timeout). **2 days.**
- [ ] PostgreSQL STARTTLS/SSLRequest negotiation. **3 days.**
- [ ] DB pool failover wired to circuit breaker. **2 days.**
- [ ] GraphQL depth + complexity analyzers wired to parsed AST; reject early; emit metric. **3 days.**
- [ ] GraphQL stitcher: ship a real implementation OR explicitly mark UC13 as "schema-pass-through, federation deferred to v1.1". **Decision day + execution.** Recommended: mark deferred; pull in `apollo-router-rs`-equivalent for v1.1.

**Total: ~2 weeks.** Owner: 1 engineer.

### Workstream 0.F — Slowloris + header-size + UC2 hygiene (UC2)

- [ ] Per-request read/idle timeouts (config-driven); 408 on incomplete header read. **2 days.**
- [ ] Header-block size cap → 431. **1 day.**
- [ ] HTTP/2 GOAWAY graceful drain on shutdown signal. **2 days.**

**Total: ~1 week.** Owner: 1 engineer.

### Workstream 0.G — Admin API truth (cross-cutting, B2)

- [ ] Real JWT verify (use `jsonwebtoken` crate or `jose` — pick one); token validation, scope check. **2 days.**
- [ ] `GET /admin/config` returns actual configuration with secrets redacted. **1 day.**
- [ ] `POST /admin/config` (validated, atomic apply, revert on failure). **3 days.**
- [ ] Backend enable/disable, weight changes (live, no restart). **2 days.**
- [ ] OpenAPI 3.1 spec generated from handlers. **2 days.**
- [ ] Audit log of admin-API actions (Postgres or sled append-only). **2 days.**

**Total: ~2 weeks.** Owner: 1 engineer.

### Workstream 0.H — Discovery quick-fix (UC12)

- [ ] Fix `should_refresh = true` always (`registry.rs:41`) — cache last-update; respect TTL. **0.5 day.**
- [ ] Consul ACL token field; mTLS to Consul/etcd. **2 days.**

**Total: ~0.5 week.** Owner: 1 engineer.

### Phase 0 deliverables

- All P0 issues from the audit closed.
- New unit + integration tests covering each fix.
- `docs/CHANGELOG.md` entry per workstream.
- `docs/KNOWN_LIMITATIONS.md` updated to remove fixed items.

### Parallelization

Workstreams **0.A, 0.B, 0.D, 0.G** are independent and parallelizable across up to 4 engineers (target 1.5 weeks elapsed). 0.C, 0.E, 0.F, 0.H need to merge against 0.A and the others touching `Runtime`. With 1 engineer: ~13 weeks total work compressed to ~5 calendar weeks if focus + minimal context-switching.

---

## Phase 1 — v1.0 GA hardening (4–6 weeks)

**Goal.** Ship a defensible public v1.0 of the 15-use-case gateway.

### 1.1 Cloud validation matrix (B9)

- [ ] Pre-build Docker images for all 15-scenario backend mocks (HTTP/3 with kernel QUIC, WebSocket echo, gRPC server with reflection, MySQL/Postgres/Redis, Consul + etcd, GraphQL test schema with subgraphs, MaxMind+IP2Location DBs, PHP-FPM with sample app). **1 week.**
- [ ] CI pipeline: spin up each scenario, run scenario tests, archive results. **3 days.**
- [ ] Cloud test pass on a real VM (Linux 6.1+ for QUIC, kTLS): record evidence; close env-failure entries from `docs/VALIDATION_REPORT_2026-01-11.md`. **3 days.**
- [ ] Update `docs/VALIDATION_REPORT_2026-05-XX.md` with green cells.

### 1.2 Long-soak validation (B10)

- [ ] 7-day stability test at 1M concurrent connections (target was deferred per `TODO.md:112-122`). **7 days elapsed.**
- [ ] Memory-leak / fd-leak diff using heap profiler at start vs end.
- [ ] 30-day soak as parallel background activity once 7-day is green.
- [ ] Chaos-engineering pass: kill backends, inject 5% packet loss, add 200ms latency, saturate CPU, force-close upstreams. Record graceful-degradation behavior. **3 days active work + observation.**

### 1.3 Operational deliverables

- [ ] `docs/DEPLOYMENT_GUIDE.md` already exists — verify still accurate (53k file dated Jan 24); update for any Phase 0 config schema changes. **2 days.**
- [ ] `docs/MONITORING.md` (NEW) — Prometheus + Grafana quickstart with importable dashboards (RED + USE) for: HTTP, TLS, WAF, cache, discovery, geo, PHP-FPM, gRPC, HTTP/3, WebSocket. **5 days.**
- [ ] `docs/TROUBLESHOOTING.md` (NEW) — common errors → root cause → fix. **3 days.**
- [ ] `docs/UPGRADE.md` (NEW) — config migration matrix from beta → v1.0. **2 days.**
- [ ] Sample systemd unit, Dockerfile, Helm chart, docker-compose. **3 days.**
- [ ] `docs/SECURITY.md` updated with disclosure policy + bounty link. **0.5 day.**

### 1.4 Cross-cutting catch-up

- [ ] OTLP gRPC exporter alongside Prometheus. **5 days.**
- [ ] W3C Trace Context + B3 propagation end-to-end test. **2 days.**
- [ ] Vault / AWS Secrets Manager / K8s Secret reference resolver in config. **5 days.**
- [ ] Cleanups: delete `src/middleware/compression_old.rs.backup`, empty `src/gateway/rate_limit/` dir, `deny.toml.backup`. **0.5 day.**
- [ ] Clippy debt to zero (currently 123 non-critical). **3 days.**

### Phase 1 exit criteria

- All 15 UCs passing in CI **with cloud-grade Docker fixtures**.
- 7-day soak green (no memory drift > 5%, no fd leaks, no panics).
- Deployment, monitoring, troubleshooting docs published.
- Public announcement post drafted.

---

## Phase 2 — UC16 MVP (4–6 weeks)

**Goal.** Ship the AI/LLM Gateway acceptance criteria from `docs/USECASE_16_AI_LLM_GATEWAY.md §13`.

### 2.1 Core scaffolding (week 1)

- [ ] Open the 13 design questions in §12 of UC16 doc with the owner; commit decisions in design notes. **2 days, decisions before code.**
- [ ] Create `src/gateway/ai/` module skeleton with files from §3.2 of UC16 doc. **0.5 day.**
- [ ] Canonical `AiRequest` / `AiResponse` types per §4. **2 days.**
- [ ] Inbound shape detect + parse for OpenAI (chat + embeddings + models). **2 days.**

### 2.2 Translators (week 2)

- [ ] OpenAI ↔ canonical (passthrough). **1 day.**
- [ ] Anthropic Messages translator (incl. tool-use shape difference, system-prompt extraction). **3 days.**
- [ ] Bedrock Converse translator + SigV4 signer (use `aws-sigv4` crate). **5 days.**
- [ ] Gemini `generateContent` / `:streamGenerateContent` translator (incl. SSE-ish streaming). **3 days.**

### 2.3 Routing + accounting (week 3)

- [ ] Model registry loader (`model_prices_and_context_window.json` from LiteLLM, vendored). **1 day.**
- [ ] Router with priority-list fallback and rate-limit-aware skip. **3 days.**
- [ ] Token counter (tiktoken cl100k+o200k bake-in). **2 days.**
- [ ] Cost calculator (post-stream + on-cancel). **1 day.**
- [ ] Per-(provider, region) HTTP/2 connection pool wired from `src/proxy/connection_pool.rs`. **2 days.**

### 2.4 Auth + budgets + cache (week 4)

- [ ] Virtual-key store (sled MVP) + `sk-hpgw-…` issuance + Argon2id hash. **3 days.**
- [ ] Per-key budget enforcement (day/month) backed by Redis counters + sled durable rollup. **3 days.**
- [ ] RPM + TPM (token-denominated) buckets — extends `gateway/ratelimit/token_bucket.rs` to support dynamic-cost consumption. **3 days.**
- [ ] Exact cache (canonical-JSON key) layered over `cache::manager`. **2 days.**

### 2.5 Streaming + admin + tests (week 5)

- [ ] SSE-aware chunker that re-emits inbound-shape SSE; per-chunk hooks for token count / log / cancel. **4 days.**
- [ ] Cancellation propagation: client SSE close → cancel upstream + record partial usage. **2 days.**
- [ ] Admin endpoints (`POST/GET /admin/ai/keys`, `GET /admin/ai/spend`, `POST /admin/ai/budgets`, `GET /admin/ai/models`). **3 days.**
- [ ] Acceptance-test suite covering all 10 criteria from UC16 §13 against recorded provider fixtures. **3 days.**

### 2.6 Polish + docs (week 6)

- [ ] DSL extension for `ai_route` block (per UC16 §10) wired to existing pest grammar. **3 days.**
- [ ] `docs/AI_GATEWAY_GUIDE.md` (NEW) — quickstart, recipe book. **3 days.**
- [ ] Prometheus metrics from UC16 §9.1 emitting. **2 days.**
- [ ] Migration test: existing OpenAI client (Python `openai==1.x`) talks to gateway against Anthropic upstream — verify zero client-code changes. **2 days.**

### Phase 2 exit criteria

- All 10 acceptance criteria from UC16 §13 pass in CI.
- 4 providers (OpenAI, Anthropic, Bedrock, Gemini) supported.
- Owner agrees v1.1 / "AI Gateway" tag is releasable.

---

## Phase 3 — UC16 Beta + cross-cutting catch-up (4–6 weeks)

### 3.1 UC16 Beta features

- [ ] Semantic cache (Redis-Stack vector or HNSW; pluggable). **5 days.**
- [ ] Provider prompt-caching passthrough with byte-stable serializer + CI byte-stability test. **3 days.**
- [ ] Pre-call guardrails: PII regex set, OpenAI moderation, Bedrock Guardrails, Llama-Guard via configured upstream. **5 days.**
- [ ] Post-call guardrails (streaming): output PII redact, JSON-schema validate, regex deny. **5 days.**
- [ ] Prompt registry + versioning (Postgres). **5 days.**
- [ ] MCP passthrough (`/v1/mcp/{server}`). **3 days.**
- [ ] Add providers: xAI, DeepSeek, Mistral, Groq, Together, Fireworks, Cohere, Vertex (non-Anthropic), Azure OpenAI. **7 days.**
- [ ] Embedding batch coalescing (combine N small embeds into one upstream batch within 50ms window). **3 days.**
- [ ] Vision passthrough (URL-fetch for providers that don't auto-fetch). **3 days.**
- [ ] Anthropic-shape inbound (`POST /v1/messages`). **3 days.**

### 3.2 Cross-cutting (parallel)

- [ ] WS `permessage-deflate` (RFC 7692). **5 days.**
- [ ] WS-over-h2 (RFC 8441) Extended CONNECT. **5 days.**
- [ ] gRPC retry budget + circuit-break per `:authority` + gzip identity-encoding negotiation. **5 days.**
- [ ] Vary-aware cache key + singleflight + RFC 7234 Cache-Control parser. **7 days.**
- [ ] HA decision (Raft control plane vs. stateless+Redis); implement chosen path. **2 weeks.**
- [ ] Plugin signing + improved per-plugin isolation. **5 days.**

---

## Phase 4 — UC16 GA + ecosystem (8–12 weeks)

### 4.1 UC16 GA

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

### 4.2 Ecosystem (UC1–UC15 → enterprise scope)

- [ ] xDS client (CDS/EDS at minimum, then RDS/LDS/SDS) — Envoy-style dynamic config. **3 weeks.**
- [ ] Kubernetes operator + Ingress controller + CRDs + Helm chart. **3 weeks.**
- [ ] kTLS sendfile complete syscall path. **5 days.**
- [ ] Post-quantum hybrid TLS (X25519+MLKEM) once `rustls` stabilizes. **conditional, ~5 days when available.**
- [ ] ECH (Encrypted Client Hello). **conditional on rustls.**
- [ ] Federated GraphQL (Apollo Federation v2 entity resolution). **3 weeks.**
- [ ] Redis Cluster slot routing + Sentinel master discovery. **2 weeks.**
- [ ] PgBouncer-style transaction-pooling mode for Postgres. **2 weeks.**
- [ ] JA3/JA4 fingerprinting + WAF integration. **5 days.**
- [ ] SIEM-format (CEF/LEEF) WAF log export. **3 days.**
- [ ] CT-log monitoring for issued certs. **5 days.**
- [ ] GitOps controller (declarative-diff/apply against Git). **2 weeks.**

---

## Risks & dependencies

| Risk | Phase | Mitigation |
|---|---|---|
| Phase 0 reveals a deeper architectural issue (e.g., runtime model can't be retrofitted for true UC10) | 0 | If discovered, dedicate a 2-week spike before continuing; document outcome in `ARCHITECTURE_v2.md`. |
| 7-day soak surfaces memory leak | 1 | Phase 1 must not be skipped; budget 1 week for diagnosis; don't ship v1.0 until green. |
| OCSP library choice creates a new dep with thin maintenance | 0 | Vet `webpki-ocsp` or vendor a small wrapper; ASN.1 is the boundary, not a moving target. |
| UC16 owner picks "Anthropic-shape inbound at MVP" — doubles scope | 2 | Fence it: keep MVP at OpenAI-shape only; Anthropic in Beta. |
| Provider API change (Anthropic adds new content-block type) breaks translator | 2+ | CI tests against recorded provider fixtures + `unknown_field` capture so unknown bits are forwarded verbatim. |
| HA decision deferred forever | 3 | Make decision in Phase 2 week 6 latest; document irreversibility. |

---

## Owner gates

The plan assumes the owner makes 4 decisions before/during the timeline. Surfacing them up front:

1. **Now (before Phase 0 start):** confirm Phase 0 priority order — agree all 10 blockers are in scope, or strike specific items with rationale.
2. **End of Phase 1:** does v1.0 launch with UC13 (GraphQL) marked "passthrough only, federation deferred"? Or block on shipping real federation in Phase 1? Recommendation: **defer to v1.1.**
3. **Start of Phase 2:** answer the 13 questions in `docs/USECASE_16_AI_LLM_GATEWAY.md §12` before code.
4. **End of Phase 3:** HA architecture decision (Raft vs stateless+Redis) — affects Phase 4 enterprise tier feasibility.

---

## Tracking

A single tracker (`docs/RELEASE_TRACKER.md`, NEW) should host the checkbox state of every workstream above, updated weekly. Each closed item links to its merged PR. This document is the plan; that document is the live progress.

---

*Plan author: claude-opus-4-7-1m, 2026-05-02. Working tree: `master @ 10752ce`.*
