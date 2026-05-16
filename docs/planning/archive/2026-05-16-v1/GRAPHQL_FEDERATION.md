# GraphQL Federation — Deferred Design

**Status:** ⏸ **Deferred for the initial v1.0 release.** UC13 ships as
*GraphQL passthrough + introspection cache + per-backend stitching scaffold*
— no Apollo Federation v2 entity resolution, no cross-backend query plan, no
type-key joining.

**Owner-of-record:** Highper Gateway maintainer.
**Created:** 2026-05-02.
**Companion documents:**

- [`ROADMAP.md`](ROADMAP.md) §3.4 (UC13 status), §5 Phase 0.E (decision row),
  §5 Phase 4.2 (target re-engagement), §6 owner gates.
- [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) — unrelated, but
  the same documentation conventions apply.

Per `CLAUDE.md` rules: every concrete claim cites a source span; items I cannot
verify are marked `(unsourced inference)`.

---

## 1. Why this document exists

The 2026-05-02 audit (`docs/AUDIT_2026-05-02.md`) raised two GraphQL-related
release blockers:

- **B5** — depth/complexity not enforced; the schema *stitcher* is an
  acknowledged stub.
- The `SchemaStitcher::stitch_schemas` impl carries the comment
  "Simplified schema stitching - placeholder for future enhancement /
  Complex query analysis and splitting is not implemented yet" at
  `src/gateway/graphql/stitcher.rs:56-57` (verified 2026-05-02).

Phase 0.E in ROADMAP currently offers two choices:

> *"GraphQL stitcher: ship a real implementation OR explicitly mark UC13 as
> 'schema-pass-through, federation deferred to v1.1'. Decision day +
> execution. Recommended: defer; pull in a real Federation library in
> Phase 4."* — `docs/planning/ROADMAP.md` Phase 0.E

This document **records the deferral choice**, fences scope, and captures
the open design questions so Phase 4.2 starts with context, not a blank
page.

---

## 2. What ships in v1.0 (UC13 in-scope)

The GraphQL gateway in v1.0 is a *passthrough proxy with introspection cache
and per-backend pre-validation*. Specifically (cited from existing code):

- Schema registry per backend with introspection fetcher —
  `src/gateway/graphql/schema.rs:73-200+`.
- Query parser via `async_graphql_parser` —
  `src/gateway/graphql/mod.rs:170-182`.
- TTL response cache (DashMap + 60 s expiry sweeper) —
  `src/gateway/graphql/cache.rs:27-105`.
- Cache key = SHA256(query + variables) —
  `src/gateway/graphql/mod.rs:176-182, 268-272`.
- Federation-mode parallel fan-out with per-backend errors —
  `src/gateway/graphql/executor.rs:26-100`. **Note:** the fan-out runs queries
  *unmodified* against each backend; it does **not** split a query across
  backends or join entity keys.
- Batch request handling with `max_batch_size` cap —
  `src/gateway/graphql/mod.rs:299-316`.

Plus the depth+complexity work delivered in Phase 0.E (B5 blocker) — depth
and complexity analyzers wired to the parsed AST with early reject.

**v1.0 advertised behavior:** "Forward GraphQL queries to a backend; cache
identical queries by hash; pre-validate against the introspected schema
to fail fast; reject queries exceeding the configured depth or complexity
budget." Nothing more.

---

## 3. What is explicitly out of scope for v1.0

The following land in **Phase 4.2** ("Federated GraphQL — Apollo Federation v2
entity resolution") at the earliest:

| Capability | What's deferred | Why |
|---|---|---|
| Apollo Federation v2 entity resolution (`@key`, `@requires`, `@provides`, `@external`, `@shareable`) | All directives ignored; no entity-key joining across subgraphs | No production-grade Rust impl shipped; rolling our own would dwarf the rest of v1.0 |
| Cross-subgraph query plan (split a query across N subgraphs, fetch slices, merge by entity key) | Stub-only at `src/gateway/graphql/stitcher.rs:56-57` | Same |
| Query plan cache | Not applicable until query plan exists | — |
| `_service` / `_entities` federation entry-point handlers | Not exposed | Same |
| Composed supergraph schema (gateway exposes union of subgraphs as one schema) | Not built | Same |
| Schema-registry push/pull integration (Apollo Studio / GraphOS / Mercurius) | Not integrated | Vendor-specific; defer until trait extraction lands |
| GraphQL subscriptions over WebSocket | Tracked separately in Phase 3.2 (folded from coverage trace); subscriptions ship before federation | Different surface area |

Operators who need real federation in v1.x should run Apollo Router in front
of highper-gateway as the federation tier and use highper as the L7 / TLS /
WAF / rate-limit / observability tier (highper does the things Apollo Router
doesn't). This pairing is documented in the Phase 1.3 Deployment Guide
update.

---

## 4. Why we deferred (decision rationale)

1. **No vetted Rust Federation v2 library exists.** Building one to
   production quality is a multi-quarter effort — comparable to UC16 in
   scope. Folding it into v1.0 doubles the v1.0 timeline.
2. **Federation is rarely the v1 use case.** Most UC13 deployments today
   are single-backend (one GraphQL service behind a gateway for caching +
   rate-limit + auth + telemetry). Passthrough already addresses that.
3. **The interface-first refactor (§4.4 in ROADMAP) lands first.** Once the
   `LoadBalancerStrategy`, `RateLimiter`, and `MetricsBackend` traits are in
   place, federation can plug into clean seams instead of into concrete
   types.
4. **Apollo Router is a known, trusted alternative.** Operators can run it
   *behind* highper today (highper for L7/TLS/WAF; Apollo Router for
   federation) — composable rather than monolithic.

---

## 5. Open design questions (for Phase 4.2 owner gate)

These must be answered before any Phase 4.2 federation work begins.

1. **Library vs. in-house.** Adopt a Rust Federation v2 library when one
   matures (track `apollo-rs` evolution, `graphgate`, others), or write a
   minimal-but-complete subset in-house? Recommend: adopt; in-house only if
   no maintained option exists at Phase 4.2 start.
2. **Schema-registry storage.** Where do composed supergraph schemas live?
   Options: filesystem watch (current pattern), admin-API push, Apollo
   GraphOS pull, `ConfigSource` trait impl (§4.4 #8). Recommend: `ConfigSource`
   trait — fits the architecture.
3. **Query-plan cache key.** Hash of normalized operation + supergraph SDL
   version? Plan size budget? Eviction policy? Defer details to design time.
4. **Entity batching across subgraphs.** Inline collapse vs. dataloader-style
   batch resolver? Configurable? Defer.
5. **Subscriptions x federation.** v1.0 ships subscriptions over WS without
   federation (Phase 3.2). When federation lands in 4.2, do subscriptions
   federate? Likely yes via subgraph connection multiplexing; defer details.
6. **Observability shape.** Per-subgraph fan-out metrics already exist; add
   per-entity-type fetch latency? Defer to OTel exemplar work in 4.1.
7. **Compatibility story.** Apollo Router config format (router.yaml) vs.
   highper-native DSL? Recommend: native DSL primary; Apollo-config import
   helper as a separate tool, not a runtime mode.

---

## 6. Phase 4.2 acceptance criteria (target)

When federation work re-engages, the following must hold before tag:

- [ ] Composed supergraph schema published; `_service` and `_entities`
      handlers respond per Apollo Federation v2 spec.
- [ ] Cross-subgraph entity resolution passes the Apollo Federation v2
      compatibility test suite (or equivalent).
- [ ] Query plan cache hit rate ≥ 80 % under repeated load against fixture
      workloads.
- [ ] Per-subgraph fetch latency, error rate, and entity batch size emitted
      as metrics + OTel spans.
- [ ] DSL extension for federation routing block reviewed and merged.
- [ ] Cookbook entry: `examples/configs/scenarios/scenario-13-graphql-federation.{proxy,yaml}`
      with a 3-subgraph fixture.
- [ ] Migration guide: "Apollo Router → highper-gateway federation" published
      under `docs/MIGRATION_APOLLO_ROUTER.md`.

---

## 7. Status tracking

This document is intentionally **frozen until Phase 4.2 begins**. Updates
land here only when:

- Phase 0.E confirms the deferral decision (initial commit — this revision).
- Phase 4.2 owner gate opens and the design questions above are answered.
- An interim release adds a federation-adjacent capability (e.g.
  subscriptions over WS in Phase 3.2) — note in §3 above so readers know
  what shipped.

Per `ROADMAP.md` §10 lifecycle: substantive revisions go in §10 of ROADMAP,
not here. This document captures the *frozen* deferred-state design.

---

## 8. References

- `src/gateway/graphql/stitcher.rs:56-57` — placeholder comment confirming
  no implementation today.
- `src/gateway/graphql/{schema, executor, cache, mod}.rs` — what *does*
  ship in v1.0.
- `docs/planning/ROADMAP.md` §0.E (decision row), §3.4 (UC13 status), §4.1
  (B5 blocker), §5 Phase 4.2 (target re-engagement).
- `docs/AUDIT_2026-05-02.md` — original gap audit raising B5.
