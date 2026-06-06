# NullContext v1 Roadmap

## Status Snapshot

Last updated: `2026-06-06`

Overall status: `meaningful pre-v1 progress, but not yet ready for release hardening`

What is materially better than when this roadmap was first written:

- Windows/NVIDIA runtime reporting is more honest about PID visibility vs byte-level VRAM visibility.
- managed llama runtimes now use isolated localhost ports instead of a shared hardcoded port
- failed runtime startups clean themselves up and report much better diagnostics
- persistent active chats are visible before completion and survive restart into understandable registry states
- session and corpus lifecycle state notes now flow through registry views and saved reports
- session and corpus registries now expose live artifact presence instead of relying only on stale index state
- session and corpus report availability is more resilient after cleanup and archived-report moves
- startup reconciliation is now visible in the web UI instead of only appearing in server logs
- corpus retrieval readiness is enforced in both backend behavior and frontend UX

What still clearly blocks `v1.0`:

- full one-shot parity validation is still incomplete
- Windows validation still needs a one-shot pass and a failed-start pass after the recent runtime changes
- the report/API/config surfaces are close to stable, but not yet explicitly frozen as `v1` contracts
- there is still no automated coverage, CI, or packaging flow

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

Status: `in progress`

Completed enough to count as real progress:

- Windows/WDDM wording is now much more conservative
- lifecycle `state_note` is present across session and corpus registry/report surfaces
- cleanup/report views are more parallel across sessions and corpora

Still required before we can call this frozen:

- do one deliberate wording pass over `successful` / `warning` / `failed` / `not_attempted`
- verify one-shot and active-chat reports use the same language for equivalent outcomes
- explicitly treat the privacy report JSON shape as stable

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

Status: `in progress`

Completed enough to count as real progress:

- active-chat lifecycle, registry, report, and cleanup behavior are much more visible than before
- retained-session and retained-corpus registry behavior is substantially more truthful
- active-chat runtime endpoint and startup recovery behavior are now exposed

Still required before this item is done:

- run a fresh one-shot success pass
- run a one-shot failed-start pass
- run a one-shot cancel/stop pass
- explicitly compare one-shot and active-chat report semantics side by side

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

Status: `partially complete`

Completed enough to count as real progress:

- active-chat Windows/NVIDIA validation was run against a real WDDM environment
- the report now preserves `PID observed, bytes unavailable`
- post-shutdown VRAM cleanup no longer overclaims success under WDDM ambiguity

Still required before this item is done:

- re-run one-shot validation on Windows with the current runtime code
- force and inspect a failed startup on Windows
- confirm the new dynamic runtime endpoint and cleanup diagnostics in those flows

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

Status: `nearly complete`

Completed enough to count as real progress:

- persistent active chats are tracked while still live
- restart reconciliation can surface abandoned retained chats as `orphaned`
- startup reconciliation is visible in the UI
- corpus lifecycle behavior now feels much closer to session lifecycle behavior

Still required before this item is done:

- decide whether the existing lifecycle states are final `v1` states without renaming
- do one explicit manual restart/recovery signoff pass using the current UI
- confirm there are no remaining mismatches between registry action semantics and saved reports

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

Status: `in progress`

Completed enough to count as real progress:

- runtime endpoint visibility is now part of the API/report surface where it matters
- session and corpus registry responses now include live artifact-presence fields
- lifecycle action responses now carry richer post-action state

Still required before this item is done:

- review every public route/response listed below as an intentional contract
- decide which optional fields are truly stable
- do one doc pass that reflects the final field set instead of the current evolving set

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

Status: `mostly complete, but worth one final sweep`

Completed enough to count as real progress:

- registry and corpus drawers use more consistent action/tooltips and status summaries
- lifecycle/report availability messaging is clearer
- separator/copy glitches were cleaned up in multiple surfaces

Still worth doing:

- one final pass over warning/failure wording and status badge consistency
- verify that startup recovery, archived reports, and report-pending states all feel equally clear

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

Status: `substantial progress`

Completed enough to count as real progress:

- bind-port failures are clearer
- closed-stdout logging panics were hardened
- failed startup cleanup is automatic
- readiness timeout errors now include the last probe result
- stale `8080` interference was removed by dynamic runtime ports

Still worth doing:

- one deliberate review of shutdown failure paths
- one deliberate review of cancellation behavior under stress
- one manual bad-launch validation pass with the new diagnostics

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

Status: `started, not finished`

Completed enough to count as real progress:

- README and AGENTS were updated with better validation commands and portable examples
- the roadmap itself now reflects actual pre-v1 work

Still required before this item is done:

- a final README/AGENTS/config example pass after the public surface is frozen
- one explicit “claims and non-claims” sweep for release wording

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

1. Finish report semantics freeze with one deliberate wording/contract pass.
2. Re-run one-shot validation, then do the missing Windows one-shot and failed-start passes.
3. Sign off on lifecycle/recovery semantics using the current startup-recovery UI.
4. Freeze public API/config/report contracts and update docs to match.
5. Do one final UX consistency sweep.
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
