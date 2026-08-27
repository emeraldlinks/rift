# Rift - TODO

> Provider-agnostic CLI coding agent forked from OpenAI Codex
> Repo: github.com/anomalyco/rift (name: rift, path: /root/rift/codex)

---

## Phase 1 - Provider Abstraction ✅ COMPLETE

- [x] Add `WireApi::ChatCompletions` variant to `model-provider-info/src/lib.rs`
- [x] Create `codex-api/src/endpoint/chat_completions.rs` — full `ChatCompletionsClient<T>`
- [x] Create `codex-api/src/endpoint/chat_completions_sse.rs` — SSE event parser
- [x] Create `codex-api/src/endpoint/chat_completions_tests.rs` — 3 unit tests
- [x] Update `core/src/client.rs` — `stream_chat_completions_api()`, match arm in `ModelClientSession::stream()`
- [x] WireApi serde changed from `rename_all = "lowercase"` to `rename_all = "snake_case"`
- [x] `FunctionCallOutputPayload` uses `Display` impl; `call_id` is `Option<String>`
- [x] `TokenUsage` fields are `i64`; `SseTelemetry` trait only has `on_sse_poll`
- [x] `StreamResponse` has `.bytes` field; `eventsource_stream::Event.event` is `String`
- [x] `ChatMessage.tool_call_id` is `Option<String>`; `ContentItem` used in `ResponseItem::Message`
- [x] Design decision: convert at transport boundary so agent core is format-agnostic

## Phase 2 - Config System ⏭️ SKIPPED

Already TOML-based (`config.toml`, `defaults.toml`, `profile.toml`, `permissions.toml`, `hooks.toml`). No `code.json` exists. No work needed.

## Phase 3 - Built-in Providers ✅ COMPLETE

- [x] Add 27 built-in providers to `model-provider-info/src/lib.rs`
  - OpenAI-compatible: opencode-zen, opencode-go, openrouter
  - Major AI: anthropic, google-gemini
  - Inference: groq, together, fireworks, mistral, deepseek, perplexity, xai, nvidia
  - Developer: huggingface, cohere, ai21, replicate, anyscale, deepinfra, sambanova, novita
  - Cloud: cloudflare, azure-openai, ibm-watsonx
  - Chinese: zhipu, moonshot, minimax
- [x] Add `create_chat_completions_provider()` helper
- [x] Add `BUILT_IN_PROVIDER_IDS` constant
- [x] Update `config_toml.rs` — replace `RESERVED_MODEL_PROVIDER_IDS` with `BUILT_IN_PROVIDER_IDS`
- [x] Update proto file `codex.thread_config.v1.proto` — add `WIRE_API_CHAT_COMPLETIONS = 2`
- [x] Update generated proto Rust code and `remote.rs` — handle ChatCompletions in both directions

## Phase 4 - CLI Provider Management 🔄 IN PROGRESS

- [x] `codex provider list` — list built-in + user-configured providers (table + JSON)
- [x] `codex provider add` — interactive and non-interactive modes
- [x] `codex provider remove` — remove custom providers from config.toml
- [x] `codex provider test` — connectivity check with API key validation
- [ ] `codex provider edit` — edit existing custom provider
- [ ] `codex provider models` — list models for a provider
- [ ] `codex provider login` — authenticate with a provider
- [x] Never print secrets (API key masked in test output)
- [x] Support environment variables for API keys (env_key field)
- [x] CLI compiles: `cargo check -p codex-cli` passes
- [x] Wired into `Subcommand` enum, dispatch, profile_v2, subcommand_name

## Phase 5 - Model Management ⬜ NOT STARTED

- [ ] `agent models` - list all models
- [ ] `agent models search`
- [ ] `agent models search free`
- [ ] `agent models search open-source`
- [ ] `agent models search coding`
- [ ] `agent model use <model>`
- [ ] Display: Model, Provider, Protocol, Context window, Capabilities, Availability, Free tier, License

## Phase 6 - Model Switching ⬜ NOT STARTED

- [ ] `/model` command shows available models
- [ ] `/model <name>` switches model mid-conversation
- [ ] Preserve conversation on switch
- [ ] Adapt context for smaller context windows

## Phase 7 - Provider Limits and Fallbacks ⬜ NOT STARTED

- [ ] Provider health tracking
- [ ] Detect: 429, quota exceeded, rate limited, billing limit, model unavailable
- [ ] Fallback configuration in TOML
- [ ] Automatic fallback (opt-in only)
- [ ] User notification on model change

## Phase 8 - Free Model Discovery ⬜ NOT STARTED

- [ ] On rate limit, suggest free alternatives
- [ ] Search across configured providers
- [ ] Lightweight discovery mechanism
- [ ] No automatic weight downloads

## Phase 9 - Model Routing ⬜ NOT STARTED

- [ ] Task-based routing config (planning, coding, search, summarization)
- [ ] Alias system: fast, cheap, strong, reasoning, local
- [ ] Optional - user can force single model

## Phase 10 - Context Efficiency ⬜ NOT STARTED

- [ ] File discovery (not full repo dump)
- [ ] Symbol/AST discovery
- [ ] Dependency relationship tracking
- [ ] Relevance ranking
- [ ] Filesystem indexes
- [ ] Compact metadata
- [ ] Incremental caches
- [ ] No heavyweight vector DB

## Phase 11 - Resource Efficiency ⬜ NOT STARTED

- [ ] Fast startup, Low idle/active RAM, Low CPU when idle
- [ ] Test on ARM64/Termux
- [ ] Lazy initialization, Bounded caches, Stream data

## Phase 12 - HTTP Interface ⬜ NOT STARTED

- [ ] `agent serve` command, localhost by default, auth support

## Phase 13 - Transactional Changes ⬜ NOT STARTED

- [ ] Snapshot before changes, test after, `agent undo`

## Phase 14 - Safety ⬜ NOT STARTED

- [ ] Command risk levels, confirmation for destructive ops

## Phase 15 - Terminal UX ⬜ NOT STARTED

- [ ] Minimal animations, `--plain`, `--json` modes

## Phase 16 - Profiles ⬜ NOT STARTED

- [ ] `agent dev`, `agent reviewer`, `agent debugger`, `agent architect`, `agent tester`

## Phase 17 - Local Models ⬜ NOT STARTED

- [ ] Ollama, llama.cpp, local OpenAI-compatible server support

---

## Infrastructure Notes

- **Toolchain:** Rust 1.95.0 (via `codex-rs/rust-toolchain.toml`)
- **Tests:** Use `just test -p <crate>` (NOT `cargo test` directly)
- **Linter:** Use `just fix -p <crate>` for Clippy
- **Formatter:** Use `just fmt` after code changes
- **Network:** This environment is sandboxed; `CODEX_SANDBOX_NETWORK_DISABLED=1` is set during shell tool use
- **`cargo nextest`** is NOT installed — use `cargo test`
