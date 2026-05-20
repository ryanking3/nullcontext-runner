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

### Session Registry

Persistent sessions are indexed locally:

```text
~/.nullcontext/index.json
```

The registry tracks:

- session IDs
- timestamps
- security mode
- workspace paths
- report paths
- cleanup state
- artifact counts

### Local Web UI

The current browser UI supports:

- local prompt execution
- runtime lifecycle visualization
- audit operation inspection
- privacy report inspection
- persistent session browsing
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

Example body:

```json
{
  "prompt": "Explain secure local inference.",
  "mode": "secure",
  "persistent": false
}
```

### List Sessions

```http
GET /api/sessions
```

### Show Report

```http
GET /api/reports/:session_id
```

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

Example:

```toml
llama_path = "C:\\dev\\llama.cpp\\build\\bin\\Release\\llama-server.exe"

model_path = "C:\\models\\qwen2.5-7b\\qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"

default_mode = "secure"

max_tokens = 128

gpu_layers = 999
```

### Notes

```toml
gpu_layers = 999
```

means:

```text
offload as many layers as possible onto the GPU
```

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
- streaming token output
- streaming audit events
- retention policy systems
- stronger memory hygiene primitives
- VRAM inspection and analysis
- model management
- forensic artifact visibility
- Linux-native low-level memory work

---

## Project Status

NullContext is currently in active early-stage development.

The project is functional and supports:

- local inference
- local browser UI
- local API execution
- persistent sessions
- artifact tracking
- cleanup reporting
- audit visualization

However, the project should not yet be considered a hardened secure inference environment.

The current focus is building transparent runtime visibility and explicit lifecycle controls before attempting stronger low-level memory guarantees.