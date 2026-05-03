# `HIGHPER_*` environment variables — authoritative reference

Per [`ROADMAP.md` §0.1](planning/ROADMAP.md): every operator-tunable value
loads from a `HIGHPER_*` environment variable at startup. Hot paths must
not call `std::env::var`. Design:
[`SETTINGS_SCAFFOLD.md`](planning/SETTINGS_SCAFFOLD.md). Stage 1 plan:
[`RUNTIME_CONFIG_STAGE1_PR_PLAN.md`](planning/RUNTIME_CONFIG_STAGE1_PR_PLAN.md).

This document grows with each Stage of Workstream 0.J:

- **Stage 1** (this revision): `Cluster` + `Plugin` sections.
- **Stage 2** (queued): `Ai` + `Body` + `Shutdown` + `Secrets` sections.
- **Stage 3** (queued): remaining 9 sections + reload-tier semantics fully
  enforced via SIGHUP atomic swap.

The `Reload tier` column documents how a value behaves on a future SIGHUP
reload (Stage 3 ships the runtime; Stage 1 ships the metadata):

- **Live** — the SIGHUP handler atomically swaps the new value; in-flight
  operations finish with the old value, subsequent reads see the new one.
- **Restart** — the new value is loaded but reported as "pending" via the
  admin diff endpoint; takes effect on next process start.

---

## Cluster (`src/runtime_config/sections/cluster.rs`)

Mirrors [`ROADMAP.md` §11.2](planning/ROADMAP.md). The four cluster types
emerge from the combination of the two backend flags (both `none` → Type 1
Stateless; only B → Type 2 +Valkey; only C → Type 3 +etcd; both → Type 4
+Valkey+etcd). Defaults per
[`OWNER_GATES_2026-05-03.md`](planning/OWNER_GATES_2026-05-03.md) §6 #4.

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_CLUSTER_INFRA` | `single` | `k8s` \| `vm` \| `baremetal` \| `single` | Restart |
| `HIGHPER_CLUSTER_TYPEB_BACKEND` | `none` | `valkey` \| `redis` \| `none` | Restart |
| `HIGHPER_CLUSTER_TYPEB_ADDRS` | (empty) | comma-separated `host:port` | Restart |
| `HIGHPER_CLUSTER_TYPEB_AUTH` | (empty) | `SecretRef` (`literal:<val>` \| `file:///<path>[?lazy=true]`) | Live |
| `HIGHPER_CLUSTER_TYPEB_TLS` | `false` | bool (`true`/`1`/`yes`/`on` \| `false`/`0`/`no`/`off`) | Restart |
| `HIGHPER_CLUSTER_TYPEC_BACKEND` | `none` | `etcd` \| `consul` \| `raft` \| `none` | Restart |
| `HIGHPER_CLUSTER_TYPEC_ADDRS` | (empty) | comma-separated `host:port` | Restart |
| `HIGHPER_CLUSTER_TYPEC_CLIENT_CERT` | (empty) | `SecretRef` | Live |
| `HIGHPER_CLUSTER_TYPEC_CLIENT_KEY` | (empty) | `SecretRef` | Live |
| `HIGHPER_CLUSTER_TYPEC_CA` | (empty) | `SecretRef` | Live |
| `HIGHPER_CLUSTER_PEER_DISCOVERY` | `none` | `static` \| `k8s_headless` \| `consul` \| `dns` \| `none` | Restart |
| `HIGHPER_CLUSTER_PEERS` | (empty) | comma-separated `host:port` | Restart |
| `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE` | `false` | bool | Restart |
| `HIGHPER_CLUSTER_ALLOW_INSECURE` | `false` | bool | Restart |

**Validation rules at boot (refuse to start on failure):**

- §11.2 rule 4 (enforced Stage 1):
  - `HIGHPER_CLUSTER_PEER_DISCOVERY=k8s_headless` requires `HIGHPER_CLUSTER_INFRA=k8s`.
  - `HIGHPER_CLUSTER_PEER_DISCOVERY=static` requires non-empty `HIGHPER_CLUSTER_PEERS`.
- §11.2 rules 1, 2, 3, 5 (deferred to Stage 2 — need the enabled-UC list
  from `Config`, which `RuntimeConfig` doesn't see at load time).

---

## Plugin (`src/runtime_config/sections/plugin.rs`)

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_PLUGIN_DRAIN` | `30s` | `>= 5s` (duration: `ms`/`s`/`m`/`h`/`d` suffix) | Live |
| `HIGHPER_PLUGIN_IDLE_POLL` | `100ms` | duration (`ms`/`s`/`m`/`h`/`d` suffix) | Live |
| `HIGHPER_PLUGIN_HOT_RELOAD_SETTLE` | `100ms` | duration | Live |

Migrates the previously-hardcoded `Duration::from_secs(30)` at
`src/plugin/manager.rs:255` and `Duration::from_millis(100)` at
`src/plugin/manager.rs:268` (post-migration line numbers; pre-migration
were `:254` and `:265` respectively). Stage 2 also migrated
`Duration::from_millis(100)` at `src/plugin/hot_reload.rs:181` to
`HIGHPER_PLUGIN_HOT_RELOAD_SETTLE`.

---

## AI / UC16 (`src/runtime_config/sections/ai.rs`) — Stage 2

25 env vars across 5 logical groups. Sources:
[`USECASE_16_AI_LLM_GATEWAY.md`](planning/USECASE_16_AI_LLM_GATEWAY.md) §3.4
(routing) / §3.5 (streaming) / §3.6 (cluster behaviour) / §5.5 (pricing) /
§6.0 (cache) / §7.1.3 (virtual-key pepper);
[`ROADMAP.md`](planning/ROADMAP.md) §5 Phase 0.J task lines 726–733.

### Secrets / state

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_KEY_PEPPER` | (empty) | `SecretRef` | Live |
| `HIGHPER_AI_STATE_BACKEND` | `redb` | `redb` \| `rocksdb` \| `scylladb` | Restart |
| `HIGHPER_AI_STATE_PATH` | `./data/highper-ai` | filesystem path | Restart |

### Caching / vector

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_CACHE_BACKEND` | `valkey` | `valkey` \| `redis` \| `memory` \| `disk` \| `multi-tier` | Restart |
| `HIGHPER_AI_VECTOR_BACKEND` | `none` | `none` \| `qdrant` \| `redis-stack` \| `pgvector` \| `hnsw` | Restart |
| `HIGHPER_AI_VECTOR_ADDRS` | (empty) | comma-separated `host:port` (required when backend ∈ {qdrant, redis-stack, pgvector}) | Restart |
| `HIGHPER_AI_VECTOR_AUTH` | (empty) | `SecretRef` | Live |

### Routing

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_RETRY_BUDGET` | `3` | `1..=10` | Live |
| `HIGHPER_AI_COOLDOWN_BACKEND` | `auto` | `auto` \| `valkey` \| `local` | Restart |
| `HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN` | `50` | u64 ms (must be < BACKOFF_MS_MAX) | Live |
| `HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX` | `200` | u64 ms (must be > BACKOFF_MS_MIN) | Live |
| `HIGHPER_AI_DEFAULT_COOLDOWN_SECS_NO_HEADER` | `30` | u64 seconds | Live |

### Streaming

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_STREAM_BUFFER_DEPTH` | `64` | `1..=4096` | Live |
| `HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY` | `drop_oldest` | `drop_oldest` \| `block` \| `error` | Live |
| `HIGHPER_AI_DEFAULT_CANCEL_ON_CLOSE` | `true` | bool | Live |
| `HIGHPER_AI_DEFAULT_TPM_HARD_STOP` | `false` | bool | Live |

### Cluster behaviour

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_VALKEY_FAIL_MODE` | `local_fallback` | `local_fallback` \| `fail_open` \| `fail_closed` | Live |
| `HIGHPER_AI_TOKEN_QUOTA_KEY_SHARDS` | `1` | `1..=1024` | Restart (re-sharding needs cluster coordination) |
| `HIGHPER_AI_KEY_CACHE_TTL_SECS` | `300` | u64 seconds | Live |
| `HIGHPER_AI_PRICING_OVERRIDE_CACHE_TTL_SECS` | `60` | u64 seconds | Live |

### Pricing (nested `AiPricingRuntimeConfig`)

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_AI_PRICING_FEED_URL` | (empty → use embedded snapshot) | URL | Live |
| `HIGHPER_AI_PRICING_FEED_SIGN_KEY` | (empty) | `SecretRef` | Live |
| `HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS` | `604800` (7d) | `>= 3600` | Live |
| `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE` | `last_known_good` | `last_known_good` \| `fail_closed` | Live |
| `HIGHPER_AI_ALLOW_FREE_TIER` | `false` | bool | Live |

### Cross-subsystem invariants (enforced at boot)

- `HIGHPER_AI_CACHE_BACKEND=valkey` requires `HIGHPER_CLUSTER_TYPEB_BACKEND` set, OR `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE=true`.
- `HIGHPER_AI_COOLDOWN_BACKEND=valkey` (forced) requires `HIGHPER_CLUSTER_TYPEB_BACKEND` set.
- `HIGHPER_AI_VECTOR_BACKEND` ∈ `{qdrant, redis-stack, pgvector}` requires `HIGHPER_AI_VECTOR_ADDRS` non-empty.

---

## Body (`src/runtime_config/sections/body.rs`) — Stage 2 (B12)

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_BODY_MAX_REQUEST` | `10MB` | byte size (`KB` / `MB` / `GB` suffix; bare = bytes) | Live |
| `HIGHPER_BODY_MAX_STREAMING` | `100MB` | byte size | Live |
| `HIGHPER_BODY_MAX_FORM` | `1MB` | byte size | Live |

Stage 3 wires the consumers: replaces literal `10 * 1024 * 1024` and similar in `src/middleware/`, `src/proxy/`, `src/http/`.

---

## Shutdown (`src/runtime_config/sections/shutdown.rs`) — Stage 2 (B14)

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_SHUTDOWN_DRAIN` | `30` (seconds) | u64 seconds | Live |
| `HIGHPER_SHUTDOWN_FORCE_KILL` | `60` (seconds) | u64 seconds (must be > drain) | Live |
| `HIGHPER_SHUTDOWN_SPAWN_TASK_DRAIN` | `10` (seconds) | u64 seconds | Live |

### Validation

- `force_kill` must be `> drain`.

---

## Secrets (`src/runtime_config/sections/secrets.rs`) — Stage 2 (Phase 1.4 prep)

| Env var | Default | Valid range | Reload tier |
|---|---|---|---|
| `HIGHPER_SECRETS_PROVIDER` | `env` | `env` \| `file` \| `vault` \| `aws` \| `k8s` | Restart |
| `HIGHPER_SECRETS_VAULT_URL` | (empty) | URL (required when provider=vault) | Restart |
| `HIGHPER_SECRETS_VAULT_TOKEN` | (empty) | `SecretRef` (required when provider=vault) | Live |
| `HIGHPER_SECRETS_AWS_REGION` | (empty) | region string (required when provider=aws) | Restart |
| `HIGHPER_SECRETS_K8S_NAMESPACE` | (empty) | namespace name (required when provider=k8s) | Restart |
| `HIGHPER_SECRETS_CACHE_TTL` | `300` | u64 seconds | Live |

### `SecretRef` URI grammar (Stage 2 update)

Stage 2 added the `secrets://` variant to the `SecretRef` URI grammar. Resolution is **stubbed** at this stage — actual Vault / AWS Secrets Manager / K8s Secret resolvers ship in Phase 1.4 with `SecretsRuntimeConfig.provider`.

| Form | Example | Resolution |
|---|---|---|
| `literal:<value>` | `literal:hunter2` | direct (use sparingly; prefer `file://` or `secrets://`) |
| `file:///<path>` | `file:///etc/highper/auth` | read at boot (eager) |
| `file:///<path>?lazy=true` | `file:///etc/highper/auth?lazy=true` | read at first use |
| `secrets://<uri>` | `secrets://kv/secret/highper/ai-pepper` | **NOT YET IMPLEMENTED** — `RuntimeConfigError::InvalidCombination` until Phase 1.4 |
| `secrets://<uri>?lazy=true` | `secrets://kv/secret/highper/ai-pepper?lazy=true` | **NOT YET IMPLEMENTED** |

---

## Adding a new env var (PR checklist)

1. Add the field to the relevant `RuntimeConfig::*` sub-struct in
   `src/runtime_config/sections/<subsystem>.rs`.
2. Tag it `Reloadable<T>` (Live) or hold it bare (Restart) per the §5.4
   classification rules in `SETTINGS_SCAFFOLD.md`.
3. Add the loader block in the same `sections/<subsystem>.rs` file.
4. Add range validation if numeric.
5. Add a row to this document with default + valid range + reload tier.
6. Update `docs/CHANGELOG.md` if the change is operator-visible.
7. CI lint (`cargo lint-runtime-config`) flags any new bare
   `std::env::var` or literal `Duration::from_*` in the Stage's scoped
   paths. Either move the value into `RuntimeConfig` or add a
   `// allow: <reason>` waiver if it is a protocol invariant or test.

---

## URI grammar for `SecretRef`

Used by env vars whose type is `SecretRef`. Stage 1 ships `Literal` and
`File` variants; `Secrets://` lands in Stage 2 with `SecretsRuntimeConfig`.

| Form | Example | Resolution |
|---|---|---|
| `literal:<value>` | `literal:hunter2` | direct (use sparingly; prefer `file://`) |
| `file:///<path>` | `file:///etc/highper/auth` | read at boot (eager) |
| `file:///<path>?lazy=true` | `file:///etc/highper/auth?lazy=true` | read at first use |

On Windows, `file://` paths must be of the form `file:///C:/path/to/file`.
