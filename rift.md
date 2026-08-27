Project: Build a Next-Generation Lightweight Coding Agent From OpenAI Codex

Start from the official OpenAI Codex repository:

https://github.com/openai/codex.git

Do NOT create a new coding agent from scratch.

Fork/clone the existing Codex codebase and evolve it into a new, independently developed CLI coding agent.

The existing Codex architecture, agent loop, terminal UI, sandboxing, tools, session handling, configuration system, and other mature functionality should be preserved wherever they are already good.

The goal is to improve and extend Codex, not unnecessarily rewrite working infrastructure.

The project must remain lightweight, native, fast, and suitable for constrained environments.

---

PRIMARY OBJECTIVE

Create a highly efficient, provider-agnostic CLI coding agent that can:

- use many AI providers
- use different API protocols
- add/configure providers directly from the CLI
- discover models
- discover free/open-source/open-weight models
- switch models during a conversation
- automatically recover from provider limits when configured
- run with an extremely small resource footprint
- work on Linux, macOS, Windows, ARM64, Termux and constrained environments
- optionally expose the agent through HTTP
- remain primarily a CLI application

The CLI must remain the primary interface.

HTTP is secondary.

---

IMPORTANT: PRESERVE CODEX'S STRENGTHS

Before modifying anything:

1. Study the entire repository.
2. Understand the Rust workspace.
3. Understand the agent loop.
4. Understand model/provider handling.
5. Understand tool execution.
6. Understand session/conversation handling.
7. Understand configuration.
8. Understand sandboxing.
9. Understand the terminal UI.
10. Understand existing performance characteristics.

Do not rewrite working systems merely to make the architecture look cleaner.

Prefer incremental architectural improvements.

Create an architectural map before making substantial changes.

---

PHASE 1 — PROVIDER ABSTRACTION

The first major architectural improvement should be the provider layer.

The agent core must not care whether the model comes from:

- OpenAI
- OpenCode
- Anthropic
- Google
- a local model
- an OpenAI-compatible API
- another provider

Create a clean provider abstraction.

A provider should be able to declare:

provider ID
display name
base URL
authentication
protocol
models
model metadata
capabilities
limits

Support multiple protocols.

At minimum architect for:

OpenAI Responses
OpenAI Chat Completions
Anthropic Messages
Gemini
OpenAI-compatible APIs

Do not force every provider through one protocol.

The agent should adapt to the provider's native protocol.

---

PHASE 2 — OPENAI COMPATIBLE PROVIDERS

Make OpenAI-compatible APIs first-class.

A provider should be able to specify:

protocol = "chat_completions"
base_url = "https://example.com/v1"
api_key = "..."

or:

protocol = "responses"
base_url = "https://example.com/v1"

The protocol must be provider configuration, not a hardcoded assumption.

This is especially important because different providers expose different APIs.

---

PHASE 3 — OPEN CODE ZEN

Use OpenCode Zen as the first real test of the new provider architecture.

Zen currently exposes models through its OpenAI-compatible Chat Completions API.

Example endpoint:

https://opencode.ai/zen/v1/chat/completions

The implementation must not hardcode Zen into the agent core.

Instead configure it as a normal provider.

Models such as:

hy3-free
big-pickle
x-preview-f-free
nemotron-3-ultra-free
nemotron-3.5-lightning-free

should be represented as provider models.

Do not assume model metadata.

Discover it when possible and gracefully fall back when metadata is unavailable.

---

PHASE 4 — CLI PROVIDER MANAGEMENT

Users must be able to configure providers without manually editing TOML.

Implement commands such as:

agent provider list
agent provider add
agent provider remove
agent provider edit
agent provider test
agent provider models
agent provider login

Interactive example:

$ agent provider add

Provider name: OpenCode Zen
Base URL: https://opencode.ai/zen/v1
Protocol: Chat Completions
API key: ********

Provider added successfully.

Also support non-interactive configuration:

agent provider add \
  --name opencode \
  --base-url https://opencode.ai/zen/v1 \
  --protocol chat_completions

Never print secrets.

Never store API keys in plaintext unnecessarily.

Support environment variables.

---

PHASE 5 — MODEL MANAGEMENT

Implement:

agent models
agent models search
agent models search free
agent models search open-source
agent models search coding
agent model use <model>

Models should be associated with providers.

Example:

OpenCode Zen

hy3-free
big-pickle
x-preview-f-free
nemotron-3-ultra-free

Display:

Model
Provider
Protocol
Context window
Capabilities
Availability
Free tier
License/open-weight status

Do not incorrectly label models as open source.

Distinguish:

Open Source
Open Weight
Free
Free Tier
Local
Paid

---

PHASE 6 — MODEL SWITCHING DURING CONVERSATIONS

This is mandatory.

Inside an active conversation:

/model

should show available models.

Allow:

/model hy3-free
/model big-pickle
/model nemotron-3-ultra-free

Switching models must NOT destroy the conversation.

The agent should intelligently adapt context when the new model has a smaller context window.

Example:

Current model:
hy3-free

Switch to:
big-pickle

Conversation preserved.

---

PHASE 7 — PROVIDER LIMITS AND FALLBACKS

Providers can return:

429
quota exceeded
rate limited
billing limit
model unavailable
temporary outage

Build provider health tracking.

Users should be able to configure fallback models:

[fallback]
enabled = true

models = [
  "hy3-free",
  "big-pickle",
  "nemotron-3-ultra-free"
]

If fallback is enabled:

Current provider
      ↓
limit reached
      ↓
next configured model
      ↓
continue conversation

Never silently change models unless the user explicitly enables automatic fallback.

---

PHASE 8 — FREE MODEL DISCOVERY

When a configured provider reaches its limits, make it possible to discover alternatives.

Example:

⚠ Provider limit reached.

Free alternatives:

1. hy3-free
2. big-pickle
3. Nemotron 3 Ultra
4. Local Ollama models

Search for more? [y/N]

The discovery mechanism should eventually support multiple model registries/providers.

Keep the discovery system lightweight.

Do not download model weights automatically.

---

PHASE 9 — MODEL ROUTING

Eventually support intelligent model routing.

Example:

[routing]
enabled = true

[routing.tasks]
planning = "strong"
coding = "strong"
search = "fast"
summarization = "cheap"

Allow aliases:

fast
cheap
strong
reasoning
local

Example:

fast      → hy3-free
strong    → big-pickle
reasoning → nemotron-3-ultra-free
local     → Ollama model

Routing must be optional.

A user should be able to force one model for the entire session.

---

PHASE 10 — CONTEXT EFFICIENCY

Do not send the entire repository to the model.

Improve Codex's context management so the agent intelligently discovers relevant files.

Build toward:

repository
    ↓
file discovery
    ↓
symbol discovery
    ↓
dependency relationships
    ↓
relevance ranking
    ↓
context compilation
    ↓
model

Prioritize low memory usage.

Do not introduce a heavyweight vector database simply because it is fashionable.

Prefer:

- filesystem indexes
- compact metadata
- incremental caches
- AST/symbol indexes
- native search
- Git information

---

PHASE 11 — RESOURCE EFFICIENCY

This is one of the most important goals.

The resulting agent should be able to run comfortably on:

- low-end laptops
- ARM64 devices
- Termux
- VPS machines
- containers
- CI runners
- Raspberry Pi-class systems

Target:

Very fast startup
Low idle RAM
Low active RAM
Low CPU when idle
No unnecessary background processes
Predictable memory usage

Do not introduce:

- Node runtime
- Bun runtime
- embedded browser
- heavyweight local database
- mandatory daemon
- unnecessary indexing service

unless there is a compelling measured reason.

Prefer native Rust implementations.

Use lazy initialization.

Use bounded caches.

Stream data instead of buffering unnecessarily.

Measure before optimizing.

---

PHASE 12 — HTTP INTERFACE

The CLI remains primary.

Add an optional HTTP interface:

agent serve

The HTTP server must:

- be disabled by default
- consume no resources when disabled
- bind localhost by default
- support authentication
- expose sessions
- expose model/provider information
- accept prompts
- stream events
- allow model switching

Architecture:

CLI ──────┐
          │
HTTP ─────┼──→ Agent Core
          │
Future UI ┘

The agent core must not depend on HTTP.

---

PHASE 13 — TRANSACTIONAL CHANGES

Make agent modifications recoverable.

Before significant changes:

snapshot
   ↓
agent modifies repository
   ↓
tests
   ↓
verification

Support:

agent undo

Prefer Git-based mechanisms where appropriate.

Do not create a second unnecessarily complicated version-control system.

---

PHASE 14 — SAFETY

Commands should have risk levels.

Safe:

git status
ls
cat
grep
cargo check

Potentially destructive:

rm
git reset
git clean
database destructive operations

Destructive commands should request confirmation unless explicitly allowed.

Do not annoy users with confirmation prompts for every shell command.

---

PHASE 15 — TERMINAL UX

Keep the terminal UI minimal.

Avoid unnecessary animations.

Show useful state:

agent · big-pickle

▸ Analyzing

  4 relevant files

▸ Editing

  ✓ auth.ts
  ✓ middleware.ts

▸ Testing

  ✓ 27 passed

✓ Completed in 18.4s

Support:

agent --plain
agent --json

for terminals and automation.

---

PHASE 16 — PROFILES

Support:

agent dev
agent reviewer
agent debugger
agent architect
agent tester

Profiles can specify:

model
provider
tools
permissions
routing
fallback

Example:

[profiles.reviewer]
model = "big-pickle"
allow_write = false
tools = ["read", "search", "git"]

---

PHASE 17 — LOCAL MODELS

Make local inference first-class.

Architect for:

Ollama
llama.cpp
local OpenAI-compatible servers

The agent should not care whether a model is remote or local.

The provider abstraction should make both look like providers.

---

ENGINEERING PRINCIPLES

1. Do not rewrite Codex unnecessarily.

Preserve mature components.

2. Do not hardcode OpenCode.

Zen should be one provider among many.

3. Do not hardcode models.

Model metadata should be dynamic.

4. Do not assume API protocols.

Providers explicitly declare their protocol.

5. Do not silently change models.

Fallback requires user configuration.

6. Do not sacrifice performance for abstraction.

Provider abstraction should have minimal runtime overhead.

7. Do not add heavyweight dependencies casually.

Every dependency should justify its resource cost.

8. Measure everything important.

Benchmark:

startup time
idle RSS
active RSS
CPU
binary size
disk usage

Compare against the unmodified Codex baseline.

9. Keep the CLI primary.

HTTP is an adapter, not the architecture.

10. Keep the codebase maintainable.

Prefer simple Rust abstractions over complex frameworks.

---

FIRST TASK

Do NOT start implementing every feature above immediately.

First:

1. Clone the official Codex repository.
2. Inspect the repository structure.
3. Inspect "codex-rs".
4. Identify the current provider/model architecture.
5. Identify where "wire_api" is implemented.
6. Identify why Chat Completions support was removed/disabled.
7. Identify the cleanest extension point for multiple protocols.
8. Identify the current configuration architecture.
9. Identify how sessions/conversations are represented.
10. Identify the current model metadata system.
11. Identify existing tests around model providers.
12. Identify resource-heavy components.
13. Establish baseline startup/RAM/CPU measurements.
14. Produce a concise architecture report.
15. Produce a phased implementation plan.

Do not modify major architecture before completing this investigation.

Then implement Phase 1 only: provider/protocol abstraction.

The first concrete success criterion is:

Codex-derived agent
      ↓
Provider abstraction
      ↓
OpenAI Responses provider
      +
OpenAI Chat Completions provider

Once that works cleanly, add OpenCode Zen as the first external provider.

The goal is not to make a clone of OpenCode.

The goal is to evolve Codex into a small, extremely efficient, provider-agnostic coding agent while preserving the excellent work already present in the Codex codebase.

its called Rift
