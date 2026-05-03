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

Migrates the previously-hardcoded `Duration::from_secs(30)` at
`src/plugin/manager.rs:255` and `Duration::from_millis(100)` at
`src/plugin/manager.rs:268` (post-migration line numbers; pre-migration
were `:254` and `:265` respectively).

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
