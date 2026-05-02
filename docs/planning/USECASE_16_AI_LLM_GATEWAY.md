# Use Case 16 — AI / LLM Gateway

**Status:** Design (greenfield — no prior implementation in highper-gateway).
**Author:** claude-opus-4-7-1m, 2026-05-02.
**Companion:** `docs/AUDIT_2026-05-02.md` (codebase audit), `docs/PHASED_RELEASE_PLAN.md` (sequencing).

---

## 1. Goal

Deliver a self-hostable AI gateway inside highper-gateway with feature parity, in scope, to LiteLLM Proxy / Portkey OSS Gateway / Helicone AI Gateway / Kong AI Gateway. The gateway must let a single OpenAI-shaped client SDK talk to **any** of OpenAI, Azure OpenAI, Anthropic, Google AI Studio, Vertex AI, AWS Bedrock, Mistral, Cohere, Together, Fireworks, Groq, xAI, DeepSeek, Perplexity, Ollama, vLLM, TGI, HF Inference, Cloudflare Workers AI, Replicate — with central routing, virtual keys, budgets, caching, guardrails, observability, and audit.

### Non-goals

- **Not** an inference engine. The gateway proxies; it does not host model weights. A local `mistral.rs`/`candle` integration is potential UC17, deliberately deferred.
- **Not** a vector DB. Semantic cache uses an external vector backend (Qdrant / Redis-Stack / PgVector / in-process HNSW — TBD per Section 12).
- **Not** an evaluation framework. Eval integration uses external runners (Promptfoo) over logged traffic; the gateway exposes hooks but does not implement scoring.

---

## 2. User-visible API surface

### 2.1 OpenAI-compatible (MVP)

```
POST /v1/chat/completions
POST /v1/embeddings
GET  /v1/models
POST /v1/moderations
POST /v1/images/generations            (Beta)
POST /v1/audio/transcriptions          (GA)
POST /v1/audio/speech                  (GA)
POST /v1/files                         (GA)
POST /v1/batches                       (Beta)
WS   /v1/realtime                      (GA)
```

### 2.2 Anthropic-shape inbound (Beta)

```
POST /v1/messages
POST /v1/messages/batches
GET  /v1/models
```

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
[Pre-call guardrails]                                      (NEW src/gateway/ai/guardrails/)
  │  - PII detect / redact / block
  │  - Jailbreak detect (Llama-Guard / regex set)
  │  - Length cap, banned topics
  │  - Returns Decision::{Allow, Block(reason), Redact(new_body)}
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
  │  - Per-chunk hook: token count, post-call-guardrail-stream, log
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
| `src/gateway/ai/cache_semantic.rs` | NEW | Embed + ANN, threshold-gated |
| `src/gateway/ai/guardrails/mod.rs` | NEW | Engine trait + chain |
| `src/gateway/ai/guardrails/pii.rs` | NEW | Regex + Presidio FFI |
| `src/gateway/ai/guardrails/moderation.rs` | NEW | OpenAI moderation, Llama-Guard, Bedrock Guardrails |
| `src/gateway/ai/guardrails/jailbreak.rs` | NEW | Pattern + small-LLM judge |
| `src/gateway/ai/sse.rs` | NEW | SSE-aware chunker; per-chunk callback |
| `src/gateway/ai/prompts.rs` | NEW | Versioned prompt registry |
| `src/gateway/ai/mcp.rs` | NEW | MCP server passthrough |

| Reused module | Used for |
|---|---|
| `src/gateway/auth/{api_key,jwt}.rs` | virtual-key validation |
| `src/gateway/ratelimit/{token_bucket,sliding_window,distributed}.rs` | RPM, TPM, RPD, TPD with dynamic-cost (= tokens) bucket |
| `src/cache/{manager,backends,disk}.rs` | exact-cache backing |
| `src/proxy/{loadbalancer,retry,circuit_breaker,connection_pool}.rs` | provider routing, retry with backoff, fail-shut on hard outage |
| `src/discovery/registry.rs` | provider registry with hot-reload |
| `src/observability/{metrics,structured_logging,tracing}.rs` | metrics, logs, traces (extended with AI tags) |
| `src/admin/{api,routes,config_persistence}.rs` | admin endpoints |
| `src/plugin/{wasm,host_functions}.rs` | guardrail extension points |
| `src/config/{dsl_parser,loader,reloader,watcher}.rs` | DSL + YAML, hot reload |
| `src/http/{proxy_streaming,streaming_body}.rs` | SSE base |
| `src/middleware/{transform,body_access,streaming_validator}.rs` | shape translation hooks |
| `src/websocket/*` | OpenAI Realtime / Gemini Live (GA) |
| `src/tls/*` | TLS to providers, ALPN h2 negotiation |

---

## 4. Canonical request / response types

The shape translators all funnel through a single in-process representation so middleware (cache, guardrails, accounting) is shape-agnostic.

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

## 5. Tokenization & cost

| Concern | Decision |
|---|---|
| OpenAI / Azure tokenizer | `tiktoken-rs` crate, `cl100k_base` + `o200k_base` baked in; ~1µs/token. |
| Anthropic | Anthropic's tokenizer (Claude 2/3/3.5/3.7 share same vocab); use `tokenizers` crate with downloaded vocab; cache vocab on disk. |
| Gemini | SentencePiece via `tokenizers`; vocab from HF. |
| Llama / Mistral | Llama-3 BPE via `tokenizers`. |
| Pre-call estimate vs post-call truth | Estimate with local tokenizer for budget gate. Post-call: prefer provider-returned `usage.{input,output}_tokens`; fall back to local tokenizer on output stream. |
| Streaming TPM enforcement | Hard-stop = inject SSE error frame + close upstream; soft = warn + log (default). Configurable per-key. |
| Cost source | Boot-load `model_prices.json` from LiteLLM (MIT) — `model_prices_and_context_window.json` — refreshed weekly via signed download. Admin can override per row. |
| Reasoning tokens | Anthropic `thinking`, OpenAI `reasoning_content`, DeepSeek `reasoning_content` — counted as output by default; per-key flag `count_reasoning_in_output: bool`. |

---

## 6. Caching

### 6.1 Exact cache

Key = `sha256(canonical_json({model, messages, tools, tool_choice, temperature, top_p, max_tokens, stop, response_format, seed}))`.

Canonical JSON = serde with `preserve_order` + sorted keys + no float reformat (preserve original byte representation). This is required for **provider prompt-cache compatibility** (Anthropic/OpenAI/Bedrock will only cache if request bytes are stable across calls).

Backend: existing `cache::manager` (memory + Redis + disk).

TTL: per-route default 1h; clients can override via `x-cache-ttl` header.

`x-cache: HIT|MISS|BYPASS|SEMANTIC` response header.

### 6.2 Semantic cache (Beta)

1. On cache miss, embed the **last user message** (or a configured slice) using a configured embedding provider (default: `text-embedding-3-small`).
2. ANN search against per-tenant index; threshold default 0.92 cosine.
3. On hit above threshold: return the cached response as-if-fresh; record cache_type=semantic.
4. On miss or below threshold: forward to provider, then on completion store `(embedding, response)` in index.

Backend candidates (Section 12 question):

- **Qdrant** — best perf, separate process
- **Redis-Stack** (Vector Set + RedisJSON) — already a possible dep, single ops surface
- **PgVector** — ops-friendly if Postgres is mandatory
- **In-process HNSW** (`hnsw_rs`) — no extra service, no HA out of the box

### 6.3 Provider prompt-cache passthrough

When forwarding to providers that support native prompt caching (Anthropic `cache_control`, OpenAI `prompt_cache_key` / fixed-prefix automatic, Bedrock prompt caching), preserve the user-supplied `cache_control` markers verbatim. Do NOT re-serialize JSON in a way that changes byte order; do NOT touch float formatting.

---

## 7. Virtual keys, budgets, multi-tenant

### 7.1 Key model

```
tenant ─┬─ workspace ─┬─ project ─┬─ key (sk-hpgw-...)
        │             │           │
        └─ rbac role  │           ├─ scopes:
                      │           │    models_allow=[...]
                      │           │    rpm=, tpm=, rpd=, tpd=
                      │           │    budget_usd_day=, _month=
                      │           │    expires_at=
                      │           │    enabled=
                      │           └─ tags:
                      │                env, team, cost_center
                      └─ kms_key_ref (BYOK + CMEK, GA tier)
```

Storage:
- MVP: `sled` embedded KV, single-node.
- Beta: Postgres for SQL queries (spend rollups, audit).
- GA: Postgres + per-tenant CMEK wrapping.

Key format: `sk-hpgw-<base64url(rand 32B)>`. Stored as Argon2id hash with salt; only prefix (8 chars) shown in UI.

### 7.2 Budgets

- **Hard caps**: gateway rejects with 402 once exceeded (plus 429 if rate-limit semantics preferred).
- **Soft warnings**: emit metric + audit event; pass-through.
- Windows: per-day, per-month, lifetime.
- Aggregation on accounting: every request decrements counters in Redis with TTL = window remaining; primary in Postgres for durable rollups.

### 7.3 Audit trail

Every key change, budget change, prompt change emits an audit event:
```
{ts, actor, action, resource_type, resource_id, before, after, ip, request_id}
```
Stored append-only in Postgres; daily JSONL export with hash-chain integrity (block-chain style: each row contains hash(prev_row || row_data)).

---

## 8. Guardrails

### 8.1 Pre-call

| Guard | Source | Trigger | Action |
|---|---|---|---|
| PII detect | regex (email/SSN/PAN/phone), Presidio FFI | request body | block / redact / log |
| Length cap | char + token count | input_tokens > limit | reject 413 |
| Topic deny | banned-keyword set, embed-similarity to deny vectors | prompt | block |
| Jailbreak | Llama-Guard / Granite-Guardian / Lakera | request | block / log |
| OpenAI moderation | upstream call to `/v1/moderations` | request | block on `harassment/threatening` etc |
| Bedrock Guardrails | upstream `apply-guardrail` | request | block / mask |
| Custom WASM | `src/plugin/` | request | any |

### 8.2 Post-call (streaming)

| Guard | Trigger | Action |
|---|---|---|
| Output PII redact | per-chunk regex | redact in stream (replace + emit) |
| Toxicity score | small classifier on accumulated text | break stream + emit error frame |
| JSON-schema validate | accumulate, validate at `[DONE]` | reject + emit error |
| Regex deny | per-chunk | break stream |
| Hallucination check (judge LLM) | post-stream async | log only (too slow for inline) |

Engine uses the same `WafEngine` trait as `src/middleware/waf/engine.rs` but operates on `AiRequest` / `AiStreamEvent` rather than HTTP. Reuses Decision/Severity types.

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
    guardrails {
        pre  = [pii_redact, length_cap=8000, openai_moderation]
        post = [output_pii_redact, json_schema_validate]
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

- Virtual keys hashed (Argon2id), constant-time comparison.
- Provider keys at rest: AES-GCM with master key from env or KMS; with CMEK at GA per-tenant key wrap.
- Outbound TLS validates provider certs (no skip-verify); pin rustls roots.
- Bedrock SigV4 signing implemented per AWS spec; no use of long-term creds where IRSA / instance profile available (AWS SDK chain).
- Vertex / Google AI: OAuth2 with GCP service-account token caching (≤55 min TTL).
- Rate-limit-aware backoff respects provider's `Retry-After` exactly; no proxy-level retry on `400/422` (request errors must surface to client).
- DOS protection: max body size enforced **before** body parse; 100MB default for chat, 25MB for embeddings, 100MB for vision (configurable).
- All admin endpoints require key with `admin:*` scope; never callable from user-traffic key.

---

## 12. Open design questions (must resolve before MVP work starts)

1. **Inbound shape priority** — OpenAI-only MVP, or OpenAI+Anthropic? Recommended: **OpenAI-only MVP**; Anthropic-shape inbound in Beta. Rationale: OpenAI is the de facto SDK target; doubling the translator matrix (4×10 vs 1×10) doubles MVP scope.
2. **Tenant model** — `tenant→workspace→project→key`, or flat `key+tags`? Recommended: **flat keys + tags** for MVP; full hierarchy at Beta. Hierarchy mostly matters for billing rollups and admin UI.
3. **Vector index backend** — Qdrant / Redis-Stack / PgVector / in-process HNSW? Recommended: **Redis-Stack** (already a likely dep for distributed rate-limit) for MVP semantic cache; pluggable trait so Qdrant can be added later.
4. **Token-counter posture** — bake all vocabs (tiktoken + Llama BPE + SentencePiece — adds ~30MB to binary) vs. lazy fetch? Recommended: **bake** for fully-offline operation; advise feature flag.
5. **Reasoning-token billing default** — count as output (charged) or not? Recommended: **count as output** (matches provider pricing); per-key opt-out flag.
6. **Cancellation semantics** — on client SSE close, do we (a) cancel upstream immediately (saves $, may lose audit trail), (b) drain upstream silently and record full usage, or (c) configurable per-key? Recommended: **(c) configurable, default (a)**.
7. **Hard-stop on TPM enforcement** — break stream with error frame, or only post-stream warning? Recommended: **post-stream warning** by default; hard-stop opt-in (hard-stop semantics are jarring in practice).
8. **Prompt registry storage** — Postgres rows vs Git-backed text vs both? Recommended: **Postgres rows MVP**, expose `git push` adapter at GA for GitOps users.
9. **MCP placement** — gateway hosts MCP server (lets LLMs query gateway state), or proxy-only? Recommended: **proxy-only MVP**, in-process MCP server in Beta.
10. **Pricing source** — vendored snapshot, periodic refresh from LiteLLM JSON, or community feed? Recommended: **boot-load from vendored LiteLLM JSON snapshot, refresh weekly via signed download, admin override at runtime.**
11. **Realtime / Voice** — OpenAI Realtime + Gemini Live needed at MVP, Beta, or GA? Recommended: **GA**; voice apps are a smaller market and the WS reuse is non-trivial.
12. **License posture** — keep Apache-2 (matches the rest of highper-gateway), or BUSL/Commons-Clause for enterprise pieces (audit, BYOK, evals)? Owner decision; affects monetization story.
13. **Inference-engine integration (UC17)** — explicit non-goal here, but if owner wants to embed `mistral.rs` / `candle` within the same binary later, the AI router must remain provider-shaped (i.e., a self-hosted model is just another `provider`). Recommended: **defer to UC17** but design the router contract today so it doesn't bake in any "always-network" assumption.

---

## 13. Acceptance criteria — MVP

A user must be able to:

1. `cargo run` highper-gateway with config that registers OpenAI, Anthropic, Bedrock, Gemini provider blocks plus 5 model aliases.
2. `curl https://localhost/v1/chat/completions` with an `sk-hpgw-...` key get back a streaming OpenAI-shape response sourced from any of those four providers (selected by alias) — including for Anthropic / Bedrock / Gemini whose native shapes differ.
3. Set a $5/day budget on the key; verify gateway returns 402 when exceeded.
4. Hit the same chat completion twice; second request returns from exact cache with `x-cache: HIT`.
5. Configure a fallback list `[claude-3-7-sonnet, gpt-4o]`; simulate Anthropic 503; verify gateway transparently fails over to OpenAI and the client sees a single successful response.
6. Cancel a streaming request mid-stream; verify upstream is cancelled (TCP RST visible in metrics) and partial usage is logged.
7. Query `/admin/ai/spend?key=...&from=...&to=...` and see token/cost rollups.
8. Watch Prometheus metrics show `ai_input_tokens_total`, `ai_cost_usd_total`, `ai_cache_hit_ratio` updating.
9. Hot-reload the DSL to add a new model alias; new alias resolvable on next request without restart.
10. Log entries show redacted prompts (configurable) and full request_id linkage.

---

## 14. Out of MVP, in Beta+ scope

- Semantic cache.
- Pre + post guardrails (PII, moderation, jailbreak; PII regex MVP, ML-based Beta).
- Anthropic-shape inbound.
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

See `docs/PHASED_RELEASE_PLAN.md` for sequencing.

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
| Guardrail false positives blocking legitimate traffic | Medium | Audit log entry per block with reason; per-tenant whitelist override; "log only" mode default for new guardrails. |

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
