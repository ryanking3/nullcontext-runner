# NullContext

NullContext is a local-first secure inference environment for running LLM sessions with explicit lifecycle visibility, audit reporting, and configurable persistence behavior.

The project currently targets macOS development using:

- Rust
- llama.cpp
- Tauri
- local GGUF models

NullContext is designed around the idea that local inference systems should expose:

- what was stored
- what was retained
- what was deleted
- what cleanup operations occurred
- what residual risks remain

rather than treating local inference as an opaque black box.

---

## Current Features

### Local Inference Runtime

- llama.cpp backend integration
- local GGUF model support
- stdin-based prompt ingestion
- configurable inference modes
- persistent and ephemeral sessions

### Security / Privacy Features

- explicit workspace lifecycle management
- recursive artifact scanning
- Rust-owned buffer zeroization
- RAM zeroization verification
- audit operation tracking
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

### Desktop Shell

The Tauri desktop shell currently supports:

- local prompt execution
- runtime log streaming
- live audit operation streaming
- privacy report inspection
- persistent session browsing

---

## Project Structure

```text
nullcontext-runner/
├── src/                    # Rust backend runtime
├── apps/
│   └── desktop/            # Tauri desktop shell
├── target/
└── README.md
```

---

## Security Modes

### secure

Default mode.

Characteristics:

- ephemeral workspace
- automatic cleanup
- audit reporting
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

## Current Limitations

NullContext does not currently guarantee:

- VRAM sanitization
- llama.cpp internal allocator sanitization
- OS swap sanitization
- shell history sanitization
- cross-process memory sanitization

The privacy reports intentionally expose these residual risks.

---

## Development Setup

### Requirements

- macOS
- Rust
- Node.js
- pnpm
- llama.cpp
- local GGUF model

### Backend

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

### Desktop App

From:

```bash
apps/desktop
```

Run:

```bash
pnpm install
pnpm tauri dev
```

---

## Configuration

Configuration file:

```text
~/.nullcontext/config.toml
```

Example:

```toml
llama_path = "/Users/yourname/dev/llama.cpp/build/bin/llama-server"
model_path = "/Users/yourname/models/model.gguf"
default_mode = "secure"
max_tokens = 256
gpu_layers = 0
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

## Current Focus

The current development focus is:

- structured runtime streaming
- retention policy systems
- stronger memory hygiene primitives
- desktop runtime orchestration
- model management
- Linux-native low-level memory work
