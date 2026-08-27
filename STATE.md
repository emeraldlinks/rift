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

### Phase 4: CLI Provider Management — IN PROGRESS

Implemented:
- `codex provider list` — table + JSON, built-in vs custom
- `codex provider add` — interactive and non-interactive (all `--id`, `--name`, `--base-url`, `--protocol`, `--env-key`, `--bearer-token` flags)
- `codex provider remove` — removes custom providers from `~/.codex/config.toml`
- `codex provider test` — connectivity check, API key validation, masked key output

Not yet implemented:
- `codex provider edit`
- `codex provider models`
- `codex provider login`

CLI compiles: `cargo check -p codex-cli` passes. No runtime errors.

## File Map (Key Changes)

| File | Change |
|------|--------|
| `model-provider-info/src/lib.rs` | `WireApi::ChatCompletions`, 27 providers, `BUILT_IN_PROVIDER_IDS` |
| `codex-api/src/endpoint/chat_completions.rs` | NEW — Chat Completions client |
| `codex-api/src/endpoint/chat_completions_sse.rs` | NEW — SSE parser |
| `codex-api/src/endpoint/chat_completions_tests.rs` | NEW — 3 tests |
| `codex-api/src/endpoint/mod.rs` | Module registration |
| `core/src/client.rs` | `stream_chat_completions_api()`, match arm |
| `config/src/config_toml.rs` | `BUILT_IN_PROVIDER_IDS` replaces old constant |
| `config/src/thread_config/proto/codex.thread_config.v1.proto` | `ChatCompletions = 2` |
| `config/src/thread_config/proto/codex.thread_config.v1.rs` | Generated Rust proto code |
| `config/src/thread_config/remote.rs` | ChatCompletions in both proto directions |
| `cli/src/provider_cmd.rs` | NEW — CLI provider commands |
| `cli/src/main.rs` | `Provider` subcommand, dispatch, profile_v2 |

## What Comes Next

1. **Phase 4 completion:** `provider edit`, `provider models`, `provider login`
2. **Phase 5:** Model management (`models`, `models search`, `model use`)
3. **Phase 6:** Model switching mid-conversation
4. **Phase 7:** Provider limits, rate limiting, automatic fallback
5. **Phases 8-17:** Free model discovery, routing, context efficiency, resource efficiency, HTTP, transactional changes, safety, UX, profiles, local models

## Test Results (at time of last check)

- `cargo check -p codex-cli` — PASS (only warnings from codex-api Phase 1 dead code)
- `cargo check -p codex-core` — PASS
- `cargo check -p codex-api` — PASS (4 warnings: unused import, 3 dead fields)
- `codex-model-provider-info` tests — 27/27 PASS
- Total workspace tests — 308/308 PASS
