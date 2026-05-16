# Archive — 2026-05-16 (v1 planning cycle)

This directory archives the planning, design, and status-tracking
documents from the v1 planning cycle (2026-01 → 2026-05-16). The
fresh roadmap that supersedes them is
[`docs/planning/ROADMAP.md`](../../ROADMAP.md) (v2; per-UC
organisation; restarted 2026-05-16).

## Why archive

After three review passes against `docs/planning/ROADMAP.md` (which
had grown to 3,962 lines + 42 §12 lifecycle entries) the document
became too dense to track at-a-glance. The user decided to:

1. **Archive the v1 planning artifacts** for historical reference.
2. **Restart with a fresh, per-UC-organised roadmap** that derives
   remaining work directly from architectural review against the
   15 deployment use cases (UC1–UC15).
3. **Defer UC16 (AI/LLM Gateway) vertical implementation** to v1.x;
   only the platform-extensibility shipped in `runtime_config::ai`,
   the cross-subsystem validator, and the §4.4 trait scaffolding
   remain in v1.0 GA scope.

The new ROADMAP.md is the single source of truth going forward. Old
cross-references inside the archived documents are preserved as-is
but are no longer load-bearing — read them only for historical
context, not current state.

## What's archived

### Planning docs (originally `docs/planning/`)

| File | Original purpose | Status now |
|---|---|---|
| `ROADMAP_v1.md` | 2026-05-02 → 2026-05-16 roadmap; 42 §12 lifecycle entries; 14 B-blockers; 8 D-flags; M1–M4 GA-scope tier | Superseded by v2 ROADMAP.md |
| `OWNER_GATES_2026-05-03.md` | Closure log for 6 owner gates (#1 Phase 0 scope; #2 UC13 federation deferral; #3 UC16 scope fence; #4 HA Type B/C + PeerDiscovery; #5 UC16 storage; #6 GA-tag re-baseline process; #7 multi-region) | Historical reference; all 7 gates were closed 2026-05-03 |
| `SETTINGS_SCAFFOLD.md` | Workstream 0.J `RuntimeConfig` design sign-off; 7 decisions captured | Implementation shipped; design preserved here |
| `RUNTIME_CONFIG_STAGE1_PR_PLAN.md` | Stage 1 PR contract (cluster + plugin sections); landed in `6897310` + `c8e1e8c` + `c9f1304` | Implementation shipped 2026-05-03 |
| `RUNTIME_CONFIG_STAGE2_PR_PLAN.md` | Stage 2 PR contract (ai + body + shutdown + secrets); landed in `67bf863` | Implementation shipped 2026-05-03 |
| `RUNTIME_CONFIG_STAGE3_PR_PLAN.md` | Stage 3 PR contract (9 remaining sections + Tier 1 SIGHUP + admin endpoints); landed in `9d7dc1e` + `e064b72` + `20aa599` + `471a336` + `7c00488` | Implementation shipped 2026-05-03 |
| `USECASE_16_AI_LLM_GATEWAY.md` | UC16 (AI/LLM Gateway) full design; 12 decisions recorded across §1–§12 | UC16 vertical deferred to v1.x; design preserved for v1.x kickoff |
| `HA_ARCHITECTURE.md` | High-availability design: 4 cluster types (Stateless / +Valkey / +etcd / +Valkey+etcd); per-UC profiles; per-infrastructure deployment notes; 12 architectural concerns F1–F12 | Type B = Valkey + Type C = etcd decisions implemented in `runtime_config::cluster`; per-cluster cookbook cells queued in v2 ROADMAP |
| `GRAPHQL_FEDERATION.md` | Frozen-state design for UC13 Apollo Federation v2 | Deferred to Phase 4.2 / v2.x; v1.0 ships UC13 passthrough + introspection cache + B5 depth/complexity only |

### Status / tracking docs (originally `docs/`)

| File | Original purpose | Status now |
|---|---|---|
| `TODO.md` | Pre-cycle todo aggregation | Superseded by v2 ROADMAP per-UC sections |
| `TODO_COMPREHENSIVE.md` | 80-TODO sweep catalog | Open items folded into v2 ROADMAP Phase 1.6 |
| `AUDIT_2026-05-02.md` | 714-line gap audit that founded the B1–B14 list | Historical reference; UC-state findings re-derived in v2 ROADMAP §2 |
| `FEATURE_IMPROVEMENT_ROADMAP.md` | Pre-2026-05-02 feature roadmap (superseded once) | Two-generations-back |
| `LIMITATION_FIX_PLAN.md` + `LIMITATION_FIX_PROGRESS.md` | Limitation-fix tracking | Resolved items absorbed into v2 ROADMAP per-UC; KNOWN_LIMITATIONS.md remains current |
| `VALIDATION_REPORT_2026-01-11.md` | 763/763 unit test pass; 6 of 15 scenarios validated; 9 env-failed | v2 ROADMAP §3.9 carries B9/B10 forward |
| `PANIC_ELIMINATION_SESSION_SUMMARY.md` | Panic-removal session notes | Folded into v2 ROADMAP — informs B8 HTTP/3 unwrap closure |
| `OBSERVABILITY_STATUS.md` | Observability subsystem status | Folded into v2 ROADMAP §3.7 |
| `SECURITY_TESTING_RESULTS.md` | Security test results snapshot | Historical reference |
| `STANDALONE_TESTBED_PLAN.md` | Testbed setup plan | Historical reference |

## What's NOT archived

The following stay in `docs/` because they describe **feature
functionality and operator-facing usage**, not status tracking or
planning. Feature documentation is the gateway's operator-facing
contract — independent of which planning cycle landed it.

- `ARCHITECTURE.md` (banner pointing to v2 ROADMAP; needs eventual refresh)
- `ADMIN_API.md`, `HOT_RELOAD.md`, `MTLS.md`, `WAF_IMPLEMENTATION.md`
- `HTTP3.md`, `HTTP3_QUICHE_MIGRATION.md`
- `DEPLOYMENT_GUIDE.md`, `BETA_TESTING_GUIDE.md`,
  `SECURITY_HARDENING_GUIDE.md`, `SECURITY_TESTING_GUIDE.md`
- `PRODUCTION_OPTIMIZATIONS.md`, `SYSTEM_OPTIMIZATIONS.md`,
  `RESOURCE_SIZING_GUIDE.md`, `LOAD_TESTING_TOOLS.md`,
  `LOAD_TEST_PREPARATION.md`, `LOAD_TEST_QUICKSTART.md`
- `CODE_REVIEW_CHECKLIST.md`, `COMPETITIVE_COMPARISON.md`,
  `DSL_SYNTAX.md`, `SECURITY_FEATURES.md`
- `CONFIG_ENV.md` (operator env-var reference; needs backfill per
  v2 ROADMAP §3.5)
- Project root: `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`,
  `KNOWN_LIMITATIONS.md`, `SECURITY.md`, `LICENSE`,
  `CODE_OF_CONDUCT.md`

The `docs/archive/dev-notes/` subdirectory was already archived in
a prior cleanup cycle and is untouched.

## Future archive cycles

Use the pattern `docs/planning/archive/<YYYY-MM-DD>-v<N>/` for each
restart. Each archive directory must include an `ARCHIVE_MANIFEST.md`
of this shape (what's archived; what supersedes it; what stays in
`docs/`). The new `docs/planning/ROADMAP.md` always references the
most-recent archive in §0 conventions.
