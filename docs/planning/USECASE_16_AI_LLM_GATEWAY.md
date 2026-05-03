# Use Case 16 — AI / LLM Gateway

**Status:** Design (greenfield — no prior implementation in highper-gateway).
**Author:** claude-opus-4-7-1m, 2026-05-02 (initial); revised 2026-05-02 against scope fence + UC16 design decisions #1–#4.
**Companion:** [`ROADMAP.md`](ROADMAP.md) (phasing + open gates), [`HA_ARCHITECTURE.md`](HA_ARCHITECTURE.md) (cluster behaviour, §3.5 UC16 storage layer).

> **Scope-fence note (2026-05-02).** This document predates the UC16 scope
> fence in `ROADMAP.md` §3.1. **The scope fence is authoritative when the two
> disagree.** This revision applies the fence — the original §8 Guardrails
> section, guardrail entries in §3 module map, the `guardrails {}` DSL block,
> and Beta guardrail items have been removed; "AI observability platforms"
> have been clarified as external; in-memory cache integration as a product
> feature has been clarified as external. Highper-gateway exposes the
> *cache engine* (§6) and the *metrics surface* (§9); the *platform layers*
> (Langfuse / Helicone-dashboards / Phoenix / vLLM / guardrail engines) live
> in customer-side services and integrate via the plugin hooks (§3.3) plus
> the metrics + OTLP surface (§9).

---

## 1. Goal

Deliver a self-hostable AI gateway inside highper-gateway as a **technical
replacement for the gateway/proxy role** played by LiteLLM Proxy and Portkey
in AI-application deployments. The gateway must let an OpenAI-shaped *or*
Anthropic-shaped client SDK talk to **any** of OpenAI, Azure OpenAI,
Anthropic, Google AI Studio, Vertex AI, AWS Bedrock, Mistral, Cohere,
Together, Fireworks, Groq, xAI, DeepSeek, Perplexity, Ollama, vLLM, TGI,
HF Inference, Cloudflare Workers AI, Replicate — with central provider
routing, virtual keys, budgets, caching, observability, and audit. AI
application developers deploy highper-gateway and connect it into their
existing flow; surrounding ecosystem layers (guardrails, AI observability
platforms, vector DBs, inference backends) remain external concerns.

### Non-goals (per `ROADMAP.md` §3.1 scope fence)

- **Not** a guardrail engine. Input/output content filtering, PII redaction, and jailbreak detection live in customer-side services. Highper exposes plugin hooks (see §3.3) so operators can wire their guardrail provider in; highper does not ship guardrail logic.
- **Not** an inference engine. The gateway proxies; it does not host model weights. A local `mistral.rs` / `candle` integration is potential UC17, deliberately deferred.
- **Not** a vector DB. Semantic cache uses an external vector backend (Qdrant / Redis-Stack / PgVector — operator chooses); the **embedding model** is also operator's choice (any provider configured under §3.2). Highper ships the cache engine, not a bundled vector store.
- **Not** an AI observability product. Highper exposes Prometheus + OTLP (§9). Integration with Langfuse, Helicone dashboards, Phoenix, Arize, etc. is via that surface — they remain external products.
- **Not** an in-memory cache product. Highper ships the cache engine (§6), backed by the existing `src/cache/` infrastructure (Type B Valkey or local). Bringing your own in-memory cache layer as a separate product is the operator's decision.
- **Not** an evaluation framework. Eval integration uses external runners (Promptfoo) over logged traffic; the gateway exposes hooks but does not implement scoring.

---

## 2. User-visible API surface

### 2.1 OpenAI-compatible (MVP — UC16 design decision #2, 2026-05-02)

```
POST /v1/chat/completions
POST /v1/embeddings
GET  /v1/models
POST /v1/moderations                   (passthrough only — operator's moderation provider; highper does not implement)
POST /v1/images/generations            (Beta)
POST /v1/audio/transcriptions          (GA)
POST /v1/audio/speech                  (GA)
POST /v1/files                         (GA)
POST /v1/batches                       (Beta)
WS   /v1/realtime                      (GA)
```

### 2.2 Anthropic-compatible (MVP — UC16 design decision #2, 2026-05-02)

```
POST /v1/messages
POST /v1/messages/batches              (Beta)
GET  /v1/models
```

Anthropic-shape inbound is **MVP-tier** alongside OpenAI. Both inbound
shapes funnel into the same canonical `AiRequest` (§4); each outbound
provider is reachable from either inbound. Cost: doubles the inbound
parser surface and adds Anthropic-shape response synthesis, but most of
the request flow (auth, rate-limit, cache, routing, accounting) is
shape-agnostic.

### 2.3 Native passthrough (always-on)

```
ANY  /provider/{name}/{rest...}
```

Used as escape hatch when a feature isn't yet shape-translated. Auth is enforced at the gateway; body is forwarded byte-stable (preserve order) to the provider.

### 2.4 Admin API (extends `src/admin/api.rs`)

```
POST   /admin/ai/keys                  Create virtual key (returns sk-hpgw-...)
GET    /admin/ai/keys                  List keys (no plaintext)
PATCH  /admin/ai/keys/{id}             Update budget / allowlist / RPM-TPM
DELETE /admin/ai/keys/{id}             Revoke

GET    /admin/ai/spend?key=&from=&to=  Per-key spend rollup
POST   /admin/ai/budgets               Create/update budget rule
GET    /admin/ai/models                Effective model registry (alias → provider)
PATCH  /admin/ai/models/{alias}        Override price, capability, allowlist

POST   /admin/ai/prompts               Create prompt template (versioned)
GET    /admin/ai/prompts/{id}/versions
POST   /admin/ai/prompts/{id}/promote  Promote draft to default

GET    /admin/ai/logs?key=&from=&to=   Log query (with redaction policy applied)
POST   /admin/ai/logs/export           SOC2 audit export (signed JSONL)
```

### 2.5 MCP (Beta)

```
ANY    /v1/mcp/{server}                Forward MCP JSON-RPC (HTTP+SSE) to a configured backing server.
```

---

## 3. Architecture

### 3.1 Request flow (chat completion, MVP)

```
Client
  │
  ▼
[hyper http1/h2 listener]                                  (existing src/proxy/server.rs)
  │  - HTTPS terminate (existing src/tls/)
  │  - Decode HTTP/JSON
  ▼
[Virtual-key auth]                                         (extends src/gateway/auth/api_key.rs)
  │  - Validate sk-hpgw-... → tenant + scopes
  │  - 401 if invalid / revoked / expired
  ▼
[Inbound shape detect & parse]                             (NEW src/gateway/ai/shape.rs)
  │  - Decide: openai_chat | openai_embed | anthropic_messages | native
  │  - Parse JSON into shared Request enum
  ▼
[Pre-call rate limit / budget]                             (extends src/gateway/ratelimit, NEW budget)
  │  - RPM, TPM (estimated input tokens), $/day cap
  │  - 429 with Retry-After if exceeded
  ▼
[Pre-call plugin hook (optional)]                          (reuses src/plugin/)
  │  - Operator-supplied guardrail / validator runs here
  │  - Returns Continue / StopIteration / Error
  │  - Highper ships no built-in guardrail logic — see §3.3
  ▼
[Cache lookup]                                             (reuses src/cache/ + NEW semantic.rs)
  │  - Exact key: sha256(canonical_request)
  │  - Semantic key: embed(prompt) → vector ANN; threshold gated
  │  - On hit: return synth response
  ▼
[Route resolve]                                            (NEW src/gateway/ai/router.rs — see §3.4)
  │  - Model alias → ordered list of (provider, key, model_id)
  │  - Layered filtering: per-key allow-list → capability flags → circuit
  │    breaker → 429 cooldown (cooldown state in Type B Valkey when available
  │    else local) → optional session affinity → priority/cost/latency order
  │  - Retry budget: 3 attempts max across providers (per-key override)
  ▼
[Outbound shape translate]                                 (NEW src/gateway/ai/translators/)
  │  - Convert Request → provider-native (OpenAI / Anthropic / Bedrock-Converse / Gemini)
  │  - Sign if SigV4 (Bedrock); add OAuth bearer if Vertex
  ▼
[HTTP client → provider]                                   (reuses src/proxy/connection_pool.rs)
  │  - Per-(provider, region) pool, h2 preferred
  │  - Idle keepalive, generous read timeout (≥600s for reasoning)
  ▼
[SSE stream chunker]                                       (extends src/http/proxy_streaming.rs — see §3.5)
  │  - Parse provider's SSE / event-stream-binary / NDJSON
  │  - Re-emit as inbound-shape SSE
  │  - Per-chunk hooks: token counter, cancellation check, plugin chain
  │    (HIGHPER_PLUGIN_CHUNK_BUDGET_US budget; fail-open on overrun),
  │    cache write buffer, metrics emit
  │  - Bounded buffer (HIGHPER_AI_STREAM_BUFFER_DEPTH; drop-oldest default)
  │  - Cancellation: client SSE close → §3.5.1 behaviour
  │    (cancel_on_close=true default, drain-and-record opt-in per virtual key)
  │  - TPM mid-stream: §3.5.2 (post-stream warning default,
  │    tpm_hard_stop opt-in injects SSE error frame)
  ▼
[Post-call accounting]                                     (NEW src/gateway/ai/accounting.rs)
  │  - Final input/output tokens (provider usage if returned, else local tokenizer)
  │  - $ = tokens × price (model registry)
  │  - Decrement budget, RPM/TPM bucket
  │  - Emit metrics, log entry, audit event
  ▼
Client (SSE complete)
```

### 3.2 Module map (new vs reused)

| New module | Path | Purpose |
|---|---|---|
| `src/gateway/ai/mod.rs` | NEW | Glue, route registration |
| `src/gateway/ai/shape.rs` | NEW | Inbound shape parsing (OpenAI/Anthropic/native) |
| `src/gateway/ai/translators/openai.rs` | NEW | OpenAI ↔ canonical ↔ OpenAI |
| `src/gateway/ai/translators/anthropic.rs` | NEW | Anthropic Messages ↔ canonical |
| `src/gateway/ai/translators/bedrock_converse.rs` | NEW | Converse + SigV4 |
| `src/gateway/ai/translators/gemini.rs` | NEW | `generateContent` / `streamGenerateContent` |
| `src/gateway/ai/translators/{cohere,mistral,...}.rs` | NEW | Beta+ providers |
| `src/gateway/ai/registry.rs` | NEW | Model alias → providers, prices, capabilities |
| `src/gateway/ai/router.rs` | NEW | Strategy: priority list, weighted, conditional, RL-aware |
| `src/gateway/ai/tokens.rs` | NEW | tiktoken (cl100k_base, o200k_base), Anthropic, Llama BPE |
| `src/gateway/ai/budget.rs` | NEW | Per-key/team/project caps, time windows |
| `src/gateway/ai/accounting.rs` | NEW | Cost calc, spend rollups |
| `src/gateway/ai/keys.rs` | NEW | Virtual-key store (Postgres or sled), scopes, hashing |
| `src/gateway/ai/cache_exact.rs` | NEW | Wraps `cache::manager` with canonical key |
| `src/gateway/ai/cache_semantic.rs` | NEW | Engine layer — embed + ANN search, threshold-gated; embedding model via configured AiProvider; vector backend via `VectorIndex` trait |
| `src/gateway/ai/vector_index/mod.rs` | NEW | `VectorIndex` trait — the stable plugin ABI for vector backends (see §6.2 + ROADMAP §4.4 row 11) |
| `src/gateway/ai/vector_index/{qdrant,redis_stack,pgvector,hnsw}.rs` | NEW | Built-in `VectorIndex` impls behind Cargo features (`ai-vector-qdrant`, `ai-vector-redis-stack`, `ai-vector-pgvector`, `ai-vector-hnsw`) |
| `src/gateway/ai/sse.rs` | NEW | SSE-aware chunker; per-chunk callback |
| `src/gateway/ai/prompts.rs` | NEW | Versioned prompt registry |
| `src/gateway/ai/mcp.rs` | NEW | MCP server passthrough |
| `src/gateway/ai/state_store/{mod,redb,rocksdb,scylladb}.rs` | NEW | `AiStateStore` trait + 3 backend impls (see §7 + §3.4) |
| `src/gateway/ai/provider.rs` | NEW | `AiProvider` trait — the stable plugin ABI (see §3.3) |
| `src/gateway/ai/providers/{openai,anthropic}.rs` | NEW | Built-in providers shipped at MVP (UC16 #2) — implementations of the `AiProvider` trait, statically linked into the binary |
| `src/gateway/ai/providers/{bedrock,gemini,…}.rs` | NEW (Phase 3 Beta) | Additional built-in providers |

| Reused module | Used for |
|---|---|
| `src/gateway/auth/{api_key,jwt}.rs` | virtual-key validation |
| `src/gateway/ratelimit/{token_bucket,sliding_window,distributed}.rs` | RPM, TPM, RPD, TPD with dynamic-cost (= tokens) bucket |
| `src/cache/{manager,backends,disk}.rs` | exact-cache backing |
| `src/proxy/{loadbalancer,retry,circuit_breaker,connection_pool}.rs` | provider routing, retry with backoff, fail-shut on hard outage |
| `src/discovery/registry.rs` | provider registry with hot-reload |
| `src/observability/{metrics,structured_logging,tracing}.rs` | metrics, logs, traces (extended with AI tags) |
| `src/admin/{api,routes,config_persistence}.rs` | admin endpoints |
| `src/plugin/{wasm,ffi,trait_def,host_functions}.rs` | hosting third-party AiProvider plugins (§3.3); pre/post hook plugins (operator-supplied guardrail / validation) |
| `src/config/{dsl_parser,loader,reloader,watcher}.rs` | DSL + YAML, hot reload |
| `src/http/{proxy_streaming,streaming_body}.rs` | SSE base |
| `src/middleware/{transform,body_access,streaming_validator}.rs` | shape translation hooks |
| `src/websocket/*` | OpenAI Realtime / Gemini Live (GA) |
| `src/tls/*` | TLS to providers, ALPN h2 negotiation |

### 3.3 Plugin model for providers (UC16 design decision #3, 2026-05-02)

**Decision recorded.** Provider integrations (OpenAI, Anthropic, Bedrock,
Gemini, …) are designed to be **plugin-loadable**, so that third parties
(model vendors, on-prem inference operators, custom cloud LLM services) can
ship their own provider plugins without modifying highper-gateway core.

#### 3.3.1 Why a separate trait, not extending `Plugin`

The existing plugin trait at `src/plugin/trait_def.rs:35-100` is
**request-pipeline shaped**: hooks are `on_request_headers`,
`on_request_body`, `on_response_headers`, `on_response_body`. That fits
guardrails / validators / transformers but not provider drivers.

`AiProvider` is **call-shaped**: translate canonical request → provider
request, send via HTTP/2 (or whatever the provider speaks), receive a
streaming response, translate → canonical events, surface tokens / cost /
errors. That's a fundamentally different lifecycle.

So `AiProvider` is its own trait — same architectural pattern as
`WafEngine` (`src/middleware/waf/engine.rs:10-40`) and `Compressor`
(`src/middleware/compression/compressor.rs:70-127`) which are also separate
traits, also pluggable, also load via the same `src/plugin/` machinery.

#### 3.3.2 Trait sketch

```rust
// src/gateway/ai/provider.rs (sketch — stabilises during MVP)
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider name as used in DSL/YAML (e.g. "openai", "anthropic", "bedrock-us-east-1")
    fn name(&self) -> &str;

    /// Provider metadata: declared models, capability flags, supported regions
    fn metadata(&self) -> ProviderMetadata;

    /// Estimate input tokens before calling — used for budget gate.
    /// Cheap; runs every request.
    fn estimate_input_tokens(&self, req: &AiRequest) -> u32;

    /// Translate canonical → provider request, send, return event stream.
    async fn call_streaming(
        &self,
        req: &AiRequest,
        cx: &CallContext,
    ) -> Result<BoxStream<'static, AiStreamEvent>, ProviderError>;

    /// Optional: declared health (last-seen status, 429 cooldown).
    fn health(&self) -> ProviderHealth { ProviderHealth::default() }

    /// Optional: parse `x-ratelimit-*` headers from provider response.
    fn parse_ratelimit_headers(&self, headers: &HeaderMap) -> Option<RateLimitState> { None }
}
```

Concrete return types (`AiRequest`, `AiStreamEvent`, `ProviderMetadata`,
`CallContext`, `ProviderError`, `RateLimitState`) live in
`src/gateway/ai/types.rs` and form the **stable ABI** that third-party
plugins compile against.

#### 3.3.3 Loading mechanisms

| Loading mode | When to use | Status |
|---|---|---|
| **Static (built-in)** | OpenAI + Anthropic at MVP; Bedrock + Gemini in Phase 3 Beta | **MVP** — vendors register themselves at startup via `inventory` crate or a manual `Registry::register_static()` call |
| **FFI dylib (`.so` / `.dll`)** | Partner-maintained native plugins; same-process speed | **Phase 3 Beta** — reuses `src/plugin/ffi.rs` infrastructure; requires Rust ABI stability between highper and the plugin (or a `extern "C"` shim) |
| **WASM (`.wasm`)** | Community plugins, sandboxed | **Phase 3 Beta** — reuses `src/plugin/wasm.rs` (wasmtime, fuel limits at `src/plugin/wasm.rs:24-28`); HTTP calls via WASI sockets when available |

**MVP shipping list:** built-in OpenAI + Anthropic providers only.
**Phase 3 Beta:** add built-in Bedrock + Gemini + the 9 more providers
listed in ROADMAP §5 Phase 3.1; **also** open the FFI / WASM loader paths
for third-party providers.

#### 3.3.4 Why we wait until Phase 3 to open third-party plugin loading

The `AiProvider` trait shape *will* drift during MVP as we discover
provider quirks (Anthropic's `thinking` blocks, OpenAI's reasoning content,
Bedrock's Converse vs Anthropic-on-Bedrock differences, Gemini's ContentParts
/ inline-data conventions). Locking the ABI before that's settled would force
a cascading break on every third-party plugin.

The MVP-first / open-after-soak sequencing is borrowed from the
`Compressor` trait's evolution path — that trait was static-only until the
algorithms list stabilised, then opened to plugins.

#### 3.3.5 Registry + configuration

Providers register into a global registry (`Arc<RwLock<HashMap<String,
Arc<dyn AiProvider>>>>` keyed by `provider.name()`). DSL / YAML configures
which provider blocks to instantiate at startup; the existing
`src/discovery/registry.rs` pattern is the model.

```
provider "openai" {
    kind = "builtin:openai"               # static built-in
    base_url = "https://api.openai.com/v1"
    api_key  = ${OPENAI_API_KEY}
}
provider "my-corp-llm" {
    kind = "wasm"                         # third-party plugin
    path = "/etc/highper/plugins/my_corp_llm.wasm"
    config = { endpoint = "https://llm.corp.local", auth_header = "X-Internal-Auth" }
}
```

The `kind` field selects the loading mode. `builtin:*` for static; `wasm`
or `ffi` for plugin-loaded.

#### 3.3.6 Why this matters strategically

This is the **ecosystem play** for highper-gateway. LiteLLM and Portkey
have built-in provider lists that vendors must contribute to upstream.
Plugin-loaded providers let vendors ship their own integration — partner
companies can build, sign, and distribute `.so` / `.wasm` files for their
LLMs without merging into highper's repo. That's an explicit
differentiator vs. monolithic gateway competitors.

#### 3.3.7 Plugin lifecycle and hot-load capability (added 2026-05-02)

Highper-gateway already ships a hot-reload monitor for plugins at
`src/plugin/hot_reload.rs` (gated behind the `plugin-hot-reload` Cargo
feature; uses the `notify` crate to watch the plugin directory) plus a
graceful drain in `src/plugin/manager.rs:226 reload_plugin()` and
`manager.rs:252 wait_for_plugin_idle()` (30 s default timeout).

`AiProvider` plugins reuse this infrastructure. The capability matrix:

| Operation | Hot-loadable? | Mechanism | When available |
|---|---|---|---|
| Update **built-in** provider configuration (model alias, endpoint URL, provider API key rotation, RPM/TPM tuning) | ✅ Yes | Existing `src/config/{watcher,reloader}.rs` config hot-reload (Phase 0) | MVP |
| Update **built-in** provider *code* (e.g. fix a translator bug in OpenAI integration) | ❌ No — requires rebuild + redeploy | Built-ins are statically linked into the highper binary | n/a |
| **Add** a new third-party WASM provider plugin (drop a `.wasm` into the plugin dir) | ✅ Yes | `notify` watcher → `manager.rs:226` reload | Phase 3 Beta (opens with third-party plugin loading) |
| **Upgrade** an existing third-party WASM provider | ✅ Yes | Replace file → old plugin drains via `wait_for_plugin_idle` (30 s) → new loads | Phase 3 Beta |
| **Add / upgrade** an FFI dylib provider | ✅ Yes (with caveats) | Same flow; FFI plugin authors must respect drain semantics — possible to crash highper if old plugin holds outstanding allocations when unloaded; the 30 s drain mitigates | Phase 3 Beta |
| **Remove** a provider plugin | ✅ Yes | Delete file → watcher fires → drain → unload; in-flight requests against that provider drain or error after timeout | Phase 3 Beta |
| **Toggle** the `plugin-hot-reload` Cargo feature | ❌ No | Build-time | n/a |

**Operational implication.** Day-to-day operator tasks — rotating provider
API keys, adding a new model alias, retuning rate-limit envelopes,
disabling a misbehaving provider — are all configuration changes that
hot-reload via Phase 0 work; **no plugin reload is required for these**.
Plugin reload kicks in only when the operator wants to ship new *code*
(third-party-maintained translator, custom on-prem provider integration,
etc.). Both classes of change happen without a highper restart.

**Drain timeout knob.** The 30 s default in `manager.rs:252` is a
constant today; Phase 0.J will move it to env-var
`HIGHPER_PLUGIN_DRAIN_SECS` per §0.1 (no hardcoded tunables).

**Interaction with the four cluster types** (`HA_ARCHITECTURE.md` §1):
plugin reload is **per-replica** — each highper-gateway replica reloads
independently as the file appears in its own plugin dir. For a multi-node
deployment (Type 2 / 3 / 4), the operator's deployment tooling (Helm
rolling, Ansible playbook, K8s ConfigMap mount sync) is responsible for
landing the new plugin file on every replica. Type 3 / 4 deployments could
*also* push plugins via etcd in Phase 4.2 once the `ConfigSource` trait
extraction lands (`ROADMAP.md` §4.4 #8).

### 3.4 Routing strategies and fallback (UC16 design decision #9, 2026-05-02)

When a virtual key requests model alias `"smart"` and the operator has
configured `model_alias_map["smart"] = [anthropic/claude-3-7, openai/gpt-4o,
bedrock/claude-3-7]`, this section defines exactly which provider serves
the request, in what order, and what happens when the chosen provider
fails.

#### 3.4.1 Layered candidate-list construction

Routing is a sequence of filters applied to the operator-declared
provider list, producing a final ordered candidate list. Each filter
removes providers that don't qualify for *this* request.

```
Operator declares: model_alias_map["smart"] = [anthropic/claude-3-7,
                                                openai/gpt-4o,
                                                bedrock/claude-3-7]
                                  │
                                  ▼
[Filter A] Per-virtual-key models_allow scope (§7.1.1)
           — drop providers the key isn't allowed to use
                                  │
                                  ▼
[Filter B] Capability flags (from pricing registry §5.5.6)
           — drop providers whose capability_flags don't match request shape
             (vision needed → drop non-vision providers; function calling
             needed → drop providers without supports_function_calling; etc.)
                                  │
                                  ▼
[Filter C] Circuit-breaker / health
           — drop providers whose breaker is OPEN (per `src/proxy/circuit_breaker.rs`)
                                  │
                                  ▼
[Filter D] Rate-limit cooldown
           — drop providers in 429 cooldown window
             (cooldown derived from provider's `Retry-After` header, fallback 30 s)
                                  │
                                  ▼
[Filter E] Session affinity (Beta — N23 Helicone-style sessions)
           — if session is set and last provider is in candidate list, pin to it
                                  │
                                  ▼
[Order]   Order remaining candidates by:
            • priority (default — operator's declared order)            ← MVP
            • cost_aware (cheapest meeting SLA)                          ← Beta (Phase 3.1)
            • latency_aware (lowest p50 / p99)                           ← GA (Phase 4.1)
                                  │
                                  ▼
[Weighted] If `weighted_split` configured (Beta): probabilistic pick over the ordered list
                                  │
                                  ▼
First candidate → attempt → on retryable error, fall to next candidate
                                  │
                                  ▼
All candidates exhausted → 503 with structured error (see §3.4.4)
```

**MVP ships filters A + B + C + D and the priority ordering.** Filters E,
weighted, cost-aware, and latency-aware land in subsequent phases.

#### 3.4.2 Retryable vs non-retryable errors

| Error class | Retryable? | Action |
|---|---|---|
| 429 Too Many Requests | ✅ Yes | Mark provider in cooldown for `Retry-After` (or 30 s default); fall through |
| 502 Bad Gateway / 503 Service Unavailable / 504 Gateway Timeout | ✅ Yes | Fall through immediately; circuit-breaker increments failure count |
| 500 Internal Server Error | ✅ Yes | Same as 502/503 |
| Network timeout / connection reset / TLS handshake failure | ✅ Yes | Same; counts toward circuit-breaker |
| 400 Bad Request / 422 Unprocessable Entity | ❌ No | Surface to client immediately — request shape is wrong; retrying gets the same error from any provider |
| 401 Unauthorized | ❌ No | Operator's provider API key is bad; surface (with provider key redacted from response) |
| 403 Forbidden | ❌ No | Provider denied (model not enabled, region restriction, content policy); surface |
| Provider-specific content-policy block (e.g. OpenAI moderation) | ❌ No | Surface with the provider's reason; operator's guardrail layer (§8) is the right place to catch this if they want to retry against another provider |

#### 3.4.3 Retry budget and backoff

| Knob | Default | Env var |
|---|---|---|
| Max attempts across providers (per request) | **3** | `HIGHPER_AI_RETRY_BUDGET` (per-virtual-key override on the scope) |
| Backoff strategy when no `Retry-After` provided | Exponential with jitter | n/a |
| Backoff floor | 50 ms | `HIGHPER_AI_DEFAULT_BACKOFF_MS_MIN` |
| Backoff ceiling | 200 ms | `HIGHPER_AI_DEFAULT_BACKOFF_MS_MAX` |
| Honor provider's `Retry-After` header | Always when present | n/a |

When `Retry-After` is set, that's authoritative — exponential backoff is
only used when the provider doesn't tell us how long to wait.

**Per-virtual-key budget override.** The virtual-key scope (§7.1.1) gains
an optional `retry_budget: u32` field. Use case: regulated environments
where one attempt is policy (e.g. financial-services workloads where a
duplicate request to a different provider is a compliance violation).

#### 3.4.4 Failure response when all candidates exhausted

When every candidate in the list has either filtered out or failed retryable,
return **503 Service Unavailable** with a structured error body so the
operator's monitoring can distinguish *this* failure from a generic 503:

```json
{
  "error": {
    "code": "all_candidates_failed",
    "message": "All providers configured for model alias 'smart' failed",
    "alias": "smart",
    "attempts": [
      {
        "provider": "anthropic",
        "model": "claude-3-7-sonnet-latest",
        "status": 429,
        "latency_ms": 12,
        "error_class": "rate_limited",
        "retry_after_secs": 60
      },
      {
        "provider": "openai",
        "model": "gpt-4o",
        "status": 503,
        "latency_ms": 4500,
        "error_class": "upstream_unavailable"
      }
    ]
  }
}
```

This mirrors what AWS API responses give when their internal-services
fail — operators see exactly which providers tried, why each failed,
how long each took.

#### 3.4.5 Where cooldown state lives

The "drop providers in 429 cooldown" filter (Filter D in §3.4.1) needs
state about *which* providers are currently rate-limited and *until
when*. Two storage options:

| Option | When to use | Trade-off |
|---|---|---|
| **Local per-replica** | Type 1 deployments (Group A only) — no Valkey available | Each replica independently observes 429 and applies cooldown; minor wasted calls when a replica that hasn't seen 429 yet routes to the same provider |
| **Shared via Valkey** (default for Type 2 / 4) | Whenever Type B is configured | Cluster-wide consistency: when one replica observes 429, every replica skips the provider until cooldown expires; saves wasted upstream calls |

**Selection logic:** if `HIGHPER_CLUSTER_TYPEB_BACKEND ≠ none`, cooldown
state lives in Valkey under key `ai:cooldown:{provider}:{model}` with
TTL = cooldown duration. Otherwise, in-process `Arc<DashMap>`. Selected
automatically; no separate env var needed.

**Override for explicit local-only behaviour:**
`HIGHPER_AI_COOLDOWN_BACKEND={auto|valkey|local}` — `auto` is the
default; operators can force one for testing or compliance reasons.

#### 3.4.6 Metrics emitted by the router

| Metric | Type | Labels |
|---|---|---|
| `ai_route_attempts_total` | counter | `alias, provider, model, status, error_class` |
| `ai_route_fallback_taken_total` | counter | `alias, from_provider, to_provider, reason` |
| `ai_route_exhausted_total` | counter | `alias` |
| `ai_route_cooldown_active` | gauge | `provider, model` (1 = in cooldown, 0 = available) |
| `ai_route_cooldown_remaining_seconds` | gauge | `provider, model` |
| `ai_route_capability_filter_drops_total` | counter | `alias, provider, reason` (vision_required, function_call_required, json_mode_required, …) |
| `ai_route_circuit_breaker_open` | gauge | `provider, model` |

Operators alert on `ai_route_exhausted_total` increasing or
`ai_route_cooldown_active` for the same provider staying high — those
are the "all-providers-failing" or "one-provider-overwhelmed" patterns.

### 3.5 Streaming and cancellation semantics (UC16 design decision #10, 2026-05-03)

Streaming responses are where AI gateways differ most from regular HTTP
gateways: tokens arrive over many seconds, clients can disappear
mid-stream, providers charge for partial output, and SSE error frames
vs HTTP error responses are not interchangeable. This section pins
down how highper-gateway behaves at every streaming sharp edge.

#### 3.5.1 Cancellation on client disconnect

When the client's TCP connection drops mid-stream (browser closed,
mobile dropped network, app killed), the gateway has three choices.
Highper ships **(a)** as default, **(c)** as the per-key opt-in:

| Choice | Behaviour | Default? |
|---|---|---|
| **(a) Cancel upstream immediately** | Detect client SSE close → immediately abort the provider request (TCP close + provider-specific cancel where supported). Record partial usage. | **Yes — default for new virtual keys.** |
| **(b) Drain upstream silently** | Provider keeps generating; gateway discards the chunks but waits for `[DONE]` to record full usage in the audit log. | Per-virtual-key opt-in via scope's `cancel_on_close: false`. |

**Why default (a):** most operators want to stop billing when the client is gone. Default (b) would surprise operators with bills for output their users never saw.

**Why opt-in (b) exists:** training-data collection / audit-log completeness / reasoning-model traces — small audience, real value, configurable.

**Implementation:**

- Gateway tracks per-stream `client_alive: AtomicBool`.
- A `select!` between `client_send.send(chunk).await` and `client_close_signal.recv()` flips the flag.
- On flag flip: invoke `AiProvider::cancel(stream_id)` — providers that don't support cancel get a TCP RST as fallback.
- Either way, the partial-usage record (`ai_request_log` row) is written with `cancelled = true` and the running token / cost totals as of cancel time.

**Per-virtual-key scope field** (extends §7.1.1):

```
cancel_on_close: bool = true      // default; flip to false for drain-and-record
```

#### 3.5.2 TPM hard-stop mid-stream vs post-stream warning

A virtual key's `tpm` (tokens-per-minute) cap is exceeded *during* a
streaming response. Highper ships **(b)** as default, **(a)** as
per-key opt-in:

| Choice | Behaviour | Default? |
|---|---|---|
| **(a) Hard-stop with SSE error frame** | Inject SSE error event (`event: error\ndata: {"code":"tpm_exceeded",...}`) mid-stream and close upstream. Client SDK sees the error mid-output. | Per-virtual-key opt-in via scope's `tpm_hard_stop: true`. |
| **(b) Post-stream warning** | Let the response complete; record over-budget; emit `ai_budget_exceeded_total{kind=tpm_overrun}` metric + audit event. Soft cap. | **Yes — default for new virtual keys.** |

**Why default (b):** hard-stop mid-stream is jarring — end-users see partial output then a sudden error frame. Most operators set TPM as a *soft* cap (smooth SLO target), not a hard cost ceiling. Default (b) gives smooth UX; the over-budget event flags the tenant for follow-up.

**Why opt-in (a) exists:** cost-sensitive batch jobs, free-tier abuse prevention, regulated environments where one over-budget request is unacceptable. Hard-stop is also safer when the per-request `max_tokens` is far below the per-minute budget — the over-budget window is small.

**Per-virtual-key scope field** (extends §7.1.1):

```
tpm_hard_stop: bool = false      // default; flip to true to inject SSE error frame mid-stream
```

**Note on RPM:** RPM (requests per minute) is checked at request *start* — pre-call. RPM doesn't have a mid-stream interpretation; the request was already accepted. Only TPM has the mid-stream question.

#### 3.5.3 Chunk hooks, buffer, and plugin budget

Each provider chunk passes through a chain of per-chunk hooks before
re-emitting to the client as inbound-shape SSE.

| Hook | Purpose | When it runs |
|---|---|---|
| **Token counter** | Increments output tokens / TPM bucket / running cost | Always, every chunk |
| **Cancellation check** | Reads `client_alive` flag; on flip, triggers §3.5.1 behaviour | Every chunk |
| **Plugin chain** | Operator-supplied per-chunk plugin (per UC16 §3.1; runs WASM/FFI plugin's `on_response_body` phase per chunk) | Every chunk; budgeted |
| **Cache write buffer** | Accumulates complete response for §6.1 exact-cache write | Every chunk; commits at `[DONE]` |
| **Metrics emit** | Updates `ai_active_streams`, `ai_inter_token_seconds`, `ai_ttft_seconds` | First chunk + every chunk + `[DONE]` |

**Plugin budget per chunk.** A misbehaving plugin can block streaming if
the chain runs synchronously per chunk. The hook chain enforces a
**per-chunk budget**:

```
HIGHPER_PLUGIN_CHUNK_BUDGET_US = 500           // microseconds; default 500 µs
```

Plugin overshooting the budget for a chunk is **logged + skipped for the
remainder of this stream** (fail-open at chunk level — the stream
continues without the plugin's contribution). Operator alert metric
`ai_plugin_chunk_budget_exceeded_total{plugin,reason}`.

**Stream buffer between provider receive and client send.** A bounded
mpsc decouples upstream-receive from client-send so a slow client doesn't
block the upstream connection.

```
HIGHPER_AI_STREAM_BUFFER_DEPTH = 64                     // default events; per stream
HIGHPER_AI_STREAM_BUFFER_OVERFLOW_POLICY = drop_oldest  // alternatives: block, error
```

**Drop-oldest** (default): when the buffer is full, drop the oldest
queued chunk and emit `ai_stream_buffer_drops_total{policy}` metric.
The client sees a momentary content gap; the upstream stays unblocked.

**Block**: backpressure all the way to the provider. Upstream throttles;
provider may eventually time out.

**Error**: terminate the stream with an SSE error frame. Strictest
posture; useful for compliance workloads where dropped chunks are
unacceptable.

#### 3.5.4 Streaming metrics

Per UC16 §9.1, streaming-specific metrics:

| Metric | Type | Labels |
|---|---|---|
| `ai_active_streams` | gauge | `tenant, model_alias, provider` |
| `ai_streaming_cancellations_total` | counter | `tenant, key_id, model_alias, reason={client_close, tpm_hard_stop, plugin_block, error}` |
| `ai_ttft_seconds` | histogram | `model_alias, provider` (time to first token) |
| `ai_inter_token_seconds` | histogram | `model_alias, provider` (token-to-token cadence; surfaces provider stalls) |
| `ai_stream_buffer_drops_total` | counter | `policy={drop_oldest, error}` |
| `ai_plugin_chunk_budget_exceeded_total` | counter | `plugin, reason` |

#### 3.5.5 Provider-side prompt-cache passthrough during streaming

The provider's `usage` block at end-of-stream may include
`cache_creation_input_tokens` and `cache_read_input_tokens` (Anthropic /
OpenAI / Bedrock formats vary). The post-stream accounting layer
(§3.1 `[Post-call accounting]` step) parses these and emits:

- `ai_input_tokens_total{kind=prompt_cached}` for the cached portion
- `ai_input_tokens_total{kind=prompt_uncached}` for the rest
- `ai_provider_cache_savings_usd_total{provider}` running savings (per §6.3)

This lets operators measure how much the provider's prompt cache is
saving them, separately from highper's own §6.1 exact cache.

---

## 4. Canonical request / response types

The shape translators all funnel through a single in-process representation so middleware (cache, accounting, plugin hooks) is shape-agnostic.

```rust
// Sketch — final lives in src/gateway/ai/shape.rs
pub struct AiRequest {
    pub model_alias: String,
    pub stream: bool,
    pub messages: Vec<Message>,
    pub system: Option<String>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub response_format: Option<ResponseFormat>,   // text | json_object | json_schema
    pub seed: Option<u64>,
    pub idempotency_key: Option<String>,
    pub user: Option<String>,                      // OpenAI's "user" field
    pub metadata: serde_json::Map<String, serde_json::Value>,
    pub raw_inbound_shape: InboundShape,           // for response shaping
    pub raw_inbound_bytes: Bytes,                  // for byte-stable provider cache
}

pub enum Message {
    System { content: String },
    User { content: Vec<ContentPart> },
    Assistant { content: Vec<ContentPart>, tool_calls: Vec<ToolCall> },
    Tool { tool_call_id: String, content: String },
}

pub enum ContentPart {
    Text(String),
    ImageUrl { url: String, detail: ImageDetail },
    ImageBase64 { mime: String, data: Bytes },
    Audio { mime: String, data: Bytes },
    File { id: String },
}

pub struct AiResponse { /* ... mirrors ... */ }
pub enum AiStreamEvent { Delta(...), ToolCallDelta(...), Usage(...), Done }
```

The translators implement `Translator<P>` traits with `to_provider(req: &AiRequest) -> ProviderReq` and `from_provider(stream: ProviderStream) -> AiStreamEvent`.

---

## 5. Tokenization & cost (UC16 design decision #6, 2026-05-02)

### 5.1 Posture — bake all vocabularies

**Decision recorded.** Tokenizer vocabularies are **baked into the
binary at build time**. Adds ~30 MB to the binary; gives the gateway
fully-offline / air-gap operation; eliminates first-request latency
from lazy-fetch; removes a runtime supply-chain dependency on a
vocabulary registry.

Lazy-fetch from a vocabulary registry was considered and **dropped**
— it fails air-gapped deployments common in regulated industries and
introduces a SPOF / attack vector. Hybrid (top-N baked, long-tail lazy)
was considered and **dropped** — two code paths and the "which N is
top" debate isn't worth the ~24 MB savings on a binary that already
ships in the 50–200 MB range.

**Cargo feature flags:**

| Feature | What ships | Binary size delta | When to pick |
|---|---|---|---|
| Default (no flags) | All MVP+Beta provider vocabularies (OpenAI cl100k + o200k, Anthropic, Gemini SentencePiece, Llama-3 BPE, Cohere, DeepSeek, Qwen, …) | +~30 MB | Production deployments |
| `ai-tokenizers-minimal` | OpenAI cl100k_base + o200k_base only | +~4 MB | Size-conscious / single-provider deployments |
| `ai-tokenizers-only=…` | Operator-selected subset | varies | Custom / regulated builds |

### 5.2 Library choice — split between two crates

**Decision recorded.** Two tokenizer crates, each used for what it
does well:

| Provider family | Crate | Vocabulary | On-disk size |
|---|---|---|---|
| OpenAI / Azure (GPT-3.5 → GPT-4o-mini, GPT-4o, o1, o3) | `tiktoken-rs` | `cl100k_base`, `o200k_base` | ~4 MB |
| Anthropic (Claude 2 / 3 / 3.5 / 3.7) | `tokenizers` (HuggingFace) | Anthropic BPE | ~2 MB |
| Google Gemini | `tokenizers` | SentencePiece | ~4 MB |
| Llama / Mistral / Mixtral | `tokenizers` | Llama-3 BPE | ~9 MB |
| Cohere Command | `tokenizers` | Cohere BPE | ~4 MB |
| DeepSeek / Qwen / others | `tokenizers` | their BPE | ~7 MB combined |

`tiktoken-rs` is faster and narrower for OpenAI-family models;
`tokenizers` is HuggingFace's unified Rust crate covering everything
else. Both are mature, both have permissive licences. No reason to
force one for both.

### 5.3 Pre-call estimate vs post-call truth

| Concern | Decision |
|---|---|
| Pre-call estimate | Local tokenizer counts input tokens; used for budget gate, RPM/TPM check, route eligibility. Estimate is exact for non-multimodal inputs; ±5 % for content with images / audio (where token cost is content-specific). |
| Post-call truth | Prefer provider-returned `usage.{input,output}_tokens` field. Fall back to local-tokenizer count of the output stream when the provider does not return usage (some streaming endpoints, some proxies). Local-only is acceptable for billing because the search space matches provider's billing model when vocabularies match. |
| Streaming TPM enforcement | Default: post-stream warning (log + metric, no inline action). Hard-stop opt-in per virtual key — when set, gateway injects an SSE error frame and closes upstream once token budget is exhausted mid-stream. Hard-stop is jarring for end-users; default-off is intentional. **Mechanics in §3.5.2** (UC16 #10). |

### 5.4 Reasoning tokens — count as output (UC16 design decision #7)

**Decision recorded.** Reasoning tokens (Anthropic `thinking` blocks,
OpenAI `reasoning_content`, DeepSeek `reasoning_content`, …) are
**counted as output tokens by default**. This matches every provider's
own pricing model — they bill operators for reasoning tokens, so the
gateway charging the same to its end-users is the correct default.

**Per-key opt-out.** Virtual-key scope gains a `count_reasoning_in_output:
bool` flag. When `false`, the gateway records reasoning tokens in
metrics / logs but does not include them in the per-key budget
decrement. Useful when the operator absorbs reasoning costs as a
quality-of-service investment and bills end-users only on visible
output.

| Field | Default | Per-key override |
|---|---|---|
| Reasoning tokens count toward TPM bucket | Yes | `count_reasoning_in_output=false` excludes |
| Reasoning tokens count toward $ budget | Yes | same |
| Reasoning tokens emitted as `ai_output_tokens_total` metric | Yes (with `kind=reasoning` label) | always emitted, regardless of billing |

### 5.5 Cost / pricing source (UC16 design decision #7, 2026-05-02)

**Decision recorded.** Vendored snapshot + weekly signed refresh + admin
override at runtime. Live-feed and maintain-in-house were considered and
dropped.

#### 5.5.1 Source

Boot-load price data from a **vendored snapshot** of LiteLLM's
[`model_prices_and_context_window.json`](https://github.com/BerriAI/litellm)
(MIT-licensed, ~150 models, ~2 community PRs / week). Snapshot is baked
into the binary at build time so initial deploy works fully offline,
matching the §5.1 tokenizer bake-all stance.

A **weekly signed refresh** task (cron-like, internal to highper) fetches
the current JSON from `HIGHPER_AI_PRICING_FEED_URL`, verifies the
signature against `HIGHPER_AI_PRICING_FEED_SIGN_KEY` (Ed25519 public key
or sigstore reference), and atomically swaps the price registry on
success. Operator can self-host the feed JSON behind their own URL and
sign it themselves; `HIGHPER_AI_PRICING_FEED_URL` defaults to the
LiteLLM upstream.

Refresh interval `HIGHPER_AI_PRICING_REFRESH_INTERVAL_SECS` (default
604 800 s = 7 days; minimum 3600 s = 1 hour) per the §0.1 no-hardcoded-
tunables rule.

Live-feed (fetch on every cold start + once a day) was considered and
dropped: requires network at startup, exposes operator to GitHub rate
limits, no air-gap support. Maintain-in-house was considered and
dropped: LiteLLM's community curates aggressively; duplicating that work
costs multi-person-weeks per quarter and is not a highper differentiator.

#### 5.5.2 Admin override at runtime

Operators **must** be able to override prices live without a refresh
cycle — provider price changes ship daily during AI-market churn, and
operators with pre-negotiated discounts (volume contracts, partner
pricing) need their own numbers in effect immediately.

**Admin API endpoints** (extends §2.4):

```
PATCH  /admin/ai/models/{alias}        # override pricing / capability flags
GET    /admin/ai/models/{alias}        # show effective row (snapshot + overrides applied)
DELETE /admin/ai/models/{alias}/override  # revert to current snapshot row
GET    /admin/ai/pricing/refresh-status   # last-refresh time, last-good-time, last-error
POST   /admin/ai/pricing/refresh-now      # manual refresh trigger (admin scope)
```

**Override scope and storage:**

- Per (provider, model) row — operator overrides any combination of `input_price_per_1M_tokens`, `output_price_per_1M_tokens`, `cached_input_price_per_1M_tokens`, `reasoning_price_per_1M_tokens`, `image_input_price_per_image`, `audio_input_price_per_minute`, `context_window_max`, capability flags, plus a free-form `note` string for change-rationale.
- Override expiry — operator may set `expires_at`; row reverts to the snapshot value automatically. Default: no expiry (sticky until explicit removal).
- Stored in the configured `AiStateStore` (UC16 #4) under a dedicated `ai/pricing_overrides/{alias}` namespace.
- Override layered on top of the snapshot at lookup time; the snapshot is never mutated.
- Hot-applied — no restart, no refresh wait. Lookup goes through `RwLock<HashMap>`; admin write swaps in the new override.

**Audit:** every PATCH / DELETE on `/admin/ai/models/{alias}` emits an
audit event into the `AiStateStore`-backed audit log per §7.3 — same
hash-chain integrity as virtual-key changes. Includes actor, before /
after, IP, timestamp.

#### 5.5.3 Refresh failure handling

| Mode | Behaviour | Env var |
|---|---|---|
| **`last_known_good` (default)** | If refresh fails (network error, signature mismatch, malformed JSON), keep the current price registry and emit `ai_pricing_refresh_failed_total{reason}` metric + structured-log warning. Service continues. | `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE=last_known_good` |
| **`fail_closed`** | If refresh fails *and* the operator has set this mode, refuse new AI requests with 503 and a `Retry-After` header until refresh succeeds. Used in regulated billing environments where stale prices are unacceptable. | `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE=fail_closed` |

**Default is `last_known_good`** because the realistic failure mode is
"GitHub had a hiccup" or "operator's signing infra is being maintained";
the realistic *price drift* in 7 days is small and the admin override
mechanism (§5.5.2) is the relief valve. `fail_closed` is opt-in for
operators whose finance team won't accept stale prices.

Health endpoint `/health/ai/pricing` reports `last_refresh_at`,
`last_good_at`, `seconds_since_last_good_refresh` — operators can
alert on staleness.

#### 5.5.4 Zero-price handling

When a request arrives for a (provider, model) pair with **no pricing
entry** (snapshot doesn't include it, no override set), the gateway:

| Mode | Behaviour | Env var |
|---|---|---|
| **Default (`false`)** | Reject with 400 Bad Request: `{"error": "model 'xxx' has no pricing configured; set an admin override or HIGHPER_AI_ALLOW_FREE_TIER=true"}` | `HIGHPER_AI_ALLOW_FREE_TIER=false` |
| **`true`** | Allow the request; log + emit metric `ai_request_no_pricing_total{provider,model}`; budget check skipped (no $ cap can apply); usage records show `cost_usd=0` | `HIGHPER_AI_ALLOW_FREE_TIER=true` |

The default-reject behaviour catches typos in `model_alias`
configuration and prevents accidental zero-cost runaway requests.
Free-tier mode is for operators running self-hosted models (UC17 future)
or genuinely-free providers where billing tracking isn't relevant.

#### 5.5.5 Per-tenant pricing overrides — Phase 3

**Out of MVP.** MVP ships operator-level overrides only (one global
override per model alias). Phase 3 adds per-tenant overrides for
multi-tenant resellers — operator marks up provider prices for their
end-customers, applies different rates per tenant tier (free / pro /
enterprise), or honours per-tenant negotiated rates.

Phase 3 schema sketch:

```
ai/pricing_overrides/global/{alias}           # MVP — operator override
ai/pricing_overrides/tenant/{tid}/{alias}     # Phase 3 — per-tenant override
```

Lookup at request time: check tenant-override first, fall back to
global-override, fall back to snapshot. Three-level lookup adds ~1 µs;
acceptable.

#### 5.5.6 Pricing registry schema fields

For every (provider, model) row, the registry tracks:

| Field | Used by |
|---|---|
| `input_price_per_1M_tokens` | Pre-call budget gate, post-call accounting |
| `output_price_per_1M_tokens` | Same |
| `cached_input_price_per_1M_tokens` | Provider-prompt-cache discount (§6.3) |
| `reasoning_price_per_1M_tokens` | When reasoning differs from output (rare; future-proofing) |
| `image_input_price_per_image` | Vision passthrough (Beta) |
| `audio_input_price_per_minute` | Audio (GA) |
| `context_window_max` | Pre-call validation; reject early on input > limit |
| `model_capability_flags` | `supports_function_calling`, `supports_vision`, `supports_streaming`, `supports_reasoning`, … — used by capability-aware routing in Phase 4.1 |
| `last_updated_at` | Drift / audit |
| `source` | `snapshot` \| `refresh` \| `override` — provenance tag for the row |

---

## 6. Caching (UC16 design decision #8, 2026-05-02)

### 6.0 Engine-plus-pluggable posture

**Decision recorded.** Highper-gateway ships the **cache engine** built-in:
canonical request hashing, lookup / write-back orchestration, TTL handling,
admin invalidation API (tag + pattern + per-tenant), streaming-replay logic,
and metrics emission. Backend storage and the embedding model used for
semantic cache are **operator's choice**, configured per deployment via
pluggable traits. Highper does not bundle a cache backend or embedding model
as a product feature.

This is the same architectural pattern as `AiProvider` (engine in highper;
providers as plugins) and `AiStateStore` (engine in highper; backends as
trait impls). Strategically, it differentiates highper from cache-as-a-product
peers (Portkey / Helicone / Cloudflare AI Gateway) and matches the
configure-don't-code operator effort of LiteLLM while removing backend
lock-in.

**Three layers operators can wire independently:**

| Layer | What it provides | Where it lives |
|---|---|---|
| **Engine** | Canonical request hashing; lookup + write-back orchestration; TTL; tag-based + pattern invalidation; streaming replay; metrics; per-tenant key-space isolation | Built-in (`src/gateway/ai/cache_exact.rs`, `cache_semantic.rs`); ships with the binary |
| **KV backend** | Where exact-cache entries are stored | Existing `src/cache/` trait — InMemory / Disk / Redis-protocol (works against Valkey) / MultiTier / Tiered. Operator picks via DSL `cache.backend` field. |
| **Vector backend** (semantic cache only) | Where embeddings + responses are indexed | New `VectorIndex` trait (§3.2 + ROADMAP §4.4 row 11). Operator picks via DSL `semantic_cache.backend` field. |
| **Embedding model** (semantic cache only) | Which provider embeds the request | Operator's `AiProvider` registry choice — same registry as outbound calls. No bundled embedding model. |

### 6.1 Exact cache (MVP)

Key = `sha256(canonical_json({model, messages, tools, tool_choice, temperature, top_p, max_tokens, stop, response_format, seed}))`.

Canonical JSON = serde with `preserve_order` + sorted keys + no float reformat (preserve original byte representation). This is required for **provider prompt-cache compatibility** (Anthropic/OpenAI/Bedrock will only cache if request bytes are stable across calls; see §6.3).

Backend: existing `src/cache/` trait — operator picks via DSL `cache.backend = "valkey" | "redis" | "memory" | "disk" | "multi-tier"`. Default for Type B deployments: the cluster's Type B Valkey (already configured at the cluster level, no separate cache-backend setup needed).

TTL: per-route default 1 h; per-request override via `x-cache-ttl` header.

Response header: `x-cache: HIT | MISS | BYPASS | SEMANTIC`.

**Streaming behaviour:** cache stores complete responses (after `[DONE]`), replayed as SSE on hit so client sees stream regardless of cache state.

**Tenant isolation:** key prefixed with virtual-key tenant + (optional) per-route namespace. No cross-tenant lookups possible.

**Admin invalidation API:**

```
POST /admin/ai/cache/invalidate
  body: { "tag": "model_alias=smart" }      # tag-based
  body: { "pattern": "tenant:abc:*" }       # pattern-based
  body: { "tenant_id": "xyz" }              # per-tenant flush
```

### 6.2 Semantic cache (Beta)

1. On exact-cache miss, embed the **last user message** (or a configured slice) using a **configured embedding provider** — chosen by the operator via the same `AiProvider` registry that handles outbound calls (§3.3). Highper ships **no bundled embedding model**; operators point at OpenAI's `text-embedding-3-small`, Bedrock Titan, a local Sentence-Transformers service via custom plugin, or anything else they prefer.
2. ANN search against per-tenant index in the configured `VectorIndex` impl; threshold default 0.92 cosine; per-tenant override.
3. On hit above threshold: return the cached response as-if-fresh; emit `x-cache: SEMANTIC` (visibly different from exact `HIT` so clients can detect).
4. On miss or below threshold: forward to provider, then on completion store `(embedding, response, request_metadata)` in the vector index.

**Vector index backend (operator chooses; highper ships impls behind Cargo features):**

| Backend | Cargo feature | Best for |
|---|---|---|
| **Qdrant** | `ai-vector-qdrant` | Most common; separate process; HA via Qdrant cluster |
| **Redis-Stack** (Vector Set + RedisJSON) | `ai-vector-redis-stack` | Co-located with Type B Valkey if operator chose Redis-Stack as their Type B backend |
| **PgVector** | `ai-vector-pgvector` | Operator already running PostgreSQL |
| **In-process HNSW** (`hnsw_rs`) | `ai-vector-hnsw` | Single-node only; dev / small-scale |

**`VectorIndex` trait** lives at `src/gateway/ai/vector_index/mod.rs` (added 2026-05-02 per UC16 #8; tracked as row 11 in ROADMAP §4.4 interface-first audit). Surface: `search(query, topk) → Vec<Match>`, `upsert(id, embedding, payload)`, `delete(id)`, `delete_by_tag(tag)`. Same separate-trait + plugin-loadable pattern as `AiProvider` and `AiStateStore`.

**Tenant isolation:** per-tenant index (no cross-tenant lookups); enforced at trait layer.

### 6.3 Provider prompt-cache passthrough

When forwarding to providers that support native prompt caching (Anthropic `cache_control`, OpenAI `prompt_cache_key` / fixed-prefix automatic, Bedrock prompt caching), preserve the user-supplied `cache_control` markers verbatim. Do NOT re-serialize JSON in a way that changes byte order; do NOT touch float formatting.

**Operator-facing metrics** for the provider's prompt cache:

- `ai_input_tokens_total{kind=prompt_cached}` — input tokens served from the *provider's* cache (cheaper)
- `ai_input_tokens_total{kind=prompt_uncached}` — full-priced input tokens
- `ai_provider_cache_savings_usd_total{provider}` — running total of savings from the provider's prompt cache, computed against §5.5 pricing rows for cached vs uncached input

Highper-side cache and provider-side prompt cache compose: highper's exact-cache (§6.1) checks first; on miss, the provider's own prompt cache may still serve at lower input price. Both layers tracked independently in metrics.

### 6.4 What's NOT in scope for cache (added 2026-05-02 — per UC16 #1 scope fence + #8 confirmation)

Mirroring §8 (guardrails), this section pins down the boundary so future
contributors don't accidentally bundle a cache backend or embedding model
as a product feature.

| Concern | Why out of scope | Where it lives instead |
|---|---|---|
| **A bundled cache backend product** ("highper Cache Cloud", vendored Redis-as-a-feature) | Operators run their own KV / vector backend; highper ships no SaaS cache layer | Operator's choice — Valkey / Redis / Qdrant / etc. |
| **A bundled embedding model** (sentence-transformers binary, OpenAI API client as a built-in dep) | Embedding-model choice is workload-specific (latency / cost / quality / data-residency); operator picks via the `AiProvider` registry | `AiProvider` registry — operator registers an embedding provider |
| **A SaaS cache dashboard** (UI for cache hit-rate visualisation, à la Portkey / Helicone) | Dashboard is a separate product layer; operators bring Grafana / Langfuse / Helicone-OSS / etc. | External — highper exposes Prometheus + OTLP per §9 |
| **Cache-policy automation** (auto-tune TTL based on observed hit rate, ML-based eviction) | Out-of-scope research direction; operators with that need build it on top of the metrics surface | External — operator's analytics / ML layer reads `ai_cache_*` metrics |
| **Cache-warming as a product** (pre-load cache from training data) | Operator-side concern; highper exposes admin-API write endpoints if operators want to script cache-warming | External — operator scripts via admin API |
| **A managed vector database service** | Operators run Qdrant / Redis-Stack / PgVector themselves | Operator's choice |
| **Built-in PII redaction in cached prompts** | Out per UC16 §8 (guardrails are external); operator wires PII redaction via the §3.1 plugin hooks **before** the cache layer; redacted prompt becomes the cache key | Operator-side guardrail plugin |

**Why this boundary matters:**
- Keeps highper-gateway as a *thin orchestration layer* — same operator effort as LiteLLM, none of the backend lock-in of Portkey / Helicone / Cloudflare.
- Lets operators evolve their cache stack independently of highper releases (swap Valkey → Dragonfly, add a new vector backend) by implementing the existing traits.
- Makes the comparison story crisp: highper ships *the engine*, the operator owns *the backend*.

### 6.5 Operator effort summary (added 2026-05-02)

For an operator deploying highper-gateway with **Valkey** as exact-cache
backend and **Qdrant** as semantic-cache backend:

| Effort | Magnitude |
|---|---|
| Rust code | **0 lines** |
| Env vars | ~10–15 (cluster type + Valkey addrs / auth + AiStateStore + AI key pepper + provider API keys) |
| DSL config | ~30 lines exact-only; ~50 lines including semantic |
| Operations | Stand up Valkey cluster (already part of Type 2 cluster requirement); stand up Qdrant if semantic enabled; stand up `AiStateStore` (ReDB single-node or ScyllaDB multi-node) |
| Admin operations (ad hoc) | HTTP calls to `/admin/ai/cache/invalidate`, `/admin/ai/keys`, etc. |

No code is written by the operator for cache integration. Valkey works against the existing `src/cache/backends.rs:182-326` Redis client (Valkey is wire-compatible with the Redis protocol; verified 2026-05-02). Qdrant requires the `ai-vector-qdrant` Cargo feature at build time but no runtime code.

---

## 7. Virtual keys, budgets, multi-tenant

### 7.1 Key model (UC16 design decisions #4 + #5, 2026-05-02)

#### 7.1.1 Shape — flat keys + tags (MVP)

The original draft sketched a four-level hierarchy
(`tenant → workspace → project → key`). MVP ships **flat keys + tags**;
hierarchy lands at Beta.

Why flat for MVP: most operators starting on highper-gateway have a small
number of consumers and want a quick path to issuing keys. Hierarchy is
useful for billing rollups and admin UI organisation but isn't on the
critical path for a working gateway. Flat keys + arbitrary tags
(`env=prod`, `team=growth`, `cost_center=ai-r&d`, …) deliver the same
filtering / rollup capability with much less schema and admin surface.

```
key (sk-hpgw-...)
├─ scopes:
│    models_allow = [...]
│    rpm = ..., tpm = ..., rpd = ..., tpd = ...
│    budget_usd_day = ..., budget_usd_month = ..., lifetime_usd_cap = ...
│    expires_at = ...
│    enabled = true | false                  (soft-disable; not deleted; preserves audit)
│    retry_budget = 3                        (per-key max retry attempts; UC16 #9 §3.4.3)
│    count_reasoning_in_output = true        (UC16 #6 §5.4; flip false to exclude from billing)
│    cancel_on_close = true                  (UC16 #10 §3.5.1; flip false to drain-and-record)
│    tpm_hard_stop = false                   (UC16 #10 §3.5.2; flip true to inject SSE error frame)
├─ tags (free-form key/value, indexable):
│    env = prod | staging | dev
│    team = growth | platform | ...
│    cost_center = ai-r&d | ...
│    (anything else the operator wants)
└─ kms_key_ref          (GA tier — BYOK + per-tenant CMEK)
```

**Revocation semantics:** `enabled = false` is the default revocation —
the key stops working but the row stays for audit. Hard delete is a
separate explicit admin action, gated by an `admin:keys:hard-delete` scope.

**Hierarchy at Beta:** add `tenant_id`, `workspace_id`, `project_id`
columns; flat keys map to a default tenant during the migration. Existing
tags continue to work; hierarchy becomes a *secondary* organisation axis
on top of tags rather than replacing them.

#### 7.1.2 Storage — `AiStateStore` trait (UC16 design decision #4)

All durable state — virtual keys, budgets, usage records, audit, prompt
registry — lives behind the `AiStateStore` trait (interface-first per
ROADMAP §4.4). Three impls ship; operator selects via env var:

| Backend | When to pick | Env var |
|---|---|---|
| **ReDB** (pure-Rust embedded) | Single-node default; dev and small prod; zero external deps | `HIGHPER_AI_STATE_BACKEND=redb` |
| **RocksDB** (mature C++ embedded with Rust bindings) | Single-node alternative; operator already has RocksDB ops experience | `HIGHPER_AI_STATE_BACKEND=rocksdb` |
| **ScyllaDB** (Cassandra-compatible distributed) | Multi-node prod | `HIGHPER_AI_STATE_BACKEND=scylladb` |

ReDB / RocksDB are single-node — cluster fault tolerance is 0% on the
storage layer (acknowledged; same semantics as `HIGHPER_CLUSTER_ALLOW_SINGLE_NODE`
in `HA_ARCHITECTURE.md` §11.2). ScyllaDB provides its own replication for
multi-node prod. See `HA_ARCHITECTURE.md` §3.5 for how this slots into the
four cluster types (UC16 is "Type 2 + UC16 state-store" or "Type 4 + UC16
state-store").

#### 7.1.3 Hash design — HMAC-SHA-256 + server pepper (UC16 design decision #5)

**Decision recorded.** Virtual keys are stored as **HMAC-SHA-256 with a
server-side pepper**, NOT Argon2id.

Why HMAC-SHA-256 instead of Argon2id:

| Property | Argon2id (original draft) | HMAC-SHA-256 + pepper (now) |
|---|---|---|
| Search space of a `sk-hpgw-<base64url(rand 32B)>` key | 2²⁵⁶ — astronomical | 2²⁵⁶ — astronomical |
| Brute-forceable from leaked DB hash? | No | No |
| Latency cost of validation | ~50–500 ms (default tuning) | ~10 µs |
| Validations per request | 1 | 1 |
| At 10 k RPS, validation CPU cost | ~5–500 vCPU | ~0.0001 vCPU |
| What an attacker who steals just the DB (no pepper) can do | Brute-force impossible (already) | Brute-force impossible AND can't even check candidate keys without pepper |
| Industry use for high-entropy random API keys | rare | standard (AWS, Stripe, OpenAI, Vault) |

Argon2id is the gold-standard for hashing **low-entropy human passwords**
where brute force is the realistic attack. For 256-bit random API keys
the search space is already astronomical; the slow hash buys nothing
against the threat model and costs ~10 000× more on every request.

**Storage row layout:**

```
key_record {
    id               = "sk-hpgw-7Ab2…"   // first 12 chars; fast lookup, UI display
    full_hash        = HMAC-SHA-256(server_pepper, full_key_bytes)
    metadata         = { scopes, tags, enabled, expires_at, … }   // per §7.1.1
    created_at       = …
    last_used_at     = …
}
```

**Validation flow (per request, target ≤10 µs):**

1. Parse incoming `sk-hpgw-XXXXXXXX…` → extract 12-char prefix.
2. Index lookup by prefix (one O(1) `AiStateStore` read; an LRU cache in
   front for hot keys).
3. HMAC-SHA-256 the full presented key with `server_pepper`.
4. Constant-time compare against stored `full_hash`.
5. Return: invalid (401) | valid + metadata.

**Server pepper.** A 32-byte random secret loaded from
`HIGHPER_AI_KEY_PEPPER` env var (or via the secrets-manager resolver
landing in Phase 1.4 — Vault / AWS Secrets / K8s Secret). The pepper
provides defense-in-depth: an attacker who steals just the database
without the pepper cannot even check candidate keys against the hashes.
**Refuse to start** if UC16 features are enabled and pepper is empty,
unless `HIGHPER_CLUSTER_ALLOW_INSECURE=true` (dev only).

**Pepper rotation:** rotating the pepper invalidates all existing virtual
keys. Operators do this only after a confirmed pepper compromise. Not a
routine operation. Future enhancement (Phase 3+): support two peppers
(active + previous) during a rotation window so existing keys keep
working until they're regenerated.

**Why not just SHA-256 (no pepper)?** A pepper-less hash is fine if the
DB is properly secured; the pepper is defense-in-depth. The marginal
operational cost (one secret to manage) is small compared to the
incremental safety. AWS / Stripe / OpenAI all use a pepper-equivalent
construction for their API keys.

Key format: `sk-hpgw-<base64url(rand 32B)>` — 256 bits of entropy.
Generated server-side, returned to the operator/admin **once** at
creation; never logged in plaintext anywhere.

### 7.2 Budgets

- **Hard caps**: gateway rejects with 402 once exceeded (plus 429 if rate-limit semantics preferred).
- **Soft warnings**: emit metric + audit event; pass-through.
- Windows: per-day, per-month, lifetime.
- Aggregation on accounting: every request decrements counters in **Valkey** (Type B hot path; per HA research line 27 "Token Quota") with TTL = window remaining; durable rollups in **`AiStateStore`** (whichever impl the operator chose — ReDB / RocksDB / ScyllaDB).

### 7.3 Audit trail

Every key change, budget change, prompt change emits an audit event:
```
{ts, actor, action, resource_type, resource_id, before, after, ip, request_id}
```
Stored append-only in the configured `AiStateStore` (ReDB / RocksDB / ScyllaDB); daily JSONL export with hash-chain integrity (block-chain style: each row contains hash(prev_row || row_data)).

---

## 8. External guardrails integration (out of scope as a product, in scope as a hook)

Per the scope fence (`ROADMAP.md` §3.1), highper-gateway does **not** ship
guardrail logic. PII detection, jailbreak detection, output filtering, content
moderation, JSON-schema validation, and similar concerns live in
**customer-side services** that the operator selects (e.g. Presidio for PII,
OpenAI moderation, Bedrock Guardrails, Lakera, Llama-Guard, Granite-Guardian,
or a custom service).

### 8.1 What highper provides

Two integration surfaces — both reuse existing highper infrastructure:

1. **Pre/post pipeline plugin hook.** The existing `src/plugin/` system
   (WASM at `src/plugin/wasm.rs`, FFI at `src/plugin/ffi.rs`, hot-reload at
   `src/plugin/hot_reload.rs`) already supports the `Plugin` trait
   (`src/plugin/trait_def.rs:35-100`) with `on_request_body` and
   `on_response_body` phases. Operators wire their guardrail provider into
   these phases — call out to Presidio HTTP, return `Continue` /
   `StopIteration`. The plugin can mutate the body for redaction or reject
   the request.
2. **Streaming chunk callback.** §3.1 step "[SSE stream chunker]" exposes
   a per-chunk hook. Operators with streaming-aware guardrail services
   (output-PII redact, toxicity score, JSON-schema validate) wire into
   that hook.

### 8.2 What highper does not provide

- **Built-in PII regex set.** Operators bring their own regex / dictionary /
  ML classifier. Plenty of OSS lists exist (Presidio, AWS Comprehend SDK,
  custom corporate sets) — highper is not the right home for those.
- **OpenAI / Bedrock moderation as a built-in step.** Operators can wire
  these as plugin hooks if they use them; they're external API calls.
- **Llama-Guard / Granite-Guardian / Lakera SDKs.** Same — operator's choice.
- **JSON-schema validation built-in.** Use the `N4 JSON-schema request validation`
  middleware (ROADMAP Phase 4.1) for non-AI traffic; for AI request bodies,
  operators should plug a validator into the §3.1 plugin hooks.

### 8.3 Why this scope

- Guardrail engines change rapidly (new jailbreak techniques, new privacy
  regs); a baked-in highper guardrail would lag.
- Customer compliance regimes vary — what's "PII" is jurisdiction-specific.
- An operator may already pay for Presidio Enterprise or Bedrock Guardrails
  and not want a parallel highper guardrail to maintain.

The result: highper stays in its lane (gateway / proxy / routing / cost
accounting), and the customer keeps full control over what content policy
they apply.

---

## 9. Observability extensions

### 9.1 New metrics (Prometheus + OTLP once OTLP exporter ships)

```
ai_requests_total{tenant,key_id,model_alias,provider,status}
ai_input_tokens_total{...}
ai_output_tokens_total{...}
ai_cost_usd_total{tenant,key_id,model_alias,provider}      counter
ai_ttft_seconds{model_alias,provider}                      histogram
ai_inter_token_seconds{model_alias,provider}               histogram
ai_request_duration_seconds{...}                           histogram
ai_cache_lookups_total{result=hit|miss|semantic_hit|bypass}
ai_cache_hit_ratio{cache=exact|semantic}                   gauge
ai_guardrail_blocks_total{stage=pre|post,kind=pii|moderation|...}
ai_provider_429_total{provider,model}
ai_fallback_taken_total{from_provider,to_provider,reason}
ai_budget_exceeded_total{tenant,key_id,window=day|month}
ai_streaming_cancellations_total{...}
ai_active_streams                                          gauge
```

### 9.2 Logs

Structured JSON; per-request: `request_id`, `trace_id`, `tenant`, `key_id`, `model_alias`, `provider`, `model_id`, `input_tokens`, `output_tokens`, `ttft_ms`, `total_ms`, `status`, `cache`, `guardrails_triggered=[]`, `cost_usd`, `prompt_redacted` (if redaction), `response_redacted`. Configurable raw-prompt capture (off by default; on per-key with retention TTL).

### 9.3 Traces

W3C Trace Context propagation; spans: `ai.request`, `ai.guardrail.pre`, `ai.cache.lookup`, `ai.translate.out`, `ai.upstream.{provider}`, `ai.translate.in`, `ai.guardrail.post`, `ai.accounting`. OTLP exporter once core OTLP work lands (cross-cutting prerequisite).

---

## 10. Configuration / DSL

Extend the existing pest grammar with an `ai_route` block:

```
ai_route "openai-passthrough" {
    inbound = openai_chat
    model_alias_map {
        "gpt-fast"     -> openai/gpt-4o-mini
        "claude-fast"  -> anthropic/claude-3-5-haiku-latest
        "smart"        -> [
            anthropic/claude-3-7-sonnet-latest,
            openai/gpt-4o,
            bedrock/anthropic.claude-3-5-sonnet-20240620-v1:0
        ]   # ordered fallback
    }
    cache {
        kind = exact
        ttl  = 1h
    }
    # Length cap is built-in (token-counter check).
    length_cap_tokens = 8000

    # Operator-supplied guardrail plugins (external; see §8 scope fence).
    # Each plugin is loaded via the existing src/plugin/ system (WASM or FFI dylib).
    plugin "presidio_pii_redact" {
        kind = wasm
        path = "/etc/highper/plugins/presidio_pii_redact.wasm"
        phase = pre_request
        on_decision = stop_on_block
    }
    plugin "json_schema_validate" {
        kind = wasm
        path = "/etc/highper/plugins/jsonschema_v7.wasm"
        phase = post_response_body
        config = { schema_path = "/etc/highper/schemas/openai_chat.json" }
    }
    rate_limit {
        rpm = 60
        tpm = 60000
        budget_usd_day = 50
    }
    log_capture = "redacted"   # off | redacted | full
}

provider "openai" {
    base_url = "https://api.openai.com/v1"
    api_key  = ${OPENAI_API_KEY}
    region   = "global"
}

provider "bedrock-us-east-1" {
    sigv4 {
        region    = "us-east-1"
        access_key = ${AWS_ACCESS_KEY_ID}
        secret_key = ${AWS_SECRET_ACCESS_KEY}
    }
    base_url = "https://bedrock-runtime.us-east-1.amazonaws.com"
}
```

YAML-equivalent shipped alongside. Hot-reloadable via existing watcher + admin API. Secrets resolve via existing env-override; Vault/AWS-Secrets/K8s-Secret integration is on the `Phase 1` shopping list of the cross-cutting plan.

---

## 11. Security

- Virtual keys hashed with **HMAC-SHA-256 + server pepper** (`HIGHPER_AI_KEY_PEPPER`), constant-time comparison. See §7.1.3 for the design rationale (Argon2id was dropped — overkill for 256-bit random keys).
- Provider keys at rest: AES-GCM with master key from env or KMS; with CMEK at GA per-tenant key wrap.
- Outbound TLS validates provider certs (no skip-verify); pin rustls roots.
- Bedrock SigV4 signing implemented per AWS spec; no use of long-term creds where IRSA / instance profile available (AWS SDK chain).
- Vertex / Google AI: OAuth2 with GCP service-account token caching (≤55 min TTL).
- Rate-limit-aware backoff respects provider's `Retry-After` exactly; no proxy-level retry on `400/422` (request errors must surface to client).
- DOS protection: max body size enforced **before** body parse; 100MB default for chat, 25MB for embeddings, 100MB for vision (configurable).
- All admin endpoints require key with `admin:*` scope; never callable from user-traffic key.

---

## 12. Open design questions

This section now reflects the resolved decisions from the 2026-05-02
discussion plus the questions still open. Resolved items are marked **DECIDED**
and reference the design-decision number.

1. **DECIDED (UC16 #2)** — Inbound shape priority: **OpenAI + Anthropic both at MVP.** §2.1 / §2.2 above.
2. **DECIDED (UC16 #3)** — Provider integration architecture: **AiProvider trait, plugin-loadable via existing src/plugin/ FFI + WASM infrastructure.** Built-in providers (OpenAI, Anthropic) ship at MVP; Bedrock + Gemini join in Phase 3. Third-party `.so` / `.wasm` plugin loading opens in Phase 3 once the trait shape has soaked. §3.3 above.
3. **DECIDED (UC16 #4)** — Storage backend: **`AiStateStore` trait with three impls.** ReDB (pure-Rust embedded, single-node default), RocksDB (mature embedded, single-node alternative), ScyllaDB (distributed, multi-node). §7.1 above. Single-node deployments use ReDB/RocksDB with 0% storage-layer fault tolerance (acknowledged); multi-node deployments use ScyllaDB.
4. **DECIDED (UC16 #5, 2026-05-02)** — Tenant model + key hashing.
   - **Tenant shape:** **flat keys + tags** for MVP; hierarchy
     (`tenant → workspace → project → key`) lands at Beta as a secondary
     organisation axis on top of tags. §7.1.1 above.
   - **Hash design:** **HMAC-SHA-256 with server pepper** (from
     `HIGHPER_AI_KEY_PEPPER` env var). Argon2id (original draft) was
     dropped — for 256-bit random keys, slow hashing buys nothing against
     the threat model and adds ~10 000× per-request validation latency.
     Industry-standard approach (AWS, Stripe, OpenAI, Vault). §7.1.3 above.
   - **Revocation:** soft-disable (`enabled = false`) by default;
     hard-delete is a separate scoped admin action.
5. **DECIDED (UC16 #8, 2026-05-02)** — Cache architecture: **engine-plus-pluggable**. Highper ships the cache *engine* (canonical hashing, lookup/write-back orchestration, TTL, tag-based invalidation, streaming replay, metrics). Backends are operator's choice via traits: existing `src/cache/` for KV (Valkey / Redis / disk / in-memory / multi-tier); new `VectorIndex` trait (row 11 of §4.4 audit) for semantic-cache vector backend (Qdrant / Redis-Stack / PgVector / HNSW behind Cargo features). Embedding model is operator's choice via `AiProvider` registry (§6.2). §6.4 codifies what's NOT in scope (no bundled backends, no SaaS dashboard, no cache-policy automation). Mirrors §8 boundary table for guardrails.
6. **DECIDED (UC16 #6, 2026-05-02)** — Token-counter posture: **bake all vocabularies into the binary**. Lazy-fetch and hybrid both dropped. Default Cargo feature ships all (~30 MB binary growth); `ai-tokenizers-minimal` ships only OpenAI's two for size-conscious builds. Library choice: **`tiktoken-rs` for OpenAI** (faster, narrower) + **`tokenizers` (HuggingFace) for everything else** (Anthropic, Gemini SentencePiece, Llama BPE, Cohere, DeepSeek). §5.1 + §5.2 above.
7. **DECIDED (UC16 #6, 2026-05-02)** — Reasoning-token billing default: **count as output**, matches provider pricing. Per-virtual-key opt-out via `count_reasoning_in_output: bool` scope flag. Metrics still emit reasoning-token counts with a `kind=reasoning` label regardless of billing inclusion. §5.4 above.
8. **DECIDED (UC16 #10, 2026-05-03)** — Cancellation on client SSE close: **(c) configurable per virtual key, default (a) cancel upstream immediately.** Per-key field `cancel_on_close: bool = true`; flip to `false` for drain-and-record (training-data collection / audit-completeness use cases). Implementation in §3.5.1.
9. **DECIDED (UC16 #10, 2026-05-03)** — TPM hard-stop mid-stream: **(c) configurable per virtual key, default (b) post-stream warning only.** Per-key field `tpm_hard_stop: bool = false`; flip to `true` to inject SSE error frame mid-stream when TPM exceeded. Default-off because mid-stream hard-stops are jarring; opt-in serves cost-sensitive batch jobs. Implementation in §3.5.2.
10. **Prompt registry storage** — durable layer in `AiStateStore` rows vs Git-backed text vs both? Recommended: **`AiStateStore` rows MVP**, expose `git push` adapter at GA for GitOps users.
11. **MCP placement** — gateway hosts MCP server (lets LLMs query gateway state), or proxy-only? Recommended: **proxy-only MVP**, in-process MCP server in Beta.
12. **DECIDED (UC16 #7, 2026-05-02)** — Pricing source: **vendored LiteLLM snapshot baked into binary** + **weekly signed refresh** from `HIGHPER_AI_PRICING_FEED_URL` (default LiteLLM upstream; operator can self-host) + **admin override at runtime** via `PATCH /admin/ai/models/{alias}` (operator-level overrides at MVP, per-tenant overrides at Phase 3). Refresh failure default `last_known_good`; `fail_closed` opt-in for regulated billing. Zero-price model lookup default-reject (`HIGHPER_AI_ALLOW_FREE_TIER=false`). §5.5 above.
13. **Realtime / Voice** — OpenAI Realtime + Gemini Live needed at MVP, Beta, or GA? Recommended: **GA**; voice apps are a smaller market and the WS reuse is non-trivial.
14. **License posture** — keep Apache-2 (matches the rest of highper-gateway), or BUSL/Commons-Clause for enterprise pieces (audit, BYOK, evals)? Owner decision; affects monetization story.
15. **Inference-engine integration (UC17)** — explicit non-goal here, but with the §3.3 plugin architecture, a self-hosted-models integration becomes "an `AiProvider` plugin that wraps `mistral.rs` / `candle` in-process." Recommended: **defer to UC17** but the router contract is now plugin-shaped which makes UC17 strictly additive.
16. **AiStateStore single → multi node migration** (new question, opened by UC16 #4): is there a documented path for an operator who starts on ReDB (single-node) and later adopts ScyllaDB (multi-node)? Options: (a) export tool that walks ReDB and writes to ScyllaDB; (b) operator runs both side-by-side during the transition (dual-write at the trait layer); (c) operator restarts fresh — accept that the single-node deployment was throwaway. Recommended for design discussion: **(a) export tool**, ship in Phase 3.
17. **DECIDED (UC16 #9, 2026-05-02)** — Routing strategies and fallback. Layered MVP: priority + rate-limit-aware skip + health-aware (circuit breaker) + capability-aware filter. Stack ships together at Phase 2.3. Cost-aware (Beta), latency-aware (GA), session affinity (Beta — N23), weighted/canary (Beta) are subsequent additions. Retry budget default **3 attempts max across providers** with per-virtual-key override. Cooldown state in Type B Valkey when configured (cluster-wide consistency), local per-replica fallback. Structured 503 with per-attempt details on exhaustion. §3.4 above.

---

## 13. Acceptance criteria — MVP

A user must be able to:

1. `cargo run` highper-gateway with config that registers OpenAI, Anthropic, Bedrock, Gemini provider blocks plus 5 model aliases.
2. `curl https://localhost/v1/chat/completions` with an `sk-hpgw-...` key get back a streaming **OpenAI-shape** response sourced from any of those four providers (selected by alias) — including for Anthropic / Bedrock / Gemini whose native shapes differ.
3. **(MVP per UC16 #2)** `curl https://localhost/v1/messages` with an `sk-hpgw-...` key get back a streaming **Anthropic-shape** response sourced from any of the four providers; the Anthropic Python SDK (`anthropic.Anthropic(base_url=...)`) talks to highper-gateway against an OpenAI upstream successfully.
4. Set a $5/day budget on the key; verify gateway returns 402 when exceeded.
5. Hit the same chat completion twice; second request returns from exact cache with `x-cache: HIT`.
6. Configure a fallback list `[claude-3-7-sonnet, gpt-4o]`; simulate Anthropic 503; verify gateway transparently fails over to OpenAI and the client sees a single successful response.
7. Cancel a streaming request mid-stream; verify upstream is cancelled (TCP RST visible in metrics) and partial usage is logged.
8. Query `/admin/ai/spend?key=...&from=...&to=...` and see token/cost rollups.
9. Watch Prometheus metrics show `ai_input_tokens_total`, `ai_cost_usd_total`, `ai_cache_hit_ratio` updating.
10. Hot-reload the DSL to add a new model alias; new alias resolvable on next request without restart.
11. Log entries show redacted prompts (configurable) and full request_id linkage.
12. **(MVP per UC16 #4)** Single-node deployment with the `redb` AiStateStore impl runs end-to-end with no external storage dependencies; same deployment switching to ScyllaDB-backed AiStateStore in multi-node configuration via env-var change only.

---

## 14. Out of MVP, in Beta+ scope

- Semantic cache.
- Prompt registry + A/B.
- MCP passthrough (then in-process server at GA).
- Vision / multimodal passthrough (image fetching, Bedrock base64 conversion).
- Batch API, Files API, fine-tuning passthrough.
- WS Realtime (OpenAI / Gemini Live).
- Cost-optimized routing (capability-aware cheapest).
- Async response with callback URL.
- Idempotency keys.
- Full SOC2 audit log + tenant CMEK.
- Rate-limit-aware LB parsing `x-ratelimit-*`.
- Eval framework integration (Promptfoo).
- xDS / K8s operator (separate cross-cutting milestone).
- Third-party-maintained `AiProvider` plugins (the trait stabilises in MVP; opening to third-party `.so` / `.wasm` plugins comes in Phase 3 once the trait shape has soaked — see §3.3).

Note: guardrails (pre + post), PII detection, moderation calls, jailbreak
detection are **out of scope as products** at every phase per the §8 scope
fence — they live in customer-side services and integrate via the
`src/plugin/` hook surface. Anthropic-shape inbound was previously listed
here but is **MVP** as of UC16 design decision #2 (2026-05-02).

See `ROADMAP.md` §5 for sequencing.

---

## 15. Risks specific to UC16

| Risk | Likelihood | Mitigation |
|---|---|---|
| Provider API churn (esp. Anthropic, Gemini) | High — new fields every quarter | Translator unit tests pinned to recorded fixtures; `unknown_field` capture so we forward what we don't know. |
| Tokenizer drift (OpenAI quietly switches `o200k` rules) | Medium | Pin tiktoken-rs version; track upstream; tolerance on cost rollup ±2%. |
| Provider prompt-cache breakage from byte instability | High | Serde `preserve_order`; explicit byte-stable serializer test in CI. |
| Semantic-cache false hits (returning irrelevant cached responses) | Medium | Threshold default conservative (0.92); per-tenant override; `x-cache: SEMANTIC` so client can detect. |
| Cost mis-billing (off by orders of magnitude) | Medium-High (finance bug) | Pre-launch reconciliation against provider's own spend data for one month; alert on >5% delta. |
| Reasoning-model timeout exceeds proxy idle | High | Default idle ≥600s for AI routes; per-route override. |
| Streaming buffer growth under slow client | Medium | Bounded mpsc between upstream-recv and client-send; backpressure must propagate. |
| Provider key leak via logs | Medium | Mask `Authorization`/`x-api-key` headers in all log paths; CI grep for raw key patterns in test output. |
| Operator-supplied guardrail plugin misbehaviour (false positives, latency, panics) | Medium | Plugin sandbox (WASM fuel limit, FFI panic-safe wrappers, per-plugin timeout from `HIGHPER_PLUGIN_BUDGET_US`); plugins fail-open by default; operator audit-logs the plugin's decisions. Highper does not own the *content* of the plugin's decisions — only the runtime safety. |

---

## 16. References (verify in network-enabled session)

- LiteLLM proxy docs: docs.litellm.ai/docs/proxy/quick_start
- Portkey OSS gateway: github.com/Portkey-AI/gateway, portkey.ai/docs/product/ai-gateway
- Helicone AI Gateway: docs.helicone.ai
- Kong AI Gateway: docs.konghq.com/gateway/latest/ai-gateway, plugin hub
- Cloudflare AI Gateway: developers.cloudflare.com/ai-gateway
- OpenRouter: openrouter.ai/docs
- Envoy AI Gateway (Tetrate): github.com/envoyproxy/ai-gateway
- OpenAI API: platform.openai.com/docs/api-reference
- Anthropic Messages API: docs.anthropic.com/en/api
- Google AI / Gemini: ai.google.dev/api
- Vertex AI generative: cloud.google.com/vertex-ai/generative-ai/docs/reference
- AWS Bedrock: docs.aws.amazon.com/bedrock/latest/APIReference
- LiteLLM model price JSON (importable): github.com/BerriAI/litellm — `model_prices_and_context_window.json` (MIT)

---

*Design author: claude-opus-4-7-1m, 2026-05-02. Working tree: `master @ 10752ce`.*

---

## 17. Document lifecycle

- **2026-05-02 (initial draft):** authored. 13 open design questions; companion to `docs/AUDIT_2026-05-02.md`.
- **2026-05-02 (scope-fence + decisions #1–#4 fold-in, current):**
  - **#1 Scope fence (per `ROADMAP.md` §3.1)**: removed §8 Guardrails section entirely (replaced with brief external-integration note); removed `guardrails {}` DSL block (replaced with `plugin { … }` references to existing `src/plugin/` system); removed guardrail module entries from §3.2 module map; clarified §6.2 semantic cache (embedding model is operator's choice); clarified §9 observability (highper exposes Prometheus + OTLP — AI observability platforms external); pruned guardrail items from §14 Out of MVP and §15 Risks; updated §1 goal + non-goals to match memory `uc16_scope.md`.
  - **#2 OpenAI + Anthropic dual-shape MVP** (was OpenAI-only MVP recommended): both inbound shapes funnel into canonical `AiRequest`. Updated §2.1 / §2.2 / §3.2 module map / §13 acceptance criteria #3 / §14 / §12 #1.
  - **#3 AiProvider as plugin architecture**: new §3.3 documenting separate trait pattern (not extending `Plugin` — same architectural choice as `WafEngine` / `Compressor` today), plugin-loadable via existing `src/plugin/` FFI + WASM infrastructure. Built-in providers ship at MVP; third-party `.so` / `.wasm` plugin loading opens Phase 3 once trait shape soaks. ROADMAP Phase 2.1 task updated.
  - **#4 AiStateStore trait + ReDB / RocksDB / ScyllaDB impls**: §7 storage subsection rewritten; PostgreSQL / FoundationDB / sled / Redis-AOF dropped from candidates. §13 acceptance criteria #12 added. Single-node → multi-node migration becomes new §12 #16. ROADMAP §6 #5 gate marked DECIDED. HA_ARCHITECTURE.md §3.5 cookbook table updated.
  - Memory `uc16_scope.md` updated to record all four decisions.
- **2026-05-02 (decision #5 + plugin lifecycle clarifications, current):** topic-#5 fold-in.
  - **Decision #5 — tenant model + key hashing:** §12 #4 marked DECIDED. **Flat keys + tags MVP** (hierarchy lands at Beta as a secondary axis on top of tags). **HMAC-SHA-256 + server pepper** replaces Argon2id (overkill for 256-bit random keys; saves ~10 000× per-request validation cost; matches AWS / Stripe / OpenAI / Vault industry practice). Soft-disable revocation default; hard-delete is a separate scoped admin action. §7.1 fully rewritten with three sub-sections (7.1.1 shape, 7.1.2 storage, 7.1.3 hash design).
  - **Plugin lifecycle clarifications:** new §3.3.7 documenting hot-load capability matrix for `AiProvider` plugins. Reuses existing `src/plugin/hot_reload.rs` infrastructure (notify-watcher + 30 s drain via `manager.rs:252 wait_for_plugin_idle()`); built-in providers update via Phase 0 config reload (no plugin reload needed for key rotation / model alias / RPM tuning); third-party plugin code updates hot-load in Phase 3 Beta. Plugin drain timeout moved to `HIGHPER_PLUGIN_DRAIN_SECS` per §0.1.
  - **§11 Security row** updated: HMAC-SHA-256+pepper replaces Argon2id reference.
  - **ROADMAP §4.4 interface-first table** extended to 10 boundaries: `AiStateStore` (row 9, greenfield, Phase 2.1) and `AiProvider` (row 10, greenfield, Phase 2.1). Both follow the established separate-trait + plugin-loadable pattern of `Compressor` / `WafEngine`. Total trait-extraction effort grows from 9.5 to 10.5 person-weeks across the project.
  - **ROADMAP Phase 0.J** gains two env vars: `HIGHPER_AI_KEY_PEPPER` (0.5 day) and `HIGHPER_PLUGIN_DRAIN_SECS` (0.5 day).
  - **ROADMAP Phase 2.4** virtual-key task updated: HMAC + pepper instead of Argon2id; AiStateStore-backed instead of sled-MVP.
- **2026-05-02 (decision #6 — tokenization posture, current):** topic-#6 fold-in.
  - **§5 Tokenization & cost** rewritten with five sub-sections (5.1
    posture / 5.2 library choice / 5.3 estimate-vs-truth /
    5.4 reasoning-tokens / 5.5 cost source). Bake all vocabularies into
    the binary (~30 MB at Phase 3 GA); lazy-fetch and hybrid both
    dropped (fail air-gap deployments + first-request latency).
  - **`tiktoken-rs` for OpenAI** + **`tokenizers` (HuggingFace) for
    everything else** — different crates, each used for what they do
    well, both mature with permissive licences.
  - **Reasoning tokens default to output billing**, with per-virtual-key
    `count_reasoning_in_output: bool` opt-out. Metrics still emit
    reasoning-token counts unconditionally.
  - **Cargo feature flags**: default ships all; `ai-tokenizers-minimal`
    ships OpenAI-only.
  - **§12 #6 and #7 marked DECIDED.**
  - ROADMAP Phase 2.3 token-counter task expanded to 3 days
    (OpenAI + Anthropic at MVP per UC16 #2); new 1-day "reasoning-token
    billing rule" task added; Phase 3 vocabulary additions noted as
    bundled with respective `AiProvider` impls.
- **2026-05-02 (decision #7 — cost / pricing source, current):** topic-#7 fold-in.
  - **§5.5 fully rewritten** with six sub-sections (5.5.1 source /
    5.5.2 admin override / 5.5.3 refresh failure handling /
    5.5.4 zero-price handling / 5.5.5 per-tenant overrides — Phase 3 /
    5.5.6 schema fields).
  - **Vendored LiteLLM snapshot** baked into the binary at build time
    (mirrors §5.1 tokenizer bake-all stance) + **weekly signed refresh**
    from `HIGHPER_AI_PRICING_FEED_URL` (default LiteLLM upstream,
    operator can self-host).
  - **Admin override at runtime** via `PATCH /admin/ai/models/{alias}`
    (per-row override of price / capability / context-window fields with
    optional expiry). Stored under `ai/pricing_overrides/global/{alias}`
    namespace in the configured `AiStateStore` (UC16 #4). Hot-applied
    via `RwLock` swap; no restart, no refresh wait. Every change emits
    a hash-chain audit event into the `AiStateStore` audit log. Five new
    admin endpoints added to §2.4.
  - **Refresh failure** default `last_known_good`; `fail_closed` opt-in
    via `HIGHPER_AI_PRICING_REFRESH_FAIL_MODE` for regulated billing.
    Health endpoint `/health/ai/pricing` reports staleness.
  - **Zero-price handling** — default-reject with 400 ("model X has no
    pricing configured"); `HIGHPER_AI_ALLOW_FREE_TIER=true` opt-in for
    self-hosted / free-tier deployments.
  - **Per-tenant overrides** queued for **Phase 3** (multi-tenant
    resellers / per-tier rates). MVP ships operator-level overrides only.
  - **§12 #12 marked DECIDED.**
  - ROADMAP Phase 0.J gains 5 pricing env vars; Phase 2.3 gains 4 new
    pricing tasks (~6 days); Phase 3.1 gains a 3-day per-tenant override
    task. Phase 3.1 also got a scope-fence cleanup pass: killed
    pre-/post-call guardrail items (per UC16 #1), deferred Anthropic-shape
    inbound (now Phase 2.1 MVP per UC16 #2), updated prompt-registry
    storage to `AiStateStore`.
- **2026-05-02 (decision #8 — cache architecture, current):** topic-#8 fold-in.
  - **§6 fully rewritten** with six sub-sections: 6.0 engine-plus-pluggable
    posture / 6.1 exact cache (MVP) / 6.2 semantic cache (Beta) with
    `VectorIndex` trait / 6.3 provider prompt-cache passthrough /
    **6.4 explicit boundary table — what's NOT in scope for cache**
    (mirrors §8 guardrails section) / 6.5 operator effort summary.
  - **Engine-plus-pluggable posture recorded:** highper ships the cache
    *engine* (canonical hashing, lookup / write-back, TTL, tag invalidation,
    streaming replay, metrics, per-tenant isolation); KV backend via
    existing `src/cache/` trait, vector backend via new `VectorIndex`
    trait, embedding model via `AiProvider` registry — all operator-chosen.
    Same architectural pattern as `AiProvider` plugins and `AiStateStore`.
    Strategically: same configure-don't-code operator effort as LiteLLM,
    none of the backend lock-in of Portkey / Helicone / Cloudflare AI
    Gateway.
  - **§3.2 module map** gains: `provider.rs`,
    `vector_index/{mod,qdrant,redis_stack,pgvector,hnsw}.rs`.
  - **§12 #5 marked DECIDED.**
  - **§6.4 boundary table** (new) explicitly fences six concerns out of
    scope: bundled cache backend product, bundled embedding model,
    SaaS cache dashboard, cache-policy automation, cache-warming as
    product, managed vector DB. Operator integrates externally.
  - **§6.5 operator effort summary** added: ~0 lines Rust code, ~10–15
    env vars, ~30–50 lines DSL for a deployment with Valkey + Qdrant.
  - ROADMAP §4.4 audit table extended to 11 boundaries (`VectorIndex`
    row 11). Phase 0.J gains 4 cache + vector env vars. Phase 2.4
    gains 3 exact-cache tasks. Phase 3.1 gains `VectorIndex` trait +
    Qdrant impl + semantic-cache engine + 3 more `VectorIndex` impls.
- **2026-05-02 (decision #9 — routing strategies, current):** topic-#9 fold-in.
  - **§3.4 (new) Routing strategies and fallback** with six sub-sections:
    3.4.1 layered candidate-list construction (5-stage filter pipeline) /
    3.4.2 retryable-vs-non-retryable error classification /
    3.4.3 retry budget + backoff (3 attempts default, exponential
    50-200 ms when no `Retry-After`) / 3.4.4 structured 503 failure
    response / 3.4.5 cooldown state location (Type B Valkey when
    available else local `DashMap`) / 3.4.6 router metrics (7 metrics).
  - **§3.1 [Route resolve] step** updated to reference §3.4.
  - **§12 entry #17 added and marked DECIDED.**
  - MVP ships layered routing: per-key allow-list + capability filter
    + circuit-breaker + rate-limit cooldown + priority ordering,
    stacked. Cost-aware (Beta), latency-aware (GA), session affinity
    (Beta — N23), weighted/canary (Beta) all phased afterwards.
  - ROADMAP Phase 2.3 router task expanded 3 → 5 days. Phase 0.J
    gains 5 routing env vars. Phase 3.1 gains 2 new tasks (cost-aware,
    weighted/canary). Phase 4.1 gains 1 new task (latency-aware).
- **2026-05-03 (decision #10 — streaming + cancellation, current):** topic-#10 fold-in.
  - **§3.5 (new) Streaming and cancellation semantics** with five
    sub-sections: 3.5.1 cancellation on client disconnect (cancel
    upstream immediately as default; per-key `cancel_on_close=false`
    opt-in for drain-and-record) / 3.5.2 TPM hard-stop vs post-stream
    warning (post-stream warning default; per-key `tpm_hard_stop=true`
    opt-in for SSE error frame mid-stream) / 3.5.3 chunk hooks +
    bounded buffer (default 64 events, drop-oldest) + plugin budget
    (default 500 µs, fail-open on overrun) / 3.5.4 streaming metrics
    (4 new) / 3.5.5 provider-side prompt-cache passthrough during
    streaming.
  - **§3.1 [SSE stream chunker]** step updated to reference §3.5.
  - **§7.1.1 virtual-key scope** gains 4 fields: `retry_budget`
    (decision #9), `count_reasoning_in_output` (decision #6),
    `cancel_on_close` (decision #10), `tpm_hard_stop` (decision #10).
  - **§5.4 streaming-TPM** row gets a back-reference to §3.5.2.
  - **§12 #8 and #9 marked DECIDED** (consolidated under decision #10).
  - ROADMAP Phase 0.J gains 5 streaming env vars; Phase 2.5 streaming
    work grows from ~6 to ~11 days (chunker, cancellation, TPM
    mid-stream, metrics).
- **Future:** edit in place. Append revision entries here; do not silently rewrite without an entry.
