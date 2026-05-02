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
[Route resolve]                                            (NEW src/gateway/ai/router.rs)
  │  - Model alias → ordered list of (provider, key, model_id)
  │  - Filter on rate-limit-aware health: skip 429-cooled-down
  │  - Conditional routing (env=prod → primary)
  ▼
[Outbound shape translate]                                 (NEW src/gateway/ai/translators/)
  │  - Convert Request → provider-native (OpenAI / Anthropic / Bedrock-Converse / Gemini)
  │  - Sign if SigV4 (Bedrock); add OAuth bearer if Vertex
  ▼
[HTTP client → provider]                                   (reuses src/proxy/connection_pool.rs)
  │  - Per-(provider, region) pool, h2 preferred
  │  - Idle keepalive, generous read timeout (≥600s for reasoning)
  ▼
[SSE stream chunker]                                       (extends src/http/proxy_streaming.rs)
  │  - Parse provider's SSE / event-stream-binary / NDJSON
  │  - Re-emit as inbound-shape SSE
  │  - Per-chunk hook: token count, log, optional plugin (operator-supplied)
  │  - Cancellation: client SSE close → cancel upstream + record partial usage
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
| `src/gateway/ai/cache_semantic.rs` | NEW | Embed + ANN, threshold-gated; embedding model via configured AiProvider |
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
| Streaming TPM enforcement | Default: post-stream warning (log + metric, no inline action). Hard-stop opt-in per virtual key — when set, gateway injects an SSE error frame and closes upstream once token budget is exhausted mid-stream. Hard-stop is jarring for end-users; default-off is intentional. |

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

### 5.5 Cost source

| Concern | Decision |
|---|---|
| Cost source | Boot-load `model_prices.json` from LiteLLM (MIT) — `model_prices_and_context_window.json` — refreshed weekly via signed download. Admin can override per row at runtime. Detail in §12 #12 (still open) — vendored snapshot vs live feed. |

---

## 6. Caching

### 6.1 Exact cache

Key = `sha256(canonical_json({model, messages, tools, tool_choice, temperature, top_p, max_tokens, stop, response_format, seed}))`.

Canonical JSON = serde with `preserve_order` + sorted keys + no float reformat (preserve original byte representation). This is required for **provider prompt-cache compatibility** (Anthropic/OpenAI/Bedrock will only cache if request bytes are stable across calls).

Backend: existing `cache::manager` (memory + Redis + disk).

TTL: per-route default 1h; clients can override via `x-cache-ttl` header.

`x-cache: HIT|MISS|BYPASS|SEMANTIC` response header.

### 6.2 Semantic cache (Beta)

1. On cache miss, embed the **last user message** (or a configured slice) using a **configured embedding provider** — chosen by the operator via the same `AiProvider` registry that handles outbound calls (§3.3). Highper ships **no bundled embedding model**; operators point at OpenAI's `text-embedding-3-small`, Bedrock Titan, a local Sentence-Transformers service, or anything else they prefer.
2. ANN search against per-tenant index; threshold default 0.92 cosine.
3. On hit above threshold: return the cached response as-if-fresh; record cache_type=semantic.
4. On miss or below threshold: forward to provider, then on completion store `(embedding, response)` in index.

Vector index backend (Section 12 question — operator chooses, highper does not bundle):

- **Qdrant** — separate process; common operator choice
- **Redis-Stack** (Vector Set + RedisJSON) — co-located with Type B Valkey if already deployed
- **PgVector** — operator already running PostgreSQL
- **In-process HNSW** (`hnsw_rs`) — single-node only; for dev / small-scale

Highper exposes a `VectorIndex` trait so any of the above can plug in; the choice is per-deployment configuration, not a built-in product feature.

### 6.3 Provider prompt-cache passthrough

When forwarding to providers that support native prompt caching (Anthropic `cache_control`, OpenAI `prompt_cache_key` / fixed-prefix automatic, Bedrock prompt caching), preserve the user-supplied `cache_control` markers verbatim. Do NOT re-serialize JSON in a way that changes byte order; do NOT touch float formatting.

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
5. **Vector index backend** for semantic cache — Qdrant / Redis-Stack / PgVector / in-process HNSW? Recommended: **Redis-Stack** (already a likely dep for distributed rate-limit / Type B Valkey) for MVP; pluggable `VectorIndex` trait so Qdrant can be added later. The embedding **model** itself is operator's choice via the AiProvider registry (§6.2).
6. **DECIDED (UC16 #6, 2026-05-02)** — Token-counter posture: **bake all vocabularies into the binary**. Lazy-fetch and hybrid both dropped. Default Cargo feature ships all (~30 MB binary growth); `ai-tokenizers-minimal` ships only OpenAI's two for size-conscious builds. Library choice: **`tiktoken-rs` for OpenAI** (faster, narrower) + **`tokenizers` (HuggingFace) for everything else** (Anthropic, Gemini SentencePiece, Llama BPE, Cohere, DeepSeek). §5.1 + §5.2 above.
7. **DECIDED (UC16 #6, 2026-05-02)** — Reasoning-token billing default: **count as output**, matches provider pricing. Per-virtual-key opt-out via `count_reasoning_in_output: bool` scope flag. Metrics still emit reasoning-token counts with a `kind=reasoning` label regardless of billing inclusion. §5.4 above.
8. **Cancellation semantics** — on client SSE close, do we (a) cancel upstream immediately (saves $, may lose audit trail), (b) drain upstream silently and record full usage, or (c) configurable per-key? Recommended: **(c) configurable, default (a)**.
9. **Hard-stop on TPM enforcement** — break stream with error frame, or only post-stream warning? Recommended: **post-stream warning** by default; hard-stop opt-in (hard-stop semantics are jarring in practice).
10. **Prompt registry storage** — durable layer in `AiStateStore` rows vs Git-backed text vs both? Recommended: **`AiStateStore` rows MVP**, expose `git push` adapter at GA for GitOps users.
11. **MCP placement** — gateway hosts MCP server (lets LLMs query gateway state), or proxy-only? Recommended: **proxy-only MVP**, in-process MCP server in Beta.
12. **Pricing source** — vendored snapshot, periodic refresh from LiteLLM JSON, or community feed? Recommended: **boot-load from vendored LiteLLM JSON snapshot, refresh weekly via signed download, admin override at runtime.**
13. **Realtime / Voice** — OpenAI Realtime + Gemini Live needed at MVP, Beta, or GA? Recommended: **GA**; voice apps are a smaller market and the WS reuse is non-trivial.
14. **License posture** — keep Apache-2 (matches the rest of highper-gateway), or BUSL/Commons-Clause for enterprise pieces (audit, BYOK, evals)? Owner decision; affects monetization story.
15. **Inference-engine integration (UC17)** — explicit non-goal here, but with the §3.3 plugin architecture, a self-hosted-models integration becomes "an `AiProvider` plugin that wraps `mistral.rs` / `candle` in-process." Recommended: **defer to UC17** but the router contract is now plugin-shaped which makes UC17 strictly additive.
16. **AiStateStore single → multi node migration** (new question, opened by UC16 #4): is there a documented path for an operator who starts on ReDB (single-node) and later adopts ScyllaDB (multi-node)? Options: (a) export tool that walks ReDB and writes to ScyllaDB; (b) operator runs both side-by-side during the transition (dual-write at the trait layer); (c) operator restarts fresh — accept that the single-node deployment was throwaway. Recommended for design discussion: **(a) export tool**, ship in Phase 3.

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
- **Future:** edit in place. Append revision entries here; do not silently rewrite without an entry.
