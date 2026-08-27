# Rift — Current State Summary

**Date:** 2026-08-27
**Fork:** OpenAI Codex → Rift (provider-agnostic CLI coding agent)

---

## What Rift Is

A provider-agnostic CLI coding agent forked from OpenAI Codex (`github.com/openai/codex`).
The goal is to support many AI providers and API protocols while preserving Codex's mature
agent loop, sandboxing, session handling, and terminal UI.

## What Has Been Done

### Phase 1: Provider Abstraction — COMPLETE

Added dual-protocol support at the transport layer. The agent core remains format-agnostic:

- **`WireApi::ChatCompletions`** variant added to `model-provider-info/src/lib.rs`
- **`ChatCompletionsClient<T>`** in `codex-api/src/endpoint/chat_completions.rs` — full request conversion from `ResponsesApiRequest` to Chat Completion format, plus streaming
- **SSE parser** in `chat_completions_sse.rs` — converts Chat Completion chunks to canonical `ResponseEvent`
- **3 unit tests** passing in `chat_completions_tests.rs`
- **`ModelClientSession::stream()`** updated — routes to `stream_chat_completions_api()` when `wire_api == ChatCompletions`

### Phase 2: Config System — SKIPPED

Already TOML-based. No `code.json` exists anywhere in the repo. No work needed.

### Phase 3: Built-in Providers — COMPLETE

27 providers registered in `model-provider-info/src/lib.rs`:
- OpenAI-compatible: opencode-zen, opencode-go, openrouter
- Major AI: anthropic, google-gemini
- Inference: groq, together, fireworks, mistral, deepseek, perplexity, xai, nvidia
- Developer: huggingface, cohere, ai21, replicate, anyscale, deepinfra, sambanova, novita
- Cloud: cloudflare, azure-openai, ibm-watsonx
- Chinese: zhipu, moonshot, minimax

Also: `BUILT_IN_PROVIDER_IDS` constant, `create_chat_completions_provider()` helper,
proto updated with `WIRE_API_CHAT_COMPLETIONS = 2`.

### Phase 4: CLI Provider Management — COMPLETE

Implemented:
- `codex provider list` — table + JSON, built-in vs custom
- `codex provider add` — interactive and non-interactive (all `--id`, `--name`, `--base-url`, `--protocol`, `--env-key`, `--bearer-token` flags)
- `codex provider remove` — removes custom providers from `~/.codex/config.toml`
- `codex provider test` — connectivity check, API key validation, masked key output
- `codex provider edit` — edit custom provider config
- `codex provider models` — list models from a provider's API
- `codex provider login` — set API key for a provider
- `codex provider status` — check connectivity to all configured providers at once (healthy/auth_failed/rate_limited/unreachable)

### Phase 5: Model Management — COMPLETE

Implemented:
- `codex model list` — list available models from bundled catalog, with `--provider`, `--json`, `--all` flags
- `codex model search <query>` — search models by slug, display name, or description
- `codex model use <model>` — set default model in config.toml, with optional `--effort` (low/medium/high)

### Phase 6: Model Switching Mid-Conversation — COMPLETE

Already implemented in TUI:
- `/model` slash command opens model+effort picker popup
- Alt+,/Alt+. keyboard shortcuts for reasoning effort stepping
- Rate limit auto-switch popups
- Full propagation: TUI → app-server → core session → context injection
- Model switch instructions injected into conversation via `<model_switch>` developer message

### Phase 7: Provider Limits & Rate Limiting — IN PROGRESS

Implemented:
- Reasoning effort now passed to ChatCompletions providers via `reasoning_effort` field (non-standard extension, ignored by providers that don't support it)
- Standard OpenAI-compatible rate limit headers parsed (`x-ratelimit-limit-requests`, `x-ratelimit-remaining-requests`, `x-ratelimit-reset-requests`, and token equivalents)
- Rate limit snapshots emitted for both Codex-specific and OpenAI-compatible providers
- **429 rate limit errors now retried at transport level** with exponential backoff (was previously immediate failure)
- **Retry-After header respected** when server provides it in 429 responses
- **`model_provider_fallback` config option** for specifying ordered fallback providers (config-level, ready for future automatic fallback)
- **`codex provider status`** — batch health check all configured providers, shows status/latency/API key presence

Not yet implemented:
- Automatic fallback to another provider when rate limited (requires session-level architecture changes)

CLI compiles: `cargo check -p codex-cli` passes. No runtime errors.

## File Map (Key Changes)

| File | Change |
|------|--------|
| `model-provider-info/src/lib.rs` | `WireApi::ChatCompletions`, 27 providers, `BUILT_IN_PROVIDER_IDS`, `retry_429: true` |
| `codex-api/src/endpoint/chat_completions.rs` | NEW — Chat Completions client, reasoning effort support |
| `codex-api/src/endpoint/chat_completions_sse.rs` | NEW — SSE parser |
| `codex-api/src/endpoint/chat_completions_tests.rs` | NEW — 3 tests |
| `codex-api/src/endpoint/mod.rs` | Module registration |
| `codex-api/src/rate_limits.rs` | OpenAI-compatible rate limit header parsing |
| `codex-client/src/retry.rs` | 429 retry support, Retry-After header parsing |
| `config/src/config_toml.rs` | `BUILT_IN_PROVIDER_IDS`, `model_provider_fallback` field |
| `core/src/config/mod.rs` | `model_provider_fallback` in Config struct |
| `core/src/client.rs` | `stream_chat_completions_api()`, match arm |
| `config/src/thread_config/proto/codex.thread_config.v1.proto` | `ChatCompletions = 2` |
| `config/src/thread_config/proto/codex.thread_config.v1.rs` | Generated Rust proto code |
| `config/src/thread_config/remote.rs` | ChatCompletions in both proto directions |
| `cli/src/provider_cmd.rs` | CLI provider commands (+ status) |
| `cli/src/model_cmd.rs` | CLI model commands (list, search, use) |
| `cli/src/main.rs` | `Provider` + `Model` subcommands, dispatch |

## What Comes Next

1. **Phase 7 completion:** Automatic fallback to another provider when rate limited, provider usage tracking
2. **Phase 8:** Free model discovery and routing
3. **Phases 9-17:** Context efficiency, resource efficiency, HTTP, transactional changes, safety, UX, profiles, local models

## Test Results (at time of last check)

- `cargo check -p codex-cli` — PASS (only warnings from codex-api Phase 1 dead code)
- `cargo check -p codex-core` — PASS
- `cargo check -p codex-api` — PASS (4 warnings: unused import, 3 dead fields)
- `codex-model-provider-info` tests — 27/27 PASS
- Total workspace tests — 308/308 PASS
