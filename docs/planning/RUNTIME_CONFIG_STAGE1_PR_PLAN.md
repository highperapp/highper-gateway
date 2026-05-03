# Workstream 0.J Stage 1 — PR plan (`src/runtime_config/` foundation + cluster + plugin)

**Status:** **draft for sign-off.** No code lands until the 5 §10 sign-off questions are answered. Once signed off, this doc is the implementation contract for the Stage 1 PR.

**Companion docs:**

- [`SETTINGS_SCAFFOLD.md`](SETTINGS_SCAFFOLD.md) — signed-off `RuntimeConfig` design (the *what*); this doc is the Stage 1 *how*.
- [`ROADMAP.md`](ROADMAP.md) §5 Phase 0.J (workstream task list), §11.2 (cluster-bootstrap shape), §0.1 (env-var-only rule).
- [`OWNER_GATES_2026-05-03.md`](OWNER_GATES_2026-05-03.md) §6 #4 (Type B = Valkey, Type C = etcd defaults).

Per `CLAUDE.md` rules: every concrete identifier below cites a source span; items I cannot directly cite are marked `(unsourced inference)`.

---

## 0. Goal recap

Land the centralized `src/runtime_config/` scaffold with **two end-to-end working sections** (cluster + plugin), migrate `src/plugin/manager.rs:254`'s hardcoded 30 s, install a CI lint **scoped to `src/plugin/`**, and produce the first row of `docs/CONFIG_ENV.md`. Reversible: if the shape turns out wrong, only this PR's two sections need refactoring — the other 14 sub-structs aren't built yet.

**Estimated effort:** ~4 days (per `SETTINGS_SCAFFOLD.md` §9.1).
**Estimated diff size:** ~600–800 LoC added (mostly straight-line loader code), ~15 LoC modified in `manager.rs` + `main.rs` + `lib.rs` + `Cargo.toml`, ~150 LoC of new tests.

---

## 1. Cargo.toml additions

```toml
# highper-gateway/Cargo.toml — add to [dependencies]
arc-swap = "1.7"   # one new dep; ~30 KB; mature; supports Tier 1 hot reload (handler lands Stage 3)
```

`once_cell` (already at `highper-gateway/Cargo.toml:167`) is **not** needed — Stage 1 uses `std::sync::OnceLock` (stable since Rust 1.70).

`tokio` and `anyhow` are already workspace deps (`highper-gateway/Cargo.toml:12, :76`).

---

## 2. New files (8 files; ~600 LoC)

### 2.1 `src/runtime_config/mod.rs` (~80 LoC)

```rust
//! Central env-var-driven runtime configuration (per ROADMAP §0.1).
//! Design: docs/planning/SETTINGS_SCAFFOLD.md.

mod error;
mod loader;
mod reload;
mod secret_ref;
mod sections;

pub use error::RuntimeConfigError;
pub use loader::load;
pub use reload::Reloadable;
pub use secret_ref::SecretRef;
pub use sections::{ClusterRuntimeConfig, PluginRuntimeConfig};

use arc_swap::ArcSwap;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub cluster: ClusterRuntimeConfig,
    pub plugin: PluginRuntimeConfig,
    // Stage 2: ai, body, shutdown, secrets
    // Stage 3: http3, tls, ratelimit, circuit_breaker, geo, cache, signals, config_watcher, observability
}

impl RuntimeConfig {
    /// Defaults-only constructor for unit tests. Reads no env vars.
    pub fn for_test() -> Self {
        Self {
            cluster: ClusterRuntimeConfig::default(),
            plugin: PluginRuntimeConfig::default(),
        }
    }
}

static CURRENT: OnceLock<ArcSwap<RuntimeConfig>> = OnceLock::new();

pub fn install(c: RuntimeConfig) {
    CURRENT
        .set(ArcSwap::from_pointee(c))
        .map_err(|_| ())
        .expect("runtime_config::install() called twice");
}

pub fn current() -> Arc<RuntimeConfig> {
    CURRENT
        .get()
        .expect("runtime_config not initialized — call install(load()?) in main()")
        .load_full()
}

#[cfg(test)]
pub(crate) fn install_for_test(c: RuntimeConfig) {
    let _ = CURRENT.set(ArcSwap::from_pointee(c));
}
```

### 2.2 `src/runtime_config/error.rs` (~60 LoC)

```rust
use std::fmt;

#[derive(Debug)]
pub enum RuntimeConfigError {
    /// Env var was set but couldn't be parsed.
    ParseError { env_var: String, value: String, expected: &'static str },
    /// Env var value is outside the documented valid range.
    OutOfRange { env_var: String, value: String, valid_range: &'static str },
    /// Cross-subsystem invariant violated (e.g., AI cache=valkey requires Cluster Type B).
    InvalidCombination { rule: &'static str, details: String },
    /// Required env var missing for the enabled feature set.
    MissingRequired { env_var: String, required_because: &'static str },
}

impl fmt::Display for RuntimeConfigError { /* operator-friendly messages with env-var name + value + expected */ }
impl std::error::Error for RuntimeConfigError {}
```

Per `SETTINGS_SCAFFOLD.md` §4 validation policy.

### 2.3 `src/runtime_config/reload.rs` (~50 LoC)

Stage 1 ships only the `Reloadable<T>` marker type (not the SIGHUP handler — that lands Stage 3).

```rust
/// Marks a field as live-reloadable on SIGHUP (atomic swap is safe).
/// Bare fields (without this wrapper) are Restart-required: loaded but
/// reported as pending until process restart.
///
/// Stage 1: defines the type; reload runtime lands in Stage 3.
#[derive(Debug, Clone)]
pub struct Reloadable<T> {
    inner: T,
}

impl<T> Reloadable<T> {
    pub fn new(value: T) -> Self { Self { inner: value } }
    pub fn get(&self) -> &T { &self.inner }
}

impl<T: Default> Default for Reloadable<T> {
    fn default() -> Self { Self::new(T::default()) }
}
```

### 2.4 `src/runtime_config/secret_ref.rs` (~80 LoC)

Stage 1 ships `Literal` + `File` variants only. `Secrets://` URI lands in Stage 2 (needs `SecretsRuntimeConfig`).

```rust
use std::path::PathBuf;
use crate::runtime_config::error::RuntimeConfigError;

#[derive(Debug, Clone)]
pub enum SecretRef {
    Literal(String),
    File { path: PathBuf, lazy: bool },
    // Secrets { uri: String, lazy: bool },  // Stage 2
}

#[derive(Debug, Clone)]
pub struct SecretValue(String);

impl SecretRef {
    /// Parse from env-var value: "literal:foo" / "file:///path?lazy=true" / etc.
    pub fn parse(env_var: &str, value: &str) -> Result<Self, RuntimeConfigError> { /* ... */ }

    /// Eagerly resolve at boot; returns `None` for `lazy=true` File variants.
    pub fn resolve_eager(&self) -> Result<Option<SecretValue>, RuntimeConfigError> { /* ... */ }
}
```

### 2.5 `src/runtime_config/loader.rs` (~80 LoC)

```rust
use crate::runtime_config::{
    RuntimeConfig, RuntimeConfigError,
    sections::{cluster, plugin},
};

pub fn load() -> Result<RuntimeConfig, RuntimeConfigError> {
    let cluster = cluster::load()?;
    let plugin  = plugin::load()?;

    let cfg = RuntimeConfig { cluster, plugin };
    validate_cross_subsystem(&cfg)?;
    Ok(cfg)
}

fn validate_cross_subsystem(_cfg: &RuntimeConfig) -> Result<(), RuntimeConfigError> {
    // Stage 1: no cross-subsystem checks (only cluster + plugin; no inter-section invariants).
    // Stage 2 adds: AI cache=valkey requires Cluster Type B configured (unless allow_single_node).
    Ok(())
}
```

### 2.6 `src/runtime_config/sections/mod.rs` (~10 LoC)

```rust
pub mod cluster;
pub mod plugin;

pub use cluster::ClusterRuntimeConfig;
pub use plugin::PluginRuntimeConfig;
```

### 2.7 `src/runtime_config/sections/cluster.rs` (~200 LoC — heaviest Stage 1 file)

Mirrors `ROADMAP.md` §11.2 + `OWNER_GATES_2026-05-03.md` §6 #4 defaults (Type B = Valkey when Group B UC enabled; Type C = etcd when Group C UC enabled). All 5 startup validations from §11.2 land here (rules 1, 2, 3, 5 partially deferred — see §8 risk #4 below).

```rust
use std::net::SocketAddr;
use crate::config::env_override::{env_string, env_bool};
use crate::runtime_config::{Reloadable, RuntimeConfigError, SecretRef};

#[derive(Debug, Clone, Default)]
pub struct ClusterRuntimeConfig {
    pub infra: ClusterInfra,
    pub typeb_backend: Reloadable<TypeBBackend>,
    pub typeb_addrs: Reloadable<Vec<SocketAddr>>,
    pub typeb_auth: Reloadable<Option<SecretRef>>,
    pub typeb_tls: Reloadable<bool>,
    pub typec_backend: Reloadable<TypeCBackend>,
    pub typec_addrs: Reloadable<Vec<SocketAddr>>,
    pub typec_client_cert: Reloadable<Option<SecretRef>>,
    pub typec_client_key:  Reloadable<Option<SecretRef>>,
    pub typec_ca:          Reloadable<Option<SecretRef>>,
    pub peer_discovery: PeerDiscoveryMode,
    pub peers: Vec<SocketAddr>,
    pub allow_single_node: bool,
    pub allow_insecure: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClusterInfra { #[default] Single, K8s, Vm, BareMetal }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypeBBackend { #[default] None, Valkey, Redis }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TypeCBackend { #[default] None, Etcd, Consul, Raft }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PeerDiscoveryMode { #[default] None, Static, K8sHeadless, Consul, Dns }

pub(crate) fn load() -> Result<ClusterRuntimeConfig, RuntimeConfigError> {
    let infra            = parse_infra(env_string("CLUSTER_INFRA"))?;
    let typeb_backend    = parse_typeb_backend(env_string("CLUSTER_TYPEB_BACKEND"))?;
    let typeb_addrs      = parse_addr_list("CLUSTER_TYPEB_ADDRS", env_string("CLUSTER_TYPEB_ADDRS"))?;
    let typeb_auth       = env_string("CLUSTER_TYPEB_AUTH").map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEB_AUTH", &v)).transpose()?;
    let typeb_tls        = env_bool("CLUSTER_TYPEB_TLS").unwrap_or(false);
    let typec_backend    = parse_typec_backend(env_string("CLUSTER_TYPEC_BACKEND"))?;
    let typec_addrs      = parse_addr_list("CLUSTER_TYPEC_ADDRS", env_string("CLUSTER_TYPEC_ADDRS"))?;
    let typec_client_cert= env_string("CLUSTER_TYPEC_CLIENT_CERT").map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CLIENT_CERT", &v)).transpose()?;
    let typec_client_key = env_string("CLUSTER_TYPEC_CLIENT_KEY").map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CLIENT_KEY", &v)).transpose()?;
    let typec_ca         = env_string("CLUSTER_TYPEC_CA").map(|v| SecretRef::parse("HIGHPER_CLUSTER_TYPEC_CA", &v)).transpose()?;
    let peer_discovery   = parse_peer_discovery(env_string("CLUSTER_PEER_DISCOVERY"))?;
    let peers            = parse_addr_list("CLUSTER_PEERS", env_string("CLUSTER_PEERS"))?;
    let allow_single_node= env_bool("CLUSTER_ALLOW_SINGLE_NODE").unwrap_or(false);
    let allow_insecure   = env_bool("CLUSTER_ALLOW_INSECURE").unwrap_or(false);

    let cfg = ClusterRuntimeConfig {
        infra,
        typeb_backend: Reloadable::new(typeb_backend),
        typeb_addrs:   Reloadable::new(typeb_addrs),
        typeb_auth:    Reloadable::new(typeb_auth),
        typeb_tls:     Reloadable::new(typeb_tls),
        typec_backend: Reloadable::new(typec_backend),
        typec_addrs:   Reloadable::new(typec_addrs),
        typec_client_cert: Reloadable::new(typec_client_cert),
        typec_client_key:  Reloadable::new(typec_client_key),
        typec_ca:          Reloadable::new(typec_ca),
        peer_discovery, peers, allow_single_node, allow_insecure,
    };

    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &ClusterRuntimeConfig) -> Result<(), RuntimeConfigError> {
    // Rule 4 (§11.2): k8s_headless requires CLUSTER_INFRA=k8s
    if cfg.peer_discovery == PeerDiscoveryMode::K8sHeadless && cfg.infra != ClusterInfra::K8s {
        return Err(RuntimeConfigError::InvalidCombination {
            rule: "k8s_headless peer discovery requires HIGHPER_CLUSTER_INFRA=k8s",
            details: format!("got infra={:?}", cfg.infra),
        });
    }
    // Rule 4 (§11.2): static peer discovery requires HIGHPER_CLUSTER_PEERS non-empty
    if cfg.peer_discovery == PeerDiscoveryMode::Static && cfg.peers.is_empty() {
        return Err(RuntimeConfigError::MissingRequired {
            env_var: "HIGHPER_CLUSTER_PEERS".into(),
            required_because: "peer_discovery=static",
        });
    }
    // Rules 1, 2, 3, 5 from §11.2: cross-section validations against UC enablement.
    // Stage 1 cannot enforce these (UC enablement lives in `Config`, not `RuntimeConfig`).
    // Stage 2 wires this when `Config` is loaded after `runtime_config::install()`:
    //   validate_cross_subsystem(&Config, &RuntimeConfig)
    Ok(())
}

// helpers: parse_typeb_backend, parse_typec_backend, parse_infra,
//          parse_peer_discovery, parse_addr_list — all return RuntimeConfigError
//          on bad value with operator-friendly message.
```

### 2.8 `src/runtime_config/sections/plugin.rs` (~60 LoC — smallest section, validates the pattern)

```rust
use std::time::Duration;
use crate::config::env_override::env_duration;
use crate::runtime_config::{Reloadable, RuntimeConfigError};

#[derive(Debug, Clone)]
pub struct PluginRuntimeConfig {
    pub drain: Reloadable<Duration>,        // HIGHPER_PLUGIN_DRAIN  (default: 30s)  [Live]
    pub idle_poll: Reloadable<Duration>,    // HIGHPER_PLUGIN_IDLE_POLL (default: 100ms) [Live]
}

impl Default for PluginRuntimeConfig {
    fn default() -> Self {
        Self {
            drain: Reloadable::new(Duration::from_secs(30)),
            idle_poll: Reloadable::new(Duration::from_millis(100)),
        }
    }
}

pub(crate) fn load() -> Result<PluginRuntimeConfig, RuntimeConfigError> {
    let drain = env_duration("PLUGIN_DRAIN").unwrap_or(Duration::from_secs(30));
    if drain < Duration::from_secs(5) {
        return Err(RuntimeConfigError::OutOfRange {
            env_var: "HIGHPER_PLUGIN_DRAIN".into(),
            value: format!("{:?}", drain),
            valid_range: ">= 5s",
        });
    }
    let idle_poll = env_duration("PLUGIN_IDLE_POLL").unwrap_or(Duration::from_millis(100));

    Ok(PluginRuntimeConfig {
        drain: Reloadable::new(drain),
        idle_poll: Reloadable::new(idle_poll),
    })
}
```

Range check (`>= 5s`) per `ROADMAP.md` §0.J line 727 ("Validates ≥ 5 s.").

---

## 3. Modified files (4 files; ~15 LoC changed)

### 3.1 `src/lib.rs` (+1 line)

```rust
// add after `pub mod plugin;` (currently at lib.rs:34):
pub mod runtime_config;
```

### 3.2 `src/main.rs` (+5 lines, in the `Start` command handler before `Runtime::new`)

```rust
use highper_gateway::runtime_config;

// in Commands::Start handler, before constructing Runtime:
let rt_cfg = runtime_config::load()
    .context("failed to load runtime configuration from HIGHPER_* env vars")?;
runtime_config::install(rt_cfg);
info!("RuntimeConfig loaded and installed");
```

### 3.3 `src/plugin/manager.rs` (`-2 lines, +3 lines` at the cited site)

`src/plugin/manager.rs:252-266` BEFORE:

```rust
async fn wait_for_plugin_idle(&self, plugin: &BoxedPlugin) {
    let start = Instant::now();
    let timeout = std::time::Duration::from_secs(30);

    while plugin.active_requests() > 0 {
        if start.elapsed() > timeout {
            tracing::warn!(
                "Plugin {} still has {} active requests after 30s, forcing unload",
                plugin.name(),
                plugin.active_requests()
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
```

AFTER:

```rust
async fn wait_for_plugin_idle(&self, plugin: &BoxedPlugin) {
    let cfg = crate::runtime_config::current();
    let timeout = *cfg.plugin.drain.get();
    let poll_interval = *cfg.plugin.idle_poll.get();
    let start = Instant::now();

    while plugin.active_requests() > 0 {
        if start.elapsed() > timeout {
            tracing::warn!(
                "Plugin {} still has {} active requests after {:?}, forcing unload",
                plugin.name(),
                plugin.active_requests(),
                timeout,
            );
            break;
        }
        tokio::time::sleep(poll_interval).await;
    }
}
```

Citations: `src/plugin/manager.rs:254` (hardcoded `Duration::from_secs(30)`) → reads from `runtime_config::current().plugin.drain`. `src/plugin/manager.rs:265` (hardcoded `Duration::from_millis(100)`) → reads from `runtime_config::current().plugin.idle_poll`. Both verified by `Read` of `manager.rs:240-267` 2026-05-03.

### 3.4 `Cargo.toml` (+1 line)

`arc-swap = "1.7"` under `[dependencies]`.

---

## 4. New tests (~150 LoC across 3 in-file modules + 1 integration test)

### 4.1 `src/runtime_config/sections/cluster.rs` `#[cfg(test)] mod tests`

- `defaults_when_no_env_vars_set`: `RuntimeConfig::for_test().cluster.infra == ClusterInfra::Single` + all backends `None`.
- `parses_typeb_valkey_with_addrs`: set `HIGHPER_CLUSTER_TYPEB_BACKEND=valkey` + `HIGHPER_CLUSTER_TYPEB_ADDRS=10.0.0.1:6379,10.0.0.2:6379`; verify struct.
- `rejects_invalid_typeb_backend`: set `HIGHPER_CLUSTER_TYPEB_BACKEND=memcached`; expect `RuntimeConfigError::ParseError`.
- `rejects_k8s_headless_when_infra_is_vm`: validation rule 4.
- `rejects_static_discovery_with_empty_peers`: validation rule 4.

Tests use `temp_env::with_vars` (see §8 risk #3) or `serial_test` to scope env-var mutation.

### 4.2 `src/runtime_config/sections/plugin.rs`

- `default_drain_is_30s`.
- `parses_HIGHPER_PLUGIN_DRAIN_45s`.
- `rejects_drain_below_5s`.

### 4.3 `src/runtime_config/loader.rs`

- `load_with_no_env_vars_returns_defaults`.
- `install_then_current_round_trips`.
- `install_twice_panics`.

### 4.4 Integration test `tests/runtime_config_e2e.rs` (~30 LoC)

Spins up a child process with `HIGHPER_PLUGIN_DRAIN=45s` set and asserts the loaded value via a test-only `--print-runtime-config` CLI flag (or via a unit helper if the CLI flag is too much for Stage 1).

---

## 5. CI lint (Stage 1 scope: `src/plugin/` only)

Per the §10 sign-off question on shell choice, ships as either `xtask/` Rust binary or as inline GitHub Actions step. Logic:

```bash
#!/usr/bin/env bash
# Stage 1 scope: src/plugin/ only.
set -euo pipefail

violations=$(grep -rn 'std::env::var' highper-gateway/src/plugin/ || true)
if [ -n "$violations" ]; then
    echo "ERROR: bare std::env::var found in src/plugin/. Use runtime_config::current() instead."
    echo "$violations"
    exit 1
fi

violations=$(grep -rn 'Duration::from_secs\|Duration::from_millis\|Duration::from_micros' highper-gateway/src/plugin/ \
    | grep -v '// allow:' || true)
if [ -n "$violations" ]; then
    echo "ERROR: hardcoded Duration::from_* found in src/plugin/. Use runtime_config::current() or add // allow: <reason>."
    echo "$violations"
    exit 1
fi
```

(Stage 2 expands the path list; Stage 3 goes project-wide.)

---

## 6. `docs/CONFIG_ENV.md` (NEW, ~80 LoC for Stage 1)

Skeleton with two sections (cluster + plugin) only. Stage 2 / 3 grow it.

```markdown
# HIGHPER_* environment variables — authoritative reference

Per ROADMAP.md §0.1: every operator-tunable value loads from a HIGHPER_*
environment variable at startup. Design: docs/planning/SETTINGS_SCAFFOLD.md.

## Cluster (sections/cluster.rs)
| Env var | Default | Valid range | Reload tier | Read at |
|---|---|---|---|---|
| HIGHPER_CLUSTER_INFRA | `single` | `k8s|vm|baremetal|single` | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEB_BACKEND | `none` | `valkey|redis|none` | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEB_ADDRS | (empty) | comma-separated host:port | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEB_AUTH | (empty) | SecretRef (literal:/file://) | Live | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEB_TLS | `false` | bool | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEC_BACKEND | `none` | `etcd|consul|raft|none` | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEC_ADDRS | (empty) | comma-separated host:port | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEC_CLIENT_CERT | (empty) | SecretRef | Live | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEC_CLIENT_KEY | (empty) | SecretRef | Live | sections/cluster.rs |
| HIGHPER_CLUSTER_TYPEC_CA | (empty) | SecretRef | Live | sections/cluster.rs |
| HIGHPER_CLUSTER_PEER_DISCOVERY | `none` | `static|k8s_headless|consul|dns|none` | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_PEERS | (empty) | comma-separated host:port | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_ALLOW_SINGLE_NODE | `false` | bool | Restart | sections/cluster.rs |
| HIGHPER_CLUSTER_ALLOW_INSECURE | `false` | bool | Restart | sections/cluster.rs |

## Plugin (sections/plugin.rs)
| Env var | Default | Valid range | Reload tier | Read at |
|---|---|---|---|---|
| HIGHPER_PLUGIN_DRAIN | `30s` | `>= 5s` | Live | sections/plugin.rs |
| HIGHPER_PLUGIN_IDLE_POLL | `100ms` | duration | Live | sections/plugin.rs |

## Adding a new env var (PR checklist)
[copy from SETTINGS_SCAFFOLD.md §7]
```

---

## 7. Acceptance script (run before merging Stage 1)

```bash
# 1. Build + unit tests pass
cd highper-gateway
cargo build
cargo test --lib runtime_config

# 2. Defaults work (no env vars set)
unset $(env | grep '^HIGHPER_CLUSTER_\|^HIGHPER_PLUGIN_' | cut -d= -f1)
cargo run --bin highper-gateway -- --help  # should not panic on RuntimeConfig load

# 3. Cluster env vars round-trip
HIGHPER_CLUSTER_TYPEB_BACKEND=valkey \
HIGHPER_CLUSTER_TYPEB_ADDRS=127.0.0.1:6379 \
cargo test --lib runtime_config::sections::cluster::tests::parses_typeb_valkey_with_addrs

# 4. Plugin env var migrated
HIGHPER_PLUGIN_DRAIN=45s cargo test --lib runtime_config::sections::plugin

# 5. Validation refuses bad input
HIGHPER_PLUGIN_DRAIN=2s cargo run --bin highper-gateway 2>&1 | grep "OutOfRange.*PLUGIN_DRAIN"

# 6. CI lint catches new violations
echo 'use std::env; let _ = env::var("HIGHPER_TEST");' >> highper-gateway/src/plugin/manager.rs
bash xtask/lint_runtime_config.sh  # should exit 1
git checkout highper-gateway/src/plugin/manager.rs
```

---

## 8. Risks / gotchas

1. **`arc-swap` dep adoption.** First time it lands; minor; mature crate. Verify with `cargo deny check` before merging.
2. **`OnceLock` vs `Lazy`.** `OnceLock::set()` returns `Result` because of the once semantics; `install()` panics on second call. Test covers this (`install_twice_panics`). For test isolation, use `install_for_test()` with a `serial_test` attribute since `OnceLock` is process-global.
3. **Test parallelism + env vars.** Cargo runs tests in parallel by default. Tests that mutate `HIGHPER_*` env vars must use `serial_test::serial` or scope env mutation via `temp_env::with_vars`. Verify the `temp_env` crate is acceptable as a dev-dep before adding (sign-off question §10 #4).
4. **§11.2 rules 1, 2, 3, 5 deferred.** These need to read the enabled UC list from `Config` (the user-facing config-file struct), which `RuntimeConfig` doesn't see at load time. Stage 2 wires this when `Config` is loaded in `main.rs` after `runtime_config::install()` — `validate_cross_subsystem(&Config, &RuntimeConfig)` runs then. **Stage 1 acceptance accepts this gap explicitly.**
5. **`SecretRef::parse` URI grammar.** Stage 1 uses a simple format (`literal:foo`, `file:///path?lazy=true`). If this turns out wrong, only `secret_ref.rs` needs revision — call sites only see `SecretRef::Literal` / `SecretRef::File`.
6. **Windows file path in `file://` URIs.** `file:///C:/path` works on Windows; `file:///path` fails. Document in `CONFIG_ENV.md`. Tested separately on Windows in CI.
7. **`.gitignore` for `data/highper-ai`.** Stage 2 lands the `HIGHPER_AI_STATE_PATH` default `./data/highper-ai`; Stage 1 is unaffected, but worth queueing the `.gitignore` line for Stage 2.

---

## 9. Out of Stage 1 scope

- SIGHUP reload runtime (Stage 3).
- Admin diff endpoint (Stage 3).
- AI / body / shutdown / secrets / observability / http3 / tls / ratelimit / circuit_breaker / geo / cache / signals / config_watcher sections (Stage 2 + 3).
- Project-wide CI lint (Stage 3).
- `src/config/defaults.rs` literal migration (Stage 3).
- `Secrets://` URI variant of `SecretRef` (Stage 2 — needs `SecretsRuntimeConfig`).

---

## 10. Sign-off questions (answer before Stage 1 PR begins)

1. Plan approved as-is, or specific changes?
2. Single PR for all of Stage 1, or split (e.g., scaffold-only PR first, then cluster + plugin PRs)?
3. `arc-swap = "1.7"` dep acceptable, or want me to evaluate alternatives (e.g., `parking_lot::RwLock<Arc<...>>`) first?
4. `temp_env` dev-dep acceptable for env-var test isolation, or use `serial_test` only (already-evaluated alternative)?
5. CI lint shipped as `xtask/lint_runtime_config.sh` (Bash), as `xtask/` Rust binary (cross-platform), or as a GitHub Actions inline step (no Bash on Windows runners)?

---

## 11. PR commit shape (when Stage 1 lands)

Recommended: single commit, structured commit message with stage label.

```
runtime_config: Stage 1 - foundation + cluster + plugin sections

- Adds src/runtime_config/ module per docs/planning/SETTINGS_SCAFFOLD.md
- ClusterRuntimeConfig mirrors ROADMAP §11.2 (14 env vars)
- PluginRuntimeConfig (2 env vars: HIGHPER_PLUGIN_DRAIN, _IDLE_POLL)
- Migrates src/plugin/manager.rs:254 hardcoded 30s + :265 hardcoded 100ms
- arc-swap 1.7 dep added for Tier 1 hot-reload (handler lands Stage 3)
- Reloadable<T> marker type defined; SIGHUP runtime deferred to Stage 3
- SecretRef Literal + File variants only; Secrets:// deferred to Stage 2
- Cross-subsystem validator skeleton; AI/Cluster invariants deferred to Stage 2
- CI lint scoped to src/plugin/ only; widens in Stage 2
- docs/CONFIG_ENV.md skeleton with cluster + plugin sections

Stage 2 (~5d): ai + body + shutdown + secrets + cross-subsystem validator.
Stage 3 (~4d): remaining 9 sections + Tier 1 SIGHUP + admin diff + project-wide lint.
```

---

*Plan author: claude-opus-4-7-1m, 2026-05-03. Owner sign-off required on §10 questions before any source code lands. Per CLAUDE.md rules.*
