# AGENTS.md

## Project Goal

NullContext is a local-first secure inference runtime and web UI for running llama.cpp-based local LLM sessions with explicit lifecycle visibility.

The project is intended to make local inference sessions observable and auditable. It should expose:

- what runtime was started
- what prompt/response artifacts were created
- what data was retained
- what data was deleted
- what cleanup and sanitization operations occurred
- what residual risks remain

NullContext is not just a chat wrapper. It is a runtime orchestration and audit environment for local LLM sessions.

## Current Status

The project currently supports:

- Rust backend runtime
- local Axum API server
- React/Vite web UI
- llama.cpp / llama-server integration
- local GGUF models
- one-shot inference mode
- active chat session mode
- streamed token output
- generation stop/cancel control
- session registry for persistent sessions
- lifecycle policy engine for retained sessions
- structured model registry and model switching
- corpus registry with txt/md/pdf ingestion
- hybrid pdf extraction with OCR for sparse pages
- browser-native corpus file uploads with drag/drop and upload progress
- chatbot-style composer uploads for grounding corpora
- one-shot grounded retrieval
- active-chat grounded retrieval
- corpus lifecycle cleanup, reconcile, and retention controls
- workspace artifact scanning
- cleanup reports
- privacy reports
- direct Windows process-memory scanning for configured llama-server markers
- live/post-shutdown prompt and response marker scan reporting
- repeated controlled canary validation passes
- cross-session memory-validation history
- platform capability matrix reporting
- Rust-owned prompt/response buffer zeroization
- llama runtime exposure reporting
- live llama runtime RAM/VRAM usage snapshots
- post-shutdown llama runtime inspection
- macOS vmmap-based RAM inspection and before/after region delta reporting
- Windows PowerShell-based process memory observation
- NVIDIA `nvidia-smi` compute-apps and `pmon` fallback inspection paths
- allocator / KV lifecycle capability reporting
- runtime-signal and cleanup-signal contract reporting
- cleanup-stage VRAM comparison, marker-aware scorecards, repeated stage trends, recommendation evidence classes, release gating, and helper-stage canary scans
- Windows/NVIDIA GPU provenance, trust-boundary, evidence-tier, claim-boundary, and context-visibility reporting
- active-chat preflight blockers before session start when model/corpus/config state is invalid
- explicit corpus detach controls for stale or cleaned-up chat bindings
- active chat final reporting

The project is functional but still early-stage. It should not be described as a hardened secure inference system.

Current security work is foundational. NullContext does not currently guarantee:

- VRAM sanitization
- OS swap/pagefile sanitization
- process-wide memory sanitization
- llama.cpp internal allocator sanitization
- CUDA memory sanitization
- forensic memory clearing outside Rust-owned buffers

Reports should continue to disclose these limitations honestly.

## Architecture Notes

### Backend

Backend language: Rust.

Important backend files:

- `src/main.rs`  
  CLI/server entry point. Keeps `main()` synchronous and creates Tokio runtime only for server mode.

- `src/config.rs`  
  CLI parsing, config loading, security modes, prompt source handling, and `SessionConfig`.

- `src/runtime.rs`  
  Starts, checks, and stops `llama-server`, performs best-effort runtime RAM/VRAM observation, runs cleanup stages, and captures helper-stage validation evidence across macOS and Windows/NVIDIA paths.

- `src/inference.rs`  
  Blocking one-shot inference path used by CLI mode.

- `src/web.rs`  
  Axum API server. Provides one-shot streaming routes, active chat routes, registry routes, report routes, and health check.

- `src/chat.rs`  
  Active chat session manager. Keeps long-lived runtime sessions, streams messages, tracks turns, writes per-turn artifacts, and finalizes active chat reports.

- `src/corpus.rs`  
  Corpus manifest types, artifact paths, and corpus lifecycle metadata.

- `src/corpus_registry.rs`  
  Persistent corpus index under `~/.nullcontext/corpora/index.json`, corpus retention metadata, startup reconciliation, and corpus lifecycle syncing.

- `src/docs.rs`  
  Local document ingestion for txt, markdown, and pdf sources, including hybrid native/OCR extraction and corpus report generation.

- `src/embed.rs`  
  Local embedding backend abstraction and persisted corpus embedding records.

- `src/retrieval.rs`  
  Corpus querying, prompt grounding, and retrieval provenance shaping.

- `src/audit.rs`  
  Privacy report structures including active chat `SessionProfile`, llama runtime exposure reporting, platform capability matrix data, cleanup-stage comparison reporting, and inspection verdicts.

- `src/process_scan.rs`  
  Direct llama-server marker scanning model and platform-specific scan phases.

- `src/runtime_capabilities.rs`  
  Declared runtime capability detection for instrumented llama builds.

- `src/runtime_introspection.rs`  
  Parsed allocator/KV/runtime lifecycle signals from llama runtime output.

- `src/gpu_inspection.rs`  
  Windows/NVIDIA GPU observation backends, evidence classification, and host-tool versus stronger-visibility reporting.

- `src/memory_validation.rs`  
  Derived validation scorecards combining process-scan, cleanup-stage, and canary evidence.

- `src/validation_harness.rs`  
  Repeated controlled canary helper validation runs and representative-pass selection.

- `src/validation_history.rs`  
  Cross-session validation-history persistence, repeated cleanup-stage trends, recommendation evidence classes, and release-gate summaries.

- `src/cleanup.rs`  
  Artifact scanning, cleanup reporting, sanitization operation records.

- `src/session.rs`  
  Session ID, workspace, started timestamp, prompt/response/report writes.

- `src/registry.rs`  
  Persistent session index under `~/.nullcontext/index.json`.

- `src/sensitive.rs`  
  Zeroizable sensitive byte buffers.

- `src/memory_scan.rs`  
  Rust-owned buffer zeroization verification helpers.

### Frontend

Frontend path:

```text
apps/web
```

Important frontend files:

- `apps/web/src/App.tsx`  
  Main UI composition layer. Wires one-shot mode, active chat mode, streaming, stop control, reports, runtime logs, audit operations, session registry, model registry, corpus registry, upload ingestion UX, and structured llama runtime inspection views through extracted hooks and components.

- `apps/web/src/App.css`  
  Minimal terminal-style dark/light UI.

### Older Desktop App

`apps/desktop` contains an older Tauri shell. The project direction has pivoted to local browser UI. Do not delete the desktop app unless explicitly asked.

## Runtime Modes

### One-shot mode

One prompt creates a full lifecycle:

- create session
- launch llama-server
- stream completion
- shutdown runtime
- scan artifacts
- sanitize Rust-owned buffers
- cleanup or retain workspace
- emit report

This is slower but has the strongest cleanup cadence.

One-shot mode can optionally bind a local corpus and inject retrieved context before inference.

### Active chat mode

A chat session creates a long-lived runtime:

- start active session
- launch llama-server once
- send multiple messages through same runtime
- keep chat context in memory
- end session explicitly
- shutdown runtime
- zeroize Rust-owned chat history
- scan artifacts
- cleanup or retain workspace
- emit report

This is faster but has session-scoped residual risk. The UI must clearly expose that risk.

Active chat currently also supports:

- model-aware prompt template selection
- bounded recent-context token budgeting
- bounded recent-context turn limits
- audit visibility when older turns are dropped from the prompt window
- optional bound corpus retrieval on every turn

## Corpus Workflows

### Corpus ingestion

NullContext currently supports local corpus ingestion for:

- `txt`
- `md`
- `pdf`

PDF ingestion uses a hybrid pipeline:

- native text extraction first
- OCR for sparse or empty pages when enabled
- page-level extraction provenance in stored artifacts

Corpus artifacts are retained under the NullContext corpus registry and can be lifecycle-managed separately from chat/session workspaces.

Corpus ingestion currently also supports browser-native file upload for `txt`, `md`, and `pdf` inputs through the local web UI, including drag/drop, upload progress, and chatbot-style composer uploads for quick grounding corpora.

## API Routes

Current routes:

```text
GET  /api/health
POST /api/run
POST /api/run/stream
GET  /api/corpora
POST /api/corpora
GET  /api/corpora/:corpus_id/report
POST /api/corpora/:corpus_id/query
POST /api/corpora/:corpus_id/retention
POST /api/corpora/:corpus_id/cleanup
POST /api/corpora/:corpus_id/reconcile
GET  /api/models
POST /api/chat/start
GET  /api/chat/:session_id/status
POST /api/chat/:session_id/cancel
POST /api/chat/:session_id/message/stream
POST /api/chat/:session_id/end
GET  /api/sessions
POST /api/sessions/:session_id/retention
POST /api/sessions/:session_id/cleanup
POST /api/sessions/:session_id/reconcile
GET  /api/reports/:session_id
```

Streaming events are JSON objects delivered as SSE-style `data:` blocks.

Known event types:

- `runtime`
- `audit`
- `model`
- `report`
- `stderr`
- `error`
- `complete`

## Configuration

User config file:

```text
~/.nullcontext/config.toml
```

Example model-registry config:

```toml
llama_path = "/Users/ryanking/dev/llama.cpp/build/bin/llama-server"
default_model = "qwen-small"
default_mode = "secure"
gpu_layers = 0
chat_template = "auto"
chat_context_token_budget = 2048
chat_context_turn_limit = 12

[[models]]
id = "qwen-small"
name = "Qwen 2.5 0.5B Instruct"
model_path = "/Users/ryanking/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
max_tokens = 64
gpu_layers = 0
chat_template = "chatml"
chat_context_token_budget = 2048
chat_context_turn_limit = 12
```

Example Windows CUDA config:

```toml
llama_path = "C:\\dev\\llama.cpp\\build\\bin\\Release\\llama-server.exe"
default_model = "qwen-7b"
default_mode = "secure"
gpu_layers = 999
chat_template = "auto"
chat_context_token_budget = 2048
chat_context_turn_limit = 12

[[models]]
id = "qwen-7b"
name = "Qwen 2.5 7B Instruct"
model_path = "C:\\models\\qwen2.5-7b\\qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"
max_tokens = 128
gpu_layers = 999
chat_template = "chatml"
chat_context_token_budget = 2048
chat_context_turn_limit = 12
```

Do not commit local config files or model files.

Active chat config notes:

- `model_id` selects a registered model by ID
- `corpus_id` binds a registered local corpus by ID
- `chat_template` supports `auto`, `generic`, `chatml`, and `llama3-instruct`
- `chat_context_token_budget` is an approximate recent-context budget
- `chat_context_turn_limit` bounds how many recent prior turns can be included
- both context settings must be greater than `0`
- legacy single-model `model_path` configs are still supported and get synthesized into a default registry entry

Corpus notes:

- corpora are stored under `~/.nullcontext/corpora`
- persistent corpora retain artifacts until manual or scheduled cleanup
- ephemeral corpora use the system temp directory under `nullcontext/corpora`
- OCR is local-only and currently depends on local CLI tooling when enabled

### Workspace Paths

NullContext session workspaces are created under the system temporary directory, in a `nullcontext` subdirectory.

Typical examples:

```text
macOS/Linux: $TMPDIR/nullcontext or /tmp/nullcontext
Windows: %TEMP%\nullcontext
```

The exact path is determined at runtime using Rust's `std::env::temp_dir()`.

## Commands

### Backend

Format:

```bash
cargo fmt
```

Build:

```bash
cargo build
```

Run server:

```bash
cargo run -- serve
```

CLI one-shot:

```bash
echo "Explain secure local inference." | cargo run -- --stdin
```

List sessions:

```bash
cargo run -- --list-sessions
```

Show report:

```bash
cargo run -- --show-report <session-id>
```

### Frontend

Install:

```bash
cd apps/web
pnpm install
```

Run dev server:

```bash
pnpm dev
```

Build/typecheck:

```bash
pnpm build
```

## API smoke tests

Health:

```bash
curl http://127.0.0.1:3333/api/health
```

Start active chat:

```bash
curl -X POST http://127.0.0.1:3333/api/chat/start \
  -H "Content-Type: application/json" \
  -d '{"mode":"secure","persistent":false,"model_id":"<configured-model-id>","chat_template":"auto","chat_context_token_budget":2048,"chat_context_turn_limit":12}'
```

Send active chat message:

```bash
curl -N -X POST http://127.0.0.1:3333/api/chat/<session-id>/message/stream \
  -H "Content-Type: application/json" \
  -d '{"prompt":"Explain secure local inference in 2 short bullet points."}'
```

End active chat:

```bash
curl -X POST http://127.0.0.1:3333/api/chat/<session-id>/end
```

Windows/NVIDIA validation checks:

```powershell
Get-Process -Id <pid>
Get-CimInstance Win32_Process -Filter "ProcessId = <pid>"
nvidia-smi
nvidia-smi --query-compute-apps=pid,used_gpu_memory --format=csv,noheader,nounits
nvidia-smi pmon -c 1
```

## Testing Expectations

No formal automated test suite currently exists.

Before making significant changes, run:

```bash
cargo fmt
cargo build
```

For frontend changes, run:

```bash
cd apps/web
pnpm build
```

Manual verification should include:

- server starts
- frontend loads
- one-shot run works
- one-shot stop works
- active chat cancel endpoint works
- active chat starts
- active chat streams a message
- active chat can send a follow-up
- active chat template selection affects prompt formatting as expected
- active chat context window truncates older turns when budget/turn limit is exceeded
- active chat audit stream reports context-window preparation or truncation
- active chat stop does not kill the session
- active chat end generates report
- llama runtime report shows shutdown method and inspection verdicts
- process scan reports show live/post-shutdown scan state when markers are available
- repeated controlled canary validation runs are attached to the report
- platform capability matrix renders current track readiness truthfully
- cleanup-stage reports show marker-aware evidence status and helper-stage scan summaries when available
- cross-session validation history updates after repeated runs in the same scope
- cleanup-stage recommendation reporting distinguishes best-stage ranking from evidence-support class and clean-claim status
- macOS runtime reports show vmmap footprint and resident-region evidence when available
- Windows runtime reports show PowerShell process-memory evidence when available
- NVIDIA-backed runs report either compute-apps VRAM bytes or `pmon` PID visibility notes when available
- Windows/NVIDIA runtime reports show GPU provenance, trust boundary, evidence tier, claim boundary, and context-visibility wording consistent with the raw evidence
- Windows/NVIDIA validation compares the report against live `Get-Process`, `Win32_Process`, `nvidia-smi compute-apps`, and `nvidia-smi pmon` output for the same llama-server PID
- lifecycle registry actions work for retained sessions
- scheduled retention cleanup works
- model registry drawer loads
- invalid model paths are marked unavailable
- corpus ingest works for txt/md/pdf inputs
- browser upload ingestion works for txt/md/pdf inputs
- corpus drawer loads and shows lifecycle state
- one-shot corpus grounding works
- active-chat corpus grounding works
- corpus lifecycle actions work for retained corpora
- persistent registry still works when using standard + persistent mode

## Coding Conventions

General:

- Prefer small, targeted changes.
- Do not rewrite large files unless necessary.
- Preserve existing behavior unless explicitly changing it.
- Do not add dependencies without justification.
- Keep security claims precise and conservative.
- Do not claim stronger sanitization than implemented.
- Keep one-shot mode and active chat mode both functional.
- Use explicit error handling through `anyhow::Result` where consistent with existing code.
- Preserve `cargo fmt`.

Rust:

- Keep blocking llama.cpp/reqwest work out of async Axum handlers using `spawn_blocking` or dedicated threads.
- Avoid dropping blocking runtimes inside async contexts.
- Keep `main()` synchronous unless the runtime design is deliberately refactored.
- Be careful with mutexes around long streaming operations.
- Treat prompt/response buffers as sensitive.
- Zeroize temporary prompt copies where feasible.

Frontend:

- Keep the UI local, minimal, terminal-like, and dark/light capable.
- Avoid heavy UI libraries unless explicitly approved.
- Keep active chat runtime risk visible.
- Keep corpus lifecycle state and residual-risk visibility explicit.
- Keep active chat template/context settings understandable and explicit.
- Keep End + Sanitize prominent while active runtime is live.
- Preserve stop button behavior.
- Preserve before-unload warning while active chat runtime is live.

## Git Workflow

Use focused commits.

Recommended commit style:

- Add active chat session lifecycle API
- Add active chat message streaming API
- Improve active chat session reporting
- Add active chat safety UX
- Add generation stop control

Before committing:

```bash
git status
cargo fmt
cargo build
```

For frontend changes:

```bash
cd apps/web
pnpm build
```

Then:

```bash
git add .
git commit -m "<clear message>"
git push
```

Do not commit:

- GGUF model files
- local config files
- generated temp workspaces
- large screenshots unless intentionally placed under `docs/images`
- `node_modules`
- `target` build artifacts

## Safety Rules

- Do not silently weaken cleanup behavior.
- Do not remove one-shot mode.
- Do not remove active chat mode.
- Do not bind the server to public interfaces without adding auth and explicit user approval.
- Do not claim full memory sanitization.
- Do not claim VRAM sanitization.
- Do not claim OS swap/pagefile sanitization.
- Do not claim llama.cpp internals are sanitized.
- Do not add network/cloud dependencies unless explicitly approved.
- Do not add telemetry.
- Do not store prompts persistently unless user selected persistent behavior.
- Do not alter security mode semantics casually.
- Do not auto-delete user files outside NullContext-owned workspaces.
- Do not add dependencies without explaining why.
- Do not rewrite large sections unnecessarily.

## Known Technical Debt

- Active session manager is in-memory only.
- Abandoned sessions after server crash need recovery strategy.
- Tauri desktop shell is stale relative to web UI.
- No automated tests.
- OCR currently relies on local CLI availability and does not implement full document-layout fidelity.
- llama runtime RAM inspection is strongest on macOS right now and still relies on best-effort host tooling rather than direct allocator introspection.
- Windows direct process scanning is currently the strongest platform-specific marker backend; broader parity is still incomplete.
- cleanup-stage and helper-stage evidence now exists per report, and repeated cross-run aggregation exists, but threshold tuning and broader real-history calibration are still incomplete.
- allocator / KV introspection is still only partial and needs deeper instrumented-runtime evidence.
- Windows/NVIDIA runtime inspection needs live validation against real driver/runtime combinations, and current VRAM evidence is still host-tooling based rather than true allocator introspection or sanitization.
