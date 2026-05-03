# Workstream 0.J Stage 2 — PR plan (AI + Body + Shutdown + Secrets sections + cross-subsystem validator)

**Status:** **draft for sign-off.** No code lands until the §11 sign-off questions are answered. Once signed off, this doc is the implementation contract for the Stage 2 PR.

**Companion docs:**

- [`SETTINGS_SCAFFOLD.md`](SETTINGS_SCAFFOLD.md) — signed-off `RuntimeConfig` design (the *what*); this doc is the Stage 2 *how*.
- [`RUNTIME_CONFIG_STAGE1_PR_PLAN.md`](RUNTIME_CONFIG_STAGE1_PR_PLAN.md) — Stage 1 contract; Stage 2 builds on the same conventions (centralized layout, hand-rolled loader, `Reloadable<T>` markers, `serial_test` for env-mutating tests, scoped CI lint).
- [`USECASE_16_AI_LLM_GATEWAY.md`](USECASE_16_AI_LLM_GATEWAY.md) — UC16 design; §3.4 / §3.5 / §3.6 / §5.5 / §6.0 / §7.1.3 are the sources for the AI env vars.
- [`ROADMAP.md`](ROADMAP.md) §5 Phase 0.J task lines 726–733 (AI env-var blocks).
- [`OWNER_GATES_2026-05-03.md`](OWNER_GATES_2026-05-03.md) §6 #4 (Type B = Valkey, Type C = etcd defaults).

Per `CLAUDE.md` rules: every concrete identifier below cites a source span; items I cannot directly cite are marked `(unsourced inference)`.

---

## 0. Goal recap

Add four sections to `RuntimeConfig`: `ai` (largest — 25 env vars), `body` (B12 body-size centralization), `shutdown` (B14 drain timeouts), `secrets` (Phase 1.4 secrets-resolver selector). Land cross-subsystem validation that closes the §11.2 rules deferred from Stage 1 once `Config` provides the enabled-UC list. Add the `Secrets://` `SecretRef` variant. Migrate `src/plugin/hot_reload.rs:181`'s remaining hardcoded `Duration::from_millis(100)`. Widen the CI lint from `src/plugin/` to also cover `src/cluster/`, `src/cache/`, `src/ai/` (when present).

**Estimated effort:** ~5 days (per `SETTINGS_SCAFFOLD.md` §9.2).
**Estimated diff size:** ~900–1100 LoC added (AI section alone is ~250 LoC; cross-subsystem validator ~80 LoC; tests ~250 LoC), ~10 LoC modified, ~100 LoC of new docs.

---

## 1. Cargo.toml additions

No new runtime deps — Stage 2 reuses `arc-swap`, `serial_test`, and the `crate::config::env_override` parser layer landed in Stage 1.

Possible new dev-deps (defer to §11 sign-off):

- `temp_env = "0.3"` if we want scoped env-var test isolation (alternative to `serial_test::serial`). Stage 1 sign-off rejected this; reconsider only if Stage 2's larger test surface makes serial_test unwieldy.

---

## 2. New files (5 files; ~700 LoC)

### 2.1 `src/runtime_config/sections/ai.rs` (~280 LoC — heaviest Stage 2 file)

Mirrors `SETTINGS_SCAFFOLD.md` §3.2. 25 env vars across 3 logical groups (state/storage, routing/streaming, pricing).

```rust
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::env_override::env_string;
use crate::runtime_config::{Reloadable, RuntimeConfigError, SecretRef};

#[derive(Debug, Clone, Default)]
pub struct AiRuntimeConfig {
    // ── secrets / state ───────────────────────────────────────────
    pub key_pepper: Reloadable<Option<SecretRef>>,
    pub state_backend: AiStateBackend,
    pub state_path: PathBuf,

    // ── caching / vector ──────────────────────────────────────────
    pub cache_backend: Reloadable<AiCacheBackend>,
    pub vector_backend: Reloadable<AiVectorBackend>,
    pub vector_addrs: Reloadable<Vec<SocketAddr>>,
    pub vector_auth: Reloadable<Option<SecretRef>>,

    // ── routing ───────────────────────────────────────────────────
    pub retry_budget: Reloadable<u32>,
    pub cooldown_backend: Reloadable<AiCooldownBackend>,
    pub default_backoff_ms_min: Reloadable<u64>,
    pub default_backoff_ms_max: Reloadable<u64>,
    pub default_cooldown_secs_no_header: Reloadable<u64>,

    // ── streaming ─────────────────────────────────────────────────
    pub stream_buffer_depth: Reloadable<u32>,
    pub stream_buffer_overflow: Reloadable<StreamOverflow>,
    pub default_cancel_on_close: Reloadable<bool>,
    pub default_tpm_hard_stop: Reloadable<bool>,

    // ── cluster behaviour ─────────────────────────────────────────
    pub valkey_fail_mode: Reloadable<ValkeyFailMode>,
    pub token_quota_key_shards: u32,                  // Restart: re-sharding needs coordination
    pub key_cache_ttl_secs: Reloadable<u64>,
    pub pricing_override_cache_ttl_secs: Reloadable<u64>,

    // ── nested pricing config ─────────────────────────────────────
    pub pricing: AiPricingRuntimeConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiStateBackend { #[default] Redb, RocksDb, ScyllaDb }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiCacheBackend { #[default] Valkey, Redis, Memory, Disk, MultiTier }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiVectorBackend { #[default] None, Qdrant, RedisStack, PgVector, Hnsw }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AiCooldownBackend { #[default] Auto, Valkey, Local }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StreamOverflow { #[default] DropOldest, Block, Error }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValkeyFailMode { #[default] LocalFallback, FailOpen, FailClosed }

#[derive(Debug, Clone, Default)]
pub struct AiPricingRuntimeConfig {
    pub feed_url: Reloadable<Option<String>>,         // HIGHPER_AI_PRICING_FEED_URL
    pub feed_sign_key: Reloadable<Option<SecretRef>>, // HIGHPER_AI_PRICING_FEED_SIGN_KEY
    pub refresh_interval: Reloadable<Duration>,       // HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS (default 604800; min 3600)
    pub refresh_fail_mode: Reloadable<PricingFailMode>, // HIGHPER_AI_PRICING_REFRESH_FAIL_MODE
    pub allow_free_tier: Reloadable<bool>,            // HIGHPER_AI_ALLOW_FREE_TIER
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PricingFailMode { #[default] LastKnownGood, FailClosed }

pub(crate) fn load() -> Result<AiRuntimeConfig, RuntimeConfigError> {
    let key_pepper = env_string("AI_KEY_PEPPER")
        .map(|v| SecretRef::parse("HIGHPER_AI_KEY_PEPPER", &v))
        .transpose()?;
    let state_backend = parse_state_backend(env_string("AI_STATE_BACKEND").as_deref())?;
    let state_path = env_string("AI_STATE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./data/highper-ai"));

    let retry_budget = parse_u32_in_range(
        "HIGHPER_AI_RETRY_BUDGET",
        env_string("AI_RETRY_BUDGET").as_deref(),
        3,
        1..=10,
    )?;

    // ... (similar block per env var — total ~150 LoC of straight-line loader code)

    let pricing = load_pricing()?;
    let cfg = AiRuntimeConfig { /* all fields */ };
    validate_ai(&cfg)?;
    Ok(cfg)
}

fn validate_ai(cfg: &AiRuntimeConfig) -> Result<(), RuntimeConfigError> {
    // Vector backend != None requires addrs non-empty.
    if *cfg.vector_backend.get() != AiVectorBackend::None
        && cfg.vector_addrs.get().is_empty()
    {
        return Err(RuntimeConfigError::MissingRequired {
            env_var: "HIGHPER_AI_VECTOR_ADDRS".into(),
            required_because: "vector_backend != none",
        });
    }
    // backoff_ms_min < backoff_ms_max
    if cfg.default_backoff_ms_min.get() >= cfg.default_backoff_ms_max.get() {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "AI default_backoff_ms_min must be < default_backoff_ms_max",
            details: format!(
                "got min={} max={}",
                cfg.default_backoff_ms_min.get(),
                cfg.default_backoff_ms_max.get()
            ),
        });
    }
    // pricing.refresh_interval >= 3600s
    if *cfg.pricing.refresh_interval.get() < Duration::from_secs(3600) {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS".into(),
            value: format!("{:?}", cfg.pricing.refresh_interval.get()),
            valid_range: ">= 3600 (1h)",
        });
    }
    // Note: AI cache=valkey requires Cluster Type B is a *cross-subsystem*
    // validation; lives in loader::validate_cross_subsystem.
    Ok(())
}
```

### 2.2 `src/runtime_config/sections/body.rs` (~80 LoC — B12 body-size centralization)

```rust
pub struct BodyRuntimeConfig {
    pub max_request_body: Reloadable<u64>,    // HIGHPER_BODY_MAX_REQUEST (default 10 MB)
    pub max_streaming_body: Reloadable<u64>,  // HIGHPER_BODY_MAX_STREAMING (default 100 MB)
    pub max_form_body: Reloadable<u64>,       // HIGHPER_BODY_MAX_FORM (default 1 MB; X-Forwarded-For trust scope)
}
```

Migration impact (Stage 3, not Stage 2): replace literal `10 * 1024 * 1024` and similar in `src/middleware/`, `src/proxy/`, `src/http/` with reads from `runtime_config::current().body.*`. Stage 2 just lands the section + env-var loader.

### 2.3 `src/runtime_config/sections/shutdown.rs` (~80 LoC — B14 drain timeouts)

```rust
pub struct ShutdownRuntimeConfig {
    pub drain_secs: Reloadable<Duration>,           // HIGHPER_SHUTDOWN_DRAIN (default 30s)
    pub force_kill_after_secs: Reloadable<Duration>, // HIGHPER_SHUTDOWN_FORCE_KILL (default 60s; >drain_secs)
    pub spawn_task_drain_secs: Reloadable<Duration>, // HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN (default 10s) — B14
}
```

Cross-validation: `force_kill_after_secs > drain_secs`.

### 2.4 `src/runtime_config/sections/secrets.rs` (~120 LoC — Phase 1.4 prep)

```rust
pub struct SecretsRuntimeConfig {
    pub provider: SecretsProvider,            // HIGHPER_SECRETS_PROVIDER = env|file|vault|aws|k8s
    pub vault_url: Option<String>,            // HIGHPER_SECRETS_VAULT_URL
    pub vault_token: Option<SecretRef>,       // HIGHPER_SECRETS_VAULT_TOKEN
    pub aws_region: Option<String>,           // HIGHPER_SECRETS_AWS_REGION
    pub k8s_namespace: Option<String>,        // HIGHPER_SECRETS_K8S_NAMESPACE
    pub cache_ttl_secs: Reloadable<u64>,      // HIGHPER_SECRETS_CACHE_TTL (default 300)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SecretsProvider {
    #[default]
    Env,
    File,
    Vault,
    AwsSecretsManager,
    K8sSecret,
}
```

Stage 2 lands the *struct + loader*. The actual secrets-resolver implementations (Vault HTTP client, AWS SDK, K8s API client) ship in Phase 1.4 — they'll consume `SecretsRuntimeConfig` via `runtime_config::current().secrets`.

### 2.5 `src/runtime_config/sections/mod.rs` — re-exports updated

```rust
pub mod cluster;
pub mod plugin;
pub mod ai;        // NEW
pub mod body;      // NEW
pub mod shutdown;  // NEW
pub mod secrets;   // NEW

pub use cluster::ClusterRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
pub use ai::AiRuntimeConfig;
pub use body::BodyRuntimeConfig;
pub use shutdown::ShutdownRuntimeConfig;
pub use secrets::SecretsRuntimeConfig;
```

---

## 3. Modified files (~7 files; ~80 LoC changed)

### 3.1 `src/runtime_config/mod.rs`

Add new sub-structs to the top-level `RuntimeConfig` struct + `for_test()`:

```rust
pub struct RuntimeConfig {
    pub cluster: ClusterRuntimeConfig,
    pub plugin: PluginRuntimeConfig,
    pub ai: AiRuntimeConfig,           // NEW
    pub body: BodyRuntimeConfig,       // NEW
    pub shutdown: ShutdownRuntimeConfig, // NEW
    pub secrets: SecretsRuntimeConfig, // NEW
    // Stage 3: http3, tls, ratelimit, circuit_breaker, geo, cache,
    //          signals, config_watcher, observability
}
```

### 3.2 `src/runtime_config/loader.rs`

Wire the four new section loaders into `load()`, then expand `validate_cross_subsystem` with the AI/Cluster invariants and §11.2 rules 1, 2, 3, 5.

```rust
pub fn load() -> Result<RuntimeConfig, RuntimeConfigError> {
    let cluster = cluster::load()?;
    let plugin  = plugin::load()?;
    let ai      = ai::load()?;
    let body    = body::load()?;
    let shutdown = shutdown::load()?;
    let secrets = secrets::load()?;

    let cfg = RuntimeConfig { cluster, plugin, ai, body, shutdown, secrets };
    validate_cross_subsystem(&cfg)?;
    Ok(cfg)
}

fn validate_cross_subsystem(cfg: &RuntimeConfig) -> Result<(), RuntimeConfigError> {
    // AI cache=valkey requires Cluster Type B configured.
    if *cfg.ai.cache_backend.get() == AiCacheBackend::Valkey
        && *cfg.cluster.typeb_backend.get() == TypeBBackend::None
        && !cfg.cluster.allow_single_node
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "HIGHPER_AI_CACHE_BACKEND=valkey requires HIGHPER_CLUSTER_TYPEB_BACKEND set (or HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true)",
            details: "Type B cluster backend is needed for hot-path AI cache counters".into(),
        });
    }
    // AI cooldown_backend=valkey requires Cluster Type B configured.
    if *cfg.ai.cooldown_backend.get() == AiCooldownBackend::Valkey
        && *cfg.cluster.typeb_backend.get() == TypeBBackend::None
    {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "HIGHPER_AI_COOLDOWN_BACKEND=valkey requires HIGHPER_CLUSTER_TYPEB_BACKEND set",
            details: "Forced Valkey cooldown without Type B cluster has no backend".into(),
        });
    }
    // Note: §11.2 rules 1, 2, 3, 5 (Group B/C UC requires backend) need the
    // enabled-UC list from `Config`. Stage 2 introduces the
    // `validate_against_config(&Config, &RuntimeConfig)` entry point, but it
    // is wired in main.rs *after* config load — see §3.3 below.
    Ok(())
}

/// Called from main.rs after both `runtime_config::install` and `Config`
/// loading. Closes the §11.2 rules 1, 2, 3, 5 deferred from Stage 1.
pub fn validate_against_config(
    rt: &RuntimeConfig,
    config: &crate::config::Config,
) -> Result<(), RuntimeConfigError> {
    let enabled_ucs = derive_enabled_ucs(config); // helper that walks Config
    // Rule 1: any Group B UC needs Type B backend (or single-node opt-in).
    // Rule 2: any Group C UC needs Type C backend (or single-node opt-in).
    // Rule 3: if both flags none and any Group B/C UC enabled, ALLOW_SINGLE_NODE must be true.
    // Rule 5: Group B and Group C are independent — Type 4 needs both.
    // ... (concrete UC-list derivation in helper)
    Ok(())
}
```

### 3.3 `src/main.rs` (+5 lines)

Wire the new validator after config load, before runtime start:

```rust
// existing:
let rt_cfg = runtime_config::load()
    .context("Failed to load runtime configuration from HIGHPER_* env vars")?;
runtime_config::install(rt_cfg);
info!("RuntimeConfig loaded and installed");

let config = load_config(&config_path).context("Failed to load configuration")?;
validate_config(&config).context("Configuration validation failed")?;

// NEW (Stage 2):
runtime_config::validate_against_config(&runtime_config::current(), &config)
    .context("Cross-subsystem validation (RuntimeConfig × Config) failed")?;
```

### 3.4 `src/runtime_config/secret_ref.rs` — add `Secrets://` variant

```rust
pub enum SecretRef {
    Literal(String),
    File { path: PathBuf, lazy: bool },
    Secrets { uri: String, lazy: bool },  // NEW (Stage 2)
}

impl SecretRef {
    pub fn parse(env_var: &str, value: &str) -> Result<Self, RuntimeConfigError> {
        // ... existing literal: + file:// branches
        if let Some(rest) = value.strip_prefix("secrets://") {
            let (uri, lazy) = parse_lazy_query(rest);
            return Ok(SecretRef::Secrets { uri: uri.to_string(), lazy });
        }
        // ...
    }

    pub fn resolve_eager(&self) -> Result<Option<SecretValue>, RuntimeConfigError> {
        // ... existing branches
        SecretRef::Secrets { uri, lazy } => {
            if *lazy { return Ok(None); }
            // Stage 2 stub: actual resolver lives in Phase 1.4.
            // For now return a clear "not implemented" error so misconfig is loud.
            return Err(RuntimeConfigError::InvalidCombination {
                rule: "secrets:// resolution not yet implemented",
                details: format!("uri={uri}; ships in Phase 1.4 with SecretsRuntimeConfig.provider"),
            });
        }
    }
}
```

### 3.5 `src/plugin/hot_reload.rs:181` — migrate the remaining hardcoded literal

```rust
// BEFORE (post-Stage-1 with waiver):
// allow: Stage 2 — migrate to PluginRuntimeConfig::hot_reload_settle (HIGHPER_PLUGIN_HOT_RELOAD_SETTLE)
tokio::time::sleep(Duration::from_millis(100)).await;

// AFTER (Stage 2):
let settle = *crate::runtime_config::current().plugin.hot_reload_settle.get();
tokio::time::sleep(settle).await;
```

Add a corresponding field to `PluginRuntimeConfig`:

```rust
pub hot_reload_settle: Reloadable<Duration>,  // HIGHPER_PLUGIN_HOT_RELOAD_SETTLE (default 100ms)
```

### 3.6 `xtask/src/lint_runtime_config.rs` — widen scope

```rust
const STAGE2_PATHS: &[&str] = &[
    "highper-gateway/src/plugin",
    "highper-gateway/src/cluster",  // NEW
    "highper-gateway/src/cache",    // NEW
    "highper-gateway/src/ai",       // NEW (when present; Phase 2)
];
```

(`src/ai/` may not exist yet pre-Phase-2; lint should silently skip non-existent paths rather than fail.)

### 3.7 `docs/CONFIG_ENV.md` — append AI/Body/Shutdown/Secrets sections

Same row format as Stage 1's cluster + plugin tables. Adds ~30 rows.

---

## 4. New tests (~250 LoC)

### 4.1 `src/runtime_config/sections/ai.rs` `#[cfg(test)] mod tests`

- `defaults_when_no_env_vars`: state_backend=Redb, cache_backend=Valkey, vector_backend=None, retry_budget=3.
- `parses_state_backend_rocksdb`.
- `rejects_invalid_state_backend`.
- `rejects_retry_budget_above_10`.
- `rejects_vector_backend_qdrant_without_addrs`.
- `rejects_backoff_min_gte_max`.
- `pricing_refresh_interval_below_1h_rejected`.
- `parses_pricing_fail_mode_fail_closed`.

### 4.2 `src/runtime_config/sections/body.rs` `#[cfg(test)] mod tests`

- `default_max_request_body_10mb`.
- `parses_HIGHPER_BODY_MAX_REQUEST_50mb`.
- `byte-size-suffix parsing` (`100KB`, `5MB`, `1GB`).

### 4.3 `src/runtime_config/sections/shutdown.rs` `#[cfg(test)] mod tests`

- `defaults_drain_30s_force_kill_60s`.
- `rejects_force_kill_lte_drain`.

### 4.4 `src/runtime_config/sections/secrets.rs` `#[cfg(test)] mod tests`

- `default_provider_env`.
- `vault_provider_requires_url_and_token`.
- `aws_provider_requires_region`.
- `k8s_provider_requires_namespace`.

### 4.5 `src/runtime_config/loader.rs` cross-subsystem tests

- `ai_cache_valkey_requires_cluster_typeb`.
- `ai_cache_valkey_with_allow_single_node_passes`.
- `ai_cooldown_valkey_requires_cluster_typeb`.

### 4.6 `src/runtime_config/secret_ref.rs` — `Secrets://` parse tests

- `parses_secrets_vault_uri`.
- `parses_secrets_lazy_query`.
- `secrets_resolve_eager_returns_not_implemented_error_in_stage_2`.

---

## 5. CI lint widening

Stage 2 expands the lint scope from `src/plugin/` only (Stage 1) to:

- `src/plugin/`
- `src/cluster/` (when present — most discovery code lives there)
- `src/cache/` (`src/cache/backends.rs:182-326` Redis/Valkey client)
- `src/ai/` (greenfield — not present until Phase 2 starts; lint skips silently)

Existing waivers in `src/plugin/` for Stage 2 follow-ups (`hot_reload.rs:181` → migrated; `types.rs:181` → may stay if owner agrees per-execution timeout doesn't belong in `RuntimeConfig`).

---

## 6. `docs/CONFIG_ENV.md` updates

Append four sections (AI / Body / Shutdown / Secrets) following the existing table format. ~30 rows added. Each row: env var, default, valid range, reload tier, read-at citation.

---

## 7. Acceptance script

```bash
# 1. Build + unit tests pass
cd highper-gateway
cargo build
cargo test --lib runtime_config

# 2. Defaults work (no env vars set)
unset $(env | grep '^HIGHPER_' | cut -d= -f1)
nerdctl run --rm highper-gateway:stage2-rc start --config /etc/highper-gateway/config.yaml 2>&1 | grep "RuntimeConfig loaded"

# 3. AI env vars round-trip + range validation
HIGHPER_AI_RETRY_BUDGET=11 nerdctl run --rm ... | grep "out of range.*RETRY_BUDGET"
HIGHPER_AI_VECTOR_BACKEND=qdrant nerdctl run --rm ... | grep "MissingRequired.*VECTOR_ADDRS"

# 4. Cross-subsystem validation refuses to boot
HIGHPER_AI_COOLDOWN_BACKEND=valkey \
HIGHPER_CLUSTER_TYPEB_BACKEND=none \
nerdctl run --rm ... | grep "InvalidCombination.*COOLDOWN_BACKEND=valkey requires.*TYPEB_BACKEND"

# 5. Body section
HIGHPER_BODY_MAX_REQUEST=5MB nerdctl run --rm ... | grep "RuntimeConfig loaded"

# 6. Plugin hot_reload migration verified
HIGHPER_PLUGIN_HOT_RELOAD_SETTLE=200ms nerdctl run --rm ... | grep "RuntimeConfig loaded"

# 7. Lint widened scope
cargo run --package xtask --bin lint-runtime-config --
# Should report 0 violations now that hot_reload.rs:181 is migrated.

# 8. Secrets:// stub returns clear error (Phase 1.4 not yet implemented)
HIGHPER_AI_KEY_PEPPER='secrets://vault/kv/highper/ai-pepper' \
nerdctl run --rm ... | grep "secrets:// resolution not yet implemented"
```

---

## 8. Risks / gotchas

1. **Cross-subsystem validator entry-point ordering.** `validate_against_config` runs *after* `Config` loads in main.rs. If a future contributor reorders, the §11.2 rules silently stop firing. Add a ROADMAP cross-reference comment in main.rs to flag the dependency.
2. **`token_quota_key_shards` non-Reloadable.** Re-sharding needs cluster coordination (per `USECASE_16_AI_LLM_GATEWAY.md` §3.6.x). Marking it bare (Restart-required) is correct — Stage 3's diff endpoint will report it as pending.
3. **`AiPricingRuntimeConfig::feed_url` defaults.** Per `USECASE_16_AI_LLM_GATEWAY.md` §5.5.1 the default is "vendored LiteLLM snapshot baked into binary" — i.e., empty `feed_url` means "use embedded snapshot, no remote fetch". Document this clearly in CONFIG_ENV.md.
4. **`Secrets://` variant ships unimplemented.** Stage 2 lands the parse path + struct field but the `resolve_eager` returns `RuntimeConfigError::InvalidCombination` for any operator that actually tries it. Phase 1.4 finishes the implementation. Acceptance: clear error message guides operators to use `literal:` or `file:///` until then.
5. **`src/cluster/` may not exist yet.** Stage 2 lint should `if path.exists()` for newly-added scopes.
6. **AiPricingRuntimeConfig nesting.** Loader needs to construct the nested struct via a helper `load_pricing()`; tests should verify nested defaults flow correctly.

---

## 9. Out of Stage 2 scope

- Hot-reload runtime (Stage 3 — SIGHUP atomic swap).
- Admin diff endpoint (Stage 3).
- Remaining 9 sections: `http3`, `tls`, `ratelimit`, `circuit_breaker`, `geo`, `cache`, `signals`, `config_watcher`, `observability` (Stage 3).
- Project-wide CI lint (Stage 3).
- `src/config/defaults.rs` literal migration (Stage 3).
- `Vault` / `AWS` / `K8s` actual secrets-resolver clients (Phase 1.4 — Stage 2 lands the *selector* + struct only).
- B12 body-size literal migration in `src/middleware/`, `src/proxy/`, `src/http/` (Stage 3).
- B14 spawned-task drain wiring (Stage 3).

---

## 10. PR commit shape (when Stage 2 lands)

```
runtime_config: Stage 2 - AI + Body + Shutdown + Secrets sections + cross-subsystem validator

- 4 new sections: AiRuntimeConfig (25 env vars), BodyRuntimeConfig (3),
  ShutdownRuntimeConfig (3), SecretsRuntimeConfig (5).
- Cross-subsystem validator: AI cache=valkey requires Cluster Type B
  (unless allow_single_node); AI cooldown=valkey requires Type B.
- New `validate_against_config(&RuntimeConfig, &Config)` entry point closes
  §11.2 rules 1/2/3/5 once Config is loaded (called from main.rs).
- SecretRef gains `Secrets://` variant (parse path + struct field; actual
  resolver implementations land in Phase 1.4).
- Migrated src/plugin/hot_reload.rs:181 hardcoded Duration::from_millis(100)
  to PluginRuntimeConfig::hot_reload_settle (HIGHPER_PLUGIN_HOT_RELOAD_SETTLE).
- CI lint widened from src/plugin/ to also cover src/cluster/, src/cache/,
  src/ai/ (skips non-existent paths silently).
- docs/CONFIG_ENV.md gains 4 sections + ~30 rows.
- Tests: ~250 LoC across 6 in-file test modules + cross-subsystem tests in
  loader.rs.

Stage 3 (~4d): remaining 9 sections + Tier 1 SIGHUP + admin diff +
project-wide lint + B12/B14 literal migrations.
```

---

## 11. Sign-off questions (answer before Stage 2 PR begins)

1. Plan approved as-is, or specific changes?
2. Single PR for all of Stage 2, or split (e.g., `ai` PR first, then `body`+`shutdown`+`secrets`+validator)?
3. Add `temp_env` dev-dep for env-var test isolation, or keep `serial_test` only (Stage 1 sign-off picked the latter; reconsider given Stage 2's larger test surface)?
4. Should `validate_against_config` also fire on SIGHUP (when Tier 1 reload lands in Stage 3) — i.e., re-run cross-subsystem checks against the live `Config` — or only at startup?
5. `Secrets://` variant: ship the parse path + stub error (current draft) or defer the entire variant to Phase 1.4 to keep Stage 2 scope tighter?
6. `src/plugin/types.rs:181` per-execution timeout (currently waived) — migrate to `PluginRuntimeConfig` in Stage 2, or leave as per-route override (out of scope)?
7. CI lint scope: Stage 2 widens to `src/plugin/` + `src/cluster/` + `src/cache/` + `src/ai/`. Add others now (e.g., `src/proxy/`, `src/middleware/`) or wait for Stage 3?

---

*Plan author: claude-opus-4-7-1m, 2026-05-03. Owner sign-off required on §11 questions before any source code lands. Per CLAUDE.md rules.*
