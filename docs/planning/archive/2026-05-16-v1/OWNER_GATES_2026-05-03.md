# Owner Gates — Closing Decisions (2026-05-03)

**Owner-of-record:** Highper Gateway maintainer.
**Date closed:** 2026-05-03.
**Status:** All 6 open owner gates from `ROADMAP.md` §6 closed. **Phase 0 unblocked. Phase 2 unblocked pending Phase 0 completion.**

**Companion documents:**

- [`ROADMAP.md`](ROADMAP.md) §6 — each gate marked DECIDED with reference back here.
- [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) — UC16 scope fence (gate #3) and 12-topic decision sequence.
- [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) — HA architecture (gate #4) and multi-region patterns (gate #7).
- [`GRAPHQL_FEDERATION.md`](GRAPHQL_FEDERATION.md) — UC13 federation deferral (gate #2).

**Pattern for future closures.** When new owner gates are opened in `ROADMAP.md` §6 and later closed, batch the closures into `docs/planning/OWNER_GATES_YYYY-MM-DD.md` files following this doc's structure. `ROADMAP.md` §6 stays as the live tracker; the dated batch files are the version-controlled audit trail.

Per `CLAUDE.md` rules: every concrete claim cites a source span (file path with line range, or a quoted span from the input). Items I cannot directly cite are marked `(unsourced inference)`.

---

## Gate-by-gate decisions

### §6 #1 — Phase 0 priority + Rancher Desktop + Phase 1.5 tool stack + SAST

**The question (per `ROADMAP.md` §6 #1, opened 2026-05-02):** confirm Phase 0 priority order — agree all 14 blockers (B1–B14) are in scope, or strike specific items. Confirm Rancher Desktop choice and Phase 1.5 tool stack.

**Decision (owner ack 2026-05-03):**

> Phase 0 starts with all 14 blockers in scope; Rancher Desktop confirmed; Phase 1.5 ships Trivy + syft+Grype + Dastardly + OWASP ZAP.

**Plus SAST addition (owner-raised question 2026-05-03):**

The owner asked specifically about SAST (Static Application Security Testing). SonarQube was considered as the SAST anchor and **dropped** — too resource-heavy for the Rancher Desktop dev / test environment (4+ GB RAM persistent Java server). Instead: a tiered SAST strategy with lightweight CLI tools running locally + on every PR; heavier deep-analysis cloud-side via GitHub Actions with zero local resource cost.

| Tier | Tool | Where it runs | When |
|---|---|---|---|
| A | `cargo-clippy` | local + CI (`-D warnings`) | every PR |
| A | `cargo-audit` (RustSec advisory DB) | local + CI | every PR |
| A | `cargo-deny` (license + advisory + ban + source policies) | local + CI | every PR |
| A | `cargo-geiger` (unsafe-code budget + threshold) | local + CI | every PR |
| A | Semgrep (`p/rust` + `p/security-audit` rulesets) | local + CI; ~30 s scan | every PR |
| B | CodeQL via GitHub Advanced Security | GH cloud — zero local resource | every PR + nightly |
| C (optional) | `cargo-vet` (supply-chain vetting) | local + CI | curated process |
| C (optional) | Custom `dylint` rules (e.g. forbid `std::env::var` outside `src/config/`) | local + CI | every PR |

**Why not SonarQube:** would consume the bulk of the Rancher Desktop dev-env memory budget (~4 GB persistent Java server + Postgres backend), interfering with the primary dev workflow (highper itself + dependent backends like Valkey / etcd / Postgres / Consul). The tiered Rust-native + cloud-side stack covers the same threat model with ~100 MB combined RSS for the CLI tools and zero local burden for CodeQL.

**Rationale for the rest:**

| Sub-decision | Rationale |
|---|---|
| All 14 blockers in scope | Each has a verified `path:LINE` citation in `ROADMAP.md` §4.1 (B1 `src/runtime/mod.rs`, B2 `src/admin/api.rs:5,125,141`, B3 `src/tls/ocsp_fetcher.rs:346-415` etc through B14). Striking any without addressing the underlying issue would ship known-broken behaviour. The scheduled `gap-auditor` cron would flag a silent demotion within a month. |
| Rancher Desktop | Owner stated 2026-05-02 ("Let us use Rancher Desktop as development & local test environment"). Containerd + nerdctl flow validated on owner's other projects per same turn. |
| Trivy + syft+Grype + Dastardly + ZAP | Owner stated 2026-05-02 ("do check SBOM using Trivy, syft+Grype and DAST using Dastardly and security scan using OWASP's ZAP"). Two-generator SBOM (Trivy + syft+Grype) catches mutual inconsistencies; two-scanner DAST (Dastardly = Burp lineage, ZAP = OWASP) catches both auth-flow + injection patterns. |

**What this unblocks:** Phase 0 release-blocker work (B1–B14 across workstreams 0.A → 0.J). Phase 1.5 SBOM + DAST + SAST baseline. Pre-Phase-0 prereqs (Rancher Desktop install + `compose.yml` for full multi-service local dev).

**Tracking:** `ROADMAP.md` §5 Phase 0 + §1.5 (with new §1.5.SAST sub-section).

---

### §6 #2 — UC13 GraphQL federation deferral

**The question (per `ROADMAP.md` §6 #2, opened 2026-05-02):** does v1.0 launch with UC13 (GraphQL) marked "passthrough only, federation deferred"? Or block on shipping real federation in Phase 1?

**Decision (owner ack 2026-05-03):**

> v1.0 ships UC13 with federation deferred to Phase 4.2 per `GRAPHQL_FEDERATION.md`.

**Rationale:**

- Apollo Federation v2 entity resolution + cross-subgraph query plans are multi-quarter work. No production-grade Rust impl exists today; in-house build comparable in scope to UC16. Bundling it into v1.0 doubles the v1.0 timeline.
- The federation design + 6 acceptance criteria are already captured in `GRAPHQL_FEDERATION.md` §6 for Phase 4.2 re-engagement. The deferred-state design note pattern keeps the work visible without compelling calendar commitment.
- Operators with federation needs at v1.0 have a clean migration path: run Apollo Router behind highper. Highper provides L7/TLS/WAF/rate-limit/observability; Apollo Router does federation. Composable rather than monolithic.

**What v1.0 ships for UC13:**
- Schema registry per backend with introspection fetcher
- Query parser via `async_graphql_parser`
- TTL response cache (DashMap + 60 s expiry sweeper) with SHA256(query + variables) key
- Federation-mode parallel fan-out with per-backend errors *(unmodified queries against each backend; no entity-key joining)*
- Batch request handling with `max_batch_size` cap
- **Phase 0.E** depth + complexity analyzers wired to parsed AST with early reject (this satisfies B5 in §4.1)

**What v1.0 does NOT ship for UC13:**
- Apollo Federation v2 directives (`@key`, `@requires`, `@provides`, `@external`, `@shareable`)
- Cross-subgraph query plan
- Composed supergraph schema
- `_service` / `_entities` federation entry-point handlers
- Schema-registry push/pull (Apollo Studio / GraphOS / Mercurius)

**Phase 4.2 acceptance criteria (deferred work; from `GRAPHQL_FEDERATION.md` §6):**

- [ ] Composed supergraph schema published; `_service` and `_entities` handlers respond per Apollo Federation v2 spec.
- [ ] Cross-subgraph entity resolution passes the Apollo Federation v2 compatibility test suite (or equivalent).
- [ ] Query plan cache hit rate ≥ 80 % under repeated load against fixture workloads.
- [ ] Per-subgraph fetch latency, error rate, and entity batch size emitted as metrics + OTel spans.
- [ ] DSL extension for federation routing block reviewed and merged.
- [ ] Cookbook entry: `examples/configs/scenarios/scenario-13-graphql-federation.{proxy,yaml}` with a 3-subgraph fixture.
- [ ] Migration guide: `docs/MIGRATION_APOLLO_ROUTER.md` published — "Apollo Router → highper-gateway federation".

**What this unblocks:** v1.0 tag narrative consistency; UC13 ships at v1.0 in passthrough mode, not blocked on federation. Phase 4.2 has a documented federation re-engagement plan with explicit acceptance criteria.

**Tracking:** `GRAPHQL_FEDERATION.md` (frozen-state design doc); `ROADMAP.md` §3.4 (UC13 status), Phase 0.E (federation deferred), Phase 4.2 (federation re-engagement task).

---

### §6 #3 — UC16 scope fence + design-doc revision

**The question (per `ROADMAP.md` §6 #3, opened 2026-05-02):** confirm the §3.1 in-scope/out-of-scope fence; revise `USECASE_16_AI_LLM_GATEWAY.md` to remove out-of-scope items (guardrails, AI observability product, in-memory cache product, vLLM); answer remaining design questions that survive the fence. **Phase 2 cannot begin without this.**

**Decision (owner ack 2026-05-03):**

> UC16 scope fence stands; design doc consistent with fence; Phase 2 unblocked pending §6 #1 + #4.

**Verification of doc-fence consistency:**

| Pass | Date | Commit | Findings |
|---|---|---|---|
| Pass 1 (cross-doc consistency) | 2026-05-03 | `060d943` | 16 issues found; 8 high+medium fixed |
| Pass 2 (architectural soundness + configurability + cookbook conventions) | 2026-05-03 | `2bd09d0` | 14 issues found; 6 R1–R6 recommendations applied |
| Pass 3 (to-do-list comprehensiveness validation) | 2026-05-03 | `b017cc7` | F1 / F2 / F3 fixes applied; 22 medium / low gaps deferred to implementation surface |

Memory `uc16_scope.md` records the fence rules (in-scope = LiteLLM/Portkey gateway role; out-of-scope = guardrails / vLLM / AI observability product / in-memory cache product). The 12-topic UC16 design sequence (decisions #1–#12) reconciled all open §12 questions either to DECIDED status or to phase-gate placement.

**The fence (re-stated for the record):**

In scope:
- Multi-provider routing & translation (OpenAI / Anthropic / Bedrock / Vertex / Azure / Cohere / Mistral / xAI / DeepSeek / Groq / Together / Fireworks / …)
- Virtual API keys + per-key budgets, rate limits, usage caps
- Provider fallback & retry orchestration
- Token + cost metering and accounting
- Semantic cache *engine* + exact cache (cache *interface* only)
- SSE / streaming pass-through and aggregation
- Single-node up to multi-node cluster topology
- Cookbook + INTEGRATION_GUIDE.md operator docs

Out of scope:
- **Guardrails** (input/output content filtering, PII redaction, jailbreak detection) — wired via `src/plugin/` hooks; operator's choice
- **vLLM / self-hosted model serving** — operator's inference backend; UC17 forward-compat via `AiProvider` plugin
- **In-memory cache integration as a product feature** — cache *interface* exists; concrete in-memory product is operator's
- **AI observability platforms (Langfuse, Helicone-dashboards, Phoenix, Arize)** — highper exposes Prometheus + OTLP; the *platform* layer is external

**What this unblocks:** Phase 2 start — once §6 #1 (Phase 0 priority) and §6 #4 (HA sub-decisions for Phase 1.4 PeerDiscovery + MetricsBackend / LogBackend traits) implementations land, UC16 work begins at Phase 2.1. `USECASE_16_AI_LLM_GATEWAY.md` §0.2 day-one setup checklist + §10.5 DSL grammar reference give the implementing engineer a single starting point.

**Tracking:** `USECASE_16_AI_LLM_GATEWAY.md` (1934 → 2207 lines via the 12-topic sequence + 3 gap-audit passes). `ROADMAP.md` §5 Phase 2.1–2.6.

---

### §6 #4 — HA architecture sub-decisions

**The question (per `ROADMAP.md` §6 #4, revised 2026-05-02):** three open sub-decisions remained after the four-cluster-type model was accepted: (i) which Type B backend the cookbooks default to and which CI exercises; (ii) which Type C backend ships first as a code path; (iii) whether highper drives peer discovery for clustering or delegates to the chosen infrastructure.

**Decision (owner ack 2026-05-03):**

> Type B default Valkey; Type C default etcd; PeerDiscovery as a trait with static / k8s_headless / dns / consul impls.

**Detail per sub-decision:**

#### (i) Type B backend default — **Valkey**

- **What ships:** Valkey is the default in cookbooks (Phase 1.3.1 + Phase 2.6 UC16 scenarios). CI exercises Valkey for Type B integration tests. **Both** Valkey and Redis cookbook'd — operators on either path get a working example.
- **Code change:** none. Existing `src/cache/backends.rs:182-326` Redis client speaks both protocols (Redis-protocol-compatible). "Valkey-default" is a docs + CI choice.
- **Rationale:** Valkey is BSD-licensed (per HA research line 5); Redis upstream switched to RSALv2/SSPLv1 then AGPLv3, and operators have moved to Valkey. Choosing Valkey as the cookbook default reflects the trajectory the open-source ecosystem is on.

#### (ii) Type C backend default — **etcd**

- **What ships:** etcd as the cookbook + CI default. Consul as parity option (already coded in `src/discovery/consul.rs`). raft-rs-embedded deferred to Phase 4.2.
- **Code change:** none. `src/discovery/etcd.rs` already exists with prefix queries, polling, in-memory cache.
- **Rationale:** etcd is mature, widely deployed (Kubernetes uses it for control plane). Consul has HashiCorp BSL since v1.18 — operators avoiding BSL prefer etcd. raft-rs is greenfield Rust embed (worth Phase 4.2 for embedded-only deployments without external etcd, but not v1.0 critical-path).

#### (iii) Peer-discovery responsibility — **`PeerDiscovery` trait**

- **What ships:** new trait at `src/discovery/peer_discovery.rs` (or similar) with four impls: `static` (operator declares peer list), `k8s_headless` (DNS-resolves K8s headless service), `dns` (SRV records or A-record list), `consul` (Consul service discovery for highper itself).
- **Phase placement:** Phase 1.4 cross-cutting catch-up. ~5 days of additional work.
- **Rationale:** matches the plugin/extensibility theme of every other trait extracted in §4.4 (LoadBalancerStrategy / RateLimiter / CircuitBreaker / AuthProvider / ConnectionPool / GeoProvider / MetricsBackend / LogBackend / ConfigSource / AiStateStore / AiProvider / VectorIndex). Strict-delegate-to-infrastructure was considered and dropped because it would force operators to pick K8s-specific tooling — bare-metal operators would have no peer discovery and be forced to maintain peer lists manually. The trait gives all deployment types automatic peer discovery.

**What this unblocks:** Phase 4 enterprise scope (`PeerDiscovery` trait extracts in Phase 1.4; Type B/C backend choices flow into Phase 1.3.1 cookbooks and Phase 1.5 CI integration tests). Confirms the multi-node story for Type 2 and Type 4 deployments is actionable, not theoretical.

**Tracking:** `HA_ARCHITECTURE.md` §1 (four cluster types) + §11 (cluster-bootstrap config shape). `ROADMAP.md` §4.4 row 12 (will be added — `PeerDiscovery` becomes the 12th interface-first trait).

> **Note:** This decision adds a 12th row to the §4.4 interface-first audit table. Total trait extractions: 11 → 12 person-weeks → ~12 person-weeks. Tracked as a Phase 1.4 task addition.

---

### §6 #6 — Re-confirm §0.5 banners at v1.0 GA tag time

**The question (per `ROADMAP.md` §6 #6, opened 2026-05-02):** confirm §0.5 reconciliation banners are still consistent with then-current code at GA tag.

**Decision (owner ack 2026-05-03):**

> Process confirmed. The scheduled docs-keeper weekly cron will catch most drift; we re-baseline at GA tag.

**Rationale:** This is a process gate, not a decision gate. It cannot close *now* — the action is at-tag time. The closure here records the process commitment.

**Process:**

1. Scheduled `docs-keeper` cron (`trig_017YZKK1gLdJNntEAcSqVE7H`, fires weekly Sunday 06:13 IST) runs the §0.5 banner-consistency check between now and v1.0 GA. Drift between banners and reality is flagged as a routine-run issue; operator addresses before next docs-keeper firing.

2. At v1.0 GA tag time, re-run the §0.5 reconciliation check explicitly:
   - `KNOWN_LIMITATIONS.md` banner — drop the "v1.0-rc, see ROADMAP §4.1 for blockers" callout; replace with "v1.0 GA notes: B1–B14 closed".
   - `README.md` banner — drop the "v1.0-rc release-status callout"; replace with v1.0 GA reference.
   - `CHANGELOG.md` banner — re-classify the historical `[1.0.0]` entry from "v1.0-rc" back to "v1.0" with a pointer to the actual GA release notes.
   - `docs/ARCHITECTURE.md` banner — drop "supersedes after refactors land"; replace with "supersedes ARCHITECTURE_v2.md, published $date".

3. The scheduled `gap-auditor` cron (`trig_012cxCxcDsxugaqdJXB6syj2`, fires monthly on day 1, 06:07 IST) verifies the post-GA banners are still consistent on its monthly run.

**What this unblocks:** v1.0 GA tag (when Phase 0 + Phase 1 complete). The process is in place; the actual reconciliation runs at tag time.

**Tracking:** `ROADMAP.md` §0.5 (reconciliation banner record); cloud routine schedule (gap-auditor + docs-keeper).

---

### §6 #7 — Multi-region commitment timing

**The question (per `ROADMAP.md` §6 #7, opened 2026-05-02):** decide *when* multi-region ships as a single-button deployment.

**Decision (owner ack 2026-05-03):**

> Multi-region deferred to post-v1.0 RFC; single-region-multi-AZ remains the v1.0 default.

**Rationale:**

- Multi-region is a meaningful operational commitment. Per `HA_ARCHITECTURE.md` §6.5.3, the realistic patterns are: per-region UC16 with central durable state (~30–100 ms latency for cross-region budget checks; probable v2 default) or active-active full state (Postgres multi-master / Spanner / FoundationDB / CockroachDB; highest consistency, highest cost; Phase 4+).
- Single-region-multi-AZ already covers most operators. The data-locality, HA, and latency profile of multi-AZ is sufficient for v1.0 target deployments.
- Committing to multi-region in Phase 4 risks under-delivering (if RFC reveals deeper requirements than anticipated) or over-investing (if operator demand doesn't materialize). RFC keeps the design documented but withholds calendar commitment until real demand surfaces.
- Phase 4.1 / 4.2 candidate slot remains documented in `ROADMAP.md` §6 #7 and `HA_ARCHITECTURE.md` §6.5.4 — fast-tracking is possible if the RFC concludes early.

**What v1.0 ships:** single-region-multi-AZ as the default. Documented patterns in `HA_ARCHITECTURE.md` §6.5.3 (GeoDNS + per-region clusters; BGP anycast + per-region clusters; active-passive cross-region; active-active per-region with eventual sync; single-region + multi-AZ as the simple answer).

**What v1.0 does NOT ship:**
- A single-button multi-region deployment
- Cross-region budget enforcement for UC16 (per `USECASE_16_AI_LLM_GATEWAY.md` §3.6.6)
- Active-active state replication for UC16

**Operators wanting multi-region at v1.0** deploy per-region highper instances independently with GeoDNS or BGP anycast in front. Per-region budgets are regional (no cross-region aggregation). This is the documented pattern; not a "single-button" experience but functional.

**RFC timing:** post-v1.0; opened by owner-side decision when operator demand surfaces (3+ qualified operator requests, or a specific commercial commitment).

**What this unblocks:** Phase 4 calendar commitment is freed from a multi-region anchor. Phase 4.1 + 4.2 deliverables (xDS + K8s operator + ConfigSource + 18 N4.2.N items) can ship without waiting for multi-region resolution. Multi-region work happens *parallel* to or *after* Phase 4 ecosystem work, on RFC's timeline.

**Tracking:** `HA_ARCHITECTURE.md` §6.5.3 (patterns documented); `USECASE_16_AI_LLM_GATEWAY.md` §3.6.6 (UC16-specific multi-region considerations); `ROADMAP.md` §6 #7 (gate + RFC trigger condition); separate RFC doc opened when triggered.

---

## Net summary — what's now unblocked

| Dependency | Status before 2026-05-03 | Status after |
|---|---|---|
| Phase 0 release-blocker work (B1–B14) | Blocked on §6 #1 owner ack | **Unblocked** |
| Phase 0.J Settings env-var scaffold | Blocked on §6 #1 + #4 | **Unblocked** (PeerDiscovery env vars added per §6 #4) |
| Phase 1.5 SBOM + DAST + SAST + UC16 CI matrix | Blocked on §6 #1 tool-stack ack | **Unblocked** (with tiered SAST stack) |
| v1.0 GA tag narrative consistency for UC13 | Blocked on §6 #2 | **Unblocked** (federation deferred per `GRAPHQL_FEDERATION.md`) |
| Phase 2 UC16 implementation | Blocked on §6 #3 + #1 + #4 | **§6 #3 + #4 cleared; pending Phase 0 completion** |
| Phase 4 enterprise scope (xDS + K8s operator + ConfigSource + federation re-engagement) | Blocked on §6 #4 + #2 | **Unblocked** |
| Multi-region UC16 single-button deployment | Open without phase placement | **Deferred to post-v1.0 RFC** |
| §0.5 banner consistency at v1.0 GA | Process not committed | **Process confirmed** (cron + at-tag re-baseline) |

---

## What's NOT unblocked (still pending)

- **Push local commits to GitHub origin** — operator action, not a gate.
- **Provision Rancher Desktop env + `compose.yml`** — Phase 0 prereq work, ~1.5 days.
- **Phase 0.J Settings scaffold first** — ~7 days, must precede other Phase 0 workstreams.
- **Phase 0 workstreams 0.A → 0.I in parallel** — ~3–5 calendar weeks once Phase 0.J settles.

---

## Document lifecycle

- **2026-05-03 (initial):** All 6 owner gates closed in one batch; this doc records the closures with full rationale.
- **Future:** new owner gates opened in `ROADMAP.md` §6 will be batched and closed in subsequent `OWNER_GATES_YYYY-MM-DD.md` docs following this template. This doc stays frozen as the 2026-05-03 audit trail.
