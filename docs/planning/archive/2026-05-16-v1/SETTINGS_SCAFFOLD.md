# Workstream 0.J — `RuntimeConfig` scaffold (signed-off design)

**Status:** **signed-off design 2026-05-03.** All 7 §8 questions decided. Stage 1 PR can begin against this doc.

**Companion docs:**

- [`ROADMAP.md`](ROADMAP.md) §0.1 (env-var-only rule), §4.1.1 (B11–B14 plan), §4.4 row 7 (`MetricsBackend`/`LogBackend`), §5 Phase 0.J (the workstream this scaffold delivers), §11.2 (cluster-bootstrap shape), §13 (status snapshot).
- [`OWNER_GATES_2026-05-03.md`](OWNER_GATES_2026-05-03.md) §6 #4 (Type B = Valkey, Type C = etcd, `PeerDiscovery` trait).
- [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) §3.5 (UC16 storage), §7.4 (cluster security baseline).
- [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) §3.4 / §3.5 / §3.6 / §5.5 / §6.0 / §7.1.3 (the env vars that land in `RuntimeConfig::ai`).

Per `CLAUDE.md` rules: every concrete identifier below cites a source span; items I cannot directly cite are marked `(unsourced inference)`.

---

## 0. Why this exists

`ROADMAP.md` §0.1 (the env-var-only rule) says **"every runtime-tunable value … is loaded from a `HIGHPER_*`-prefixed environment variable at startup"** and **"defaults live in a single `Settings` / `Config` struct loaded once at boot; hot paths must not call `std::env::var`."** Today:

- `src/config/env_override.rs` has typed parsers (`env_usize`, `env_u32`, `env_bool`, `env_string`, `env_duration`, `env_byte_size`) but **no central struct**. Cited at `highper-gateway/src/config/env_override.rs:24-58`.
- `src/config/schema.rs` is the **user-facing `Config`** (server / TLS / upstreams / routes / observability / WebSocket / gRPC / admin / cache / rate-limit / WAF / GraphQL / webserver) loaded from YAML/DSL/JSON/TOML — not a runtime-tunable struct. Cited at `highper-gateway/src/config/schema.rs:7-40`.
- `src/config/defaults.rs` `ProtocolDefaults` carries hardcoded literals (`read_buffer_size: 8192`, `max_connections: 10000`, `Duration::from_secs(60)`). Cited at `highper-gateway/src/config/defaults.rs:14-60`. These literals are exactly what the §0.1 rule calls out as bugs.

`RuntimeConfig` is the **new** struct that holds the operator-tunable defaults. `Config` continues to carry the routing/listener/upstream shape; the two are orthogonal and parallel naturally (`Config` = file-driven; `RuntimeConfig` = env-var-driven runtime tunables). Naming chosen to disambiguate vs the generic "Settings" term that overlaps with "configuration" in operator vocabulary.

---

## 1. Module layout (centralized)

**Decision:** centralized layout. All sub-struct definitions, the loader, and validation live under `src/runtime_config/`. Consumer modules (e.g., `src/cache/`, `src/cluster/`) depend on `runtime_config`; `runtime_config` does **not** depend back on any consumer module. Dep graph is a clean DAG — no circular-dep risk by construction.

```
highper-gateway/src/runtime_config/
  mod.rs                         // re-exports + RuntimeConfig top-level struct + OnceLock accessor
  loader.rs                      // env-var loader + range validation + refuse-to-start errors
  error.rs                       // RuntimeConfigError + structured error messages with env-var citations
  reload.rs                      // SIGHUP + ArcSwap<RuntimeConfig> + Reloadable<T> wrapper + diff report
  secret_ref.rs                  // SecretRef enum (literal | file:// | secrets://) + eager resolution
  sections/
    http3.rs                     // RuntimeConfig::http3
    body.rs                      // B12 body-size centralization
    shutdown.rs                  // B14 drain windows
    signals.rs
    config_watcher.rs
    tls.rs
    ratelimit.rs                 // B4 distributed + N1 GCRA hooks
    circuit_breaker.rs           // B7 (currently duplicated in proxy/ + tcp/ per §4.4 row 3)
    geo.rs                       // UC15 P0
    cache.rs
    cluster.rs                   // HA §11.2 cluster-bootstrap shape
    ai.rs                        // UC16: pricing, vector, routing, streaming, pepper, state path
    plugin.rs                    // drain window, chunk budget
    secrets.rs                   // Phase 1.4 secrets resolver (vault | aws | k8s | env)
    observability.rs             // Phase 1.4 MetricsBackend/LogBackend selector
```

**Why centralized (not hybrid or distributed):**

- **Avoids circular dep by construction.** Hybrid would have leaked `pub use` re-exports back the wrong way the moment a sub-struct needed a shared primitive like `SecretRef`. Centralized keeps the dep graph one-directional.
- **Operator-friendly.** Single subtree to audit; one CONFIG_ENV.md generator walks one tree; CI lint allowlist is a single path.
- **Cross-subsystem validation locality.** Validations like "AI `cache_backend=valkey` requires Cluster Type B configured" live next to the structs they reference — not split across modules.
- Per-subsystem developer locality is a real cost, but operator-friendliness is the primary goal here. Developer effort is in service of that goal.

---

## 2. Top-level `RuntimeConfig` shape

```rust
// highper-gateway/src/runtime_config/mod.rs

mod loader;
mod error;
mod reload;
mod secret_ref;
mod sections;

pub use loader::load;
pub use error::RuntimeConfigError;
pub use reload::{Reloadable, ReloadDiff};
pub use secret_ref::SecretRef;
pub use sections::{
    Http3RuntimeConfig, BodyRuntimeConfig, ShutdownRuntimeConfig,
    SignalsRuntimeConfig, ConfigWatcherRuntimeConfig, TlsRuntimeConfig,
    RatelimitRuntimeConfig, CircuitBreakerRuntimeConfig, GeoRuntimeConfig,
    CacheRuntimeConfig, ClusterRuntimeConfig, AiRuntimeConfig,
    PluginRuntimeConfig, SecretsRuntimeConfig, ObservabilityRuntimeConfig,
};

use arc_swap::ArcSwap;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub http3: Http3RuntimeConfig,
    pub body: BodyRuntimeConfig,
    pub shutdown: ShutdownRuntimeConfig,
    pub signals: SignalsRuntimeConfig,
    pub config_watcher: ConfigWatcherRuntimeConfig,
    pub tls: TlsRuntimeConfig,
    pub ratelimit: RatelimitRuntimeConfig,
    pub circuit_breaker: CircuitBreakerRuntimeConfig,
    pub geo: GeoRuntimeConfig,
    pub cache: CacheRuntimeConfig,
    pub cluster: ClusterRuntimeConfig,
    pub ai: AiRuntimeConfig,
    pub plugin: PluginRuntimeConfig,
    pub secrets: SecretsRuntimeConfig,
    pub observability: ObservabilityRuntimeConfig,
}

impl RuntimeConfig {
    /// Defaults-only constructor for unit tests. Does not read env vars.
    pub fn for_test() -> Self {
        Self { /* every field uses its sub-struct's default() */ }
    }
}

static CURRENT: OnceLock<ArcSwap<RuntimeConfig>> = OnceLock::new();

pub fn install(c: RuntimeConfig) {
    CURRENT.set(ArcSwap::from_pointee(c))
        .expect("runtime_config::install() called twice");
}

pub fn current() -> Arc<RuntimeConfig> {
    CURRENT
        .get()
        .expect("runtime_config not initialized — call runtime_config::install(load()?) in main()")
        .load_full()
}
```

**Singleton via `OnceLock<ArcSwap<RuntimeConfig>>`:**

- `OnceLock` for one-time init enforcement (signed-off Q2).
- `ArcSwap` because Tier 1 hot-reload (signed-off Q3) atomically swaps the inner `RuntimeConfig` on SIGHUP. Workers reading via `current()` get the new struct on the next read with no locking.
- Hot-path discipline (§0.1: "hot paths must not call `std::env::var`") is enforced by the type system: there is no other accessor.

---

## 3. Sub-struct shapes (selected — full set in CONFIG_ENV.md)

### 3.1 `ClusterRuntimeConfig` (mirrors `ROADMAP.md` §11.2)

```rust
pub struct ClusterRuntimeConfig {
    pub infra: ClusterInfra,                              // HIGHPER_CLUSTER_INFRA = k8s|vm|baremetal|single (default: single)
    pub typeb_backend: Reloadable<TypeBBackend>,          // HIGHPER_CLUSTER_TYPEB_BACKEND = valkey|redis|none (default: none — but valkey if Group B UC enabled per §6 #4)  [Restart]
    pub typeb_addrs: Reloadable<Vec<SocketAddr>>,         // HIGHPER_CLUSTER_TYPEB_ADDRS (comma-separated host:port; unset by default)  [Restart]
    pub typeb_auth: Reloadable<Option<SecretRef>>,        // HIGHPER_CLUSTER_TYPEB_AUTH (literal | file:// | secrets://)  [Live — secret rotation]
    pub typeb_tls: Reloadable<bool>,                      // HIGHPER_CLUSTER_TYPEB_TLS (default: false)  [Restart]
    pub typec_backend: Reloadable<TypeCBackend>,          // HIGHPER_CLUSTER_TYPEC_BACKEND = etcd|consul|raft|none (default: none — but etcd if Group C UC enabled per §6 #4)  [Restart]
    pub typec_addrs: Reloadable<Vec<SocketAddr>>,         // HIGHPER_CLUSTER_TYPEC_ADDRS  [Restart]
    pub typec_client_cert: Reloadable<Option<SecretRef>>, // HIGHPER_CLUSTER_TYPEC_CLIENT_CERT  [Live — cert rotation]
    pub typec_client_key:  Reloadable<Option<SecretRef>>, // HIGHPER_CLUSTER_TYPEC_CLIENT_KEY   [Live — cert rotation]
    pub typec_ca:          Reloadable<Option<SecretRef>>, // HIGHPER_CLUSTER_TYPEC_CA           [Live — cert rotation]
    pub peer_discovery: PeerDiscoveryMode,                // HIGHPER_CLUSTER_PEER_DISCOVERY = static|k8s_headless|consul|dns|none  [Restart]
    pub peers: Vec<SocketAddr>,                           // HIGHPER_CLUSTER_PEERS (when peer_discovery = static)  [Restart]
    pub allow_single_node: bool,                          // HIGHPER_CLUSTER_ALLOW_SINGLE_NODE (default: false)  [Restart]
    pub allow_insecure: bool,                             // HIGHPER_CLUSTER_ALLOW_INSECURE (default: false)  [Restart]
}
```

The `Reloadable<T>` marker (defined in `reload.rs`) carries metadata the Tier 1 reload report uses to classify each field as `Live` (atomic swap is safe) or `Restart` (loaded but needs process restart to take effect). All five startup validations from `ROADMAP.md` §11.2 (rules 1–5) live in `loader::validate_cluster()`.

### 3.2 `AiRuntimeConfig` (UC16 surface — gathered from §0.J task lines 726–733)

```rust
pub struct AiRuntimeConfig {
    pub key_pepper: Reloadable<Option<SecretRef>>,                // HIGHPER_AI_KEY_PEPPER (refused-empty when UC16 enabled unless allow_insecure)  [Live]
    pub state_backend: AiStateBackend,                            // HIGHPER_AI_STATE_BACKEND = redb|rocksdb|scylladb (default: redb)  [Restart]
    pub state_path: PathBuf,                                      // HIGHPER_AI_STATE_PATH (default: ./data/highper-ai)  [Restart]
    pub cache_backend: Reloadable<AiCacheBackend>,                // HIGHPER_AI_CACHE_BACKEND = valkey|redis|memory|disk|multi-tier (default: valkey)  [Restart]
    pub vector_backend: Reloadable<AiVectorBackend>,              // HIGHPER_AI_VECTOR_BACKEND = none|qdrant|redis-stack|pgvector|hnsw (default: none)  [Restart]
    pub vector_addrs: Reloadable<Vec<SocketAddr>>,                // HIGHPER_AI_VECTOR_ADDRS  [Restart]
    pub vector_auth:  Reloadable<Option<SecretRef>>,              // HIGHPER_AI_VECTOR_AUTH  [Live]
    pub retry_budget: Reloadable<u32>,                            // HIGHPER_AI_RETRY_BUDGET (default: 3)  [Live]
    pub cooldown_backend: Reloadable<AiCooldownBackend>,          // HIGHPER_AI_COOLDOWN_BACKEND = auto|valkey|local (default: auto)  [Restart]
    pub default_backoff_ms_min: Reloadable<u64>,                  // HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN (default: 50)  [Live]
    pub default_backoff_ms_max: Reloadable<u64>,                  // HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX (default: 200)  [Live]
    pub default_cooldown_secs_no_header: Reloadable<u64>,         // HIGHPER_AI_DEFAULT_COOLDOWN_SECS_NO_HEADER (default: 30)  [Live]
    pub stream_buffer_depth: Reloadable<u32>,                     // HIGHPER_AI_STREAM_BUFFER_DEPTH (default: 64)  [Live]
    pub stream_buffer_overflow: Reloadable<StreamOverflow>,       // HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY = drop_oldest|block|error (default: drop_oldest)  [Live]
    pub default_cancel_on_close: Reloadable<bool>,                // HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE (default: true)  [Live]
    pub default_tpm_hard_stop: Reloadable<bool>,                  // HIGHPER_AI_DEFAULT_TPM_HARD_STOP (default: false)  [Live]
    pub valkey_fail_mode: Reloadable<ValkeyFailMode>,             // HIGHPER_AI_VALKEY_FAIL_MODE = local_fallback|fail_open|fail_closed (default: local_fallback)  [Live]
    pub token_quota_key_shards: u32,                              // HIGHPER_AI_TOKEN_QUOTA_KEY_SHARDS (default: 1)  [Restart — re-sharding needs coordination]
    pub key_cache_ttl_secs: Reloadable<u64>,                      // HIGHPER_AI_KEY_CACHE_TTL_SECS (default: 300)  [Live]
    pub pricing_override_cache_ttl_secs: Reloadable<u64>,         // HIGHPER_AI_PRICING_OVERRIDE_CACHE_TTL_SECS (default: 60)  [Live]
    pub pricing: AiPricingRuntimeConfig,                          // 5-field nested struct per §0.J line 728
}
```

### 3.3 Other sub-structs (one-liners)

- `Http3RuntimeConfig` — buffering bounds, migration window, max streams (B8 surface).
- `BodyRuntimeConfig` — `max_request_body`, `max_streaming_body`, per-route override hook (B12 centralization).
- `ShutdownRuntimeConfig` — drain seconds per pool (B14 — was hardcoded 30 s in `src/plugin/manager.rs:252`).
- `RatelimitRuntimeConfig` — distributed-vs-local, `key_shards`, X-Forwarded-For trust (B4).
- `CircuitBreakerRuntimeConfig` — failure threshold, half-open probes (B7 — currently duplicated in `src/proxy/circuit_breaker.rs:75-150` and `src/tcp/circuit_breaker.rs:81-200` per §4.4 row 3).
- `PluginRuntimeConfig` — `drain_secs` (was hardcoded 30 s; new env var `HIGHPER_PLUGIN_DRAIN_SECS`), per-chunk budget (`HIGHPER_PLUGIN_CHUNK_BUDGET_US`).
- `SecretsRuntimeConfig` — provider selector for Phase 1.4 (`vault | aws-secrets | k8s-secret | env`).
- `ObservabilityRuntimeConfig` — `MetricsBackend` / `LogBackend` selector (Phase 1.4 — `prometheus | otlp`).

(Full enumeration in `CONFIG_ENV.md` once Stage 3 lands.)

---

## 4. Loader strategy (hand-rolled)

**Decision:** hand-rolled, extending the existing `src/config/env_override.rs:24-58` parsers.

```rust
// highper-gateway/src/runtime_config/loader.rs

use crate::config::env_override::{env_usize, env_u32, env_u64, env_bool, env_string, env_duration, env_byte_size};

pub fn load() -> Result<RuntimeConfig, RuntimeConfigError> {
    let cluster       = sections::cluster::load()?;
    let secrets       = sections::secrets::load()?;
    let ai            = sections::ai::load(&cluster)?;
    let observability = sections::observability::load()?;
    // ... (other sub-loaders; each returns Result<SubStruct, RuntimeConfigError>)

    let cfg = RuntimeConfig {
        http3, body, shutdown, signals, config_watcher, tls,
        ratelimit, circuit_breaker, geo, cache,
        cluster, ai, plugin, secrets, observability,
    };

    validate_cross_subsystem(&cfg)?;  // e.g., AI cache=valkey requires Cluster Type B
    Ok(cfg)
}
```

**Why hand-rolled (not `figment`):**

- Existing parsers in `src/config/env_override.rs:24-58` already cover the primitive types — no boilerplate gain from adding a dep.
- Per-field validation messages can be tailored exactly (figment gives generic "couldn't deserialize" errors).
- Cross-subsystem validation doesn't fit cleanly into figment's serde-shaped pipeline.
- Costs ~50 lines of boilerplate per sub-struct (~800 lines total across 16 sub-structs). Acceptable.

**Validation policy:**

- Range checks at parse time (e.g., `HIGHPER_AI_RETRY_BUDGET` must be `1..=10`; refuse to boot otherwise).
- Cross-subsystem checks in `validate_cross_subsystem()` (e.g., "`HIGHPER_AI_VECTOR_BACKEND ≠ none` requires `HIGHPER_AI_VECTOR_ADDRS` non-empty").
- All 5 cluster validations from §11.2 in `validate_cluster()`.
- Errors carry the env-var name + value + valid range for operator-friendly messages.

---

## 5. Hot reload — Tier 1 / Tier 2 / Tier 3

**Decision:** ship Tier 1 in Phase 0.J (Stage 3). Tier 2 in Phase 1.4. Tier 3 (xDS / GitOps) in Phase 4.2.

### 5.1 Tier 1 — process-internal SIGHUP (Phase 0.J Stage 3)

- SIGHUP triggers `runtime_config::reload()` which re-runs the loader, validates, and atomically swaps the inner `RuntimeConfig` via `ArcSwap`.
- Workers reading via `current()` get the new struct on the next read with no locking.
- **Field-level reloadability classification:** every field is wrapped in `Reloadable<T>` (Live) or held bare (Restart). Loader builds a `ReloadDiff` report listing:
  - `Live` fields whose value changed → swapped immediately.
  - `Restart` fields whose value changed → loaded into the new struct but logged as **"pending restart for these N settings"**. The diff persists until the next process start.
- **+1 day of work in Stage 3** to define `Reloadable<T>` and tag every field. Already accepted in the effort estimate.

### 5.2 Tier 2 — multi-node operator orchestration (Phase 1.4)

- Multi-node coordination is the **operator's** job (k8s `kubectl rollout restart`, systemd `systemctl restart`). Highper-gateway does **not** try to coordinate the cluster directly — it just guarantees per-node atomicity (Tier 1) and exposes a diff endpoint.
- Admin API endpoint `/admin/config/diff` (lands in Stage 3, refined in Phase 1.4) returns:
  - The current effective `RuntimeConfig` (sanitized — secrets shown as `***`).
  - The proposed new state from current env vars (if SIGHUP were sent).
  - The list of `Restart`-classified fields that would need a process cycle.
- Phase 1.4 adds `docs/OPERATIONS_GUIDE.md` documenting the rolling-restart pre-flight pattern: operator's deployment script calls `/admin/config/diff` on each node before triggering the rollout; aborts if any node rejects the new state.

### 5.3 Tier 3 — push-based config (Phase 4.2)

- xDS / GitOps push-based config via the `ConfigSource` trait (§4.4 row 8). Out of scope for v1.0.
- When this lands, `RuntimeConfig` gains a `ConfigSource::EnvVars` (today's default) and `ConfigSource::Xds` / `ConfigSource::Git` variants.

### 5.4 Reloadability classification rules

For each new field, the author tags it Live or Restart per these rules:

- **Live** if changing the value mid-flight only affects subsequent operations (e.g., a timeout, a retry budget, a cache TTL, a feature toggle).
- **Restart** if changing the value mid-flight would corrupt in-flight state, require reconnecting to a backend, or rebind a listener (e.g., `cluster.typeb_backend`, `ai.state_backend`, `tls.cert_path` on a bound listener, `token_quota_key_shards`).
- **When in doubt, classify as Restart.** False-Live causes silent corruption; false-Restart only inconveniences operators with an unnecessary restart.

---

## 6. `SecretRef` — eager resolution with `lazy:bool` opt-out

**Decision:** eager resolution at boot; per-secret `lazy: bool` opt-out for transient-resolver-tolerance cases.

```rust
// highper-gateway/src/runtime_config/secret_ref.rs

#[derive(Debug, Clone)]
pub enum SecretRef {
    Literal(String),                    // direct env-var value
    File { path: PathBuf, lazy: bool }, // file:// — eager by default
    Secrets { uri: String, lazy: bool },// secrets:// — resolved via Phase 1.4 secrets resolver
}

impl SecretRef {
    /// Eagerly resolve at boot. Called by the loader. Returns the secret's plaintext.
    /// File / Secrets variants with `lazy=true` defer resolution to first use.
    pub fn resolve_eager(&self) -> Result<Option<SecretValue>, SecretError> { /* ... */ }

    /// Lazy resolution at first use (for `lazy=true` variants only).
    pub fn resolve_lazy(&self) -> Result<SecretValue, SecretError> { /* ... */ }
}
```

- **Default eager.** Catches misconfigured secrets at boot, before any traffic. Matches the "refuse to start" policy used by §11.2 cluster validation.
- **`lazy:bool` opt-out.** For ops who explicitly want fail-soft against transient secrets-resolver outages (e.g., short-lived Vault hiccups). Operator opts in per-secret by appending `?lazy=true` to the `secrets://` URI, or by setting an env var like `HIGHPER_AI_KEY_PEPPER_LAZY=true`.
- **Phase 1.4 dependency.** The `Secrets` resolver implementation lands in Phase 1.4 (`SecretsRuntimeConfig::provider`). Stage 1–3 of this scaffold ships only `Literal` + `File` variants; `Secrets` is stubbed and returns "not implemented" until Phase 1.4.

---

## 7. `docs/CONFIG_ENV.md` skeleton

```markdown
# HIGHPER_* environment variables — authoritative reference

Per ROADMAP.md §0.1: every operator-tunable value loads from a HIGHPER_*
environment variable at startup. This document lists every such variable
with its default, valid range, subsystem owner, reload tier, and the
source span where it is read.

| Env var | Default | Valid range | Owner sub-struct | Reload tier | Read at |
|---|---|---|---|---|---|
| HIGHPER_CLUSTER_INFRA | `single` | `k8s|vm|baremetal|single` | `cluster.infra` | Restart | `src/runtime_config/sections/cluster.rs:NN` |
| HIGHPER_CLUSTER_TYPEB_BACKEND | `none` | `valkey|redis|none` | `cluster.typeb_backend` | Restart | `src/runtime_config/sections/cluster.rs:NN` |
| HIGHPER_AI_RETRY_BUDGET | `3` | `1..=10` | `ai.retry_budget` | Live | `src/runtime_config/sections/ai.rs:NN` |
| ... (one row per env var)

## Validation rules

- Cluster validations: see ROADMAP §11.2 rules 1–5.
- AI validations: see USECASE_16_AI_LLM_GATEWAY.md §3.6.5.
- Cross-subsystem validations: see `src/runtime_config/loader.rs::validate_cross_subsystem()`.

## Adding a new env var (PR checklist)

1. Add field to the relevant `RuntimeConfig::*` sub-struct (in `src/runtime_config/sections/`).
2. Tag it `Reloadable<T>` (Live) or bare (Restart) per §5.4 rules.
3. Add loader block in the same `sections/<subsystem>.rs` file.
4. Add range validation if numeric.
5. Add row to this document with default + valid range + reload tier + read-at citation.
6. Update `docs/CHANGELOG.md` if user-visible.
7. CI lint will fail the PR if a bare `std::env::var("HIGHPER_…")` appears
   outside `src/runtime_config/` or `src/config/env_override.rs`.
```

(Filled in row-by-row across Stages 1 → 2 → 3.)

---

## 8. CI lint

Three rules, enforced by a small grep-based script under `.github/workflows/` or `xtask/`:

1. **No bare `std::env::var`** in `highper-gateway/src/**/*.rs` outside `src/runtime_config/` and `src/config/env_override.rs`. Allowed in `tests/`.
2. **No literal `Duration::from_secs(N)` / `Duration::from_millis(N)` / `Duration::from_micros(N)`** in production code outside `src/runtime_config/`. Tests, examples, and hardcoded protocol invariants (e.g., TCP keepalive RFC values) get a `// allow: <reason>` waiver.
3. **No literal `* 1024 * 1024`** outside `src/runtime_config/`. Same waiver mechanism.

`ROADMAP.md` §0.J line 724 specifies these checks — drafted here as grep regex, not clippy rules, because clippy can't easily express "outside this module".

**Tradeoff:** grep-based linters give false positives on legitimate uses (e.g., `Duration::from_secs(0)` for a sentinel). Document the waiver pattern up front; tune over the first 3 PRs.

**Progressive scope:**

- Stage 1 lint scope: `src/plugin/` only (the one consumer migrated in Stage 1).
- Stage 2 lint scope: `src/cluster/`, `src/cache/`, `src/ai/` (when present).
- Stage 3 lint scope: project-wide.

---

## 9. Progressive 3-stage rollout

The 12-day Phase 0.J workstream lands as 3 reversible PRs.

### Stage 1 — Foundation + 2 sub-structs (~4 days, single PR)

- `src/runtime_config/{mod.rs, loader.rs, error.rs, reload.rs, secret_ref.rs}` — `RuntimeConfig` struct, `OnceLock<ArcSwap>` accessor, `for_test()` constructor, `ArcSwap` skeleton (no SIGHUP handler yet — that's Stage 3).
- `src/runtime_config/sections/cluster.rs` (mirrors §11.2 — most-cited; blocks B1 / B11 / B14).
- `src/runtime_config/sections/plugin.rs` (smallest section — validates the pattern end-to-end with a single env var).
- Migrate `src/plugin/manager.rs:252`'s hardcoded 30 s to `runtime_config::current().plugin.drain_secs`.
- `src/main.rs` calls `runtime_config::install(runtime_config::load()?)` at startup.
- `docs/CONFIG_ENV.md` skeleton with these two sections only.
- CI lint scoped to `src/plugin/` first — proves the lint works before going wide.

**Stage 1 acceptance:**

- Cargo check + cargo test pass.
- `HIGHPER_CLUSTER_*` env vars round-trip through the new loader.
- `HIGHPER_PLUGIN_DRAIN_SECS=45 ./highper-gateway` produces a process where `current().plugin.drain_secs == 45`.
- CI lint flags any new `std::env::var` in `src/plugin/`.

### Stage 2 — UC16 + remaining release-blocker surface (~5 days, single PR)

- `sections/ai.rs` (the largest sub-struct; unblocks Phase 2).
- `sections/body.rs` (B12), `sections/shutdown.rs` (B14), `sections/secrets.rs` (Phase 1.4 prep — `Literal` + `File` variants of `SecretRef`; `Secrets` stubbed).
- Cross-subsystem validator lands ("AI `cache_backend=valkey` requires Cluster Type B configured", etc.).
- CONFIG_ENV.md grows to include AI / body / shutdown / secrets sections.
- Lint extended to `src/cluster/`, `src/cache/`, `src/ai/` (when present).

**Stage 2 acceptance:**

- All UC16 env vars from §0.J task lines 726–733 land + are documented.
- Cross-subsystem validation refuses to boot when AI is enabled with `cache_backend=valkey` and Cluster Type B is `none` (unless `allow_single_node=true`).
- B12 + B14 release-blocker fixes consume `runtime_config::current()` instead of any new hardcoded literals.

### Stage 3 — Remaining sub-structs + Tier 1 reload + admin diff (~4 days + 1 day reload classification, single PR)

- `sections/{http3, tls, ratelimit, circuit_breaker, geo, cache, signals, config_watcher, observability}.rs`.
- Tier 1 SIGHUP reload lands with the field-level `Reloadable<T>` classification (the +1 day already accepted) — every field tagged `Live` or `Restart`; loader builds a `ReloadDiff` report.
- Admin API endpoint `/admin/config/diff` returns the diff report (Tier 2 hook for multi-node operators' pre-flight scripts).
- CONFIG_ENV.md fills out to full table.
- CI lint goes project-wide; existing `src/main.rs` + `src/config/defaults.rs` literals migrated to `runtime_config::current()`.

**Stage 3 acceptance:**

- All 16 sub-structs ship.
- SIGHUP to a running process atomically swaps the Live subset; Restart subset is loaded and reported as pending.
- `curl /admin/config/diff` returns a structured JSON diff.
- Project-wide CI lint is green.
- All hardcoded literals from `src/config/defaults.rs` (e.g., `read_buffer_size: 8192`, `max_connections: 10000`) consume `runtime_config::current()`.

### After Phase 0.J

- **Tier 2 (Phase 1.4):** `docs/OPERATIONS_GUIDE.md` documents the rolling-restart pre-flight pattern; no new code change beyond what Stage 3 ships.
- **Tier 3 (Phase 4.2):** `ConfigSource` trait extension for xDS / GitOps push-based config.

### Why progressive matters

- Each PR is reversible — if Stage 1's `RuntimeConfig` shape turns out wrong, only `cluster` + `plugin` need refactoring, not all 16 sub-structs.
- Stage 1 surfaces the actual ergonomics with two real consumers before we spend ~7 more days on the rest.
- Operator gets the cluster-config surface first (the one most needed for Phase 0 cloud validation in B9).

---

## 10. Decisions (signed off 2026-05-03)

| # | Question | Decision | Rationale |
|---|---|---|---|
| 1 | Module location | **Centralized** (`src/runtime_config/`) | Avoids circular dep by construction; operator-friendly single tree to audit. |
| 2 | Singleton | **`OnceLock<ArcSwap<RuntimeConfig>>`** | Enforces "no `std::env::var` on hot paths" via type system; supports Tier 1 hot reload. |
| 3 | Hot reload | **Tier 1 (Stage 3) + Tier 2 (Phase 1.4) + Tier 3 (Phase 4.2)** with field-level `Reloadable<T>` classification | Per-setting reloadability is the right granularity; multi-node coordination stays with the operator's deployment system. +1 day for classification accepted. |
| 4 | Test ergonomics | **`RuntimeConfig::for_test()`** defaults-only constructor | Unit tests don't need to set env vars or share global state. |
| 5 | Loader | **Hand-rolled**, extending `src/config/env_override.rs:24-58` | Operator-friendly error messages; no new dep; cross-subsystem validation fits cleanly. |
| 6 | `SecretRef` resolution | **Eager with `lazy:bool` opt-out** | Catches misconfig at boot; opt-out preserves fail-soft for ops who want it. |
| 7 | Naming | **`RuntimeConfig`** | Parallels existing `Config` (file-driven) cleanly; operator vocabulary blurs "settings"/"configuration" otherwise. |

---

## 11. Estimated effort

Per `ROADMAP.md` Phase 0.J task list + the +1 day for Reloadable classification:

- Top-level `RuntimeConfig` struct + 16 sub-structs: **3 days** (line 721).
- Hand-rolled loader + range validation: **2 days** (line 722).
- `docs/CONFIG_ENV.md` initial table: **1 day** (line 723).
- CI lint (grep-based): **1 day** (line 724).
- Cluster security env vars: **1.5 days** (line 725).
- UC16 envs (pepper, pricing, cache+vector, routing, streaming, cluster behaviour, state path): **2.6 days** (lines 726–733 sum).
- Cross-subsystem validation: **0.5 day** *(unsourced inference — not in line-item list; estimated)*.
- **Reloadable classification + Tier 1 SIGHUP + admin diff endpoint: +1 day** (signed-off addition).
- **Total: ~12.6 days** distributed across 3 PR stages (Stage 1 ≈ 4d, Stage 2 ≈ 5d, Stage 3 ≈ 4d incl. +1d reload).

---

## 12. What does NOT belong in `RuntimeConfig`

To keep the boundary clean:

- **User-facing config file** (`Config` from `src/config/schema.rs:7-40`) — that's listener / route / upstream / WAF config, loaded from YAML/DSL. Stays as-is.
- **Per-request state** — request IDs, trace context, circuit-breaker counts. Lives in request-scope structures.
- **Per-tenant overrides for UC16** — virtual-key budgets, per-key routing rules. Live in `AiStateStore`, not `RuntimeConfig`.
- **Hardcoded protocol invariants** — TCP MSS, HTTP/2 frame sizes, RFC-defined timeouts. These are not operator-tunable; they stay as `const`s in the modules that own them, with `// allow: protocol invariant` waivers from the CI lint.

---

*Design author: claude-opus-4-7-1m. Owner sign-off 2026-05-03 on §10 decisions table. Stage 1 PR ready to begin against this doc. Per CLAUDE.md rules.*
