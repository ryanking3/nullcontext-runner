# Pre-Windows Validation Plan

## Purpose

Use the week before Windows/NVIDIA access to improve confidence in
NullContext's portable lifecycle, report, and evidence logic. The goal is to
arrive at the Windows validation session with a repeatable evidence runbook and
fewer unrelated regressions.

This Mac can validate local inference lifecycle behavior and macOS RAM
observation. It cannot validate the Windows-only direct process-memory scan or
the Windows/NVIDIA CUDA inspection path.

## How to Use This Plan

- Check off a task only when its stated acceptance criteria are met.
- Record the commit that completed a code task below the checkbox.
- Keep blocked tasks unchecked and explain the blocker in **Current Blockers**.
- Do not turn a marker miss, process exit, host-tool observation, or declared
  capability into a full-memory-clear claim.

## Current Blockers

- [ ] Install Rust/Cargo and `rustfmt` on this Mac.
- [ ] Install `pnpm` for the browser UI.
- [ ] Provide an arm64/Metal-capable `llama-server` build.
- [ ] Provide a small local GGUF model and an uncommitted
  `~/.nullcontext/config.toml`.

Until these are resolved, portable unit tests can be added and statically
checked, but Rust formatting, compilation, and runtime smoke tests cannot run
locally.

## Guardrails

- [ ] Preserve one-shot and active-chat behavior throughout all changes.
- [ ] Keep security claims conservative: a marker miss is limited to configured
  markers and scanned regions.
- [ ] Do not claim RAM/VRAM sanitization, swap/pagefile sanitization, or
  allocator zeroization without direct supporting evidence.
- [ ] Avoid broad rewrites of `src/runtime.rs`, `src/web.rs`, and active-chat
  lifecycle code until the local smoke path passes.
- [ ] Do not add cleanup stages merely to increase the number of experiments.

The guardrails are ongoing rules, not completion tasks; leave them unchecked
unless a final review explicitly verifies them for the full pre-Windows scope.

## Track 1: Portable Evidence Tests

### 1.1 Process-scan evidence boundaries

- [x] Cover report aggregation precedence when any phase detects a marker.
- [x] Cover completed marker misses as scoped evidence, not full clearance.
- [x] Cover unsupported backend status separately from failed/incomplete scans.
- [x] Cover post-shutdown PID absence without converting it into marker
  clearance.

Completed in `d3581ad` — `Add process scan evidence boundary tests`.

### 1.2 Controlled-canary aggregation

- [x] Cover repeated clear canary aggregation.
- [x] Cover repeated unsupported-platform aggregation.
- [x] Cover marker detection overriding otherwise clear passes.
- [x] Cover mixed clear and unsupported results remaining inconclusive.
- [x] Make canary test fixtures model marker-detected scan phases correctly.

Completed in `fdf4a53` — `Test controlled canary evidence aggregation`.

### 1.3 Validation history and release-gating

- [ ] Add a focused regression test for the release-readiness verdict when the
  leading stage lacks marker-backed evidence.
- [ ] Add a focused regression test for insufficient repeated history.
- [ ] Add a focused regression test for a marker-backed stage that satisfies
  the repeated-evidence gate.
- [ ] Confirm stage ranking continues to demote session-fallback and
  runtime-global-only evidence.

Acceptance criteria: the tests distinguish a *best available* stage from a
stage eligible for a stronger clean-stage/release-ready claim.

### 1.4 Runtime introspection contracts

- [ ] Test manifest parsing for canonical signal IDs and aliases.
- [ ] Test declared-but-unobserved signals remain distinct from observed ones.
- [ ] Test undeclared observed signals remain visible in the contract gap.
- [ ] Test stage-local helper-runtime cleanup signals remain distinct from
  runtime-global signals.

Acceptance criteria: a report cannot silently upgrade manifest declarations
into observed allocator/KV cleanup events.

### 1.5 Config, corpus, and report compatibility

- [ ] Add model/session configuration validation tests for invalid paths and
  invalid active-chat context bounds.
- [ ] Add corpus chunking and retrieval-provenance tests.
- [ ] Add sanitized legacy and current privacy-report JSON fixtures.
- [ ] Verify fixture deserialization preserves conservative defaults for absent
  newer evidence fields.

Acceptance criteria: portable user flows and older reports do not regress when
evidence/report schemas evolve.

### 1.6 Run the portable suite

- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo test`.
- [ ] Record the passing commands and any intentionally skipped platform tests.

Blocked by the missing Rust toolchain.

## Track 2: Local macOS Smoke Path

### 2.1 Build and UI baseline

- [ ] Run `cargo fmt` and `cargo build`.
- [ ] Run `pnpm build` from `apps/web`.
- [ ] Start `cargo run -- serve` and confirm `/api/health` responds.
- [ ] Confirm the browser UI loads locally.

### 2.2 Session lifecycle smoke tests

- [ ] Run a secure one-shot inference.
- [ ] Verify one-shot stop/cancel behavior.
- [ ] Start active chat, stream a message, send a follow-up, and cancel a
  generation without ending the session.
- [ ] End active chat and verify the End + Sanitize report.
- [ ] Verify persistent-session retention, cleanup, and reconciliation.

### 2.3 Corpus lifecycle smoke tests

- [ ] Ingest a small txt or markdown corpus.
- [ ] Query the corpus and verify retrieval provenance.
- [ ] Run one-shot corpus grounding.
- [ ] Run active-chat corpus grounding.
- [ ] Verify corpus retention, cleanup, and reconciliation.

### 2.4 macOS report checks

- [ ] Confirm reports include `ps`/`vmmap -summary` RAM observation when
  available.
- [ ] Confirm live/post-shutdown process observations are represented honestly.
- [ ] Confirm macOS reports identify direct process scanning as unsupported.
- [ ] Confirm controlled-canary output identifies the unsupported scan backend
  as a platform limitation, not clean RAM evidence.

Acceptance criteria: the host-side lifecycle/report path works end-to-end and
all unsupported macOS direct-scan statuses are clear and conservative.

## Track 3: Focused Refactoring and Report Fixtures

- [ ] Identify one repeated `PrivacyReportViewer` report-grid pattern that
  obscures evidence semantics.
- [ ] Extract it into a small typed component without changing wording or
  report data.
- [ ] Add or update a representative sanitized fixture for the extracted UI.
- [ ] Verify TypeScript build after the refactor.
- [ ] Review `src/audit.rs` for one isolated pure evidence-derivation helper
  that can be separated from serialization safely.

Acceptance criteria: a narrow refactor reduces repetition or makes evidence
semantics easier to review, with no lifecycle or claim-boundary change.

## Track 4: Claim-Boundary Audit

- [ ] Inventory user-facing uses of `clean`, `cleared`, `sanitized`, `memory`,
  and `VRAM` across the CLI, report summaries, and web UI.
- [ ] Classify each statement as one of: marker detected; marker miss in scanned
  regions; process not observable; host-tool/process-level observation; direct
  runtime signal; unsupported/inconclusive.
- [ ] Rewrite any statement that implies complete RAM, VRAM, or allocator
  clearing without matching evidence.
- [ ] Add regression coverage for any corrected status semantics.
- [ ] Record the final claim-boundary review commit.

Acceptance criteria: every operator-facing security statement identifies both
its evidence level and its remaining limitation.

## Track 5: Windows Evidence Session Preparation

### 5.1 Runbook and metadata

- [ ] Update `WINDOWS_RUNTIME_VALIDATION.md` or add a companion runbook.
- [ ] Add a template to record Windows version, GPU, NVIDIA driver,
  llama-server build, NullContext commit, model class, and offload settings.
- [ ] Add expected report statuses beside every scenario and host-tool capture.

### 5.2 Required scenarios

- [ ] Normal one-shot CPU or low-offload run.
- [ ] GPU-offloaded one-shot run.
- [ ] Intentional runtime-start failure.
- [ ] Active-chat start, message, cancel, end, and report generation.
- [ ] Controlled-canary validation run.

### 5.3 Required evidence for each scenario

- [ ] Privacy report JSON and relevant NullContext logs.
- [ ] Runtime PID plus `Get-Process` and `Win32_Process` output.
- [ ] `nvidia-smi` summary, `compute-apps`, and `pmon` output for GPU-offloaded
  runs.
- [ ] A recorded comparison of expected and observed status/wording.

Acceptance criteria: the Windows session can produce comparable evidence for
every required scenario without ad-hoc capture decisions.

## Final Pre-Windows Exit Check

- [ ] Rust build, formatting, and portable tests pass locally.
- [ ] The macOS lifecycle/report smoke path has passed.
- [ ] Unsupported macOS direct-scan behavior has been verified as honest.
- [ ] Claim-boundary audit findings have been resolved and committed.
- [ ] The Windows runbook and evidence templates are ready.

The plan is complete only when every applicable exit item is checked or
explicitly documented as blocked by missing hardware rather than missing
preparation.
