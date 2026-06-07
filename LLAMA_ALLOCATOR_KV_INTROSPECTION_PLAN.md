# Llama Allocator / KV Introspection Plan

This document is the concrete implementation plan for **Track B** in [V1_ROADMAP.md](/Users/ryanking/dev/nullcontext-runner/V1_ROADMAP.md).

It exists to keep the allocator / KV work disciplined and evidence-driven instead of drifting into vague “maybe patch llama.cpp later” territory.

## Goal

Give NullContext a real path to report allocator- and KV-specific evidence from `llama-server`, not just host-tool RAM/VRAM observations.

Before this track is complete, NullContext should be able to say more than:

- allocator unverified
- KV/cache unverified
- model unload not observed directly

Instead, it should eventually be able to distinguish things like:

- stock runtime, no allocator hooks
- instrumented runtime, allocator hooks present
- KV cache initialized
- KV cache reused
- KV cache clear observed
- allocator reset observed
- model unload observed
- signal unavailable because startup failed

## Current state in the repo

As of now:

- `src/audit.rs` already has a structured `llama_runtime.introspection` block.
- `src/runtime_capabilities.rs` can detect a sidecar manifest next to `llama-server` and expose declared capability flags.
- The UI already renders a `runtime introspection capabilities` section in the privacy report viewer.

That means the **report contract exists**, but NullContext still lacks **real runtime evidence** from inside llama.cpp.

## Track B roadmap alignment

This file corresponds to:

7. `Document llama allocator and KV introspection plan`
8. `Add runtime capability flags for instrumented llama builds`
9. `Expose KV cache lifecycle signals in reports`
10. `Expose allocator reset signals in reports`

Track `8` is partially underway already through the sidecar manifest path, but we should still treat the remaining items in sequence and keep the work tied to this plan.

## What we actually need from llama.cpp

We do **not** need full allocator forensics immediately.

We do need runtime-visible signals for these domains:

### 1. Runtime build profile

Questions:

- Is this a stock upstream-style `llama-server` build?
- Is this a NullContext-instrumented build?
- What instrumentation backend is active?

Desired evidence:

- runtime build profile string
- instrumentation backend string
- capability source

### 2. KV cache lifecycle

Questions:

- Was a KV cache initialized for the runtime/session?
- Was it reused across messages?
- Was a KV/cache clear/reset operation observed?
- Did startup fail before KV/cache state could be established?

Desired evidence:

- `kv_cache_initialized`
- `kv_cache_reused`
- `kv_cache_clear_observed`
- optional counts or simple lifecycle notes

### 3. Allocator lifecycle

Questions:

- Was an allocator reset hook invoked?
- Was model-memory teardown explicitly signaled?
- Is there any explicit “allocator cleared” signal from the instrumented runtime?

Desired evidence:

- `allocator_reset_observed`
- `model_unload_observed`
- `allocator_introspection_supported`

## Evidence sources in preferred order

We should use the simplest trustworthy source first and only move deeper when needed.

### Source A: Sidecar capability manifest

This already exists conceptually in the repo and is the current capability declaration path.

Use for:

- build profile
- instrumentation backend
- declared support flags

Do not use it as proof that a lifecycle event actually happened.

This source answers:

- what the runtime *claims it can report*

It does **not** answer:

- what the runtime *actually did during this session*

### Source B: Runtime stdout / stderr lifecycle signals

This is probably the safest first real evidence path.

Idea:

- instrumented `llama-server` emits machine-parseable lines on startup / reuse / teardown
- NullContext captures and parses those lines into structured session evidence

Example signal categories:

- KV cache initialized
- KV cache reused
- KV cache clear observed
- allocator reset observed
- model unload observed

Why this is a good next step:

- minimal protocol surface
- works with local process execution model we already have
- easy to preserve in failed-start reports too
- doesn’t require building a custom control API immediately

### Source C: Instrumented local endpoint

If stdout/stderr is too weak or too noisy, the next path is a patched local endpoint exposed by the runtime.

Possible examples:

- `/nullcontext/capabilities`
- `/nullcontext/lifecycle`
- `/nullcontext/last-cleanup`

Use for:

- richer structured evidence
- explicit allocator/KV state transitions

This is stronger than log parsing, but also a bigger fork/patch commitment.

## Suggested implementation order

### Slice 1

Keep what we already have:

- report schema for introspection
- capability detection via sidecar manifest

Goal:

- capability declaration path exists

### Slice 2

Add a concrete manifest contract file example and tighten detection semantics.

Goal:

- a future instrumented build has an exact format to emit

Candidate follow-up artifact:

- `runtime_introspection_manifest.example.json`

### Slice 3

Add runtime lifecycle event parsing from stdout/stderr.

Goal:

- real per-session allocator / KV evidence enters reports

Suggested event names:

- `kv_cache_initialized`
- `kv_cache_reused`
- `kv_cache_clear_observed`
- `allocator_reset_observed`
- `model_unload_observed`

### Slice 4

Thread those events into `llama_runtime.introspection`.

Goal:

- report capability flags and real observed events separately

### Slice 5

Refine memory-domain notes in `src/audit.rs` using observed lifecycle signals.

Goal:

- allocator and KV domain notes become evidence-backed instead of purely cautionary

## Data model direction

The current `LlamaRuntimeIntrospectionReport` should stay the top-level summary.

The next likely addition should be a structured event/evidence list, something like:

- `observed_events: Vec<LlamaRuntimeIntrospectionEvent>`

Potential fields:

- `event`
- `status`
- `source`
- `timestamp_relative_ms`
- `details`

Examples:

- `kv_cache_initialized`
- `kv_cache_clear_observed`
- `allocator_reset_observed`
- `model_unload_observed`

This gives us a clean separation:

- capability declaration
- actual observed lifecycle evidence

## What counts as success for Track B

Before we call Track B meaningfully underway, NullContext should be able to produce a report where:

- capability source is explicit
- stock vs instrumented runtime is explicit
- KV/cache lifecycle evidence is either:
  - observed
  - not observed
  - unavailable
- allocator reset evidence is either:
  - observed
  - not observed
  - unavailable

That is enough to materially improve the honesty of allocator/KV reporting before full sanitization exists.

## Non-goals for this track

This track does **not** require:

- proving allocator zeroization
- proving OS page clearing
- proving freed VRAM clearing
- replacing the direct process-scan work

This track is about **internal lifecycle visibility**, not full sanitization proof.

## Recommended next exact commits after this doc

1. `Add runtime introspection manifest example`
2. `Parse instrumented runtime lifecycle signals from stdout and stderr`
3. `Expose KV cache lifecycle signals in reports`
4. `Expose allocator reset signals in reports`

That sequence follows the roadmap more closely and keeps the work incremental.
