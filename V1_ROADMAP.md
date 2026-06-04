# NullContext v1 Roadmap

## Goal

Ship a trustworthy `v1.0` of NullContext as a local-first runtime orchestration and audit environment for llama.cpp-based inference.

`v1.0` should feel:

- reliable in normal local use
- honest about cleanup and residual risk
- consistent across one-shot and active chat flows
- clear about what was retained, deleted, observed, and not provable

`v1.0` should **not** claim:

- full RAM sanitization
- VRAM sanitization
- swap/pagefile sanitization
- llama.cpp internal allocator sanitization
- forensic-grade memory clearing

## Release Standard

NullContext is ready for `v1.0` when all of the following are true:

1. Core lifecycle and privacy-report semantics are stable.
2. One-shot and active-chat behavior are aligned enough to explain simply.
3. Windows/NVIDIA caveats are validated and disclosed conservatively.
4. Crash/restart recovery behavior is predictable and visible in the UI.
5. The main API/config/report surfaces are stable enough to treat as `v1` contracts.
6. Core flows have automated coverage.
7. Build, CI, and packaging are repeatable.

## Must-Have Before v1

### 1. Report Semantics Freeze

Lock the meaning and wording of:

- `successful`, `warning`, `failed`, and `not_attempted`
- lifecycle `state_note`
- cleanup summaries
- residual-risk summaries
- Windows/NVIDIA visibility-limited wording
- RAM vs VRAM inspection verdicts

Acceptance criteria:

- the same situation produces the same verdict language across one-shot, active chat, registry views, and saved reports
- no wording implies stronger sanitization than NullContext actually performs
- the privacy report JSON shape is treated as stable

### 2. One-Shot and Active-Chat Parity Validation

Run full manual validation for both runtime modes with the current behavior:

- one-shot success
- one-shot startup failure
- one-shot cancellation/stop behavior
- active-chat start, message, follow-up, cancel, and end
- retained-session registry behavior
- retained-corpus registry behavior

Acceptance criteria:

- the differences between one-shot and active chat are intentional and documented
- both modes produce coherent cleanup and inspection reports
- registry/report behavior matches actual artifact presence

### 3. Windows Validation Pass

Re-run targeted Windows validation after the recent runtime changes:

- dynamic per-session llama endpoint
- failed-startup cleanup
- one-shot report correctness
- active-chat report correctness
- WDDM `compute-apps` plus `pmon` visibility-limited behavior

Acceptance criteria:

- Windows wording remains conservative
- no path interprets WDDM ambiguity as proof of VRAM cleanup
- runtime endpoint, PID, and process/GPU observations line up with the host tools

### 4. Lifecycle and Recovery Freeze

Confirm and freeze the lifecycle model for:

- `active`
- `completed_retained`
- `cleanup_pending`
- `cleanup_succeeded`
- `cleanup_failed`
- `orphaned`

Also confirm current startup reconciliation behavior is good enough for `v1`.

Acceptance criteria:

- persistent active chats are visible before they end
- abandoned persistent chats become understandable `orphaned` entries after restart
- manual reconcile and cleanup actions are predictable
- corpus and session lifecycle behavior feel intentionally parallel

### 5. Public Surface Freeze

Treat these as `v1` contracts unless there is a deliberate breaking change:

- `~/.nullcontext/config.toml` keys and defaults
- `/api/models`
- `/api/corpora`
- `/api/chat/start`
- `/api/chat/:session_id/status`
- `/api/chat/:session_id/end`
- `/api/sessions`
- `/api/reports/:session_id`
- privacy report JSON

Acceptance criteria:

- field names are stable
- optional vs required fields are deliberate
- dynamic runtime endpoint and lifecycle visibility are included where useful

## Should-Have Before v1

### 1. Final UX Consistency Pass

Polish the operator experience around:

- active runtime state
- report availability
- registry actions
- inconclusive inspection results
- warning/failure language

Nice-to-have outcomes:

- consistent action/tooltips across session and corpus drawers
- consistent badge/status language
- no obvious formatting or wording glitches

### 2. Runtime Failure-Path Review

Review the remaining sharp edges in:

- launch readiness timeouts
- failed startup cleanup
- runtime shutdown
- cancellation behavior
- stale-process interference

Nice-to-have outcomes:

- launch errors are actionable
- shutdown failures remain visible
- runtime logs make local debugging easier

### 3. Documentation Release Pass

Update top-level docs so they match the actual `v1` product:

- README
- AGENTS.md
- config examples
- Windows caveats
- operator validation commands

Nice-to-have outcomes:

- docs describe NullContext as an audit/runtime tool, not a hardened secure system
- release claims and non-claims are explicit

## After Must-Have Work

Once the items above are complete, move to release hardening:

### 1. Automated Tests

Highest-priority backend tests:

- lifecycle transitions
- startup reconciliation
- session/corpus registry snapshot shaping
- audit/report wording helpers
- Windows GPU visibility-limited inspection cases
- runtime startup failure cleanup

Highest-priority frontend tests or smoke coverage:

- registry action availability
- report availability behavior
- report parsing/rendering
- active chat state transitions

### 2. CI/CD

Minimum CI scope:

- `cargo fmt --check`
- `cargo check`
- frontend build
- test suite once present

### 3. Packaging and Release Prep

- stable run instructions
- release notes
- versioning policy
- artifact naming
- platform caveats

## Post-v1

These are valuable, but should not block `v1.0`:

- richer automated integration tests
- better Linux-specific runtime memory inspection
- deeper allocator-aware introspection
- more document formats
- desktop shell refresh
- stronger graceful shutdown hooks if llama.cpp exposes better primitives

## Recommended Execution Order

1. Freeze report semantics.
2. Re-run one-shot and active-chat validation, especially on Windows.
3. Freeze lifecycle/recovery semantics.
4. Freeze public API/config/report contracts.
5. Do final UX and documentation pass.
6. Add automated tests.
7. Add CI/CD.
8. Package and cut `v1.0-rc1`.

## Definition of Done for v1.0

Call NullContext `v1.0` when:

- the product tells the truth consistently
- crash/restart behavior is understandable
- Windows caveats are validated and disclosed
- main flows are covered by tests
- release/build steps are repeatable
- the team can describe the product boundary in one sentence without hedging

That sentence should be close to:

> NullContext is a local-first runtime orchestration and audit environment for llama.cpp sessions that makes lifecycle, retention, cleanup, and residual-risk visibility explicit.
