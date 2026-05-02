# Highper Gateway — HA Architecture

**Owner-of-record:** Highper Gateway maintainer.
**Created:** 2026-05-02.
**Status:** Authoritative reference for cluster types, deployment patterns, and per-UC HA characteristics. Tracking and phased rollout live in `docs/planning/ROADMAP.md`; this document is design / reference, not a tracker.

**Companion documents:**

- [`ROADMAP.md`](ROADMAP.md) — phased plan, release-blocker tracking, owner gates. References this document from §11.
- [`research-on-HA-architecture-highper-gateway-16-deployment-usecases.txt`](research-on-HA-architecture-highper-gateway-16-deployment-usecases.txt) — original cluster-persona research; cited throughout.
- [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) — UC16 design draft (predates the 2026-05-02 scope fence; see ROADMAP §3 for current scope).

Per `CLAUDE.md` rules: every concrete claim cites a source span (research file line, source file `path:LINE`, or doc reference). Items I cannot directly cite are marked `(unsourced inference)`.

---

## 1. Overview — four cluster types

A highper-gateway deployment is one of four cluster types, distinguished by which coordination layers it provides. The cluster type follows mechanically from the set of use cases the operator enables.

| Type | Components | Adds (vs Type 1) | UCs supported (count) |
|---|---|---|---|
| **Type 1 — Stateless** | N highper-gateway replicas | — | 7 (Group A) |
| **Type 2 — Stateless + Valkey** | N highper replicas + Valkey/Redis cluster | Valkey for global counters / shared k-v / pub-sub | 14 (Group A + Group B) |
| **Type 3 — Stateless + etcd** | N highper replicas + etcd / Consul / Raft cluster | Consensus k-v for cert state and service registry | 9 (Group A + Group C) |
| **Type 4 — Stateless + Valkey + etcd** | N highper replicas + Valkey + etcd | Both | 16 (all UCs) |

**Why four and not one.** Operators pay only for the coordination layers their UC selection requires. A pure HTTP load balancer (Type 1) costs 1 minimum node; a production AI gateway with mTLS (Type 4) costs 8+ for HA. Resource difference is significant; supporting all four respects that.

**Why the mapping is mechanical.** Each UC declares which coordination it needs (Group A / B / C — see §3 below). Operator picks UCs → take the union of groups → that determines the type. There is no "default type"; there is a default *for the empty UC set* (Type 1).

### 1.5 Per-type performance envelope (added 2026-05-02)

This section translates the cluster-type model into operator-relevant
throughput / latency / scaling envelopes. Numbers below are
**order-of-magnitude estimates** for production-grade hardware (AWS m6i / c6i
or equivalent); exact figures depend on workload profile. Per `CLAUDE.md`
rule #4, items not directly cited from a benchmark report are tagged
`(unsourced inference)`.

#### 1.5.1 Throughput ceilings per type

| Type | Theoretical RPS ceiling | Where it bottlenecks first | Mitigation |
|---|---|---|---|
| **Type 1** | line-rate × N replicas; ~200k RPS per 4-vCPU per `README.md:7` (cited) — linear scale-out | NIC + CPU per replica | Add replicas; scale horizontally |
| **Type 2** | bounded by Valkey shard hot-key throughput; ~100k ops/sec per Valkey shard *(unsourced inference)* | **Single Valkey shard** if a hot key exists (e.g. one popular UC04 rate-limit key) | Pre-shard rate-limit keys (`HIGHPER_RATELIMIT_KEY_SHARDS`); use Valkey Cluster slot routing |
| **Type 3** | bounded by etcd writes for control-plane churn; **~10k writes/sec ceiling** *(unsourced inference; etcd documented limit)* | **etcd write throughput** if UC12 service-discovery churn is high | Reduce churn (longer TTLs); shard service registry by namespace; consider Consul for high-churn |
| **Type 4** | min(Type 2 ceiling, Type 3 ceiling); for typical workloads, Valkey shard limits first | depends on UC mix — UC04 hot keys hit Valkey ceiling, UC12 churn hits etcd ceiling | apply Type 2 + Type 3 mitigations independently |

#### 1.5.2 Latency adders per layer

For a request that lands at highper and needs each coordination layer once:

| Layer | Operation | Added latency | Notes |
|---|---|---|---|
| highper local | per-request work | 0.05–0.5 ms | hot path; SIMD path matching at `src/gateway/routing/matcher.rs:11-90` |
| Valkey local-cluster | k/v read + counter increment | **0.5–2 ms** *(unsourced inference)* | over-LAN; sub-ms on co-located shard |
| Valkey cross-AZ | same as above | 1–3 ms *(unsourced inference)* | AZ hop adds ~1 ms |
| etcd local-cluster read (cached on highper) | k/v read | 0.1 ms (cache hit) | `src/discovery/etcd.rs` watch caches |
| etcd local-cluster read (cache miss) | k/v read | 5–10 ms *(unsourced inference)* | quorum read |
| etcd write | quorum commit | **5–10 ms** *(unsourced inference)* | linearizable; not for hot paths |

**Implication for SLOs.** A Type 4 deployment with UC04 + UC12 enabled
adds ~1–3 ms p50 to every request (Valkey hop) and a 5–10 ms tail when an
etcd cache miss occurs. p99 latency targets <5 ms become hard.

#### 1.5.3 Scaling ceilings — when each type runs out of room

| Type | Ceiling onset (typical) | What fails | What to do |
|---|---|---|---|
| **Type 1** | ~10–20 replicas behind a single LB | LB or NIC at the front | Multi-LB or anycast (see §6.5 multi-region) |
| **Type 2** | ~1M RPS aggregate (Valkey-cluster, sharded) | Valkey hot-key contention | **F1**: pre-shard hot keys; or accept eventual-consistency across shards |
| **Type 3** | ~5k–10k *control-plane writes/sec* | etcd quorum throughput; **does not scale by adding nodes** | **F2**: shard service registry; reduce churn; alt backend (Consul, raft-rs Phase 4.2) |
| **Type 4** | min of the two above | whichever fires first | apply Type 2 / Type 3 mitigations |

**Real-world targets (recommend documenting after Phase 1.2 soak):**

- Type 1 single-region: **1–2 M RPS** at 10 replicas *(unsourced inference)*
- Type 2 single-region: **500k–1 M RPS** with Valkey-cluster *(unsourced inference)*
- Type 3 single-region: **500k–1 M RPS** for read-heavy traffic; **<10k writes/sec** for control-plane churn
- Type 4 single-region: same as min(Type 2, Type 3)

Anything beyond these single-region targets requires multi-region (see §6.5).

#### 1.5.4 Two architectural sharp-edges to know

- **F1 — Valkey hot-key bottleneck.** A single popular API endpoint produces
  one rate-limit key. All RPS hits one shard. Cluster-mode doesn't help when
  the *key* is the same. Mitigation: when configuring UC04, decompose hot
  keys into N shards keyed on a hash prefix (e.g. `ratelimit:apikey:<sha1>:<shard>`)
  and aggregate counts at decision time. Adds ~0.1 ms but eliminates the
  bottleneck. Tracked as a Phase 0.C cookbook task.
- **F2 — etcd write-throughput ceiling.** No matter how many etcd nodes are
  added, write throughput is bounded by the leader's commit pipeline (~10k
  writes/sec). For UC12 service-discovery on a fleet with high pod churn
  (ephemeral microservices spinning up/down), this is the visible ceiling.
  Mitigation: namespace the registry (one etcd cluster per namespace) or
  switch to Consul for high-churn workloads (existing `src/discovery/consul.rs`
  client; reduces churn pressure via session-bound entries). UC03 ACME is
  fine — writes are rare.

---

## 2. Cluster-type details

### 2.1 Type 1 — Stateless

**Capability provided.** N replicas of highper-gateway, each independent. No external state stores. HA from front-LB redundancy.

**UCs supported (Group A — 7 UCs).** UC01, UC02, UC05, UC08, UC09, UC14, UC15. Source: research lines 12, 13, 16, 19, 20, 25, 26.

**Sizing for HA.**

| Mode | Components | HA at this size |
|---|---|---|
| Single-node dev | 1 highper | **No HA** — single point of failure; if the node dies, traffic stops. Acceptable for dev / staging only. |
| Multi-node prod | 2+ highper behind LB / VIP / Anycast | Yes — N+1 redundancy; tolerates 1 replica loss |
| Recommended cloud prod | 3 highper across 3 AZs + cloud LB | Yes — zone-tolerant; survives a full AZ outage |

**Common deployment scenarios** *(unsourced inference; based on Group A UC capabilities):*

- Pure L4 TCP load balancer (UC01)
- Simple HTTPS LB without ACME (UC02 + manual certs)
- Geo-routing edge (UC02 + UC15)
- Static + PHP-FPM web tier (UC14)
- HTTP/3 edge accelerator (UC05; bare metal preferred per research line 16)

**Pick Type 1 when:** UC selection is entirely from Group A.

### 2.2 Type 2 — Stateless + Valkey

**Capability provided.** N stateless highper replicas + a Valkey cluster (Redis-protocol-compatible, BSD-licensed, named in research line 5 as the reference Type B backend). Valkey provides: atomic counters (rate limits), shared k-v (sessions, schemas), pub-sub (cache purge), high write throughput with eventual consistency.

**UCs supported (Group A + Group B — 14 UCs).** Group A as above. Group B: UC04, UC06, UC07, UC10 (mixed Type A+B), UC11, UC13, UC16. Source: research lines 15, 17, 18, 21, 22, 24, 27.

**Sizing for HA.**

| Mode | highper | Valkey | Total nodes | HA story |
|---|---|---|---|---|
| Single-node dev | 1 | 1 (same host or sidecar) | 1–2 | "0 % fault tolerance on Valkey" — research line 29 |
| Small prod | 2 | 3 (Sentinel) | 5 | Tolerates 1 highper + 1 Valkey loss |
| Production | 3+ | 3 primary + 3 replica | 9+ | Read scaling + zone tolerance |

**Common deployment scenarios** *(unsourced inference):*

- API gateway with global rate limiting (UC02 + UC04)
- WebSocket gateway with sticky sessions (UC06)
- gRPC gateway with shared circuit-breaker state (UC07 + UC02)
- CDN edge cluster (UC02 + UC11)
- GraphQL federation tier (UC13)
- **AI/LLM gateway** (UC16) — primary UC16 deployment per research line 27 footprint "K8s / BM (GPU)"
- Hybrid multi-protocol (UC10 = UC01 + UC02 + UC06 with shared LB pool)

**Mixed-mode isolation rule.** When both UC04 (rate limit) and UC16 (AI token quota) are enabled, shard Valkey so high-frequency rate-limit keys don't contend with AI quota writes. Source: research line 40.

**Pick Type 2 when:** UC selection includes any Group B UC and zero Group C UCs.

### 2.3 Type 3 — Stateless + etcd

**Capability provided.** N stateless highper replicas + an etcd / Consul / Raft cluster. Strong consistency (linearizable reads, quorum writes). Lower throughput than Valkey; designed for *configuration state* and *leader election*, not hot counters.

**UCs supported (Group A + Group C — 9 UCs).** Group A as above. Group C: UC03, UC12. Source: research lines 14, 23.

**Sizing for HA.**

| Mode | highper | etcd | Total | HA story |
|---|---|---|---|---|
| Single-node dev | 1 | 1 | 2 | "0 % fault tolerance on etcd" — research line 29 |
| Production | 2+ | 3 (or 5) | 5+ | Quorum tolerates (N-1)/2 etcd failures |

**Common deployment scenarios** *(unsourced inference):*

- TLS terminator with cluster-wide ACME automation (UC02 + UC03)
- Service-discovery-driven HTTP LB (UC02 + UC12)
- Production HTTPS edge with discovered backends (UC02 + UC03 + UC12)

**Why not use etcd for Group B too?** etcd's quorum write path (~10 ms) is too slow for hot rate-limit counters that may see millions of writes/sec. Group B work is engineered for Valkey's 100k+ ops/sec single-node throughput. *(unsourced inference; based on etcd / Valkey performance characteristics.)*

**Pick Type 3 when:** UC selection includes any Group C UC and zero Group B UCs.

### 2.4 Type 4 — Stateless + Valkey + etcd

**Capability provided.** Type 2 + Type 3 combined. Both stores run side-by-side. Highper uses each for what it's good at.

**UCs supported.** All 16.

**Sizing for HA.**

| Mode | highper | Valkey | etcd | Total | HA story |
|---|---|---|---|---|---|
| Single-node dev | 1 | 1 | 1 | 3 | Both stores at "0 % FT" — research line 29 |
| Small prod | 2 | 3 | 3 | 8 | Quorum on etcd; Sentinel on Valkey |
| Production | 3+ | 3 primary + 3 replica | 3 (or 5) | 12+ | Zone-tolerant |

**Common deployment scenarios** *(unsourced inference):*

- **Production AI gateway with mTLS + ACME** (UC16 + UC03 + UC09) — the typical real-world UC16 deployment
- Full API platform: rate-limit + WAF + ACME + service discovery (UC04 + UC09 + UC03 + UC12 + UC02)
- "Replace LiteLLM/Portkey + cert-manager + rate-limiter all in one binary" (UC16 + UC03 + UC04)

**Two isolation rules to apply.**

1. **Valkey sharding for UC04 vs UC16** — research line 40.
2. **Failure-domain separation for Valkey vs etcd** — keep them on separate AZs / racks. Co-location turns a zonal failure into a both-layers outage. *(unsourced inference; standard distributed-systems practice.)*

**Pick Type 4 when:** UC selection mixes Group B and Group C UCs.

---

## 3. All-in-one mapping (every UC × every cluster type)

This is the single source of truth for UC support across cluster types. Group letter is from research lines 11–27.

| UC | Use case | Group | Type 1 Stateless | Type 2 +Valkey | Type 3 +etcd | Type 4 +V+E |
|---|---|---|:---:|:---:|:---:|:---:|
| UC01 | L4 TCP Proxy | A | ✅ | ✅ | ✅ | ✅ |
| UC02 | L7 HTTP/1.1 LB | A | ✅ | ✅ | ✅ | ✅ |
| UC03 | HTTPS / ACME / mTLS | C | ❌ | ❌ | ✅ | ✅ |
| UC04 | API GW / Rate Limiting | B | ❌ | ✅ | ❌ | ✅ |
| UC05 | HTTP/3 QUIC | A | ✅ | ✅ | ✅ | ✅ |
| UC06 | WebSocket LB | B | ❌ | ✅ | ❌ | ✅ |
| UC07 | gRPC Gateway | B | ❌ | ✅ | ❌ | ✅ |
| UC08 | Database LB | A | ✅ | ✅ | ✅ | ✅ |
| UC09 | WAF + mTLS | A | ✅ | ✅ | ✅ | ✅ |
| UC10 | Hybrid Multi-Protocol | A+B | ❌ | ✅ | ❌ | ✅ |
| UC11 | CDN Edge Cache | B | ❌ | ✅ | ❌ | ✅ |
| UC12 | Microservices Discovery | C | ❌ | ❌ | ✅ | ✅ |
| UC13 | GraphQL Gateway | B | ❌ | ✅ | ❌ | ✅ |
| UC14 | Static + PHP-FPM | A | ✅ | ✅ | ✅ | ✅ |
| UC15 | Geographic LB | A | ✅ | ✅ | ✅ | ✅ |
| UC16 | AI / LLM Gateway | B | ❌ | ✅ | ❌ | ✅ |
| **Total UCs supported** | | | **7** | **14** | **9** | **16** |

### 3.5 UC16 storage layer — a 5th component (added 2026-05-02)

UC16 has **two distinct state requirements** that the four-cluster-type model
alone does not capture:

1. **Hot-path counters** — token-quota writes per request. High volume,
   eventual-consistency-OK. **Lives in Valkey** (Type B layer per research
   line 27 footprint "K8s / BM (GPU)"; the four-type model handles this).
2. **Durable state** — virtual-key issuance, per-key budgets (month-to-date
   spend), usage records, audit logs. Low volume by comparison, but **must
   survive crashes** and provide accurate billing. **Not Valkey** — Valkey's
   default RDB persistence has a data-loss window; not appropriate for
   billing.

**Implication:** a UC16 deployment is *not* simply "Type 2" or "Type 4." It
is "Type 2 (or Type 4) **+ a UC16 state-store**." The state-store is a
separate component the operator deploys and configures.

**Decision recorded (UC16 design decision #4, 2026-05-02).** The
`AiStateStore` trait ships **three impls** — operator chooses per
deployment via `HIGHPER_AI_STATE_BACKEND`:

| UC16 deployment shape | Hot-path counters | Durable state | Total cluster components |
|---|---|---|---|
| **Single-node dev / small prod (default)** | local Valkey | **ReDB** (pure-Rust embedded; zero external deps) | 1 highper (ReDB embedded; Valkey beside it) |
| **Single-node prod (RocksDB alternative)** | Valkey | **RocksDB** (mature C++ embedded with Rust bindings; same paradigm as ReDB) | 1 highper (RocksDB embedded; Valkey beside it) |
| **Multi-node prod** | Valkey cluster | **ScyllaDB** (Cassandra-compatible, distributed, horizontally scalable) | highper replicas + Valkey cluster + ScyllaDB cluster |
| **Type 4 + ScyllaDB** | Valkey + etcd (for ACME / discovery) | ScyllaDB | highper + Valkey + etcd + ScyllaDB |

**Trait + impls (Phase 2.1 in `ROADMAP.md`):** `AiStateStore` trait at
`src/gateway/ai/state_store/mod.rs`. Three sibling modules: `redb.rs`,
`rocksdb.rs`, `scylladb.rs`. Selected at startup; no runtime switching.

**Single-node → multi-node transition** is an open follow-on (UC16 §12
#16): three options under consideration — (a) export tool that walks ReDB
and writes to ScyllaDB; (b) dual-write at the trait layer during a
transition window; (c) accept that the single-node deployment is throwaway
and operators reset state when scaling up. Recommended: **(a) export tool**,
shipped in Phase 3.

**ROI comparison (UC16 storage, `unsourced inference`):**

| Option | Single-node? | Multi-node HA? | Monthly cost (mid-size) | Operational complexity | Notes |
|---|---|---|---|---|---|
| **ReDB** (embedded, pure Rust) | ✅ | ❌ | $0 (local disk) | Lowest | Default for dev and single-node prod; matches highper's pure-Rust stack |
| **RocksDB** (embedded, C++ bindings) | ✅ | ❌ | $0 (local disk) | Low | Mature; widely deployed (etcd, CockroachDB, TiKV); operator picks if they already have RocksDB ops experience |
| **ScyllaDB** (distributed) | ✅ (single-node also runs) | ✅ | ~$300–800 (3-node cluster) | Higher | Horizontally scalable; right answer for multi-tenant high-volume |
| ~~PostgreSQL~~ | ~~✅~~ | ~~✅~~ | ~~~$200–500~~ | ~~Medium~~ | **Dropped from candidate list** — heavier than ReDB, not horizontally scalable like ScyllaDB. Can revisit as a 4th impl in Phase 4 if operator demand surfaces. |
| ~~sled / FoundationDB / Redis-AOF~~ | — | — | — | — | **Dropped from candidate list** during UC16 #4 decision. |

**Why these three.** Three impls covers the spectrum — pure-Rust embedded
(ReDB) for "no external deps, everything in one binary", mature embedded
(RocksDB) for ops familiarity, and distributed (ScyllaDB) for multi-node
scale. PostgreSQL and FoundationDB were considered and dropped per the
2026-05-02 design discussion: PostgreSQL is operationally heavier than
ReDB without giving the horizontal scale of ScyllaDB; FoundationDB has a
smaller community.

---

## 4. Per-UC profiles

For each UC: group letter, supported cluster types (from §3), LB algorithms available, infrastructure footprint preference (from research), single-node behaviour, scaling pattern.

LB algorithm citations:

- **TCP / L4 algorithms** (`src/tcp/mod.rs:203-224`): RoundRobin, LeastConnections, LeastResponseTime, ConsistentHash, IpHash, WeightedRoundRobin, Random — **7 algorithms**.
- **HTTP / L7 algorithms** (`src/config/schema.rs:450-460` and dispatch at `src/proxy/loadbalancer.rs:366-392`): RoundRobin, LeastConn, LeastResponseTime, Random, IpHash, ConsistentHash, PowerOfTwo, Geographic, Maglev — **9 algorithms**.
- **gRPC algorithms** (`src/grpc/mod.rs:114-125`): RoundRobin, LeastRequest, Random, PowerOfTwo, ConsistentHash — **5 algorithms**.
- **DSL grammar exposes** (`src/config/dsl_ast.rs:402-410`): RoundRobin, LeastConnections, LeastResponseTime, IpHash, Random, Weighted, ConsistentHash — **7 algorithms** (subset of HTTP set).

| UC | Group | Types supported | LB algorithms (cited) | Footprint | Single-node | Scaling pattern |
|---|---|---|---|---|---|---|
| **UC01** L4 TCP | A | 1 / 2 / 3 / 4 | TCP set (7) — `src/tcp/mod.rs:203-224` | All (research line 12) | No HA at N=1 | N replicas behind anycast / VIP / cloud-LB |
| **UC02** L7 HTTP/1.1 | A | 1 / 2 / 3 / 4 | HTTP set (9) — `src/config/schema.rs:450-460` | All (line 13) | No HA at N=1 | N replicas; supports active-active per research line 13 |
| **UC03** HTTPS / ACME / mTLS | C | 3 / 4 only | HTTP set (9) for proxy traffic; cert state lives in etcd | K8s/VM (line 14) | 0 % FT on etcd (line 29) | N highper + 3-node etcd; ACME order coordination via etcd lease |
| **UC04** Rate Limiting | B | 2 / 4 only | HTTP set (9) | All (line 15) | 0 % FT on Valkey (line 29) | N highper + Valkey cluster; counters in Valkey |
| **UC05** HTTP/3 QUIC | A | 1 / 2 / 3 / 4 | HTTP set (9) inherited | **Bare Metal** (line 16) | No HA at N=1 | UDP steering or anycast; kernel UDP perf matters *(unsourced inference)* |
| **UC06** WebSocket LB | B | 2 / 4 only | HTTP set (9); session-stickiness via Valkey | All (line 17) | 0 % FT on Valkey | Sticky-session backend; cookie-based or Valkey lookup |
| **UC07** gRPC | B | 2 / 4 only | gRPC set (5) — `src/grpc/mod.rs:114-125` | K8s/VM (line 18) | 0 % FT on Valkey | L7 steering per method; circuit-breaker state in Valkey |
| **UC08** DB LB | A | 1 / 2 / 3 / 4 | TCP set (7); protocol-aware (MySQL / PostgreSQL / Redis) | VM/BM (line 19) | No HA at N=1 | Per-conn pool per node; no shared state |
| **UC09** WAF + mTLS | A | 1 / 2 / 3 / 4 | HTTP set (9) | **Bare Metal** (line 20) | No HA at N=1 | Per-node policy engine; node-local decision |
| **UC10** Hybrid | A+B | 2 / 4 only | TCP (7) + HTTP (9) sets — operator picks per listener | All (line 21) | 0 % FT on Valkey | Multi-tier: stateless replicas with Valkey for the B-flavoured listeners |
| **UC11** CDN Cache | B | 2 / 4 only | HTTP set (9); cache lookup on top | **BM (Performance)** (line 22) | 0 % FT on Valkey | Cache purge bus via Valkey pub/sub |
| **UC12** Discovery | C | 3 / 4 only | n/a — provides backends to other UCs | K8s/VM (line 23) | 0 % FT on etcd | Watch-streaming from etcd / Consul; pushes updates |
| **UC13** GraphQL | B | 2 / 4 only | HTTP set (9) | K8s/VM (line 24) | 0 % FT on Valkey | Schema sync via Valkey; query cache shared |
| **UC14** Static + PHP-FPM | A | 1 / 2 / 3 / 4 | HTTP set (9) for routing; FastCGI per-node | VM/BM (line 25) | No HA at N=1 | Per-node FastCGI pool; shared FS for content |
| **UC15** Geo LB | A | 1 / 2 / 3 / 4 | HTTP set (9) — uses `Geographic` algorithm specifically | All (line 26) | No HA at N=1 | Per-node IP-DB lookup; GeoDNS / anycast in front |
| **UC16** AI/LLM Gateway | B | 2 / 4 only | HTTP set (9); provider-priority routing on top | **K8s / BM (GPU)** (line 27) | 0 % FT on Valkey | Token-quota counters in Valkey; durable state TBD (open gate) |

### 4.1 LB algorithms — per-replica vs cluster-consistent (added 2026-05-02)

Not every LB algorithm produces the same backend choice on every highper
replica. Some algorithms are **deterministic across replicas** (any replica
given the same input picks the same backend); others use **per-replica
local state** (each replica's choice depends on its own observation history).

This matters for HA: with N highper replicas, traffic distribution can drift
across replicas if the algorithm is per-replica. The drift is bounded but
real for "least connections" / "least response time" style algorithms.

| Algorithm | Determinism | Why |
|---|---|---|
| `RoundRobin` | **Per-replica** | Each replica keeps its own counter; replicas are not synchronised |
| `WeightedRoundRobin` | **Per-replica** | Same as above; the *weights* are global config but the rotation cursor is local |
| `Random` | Effectively cluster-consistent in aggregate | Distribution converges across N replicas if random source is unbiased |
| `IpHash` | **Cluster-consistent** | Same client IP → same backend on every replica (deterministic hash) |
| `ConsistentHash` | **Cluster-consistent** | Same key → same backend on every replica |
| `Maglev` | **Cluster-consistent** | Same input → same backend; minimal disruption on backend membership change |
| `LeastConnections` (L4) / `LeastConn` (L7) | **Per-replica** | Each replica counts only its own active connections; global least-connections requires shared state in Valkey (not implemented today) |
| `LeastRequest` (gRPC) | **Per-replica** | Same as above |
| `LeastResponseTime` | **Per-replica** | Each replica's rolling window is local |
| `PowerOfTwo` | **Per-replica** *(loosely)* | Each replica picks 2 random backends and chooses the less-loaded — local view only; effective in aggregate but not exact |
| `Geographic` | **Cluster-consistent** | Backend choice driven by client IP geolocation; deterministic given the same Geo DB |

**Operational implication.** For tightly-balanced backends (e.g. UC02 / UC07
upstreams behind highper Type 2), if drift is observable in monitoring,
either:

1. Switch to a cluster-consistent algorithm (`ConsistentHash`, `Maglev`).
2. Wait for Phase 3.2's "EWMA + retry budget" item which begins moving
   peer-scoring to a shared structure.
3. Accept the drift — it's bounded and self-correcting at the response level.

Future direction: a `StateStore`-backed "cluster-consistent least-connections"
implementation in Type 2/4 deployments. Tracked under Phase 3.2 cross-cutting
work in `ROADMAP.md`.

> **Note on DSL exposure.** The DSL grammar at `src/config/dsl_ast.rs:402-410`
> exposes 7 of the 9 HTTP algorithms (`Maglev` and `Geographic` not exposed via
> DSL today; only via YAML / TOML schema). Filed as a low-priority code fix in
> ROADMAP P1 hygiene; for now, DSL operators needing those algorithms must
> use YAML or TOML config.

---

## 5. Single-node → multi-node conversion

Same code, same configuration shape — only component count and addresses change. No code change needed to scale.

| Type | Single-node dev | Multi-node prod conversion path |
|---|---|---|
| **1** | 1 highper. `HIGHPER_CLUSTER_INFRA=single`. | Add replicas; put a LB / VIP / anycast in front. No env-var change to highper itself. |
| **2** | 1 highper + 1 Valkey on same host. `HIGHPER_CLUSTER_TYPEB_ADDRS=localhost:6379`. `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true`. | Stand up 3-node Valkey cluster. Update `HIGHPER_CLUSTER_TYPEB_ADDRS` to comma-list of cluster nodes. Set `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=false`. Restart highper. |
| **3** | 1 highper + 1 etcd. `HIGHPER_CLUSTER_TYPEC_ADDRS=localhost:2379`. `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true`. | Stand up 3-node etcd cluster. Update `HIGHPER_CLUSTER_TYPEC_ADDRS`. Set `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=false`. Restart. |
| **4** | 1 highper + 1 Valkey + 1 etcd. Both addrs point at localhost. `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true`. | Stand up both clusters. Update both addr lists. Set `_ALLOW_SINGLE_NODE=false`. Restart. |

**Migration path (zero-downtime).** Bring up the production storage clusters first, point one highper instance at them as a canary, validate, then migrate the rest. Highper itself is stateless across this transition — only configuration changes.

### 5.1 Cluster-aware configuration reload (added 2026-05-02)

Today's config reload (`src/config/{watcher,reloader}.rs`) is **per-node from
local files**. In a multi-node cluster of any type, this means each node
reloads its own copy of the config — typically pushed by the operator's
orchestrator (Helm rolling upgrade, Ansible playbook, etc.).

For Type 3 / Type 4 deployments, etcd already provides a natural channel
for config push. The Phase 4.2 `ConfigSource` trait extraction
(see `ROADMAP.md` §4.4 #8) opens the door for an etcd-driven config source:
operator pushes once to etcd; every highper node sees the change.

| Behaviour | Today | After §4.4 ConfigSource trait (Phase 4.2) |
|---|---|---|
| Config source | local YAML / JSON / TOML / DSL file | trait — file source today; etcd / xDS / GitOps as additional impls |
| Reload trigger | SIGHUP or watcher-detected file change | trait — file watcher today; etcd watch / xDS push / Git poll as additional impls |
| Cluster-wide consistency | operator-orchestrated (rolling) | etcd / xDS guarantees same version visible to every node |
| Type 3 / 4 advantage | none today | etcd is already deployed; reusing it for config push is "free" |

**Implication for Phase 0 hot-reload work.** The Phase 0 reload fixes
(Workstream 0.A wiring, 0.G admin-API config push) should land **without
assuming** a particular config source — they read `Settings`, which the
loader fills from whatever source is configured. The etcd-backed source
is then a Phase 4.2 add that doesn't disturb Phase 0 code.

---

## 6. Active/Standby vs Active/Active vs Anycast — front-LB patterns

Highper-gateway is stateless internally. *How* clients reach the cluster is an infrastructure choice, independent of cluster type.

| Pattern | How it works | When it applies | Failover time *(unsourced inference)* | Trade-offs |
|---|---|---|---|---|
| **VIP via Keepalived (VRRP)** — research line 33 | One node owns a Virtual IP; if it fails, VRRP elects another | Active/Standby; same L2 segment required | **~3 s** (default VRRP advertisement interval; tunable down to ~1 s) | Simple; predictable; only one node serves traffic at a time |
| **Active-Active with cloud LB (NLB / GLB / MetalLB)** — research line 13 | Cloud or K8s LB distributes connections across all nodes | Most cloud and K8s deployments | **~30 s typical** (cloud-LB health-check interval) | Standard pattern; LB itself is HA per cloud SLA; cost scales with traffic |
| **BGP Anycast** — research line 12 | Same IP advertised from multiple nodes; routers steer to nearest | Multi-region edge; UC01, UC05 (HTTP/3), UC15 | **10–60 s** (BGP convergence, depends on upstream peer keepalives) | Zero-downtime client routing once converged; lowest client RTT; **requires BGP peering with upstream router/ISP — not universal** |
| **K8s Service of type LoadBalancer** — research line 32 | Cloud-LB integration via K8s controller | K8s deployments | **~30–60 s** (kube-proxy health propagation + cloud LB recheck) | Familiar; same as cloud-LB above with K8s glue |
| **Kernel UDP steering** — research line 16 | XDP / eBPF flow-direct to specific replica | UC05 HTTP/3 specifically; bare metal | **<1 s** (eBPF map update + connection migration) | Highest throughput for QUIC; requires kernel skill |

**Per-UC pattern guidance** *(unsourced inference; based on UC characteristics):*

- UC01 / UC08 (L4): VIP (small) → Anycast (multi-region). Active/Active works behind a cloud-LB.
- UC02 / UC03 / UC04 / UC09 / UC11 / UC13 / UC15: Active/Active behind cloud LB or K8s Service.
- UC05 (HTTP/3): Anycast preferred; UDP steering on bare metal for highest throughput.
- UC06 (WebSocket): Active/Active with sticky-session backend (UC06 in Type 2 deployments uses Valkey for the affinity table).
- UC07 (gRPC): Active/Active behind L7 LB; cloud-LB or K8s Service typically.
- UC10 (Hybrid): combine — TCP listeners get one pattern, HTTP another, on the same instances.
- UC14 (PHP-FPM): VIP (single-region) or cloud-LB (multi-region); shared FS for the document root.
- UC16 (AI/LLM): K8s Service or cloud-LB; sticky-by-virtual-key not required (each request is independent unless using sessions per N23 in ROADMAP §4.6).

### 6.5 Multi-region (added 2026-05-02)

Sections 1–6 above describe **single-region** clusters. Multi-region adds a
second axis: how is state coordinated between regions, and how are clients
routed to the nearest healthy region?

#### 6.5.1 What changes in each cluster type

| Type | Multi-region behaviour | Key trade-off |
|---|---|---|
| **Type 1** | Easy — independent stateless clusters per region; client routing via GeoDNS or BGP anycast | No shared state means each region's behaviour is independent (no global rate limit, no shared cache) |
| **Type 2** | **Cross-region Valkey is hard.** Active-active replication adds latency that defeats Valkey's reason-for-being. Recommended: per-region Valkey + accept eventual consistency between regions for UC04 (rate limits) and UC11 (caches). UC06 sticky sessions need region affinity. | Global state becomes regional state; UC04 rate limit per-region not global |
| **Type 3** | etcd cross-region quorum **kills throughput** — quorum writes hop between regions. Recommended: per-region etcd cluster + cross-cluster sync for cert state (e.g. ACME orders dedup'd via DNS). | UC03 ACME orders may double-issue across regions without external coordination |
| **Type 4** | Worst of both — apply Type 2 + Type 3 multi-region trade-offs independently | Operationally complex; full mesh of N regions × 2 storage layers |

#### 6.5.2 Recommended multi-region patterns

| Pattern | When to pick | How |
|---|---|---|
| **GeoDNS + per-region clusters** | Most common starting point; UC15 fits naturally | DNS routes client to nearest region; each region is a self-contained cluster of any of types 1–4 |
| **BGP anycast + per-region clusters** | Multi-AZ-region edge (UC01 / UC05) | Same IP advertised from each region; routers steer to nearest; failover via BGP withdraw (10–60 s convergence) |
| **Active-passive cross-region** | Disaster recovery, regulated workloads | Primary serves all traffic; secondary on standby with replicated state (Postgres streaming repl for UC16; Valkey replica) |
| **Active-active per-region with eventual sync** | High-traffic global services | Each region is independent for hot path; durable state syncs eventually (UC16 budget reconciliation lags by minutes) |
| **Single-region + multi-AZ** *(not multi-region)* | The simpler answer when latency budget allows | Often the right answer; multi-region is expensive — only do it when single-region SLA is insufficient |

#### 6.5.3 UC16 multi-region considerations

UC16 deployments often span regions for client latency. Three patterns:

- **Per-region UC16, regional virtual keys.** Each region issues its own virtual keys; no cross-region budget enforcement. Simple but per-tenant budget is regional. Acceptable for some pricing models.
- **Per-region UC16, central durable state.** Token-quota counters (Valkey) are regional; virtual keys + budgets (Postgres) are central. Adds 30–100 ms latency for budget checks; can be amortized via local cache + async budget reconciliation. Probable default for v2.
- **Active-active full state.** Postgres multi-master (or Spanner / FoundationDB / CockroachDB). Highest consistency, highest cost. Phase 4 candidate.

#### 6.5.4 Open owner gate — multi-region commitment timing

`ROADMAP.md` §6 has a new owner gate #7 for this: **when** does multi-region
ship? Recommendation: not in v1.0 (Phase 1). Candidate for Phase 4.x
ecosystem work alongside xDS / K8s operator. Single-region-multi-AZ is the
v1.0 default, with explicit "multi-region pattern N is documented but not
shipped as a single-button deployment."

---

## 7. Per-infrastructure deployment notes

Every cluster type works on every infrastructure. Only the *bring-up* differs.

### 7.1 Kubernetes (K8s)

Per research line 32: "Native support for Active-Active and Type B/C through StatefulSets and operators."

| Type | K8s shape |
|---|---|
| 1 | `Deployment` for highper + HPA + `Service` (LoadBalancer or ClusterIP+MetalLB) |
| 2 | `Deployment` for highper + `StatefulSet` for Valkey (Bitnami chart or Valkey operator) + 2 `Service`s |
| 3 | `Deployment` for highper + etcd-operator (or k3s-embedded etcd) + 2 `Service`s |
| 4 | `Deployment` for highper + `StatefulSet` for Valkey + etcd-operator + 3 `Service`s |

**Note (corrects an earlier doc draft):** highper-gateway itself is **stateless** in all four types. Only the storage layers (Valkey, etcd) use `StatefulSet`. Highper uses `Deployment` for elastic horizontal scaling.

**Caveat — kernel-privilege features in K8s** *(unsourced inference; standard K8s constraint):*

- **kTLS / sendfile fast path** (`src/webserver/static_files.rs:313+` for UC14; planned for TLS in Phase 4.2) requires kernel privilege. K8s pods need `securityContext.privileged: true` or `hostNetwork: true` plus `NET_ADMIN` capability — none are managed-K8s defaults.
- **XDP / eBPF UDP steering** for UC05 needs the same privilege escalation; some managed K8s offerings (GKE Sandbox, EKS Bottlerocket) restrict it further.
- **Workaround:** for managed K8s, deploy UC05 / UC14 on bare metal or self-managed nodes; treat the K8s deployment as L7-only for those use cases.

### 7.2 Virtual Machines (VM)

Per research line 33: "Ideal for Active/Standby using Keepalived for a Virtual IP (VIP). Scaling is slower than K8s but more predictable."

| Type | VM shape |
|---|---|
| 1 | systemd unit on each VM + Keepalived VIP for active/standby (or external L4 LB for active/active) |
| 2 | highper VMs + dedicated Valkey VM cluster (3-node minimum for HA) + Sentinel for failover |
| 3 | highper VMs + dedicated etcd VM cluster (3 or 5 nodes) |
| 4 | highper VMs + Valkey cluster + etcd cluster (typically separate VM groups) |

**Provisioning.** Ansible playbook is the natural fit; ROADMAP Phase 1.3 includes Ansible templates for each type.

### 7.3 Bare Metal (BM)

Per research line 34: "Recommended for Type A (L4/L7 throughput) and AI/LLM Gateways (GPU access), offering up to 2x efficiency over virtualised environments."

| Type | BM shape |
|---|---|
| 1 | highper on dedicated nodes + BGP anycast (preferred for UC05 / UC15) or per-region LB |
| 2 | highper nodes + Valkey on dedicated nodes (low-latency interconnect, ideally same rack) — recommended for UC11 (BM Performance per research line 22) |
| 3 | highper nodes + etcd on dedicated nodes — sub-ms quorum writes for UC03 / UC12 when latency budget demands |
| 4 | highper nodes + Valkey nodes + etcd nodes; **keep Valkey and etcd on different failure domains** *(unsourced inference)* |

**UC16-on-BM specific** *(unsourced inference, supporting research line 27 "K8s / BM (GPU)"):* when self-hosted models live on adjacent GPU nodes, bare-metal placement of highper next to the inference cluster avoids network hop overhead per token — material at scale.

### 7.4 Cluster security baseline (added 2026-05-02)

The cluster types add new components — Valkey, etcd — that **expand the
attack surface**. None of the security measures below are optional in
production. Highper-gateway is being hardened (Phase 0 closes
release blockers B1–B14, including admin-API, OCSP, and distributed-rate-limit
issues — see ROADMAP §4.1); the storage layers it depends on must be hardened
independently and remain the operator's responsibility regardless.

#### 7.4.1 Required per-type security controls

| Control | Type 1 | Type 2 | Type 3 | Type 4 |
|---|---|---|---|---|
| TLS 1.2+ on all client-facing ports | ✅ | ✅ | ✅ | ✅ |
| mTLS for admin API access (UC09 fix in Phase 0.G) | ✅ | ✅ | ✅ | ✅ |
| **Valkey AUTH** (`requirepass` or ACL with role-scoped users) | — | ✅ **required** | — | ✅ **required** |
| **Valkey TLS** (in-transit encryption) | — | ✅ recommended | — | ✅ recommended |
| **Valkey network isolation** (private subnet / NetworkPolicy) | — | ✅ **required** | — | ✅ **required** |
| **etcd client cert mTLS** (every highper node has its own cert) | — | — | ✅ **required** | ✅ **required** |
| **etcd peer mTLS** (etcd-to-etcd cluster traffic) | — | — | ✅ **required** | ✅ **required** |
| **etcd RBAC** (per-prefix permissions; highper read-only on `/services`, write on `/acme`) | — | — | ✅ **required** | ✅ **required** |
| Secrets rotation policy (Valkey password / etcd certs) | — | ≤90 days | ≤90 days | ≤90 days |
| Egress restrictions from highper (which upstreams it can call) | recommended | recommended | recommended | recommended |
| Rate-limit on admin API (Phase 0.G + Phase 0.C) | ✅ | ✅ | ✅ | ✅ |

#### 7.4.2 What "required" means in practice

- **Valkey AUTH**: configure `HIGHPER_CLUSTER_TYPEB_AUTH` (env var to be added
  in Phase 0.J alongside the addrs); refuse to start if Type B is enabled
  with no AUTH set unless `HIGHPER_CLUSTER_ALLOW_INSECURE=true` (dev only).
- **etcd mTLS**: configure `HIGHPER_CLUSTER_TYPEC_CLIENT_CERT` /
  `_CLIENT_KEY` / `_CA` env vars; same refuse-to-start rule unless
  `HIGHPER_CLUSTER_ALLOW_INSECURE=true`.
- **Network isolation**: K8s `NetworkPolicy` denying ingress to Valkey/etcd
  from anywhere except highper pods; on VMs and bare metal, firewall rules
  with the same effect.

#### 7.4.3 Implementation tracking

These are folded into Phase 0:

- **Phase 0.J** (`Settings` scaffold) adds the auth + cert env vars listed above.
- **Phase 0.B** (TLS honesty) covers highper's own TLS hardening; Valkey/etcd certs are operator-provisioned.
- **Phase 0.C** (rate-limit / WAF safety) adds the admin-API rate limit.
- **Phase 1.5** (SBOM + DAST + ZAP) catches dep-chain vulnerabilities including Valkey/etcd client libraries.

A **`docs/SECURITY_CLUSTER_BASELINE.md`** companion is queued in Phase 1.3 with
hardening templates per cluster type (AUTH config, NetworkPolicy YAML,
firewall rules, secrets-rotation playbook).

### 7.5 Per-cloud-provider front-LB matrix (added 2026-05-02)

Not every front-LB pattern from §6 works on every infrastructure provider.
This matrix tells operators what's actually available where. Items below
are *(unsourced inference)* based on documented provider capabilities;
verify against current provider docs before deploying.

| Pattern (from §6) | AWS | GCP | Azure | On-prem / Bare metal | K8s (any cloud) |
|---|---|---|---|---|---|
| **VIP via Keepalived (VRRP)** | ❌ no L2 multicast on EC2; use NLB instead | ❌ no L2; use TCP/UDP LB | ❌ no L2; use Standard LB | ✅ standard | ⚠️ MetalLB (L2 mode requires bare-metal nodes; BGP mode is the cloud-friendly variant) |
| **Cloud LB Active-Active** | ✅ NLB (L4) / ALB (L7) | ✅ Network LB (L4) / HTTPS LB (L7) | ✅ Standard LB (L4) / Application Gateway (L7) | n/a | ✅ Service type=LoadBalancer (cloud-LB integration) |
| **BGP Anycast** | ⚠️ AWS Global Accelerator (managed anycast); raw BGP not available | ⚠️ Premium Tier global LB does anycast under the hood; raw BGP via partner interconnect | ⚠️ Front Door / Cross-region LB use anycast managed; raw BGP requires ExpressRoute | ✅ standard with upstream peer | ⚠️ MetalLB BGP mode requires bare-metal nodes |
| **K8s Service type=LoadBalancer** | ✅ provisions NLB or ALB (with annotations) | ✅ provisions Network LB | ✅ provisions Standard LB | ⚠️ requires MetalLB or similar | ✅ native |
| **Kernel UDP steering (XDP/eBPF)** | ⚠️ requires Bottlerocket or custom AMI; not on most managed K8s | ⚠️ similar; GKE Sandbox blocks XDP | ⚠️ similar | ✅ standard | ⚠️ host-network privileged pods only |

**Recommended defaults per cloud:**

- **AWS:** NLB (Type 1 / 4 L4), ALB (L7), or Global Accelerator (multi-region anycast); Keepalived **does not work**.
- **GCP:** Network LB (L4), HTTPS LB (L7); Premium Tier for global; Keepalived **does not work**.
- **Azure:** Standard LB (L4), Application Gateway (L7), Front Door (multi-region); Keepalived **does not work**.
- **On-prem / Bare metal:** Keepalived for small (Active/Standby); BGP anycast for larger / multi-region; cloud-LB n/a.
- **K8s on any cloud:** Service type=LoadBalancer is the default; MetalLB only when you control the underlying nodes.

**Operator pitfall to surface in cookbooks:** the "Type X × VM" cookbook
templates assume Keepalived works. **It doesn't on AWS / GCP / Azure.** The
Phase 1.3.1 VM cookbooks must include a "cloud-VM" sub-cell that uses the
cloud's LB instead of Keepalived. Tracked as a Phase 1.3.1 deliverable.

---

## 8. Decision flow for an operator

1. **List the UCs you want to enable.**
2. **Map each to its group letter** (A / B / C — see §3 mapping table).
3. **Take the union of letters** in your selection:
   - Only A → **Type 1**
   - A and B (no C) → **Type 2**
   - A and C (no B) → **Type 3**
   - A, B, and C → **Type 4**
4. **Pick your infrastructure** (K8s / VM / Bare Metal) — independent of cluster type.
5. **Pick a front-LB pattern** (§6) — independent of cluster type.
6. **Size your storage clusters** (§2.x table for the chosen type) based on whether you're dev or prod.
7. **Provision** using the Phase 1.3 cookbook for that (type × infrastructure) cell — see ROADMAP §4.5.

---

## 9. Open decisions (tracked in ROADMAP)

These remain open and are tracked in `ROADMAP.md` §6 owner gates and §11.3:

- **Type B backend default.** Valkey vs Redis — both speak the same protocol; the existing client at `src/cache/backends.rs:182-326` and `src/gateway/ratelimit/distributed.rs:150-204` works against either. Mostly a cookbook + CI choice.
- **Type C backend default.** etcd (already coded — `src/discovery/etcd.rs`) vs Consul (already coded — `src/discovery/consul.rs`) vs `raft-rs`-embedded (not yet coded). Recommendation: etcd as default, Consul as parity option, raft-rs Phase 4.2.
- **Peer-discovery responsibility.** Whether highper drives peer discovery via a `PeerDiscovery` trait (with `static` / `k8s_headless` / `dns` / `consul` impls), or strictly delegates to the chosen infrastructure.
- **UC16 storage backend** (the durable layer for virtual keys / budgets / usage records). Hot-path counters lean Valkey per research line 27; durable layer is open — PostgreSQL vs alternative. ROADMAP §6 gate #5.

---

## 10. References

- `docs/planning/research-on-HA-architecture-highper-gateway-16-deployment-usecases.txt` — original cluster-persona research (60 lines). Cited extensively above.
- `docs/planning/ROADMAP.md` — phased plan, blockers, gates. References this document from §11.
- `src/tcp/mod.rs:203-224` — TCP LB algorithms (7).
- `src/config/schema.rs:450-460`, `src/proxy/loadbalancer.rs:366-392` — HTTP LB algorithms (9).
- `src/grpc/mod.rs:114-125` — gRPC LB algorithms (5).
- `src/config/dsl_ast.rs:402-410` — DSL-exposed LB algorithms (7).
- `src/discovery/{consul, etcd, registry}.rs` — Type C backend implementations.
- `src/cache/backends.rs:182-326`, `src/gateway/ratelimit/distributed.rs:150-204` — Type B (Redis-protocol) consumers.

---

## 11. Document lifecycle

- **2026-05-02 (initial):** created. Cluster-type / UC-group / LB-algorithm-per-UC mapping derived from research file lines 11–27 + source verification of LB enums. Replaces the earlier in-line §11.5 / §11.6 / §11.7 draft in `ROADMAP.md` (which had several validation errors per `ROADMAP.md` §12 lifecycle entry); ROADMAP §11 now points here.
- **2026-05-02 (P0 + P1 review fold-in):** 360° architectural review surfaced 12 concerns (F1–F12); P0 + P1 fixes applied here:
  - **F1 + F2 + F7** → new §1.5 per-type performance envelope: throughput ceilings, latency adders, scaling onset, the Valkey hot-key bottleneck and etcd write ceiling called out explicitly.
  - **F6** → new §3.5 UC16 storage layer: durable state modelled as a 5th component separate from cluster type; cookbook-shape table for sled / Postgres / Scylla / FoundationDB / Redis-AOF.
  - **F3** → new §6.5 multi-region: cross-region behaviour per cluster type; recommended patterns (GeoDNS, BGP anycast, active-passive, active-active); UC16 multi-region considerations; ROADMAP §6 owner gate #7.
  - **F10** → new §7.4 cluster security baseline: required Valkey AUTH / etcd mTLS / network isolation per type; tracked in Phase 0.J / 0.B / 0.C / 0.G.
  - **F4** → §6 patterns table now has a "failover time" column (VRRP ~3 s, BGP 10–60 s, cloud-LB ~30 s, kernel UDP <1 s).
  - **F5** → new §4.1 LB algorithms per-replica vs cluster-consistent; operator implications; future direction toward shared-state LB in Type 2/4.
  - **F8** → new §5.1 cluster-aware configuration reload: today's per-node file watcher; Phase 4.2 ConfigSource trait opens etcd-driven push for Type 3/4.
  - **F9** → new §7.5 per-cloud-provider front-LB matrix: AWS / GCP / Azure don't support Keepalived; cloud-LB is the cloud default; BGP requires upstream peer.
  - **F11** → §4.1 footnote: DSL exposes 7 of 9 HTTP algorithms; Maglev/Geographic require YAML/TOML.
  - **F12** → §7.1 caveat: kTLS / XDP need kernel privilege not granted on managed K8s.
  - Tracking: ROADMAP Phase 1.2 gains a per-type RPS benchmark task; ROADMAP §6 gains owner gate #7 (multi-region commitment timing).
- **2026-05-02 (review fold-in — R1 + R2 + R5 + R8 / R3 + R4 + R6 + R7):** post-review consistency pass.
  - **R1** §9 line corrected: ROADMAP cross-reference §11.7 → §11.3 (the slim renumbered Open-Decisions section).
  - **R2** Phase 0.J in ROADMAP gains the cluster-security env vars: `HIGHPER_CLUSTER_TYPEB_AUTH`, `_TYPEB_TLS`, `_TYPEC_CLIENT_CERT/KEY/CA`, `_ALLOW_INSECURE` — making §7.4.3's "Phase 0.J adds these" claim accurate.
  - **R5** §2.1 sizing table + §4 per-UC profiles: "Full HA at N=1" replaced with "No HA at N=1 (single point of failure)" everywhere. N=1 is dev only.
  - **R8** §7.4 intro softened: "Highper-gateway is being hardened — Phase 0 closes B1–B14" instead of the overstated "is hardened".
  - **R3** ROADMAP Phase 1.3 gains `docs/SECURITY_CLUSTER_BASELINE.md` as a 3-day deliverable, making §7.4 closing claim accurate.
  - **R4** ROADMAP Phase 1.3.1 gains the cloud-VM sub-cells (`type-{a,b,c}-vm/cloud-vm/`) addressing §7.5's "Keepalived doesn't work on AWS/GCP/Azure" issue.
  - **R6** ROADMAP Phase 0.C gains a hot-key-sharding cookbook task with `HIGHPER_RATELIMIT_KEY_SHARDS` env var, addressing §1.5.4 F1.
  - **R7** ROADMAP Phase 1.6 gains a DSL-grammar extension task to add Maglev + Geographic, resolving the §4.1 footnote.
- **Future:** edit in place. Append revision entries here; do not silently rewrite without an entry.
