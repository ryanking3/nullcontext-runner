# NullContext

NullContext is a local-first secure inference environment for running LLM sessions with explicit lifecycle visibility, audit reporting, configurable persistence behavior, and local browser-based runtime inspection.

The project currently targets local inference workflows using:

- Rust
- llama.cpp
- Axum
- React
- local GGUF models
- CUDA acceleration (Windows)
- browser-based localhost UI

NullContext is designed around the idea that local inference systems should expose:

- what was stored
- what was retained
- what was deleted
- what cleanup operations occurred
- what residual risks remain

rather than treating local inference as an opaque black box.

---

## Current Architecture

```text
Browser UI
    ↓
Local Axum API server
    ↓
NullContext runtime
    ↓
llama.cpp
    ↓
Local GGUF model
```

The entire stack runs locally.

No cloud inference is required.

---

## Current Features

### Local Inference Runtime

- llama.cpp backend integration
- local GGUF model support
- stdin-based prompt ingestion
- one-shot streaming inference
- one-shot corpus-grounded retrieval
- active chat sessions with runtime reuse
- active-chat corpus-grounded retrieval
- configurable inference modes
- persistent and ephemeral sessions
- configurable token limits
- configurable GPU offload
- Windows CUDA support
- local HTTP API server

### Security / Privacy Features

- explicit workspace lifecycle management
- recursive artifact scanning
- Rust-owned buffer zeroization
- RAM zeroization verification
- audit operation tracking
- sanitization operation reporting
- structured privacy reports
- configurable retention behavior
- manual cleanup and reconcile actions for retained sessions
- scheduled retention expiry cleanup
- startup lifecycle reconciliation for orphaned sessions/workspaces
- lifecycle-aware privacy reporting
- corpus lifecycle cleanup and reconcile actions
- corpus retention policy controls
- corpus report syncing after lifecycle changes
- explicit End + Sanitize workflow for active chat
- residual risk reporting for long-lived runtimes

### Session Registry

Persistent sessions are indexed locally:

```text
~/.nullcontext/index.json
```

The registry tracks:

- session IDs
- timestamps
- security mode
- selected model IDs and names
- workspace paths
- report paths
- cleanup state
- lifecycle state
- retention policies and deadlines
- artifact counts

### Model Registry

The local model registry supports:

- default model selection
- named model IDs
- per-model token, GPU, template, and context defaults
- model switching in the browser UI and API
- model file validation
- llama-server runtime readiness reporting

### Corpus Registry

The local corpus registry supports:

- txt, markdown, and pdf ingestion
- hybrid pdf extraction with OCR for sparse pages
- persistent and ephemeral corpora
- local chunking and embedding artifacts
- direct corpus querying through the API
- one-shot and active-chat grounding
- corpus lifecycle cleanup, reconcile, and retention controls
- startup lifecycle reconciliation for orphaned corpora
- retained ingestion reports with lifecycle metadata

### Local Web UI

The current browser UI supports:

- one-shot prompt execution
- one-shot corpus selection for grounded runs
- active chat session start, stream, stop, and end
- active chat corpus binding for grounded sessions
- dedicated model registry browser
- dedicated corpus registry browser
- path-based corpus ingestion
- model selection for one-shot and active chat
- model-default versus manual-override controls
- selectable active chat prompt template
- configurable active chat context token budget and turn limit
- runtime lifecycle visualization
- audit operation inspection
- privacy report inspection
- runtime log inspection
- persistent session browsing
- dark/light terminal-style UI
- before-unload warning while active chat runtime is live
- local-only API interaction
- localhost-only execution


---

## Security Modes

### secure

Default mode.

Characteristics:

- ephemeral workspace
- automatic cleanup
- audit reporting
- artifact scanning
- buffer sanitization
- stdin prompt ingestion recommended

### standard

Allows persistent sessions.

Characteristics:

- retained workspace
- retained reports
- session registry indexing

### air-gapped

Reserved for stricter future runtime policies.

Currently behaves similarly to secure mode.

---

## Runtime Lifecycle

A typical session lifecycle:

```text
1. Prompt ingestion
2. Runtime launch
3. Local inference
4. Artifact scan
5. Audit operation emission
6. Buffer sanitization
7. Workspace cleanup or retention
8. Privacy report generation
9. Session indexing (persistent only)
```

### One-shot mode

One prompt creates a full lifecycle:

```text
create session
→ launch llama-server
→ stream completion
→ shutdown runtime
→ scan artifacts
→ sanitize Rust-owned buffers
→ cleanup or retain workspace
→ emit privacy report
```

### Active chat mode

A chat session creates a long-lived runtime:

```text
start active session
→ launch llama-server once
→ send multiple messages through same runtime
→ keep chat context in memory until session end
→ end session explicitly
→ shutdown runtime
→ zeroize Rust-owned chat history
→ scan artifacts
→ cleanup or retain workspace
→ emit privacy report
```

Active chat uses:

- model-aware prompt templates
- bounded recent-context management
- audit visibility when older turns are dropped from the prompt window
- optional bound corpus retrieval on every turn

---

## Current API

The local API server currently exposes:

### Health

```http
GET /api/health
```

### Run Session

```http
POST /api/run
```

Runs a non-streaming one-shot session and returns collected stdout/stderr.

### Stream Run Session

```http
POST /api/run/stream
```

Runs a streaming one-shot session and emits SSE-style `data:` JSON payloads.

Example body:

```json
{
  "prompt": "Explain secure local inference.",
  "mode": "secure",
  "persistent": false,
  "model_id": "qwen-small",
  "corpus_id": "incident-briefing",
  "chat_template": "auto",
  "chat_context_token_budget": 2048,
  "chat_context_turn_limit": 12
}
```

When `corpus_id` is present, `/api/run/stream` retrieves local corpus context first and injects a grounded prompt wrapper before inference.

### Corpus Registry

```http
GET /api/corpora
POST /api/corpora
GET /api/corpora/:corpus_id/report
POST /api/corpora/:corpus_id/query
POST /api/corpora/:corpus_id/retention
POST /api/corpora/:corpus_id/cleanup
POST /api/corpora/:corpus_id/reconcile
```

Example ingest body:

```json
{
  "name": "incident-briefing",
  "paths": [
    "/Users/you/docs/briefing.pdf",
    "/Users/you/docs/notes"
  ],
  "persistent": true,
  "ocr_enabled": true
}
```

### Model Registry

```http
GET /api/models
```

### Start Active Chat Session

```http
POST /api/chat/start
```

Example body:

```json
{
  "mode": "secure",
  "persistent": false,
  "model_id": "qwen-small",
  "corpus_id": "incident-briefing",
  "chat_template": "auto",
  "chat_context_token_budget": 2048,
  "chat_context_turn_limit": 12
}
```

### Active Chat Status

```http
GET /api/chat/:session_id/status
```

### Stream Active Chat Message

```http
POST /api/chat/:session_id/message/stream
```

Example body:

```json
{
  "prompt": "Explain secure local inference in 2 short bullet points."
}
```

### Active Chat Template And Context Fields

The one-shot and active chat APIs support these optional fields:

- `model_id`
Selects a registered model by ID
- `corpus_id`
Binds a registered local corpus by ID for grounded retrieval
- `chat_template`
Values: `auto`, `generic`, `chatml`, `llama3-instruct`
- `chat_context_token_budget`
Approximate token budget for recent active-chat context selection
- `chat_context_turn_limit`
Maximum number of recent prior turns to include in active-chat context

When `chat_template` is `auto`, NullContext resolves a template from the selected model path.
If the UI is using model defaults, it omits these override fields and lets the selected model drive the effective template and context settings.
If `corpus_id` is provided when starting active chat, NullContext binds that corpus for retrieval on every subsequent turn until the session ends.

### End Active Chat Session

```http
POST /api/chat/:session_id/end
```

### Cancel Active Chat Generation

```http
POST /api/chat/:session_id/cancel
```

### List Sessions

```http
GET /api/sessions
```

### Update Session Retention Policy

```http
POST /api/sessions/:session_id/retention
```

### Cleanup Retained Session

```http
POST /api/sessions/:session_id/cleanup
```

### Reconcile Session Lifecycle State

```http
POST /api/sessions/:session_id/reconcile
```

### Show Report

```http
GET /api/reports/:session_id
```

### Streaming Event Types

Streaming endpoints emit SSE-style `data:` blocks containing JSON events. Current event types include:

- `runtime`
- `audit`
- `model`
- `report`
- `stderr`
- `error`
- `complete`

---

## Current Limitations

NullContext does not currently guarantee:

- VRAM sanitization
- llama.cpp internal allocator sanitization
- OS swap sanitization
- shell history sanitization
- cross-process memory sanitization
- CUDA memory sanitization
- forensic memory clearing outside Rust-owned buffers
- perfect PDF layout reconstruction
- OCR accuracy for every scanned or image-only PDF

Active chat also keeps a long-lived llama.cpp runtime and in-memory context alive until the user explicitly ends the session.
Corpus ingestion can recover text from many PDFs, including scanned pages via OCR, but complex layouts, tables, and poor scans may still extract imperfectly.

The privacy reports intentionally expose these residual risks.

---

## Development Setup

### Requirements

### Windows

- Rust
- Node.js
- pnpm
- Visual Studio Build Tools
- CUDA Toolkit
- llama.cpp
- local GGUF model

### macOS

- Rust
- Node.js
- pnpm
- Xcode Command Line Tools
- llama.cpp
- local GGUF model

---

## llama.cpp Setup

Clone:

```bash
git clone https://github.com/ggml-org/llama.cpp
```

### Windows CUDA Build

From:

```text
x64 Native Tools Command Prompt for VS
```

Run:

```bash
cmake -B build -DGGML_CUDA=ON
cmake --build build --config Release
```

Expected binaries:

```text
build/bin/Release/llama-server.exe
build/bin/Release/llama-cli.exe
```

### macOS Build

```bash
cmake -B build
cmake --build build --config Release
```

---

## Configuration

Configuration file:

```text
~/.nullcontext/config.toml
```

Example model-registry config:

```toml
llama_path = "C:\\dev\\llama.cpp\\build\\bin\\Release\\llama-server.exe"
default_model = "qwen-small"
default_mode = "secure"
max_tokens = 128
gpu_layers = 999
chat_template = "auto"
chat_context_token_budget = 2048
chat_context_turn_limit = 12

[[models]]
id = "qwen-small"
name = "Qwen 2.5 0.5B Instruct"
model_path = "C:\\models\\qwen2.5-0.5b\\qwen2.5-0.5b-instruct-q4_k_m.gguf"
max_tokens = 128
gpu_layers = 999
chat_template = "chatml"
chat_context_token_budget = 2048
chat_context_turn_limit = 12

[[models]]
id = "llama3-8b"
name = "Llama 3 8B Instruct"
model_path = "C:\\models\\llama3-8b\\meta-llama-3-8b-instruct-q4_k_m.gguf"
max_tokens = 256
gpu_layers = 999
chat_template = "llama3-instruct"
chat_context_token_budget = 3072
chat_context_turn_limit = 16
```

### Notes

```toml
gpu_layers = 999
```

means:

```text
offload as many layers as possible onto the GPU
```

Additional active chat options:

```toml
chat_template = "auto"
chat_context_token_budget = 2048
chat_context_turn_limit = 12
```

Template options:

- `auto`
- `generic`
- `chatml`
- `llama3-instruct`

`chat_context_token_budget` and `chat_context_turn_limit` must both be greater than `0`.

Legacy single-model configs using only `model_path` are still supported. When no `[[models]]` array is present, NullContext synthesizes a default model entry automatically.

### Workspace Paths

NullContext session workspaces are created under the system temporary directory, in a `nullcontext` subdirectory.

Typical examples:

```text
macOS/Linux: $TMPDIR/nullcontext or /tmp/nullcontext
Windows: %TEMP%\nullcontext
```

The exact path is determined at runtime using Rust's `std::env::temp_dir()`.

---

## Backend Runtime

Build the Rust runtime:

```bash
cargo build
```

Run directly:

```bash
echo "Explain secure local inference." | cargo run -- --stdin
```

Persistent session example:

```bash
echo "Explain persistent audit trails." | cargo run -- --mode standard --persistent --stdin
```

---

## Local API Server

Start the local API server:

```bash
cargo run -- serve
```

Default address:

```text
http://127.0.0.1:3333
```

Health check:

```text
http://127.0.0.1:3333/api/health
```

Streaming one-shot example:

```bash
curl -N -X POST http://127.0.0.1:3333/api/run/stream \
  -H "Content-Type: application/json" \
  -d '{"prompt":"Explain secure local inference in 2 short bullet points.","mode":"secure","persistent":false}'
```

Active chat example:

```bash
curl -X POST http://127.0.0.1:3333/api/chat/start \
  -H "Content-Type: application/json" \
  -d '{"mode":"secure","persistent":false,"model_id":"qwen-small","corpus_id":"incident-briefing"}'
```

---

## Web UI

From:

```bash
apps/web
```

Install dependencies:

```bash
pnpm install
```

Run development server:

```bash
pnpm dev
```

Default UI address:

```text
http://localhost:5173
```

The active chat session config panel lets you:

- browse the registered model catalog
- browse the registered corpus catalog
- pick a model by ID/name before starting a session
- select a local corpus for grounded one-shot runs or for the next active chat session
- use per-model defaults or manual overrides for template/context settings
- choose a prompt template or auto-detect it from the model path
- set a bounded recent-context token budget
- set a bounded recent-context turn limit

The corpus browser also lets you:

- ingest corpora from absolute local file and directory paths
- inspect corpus lifecycle state and retained artifact paths
- load retained corpus reports
- run corpus reconcile, cleanup, and retention actions

The model browser also shows:

- whether each model file path is launchable
- whether the configured `llama-server` path is ready
- the exact model path, template default, token limit, GPU setting, and context defaults

After a session starts, the runtime banner shows the selected model, any bound corpus, the resolved template, and the active context policy.

---

## Session Commands

List persistent sessions:

```bash
cargo run -- --list-sessions
```

Show report:

```bash
cargo run -- --show-report <session-id>
```

---

## Current Development Focus

The current development focus is:

- structured runtime streaming
- Server-Sent Events
- local corpus ingestion and retrieval lifecycle management
- streaming token output
- streaming audit events
- stronger memory hygiene primitives
- VRAM inspection and analysis
- forensic artifact visibility
- Linux-native low-level memory work

---

## Project Status

NullContext is currently in active early-stage development.

The project is functional and supports:

- local inference
- local browser UI
- local API execution
- one-shot streaming
- one-shot grounded retrieval
- active chat sessions
- active-chat grounded retrieval
- generation stop control
- explicit active chat cancellation
- persistent sessions
- lifecycle policy engine
- structured model registry and model switching
- txt/md/pdf corpus ingestion with hybrid OCR extraction
- corpus lifecycle controls
- artifact tracking
- cleanup reporting
- audit visualization

However, the project should not yet be considered a hardened secure inference environment.

The current focus is building transparent runtime visibility and explicit lifecycle controls before attempting stronger low-level memory guarantees.
