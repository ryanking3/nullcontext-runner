# Pre-Windows Validation Plan

## Purpose

Use the week before Windows/NVIDIA access to improve confidence in NullContext's
portable lifecycle, report, and evidence logic. The goal is to arrive at the
Windows validation session with a repeatable evidence runbook and fewer
unrelated regressions.

This Mac can validate local inference lifecycle behavior and macOS RAM
observation. It cannot validate the Windows-only direct process-memory scan or
the Windows/NVIDIA CUDA inspection path.

## Scope and Guardrails

- Keep security claims conservative: a marker miss in scanned memory is not a
  full-memory-clear claim.
- Do not claim VRAM sanitization, swap/pagefile sanitization, or allocator
  zeroization.
- Prefer small, testable changes over broad rewrites.
- Do not add cleanup stages merely to increase the number of experiments.
- Preserve both one-shot and active-chat behavior.

## Priority 1: Establish a Local macOS Smoke Path

### Prerequisites

- Install the Rust toolchain and `pnpm`.
- Provide a local arm64/Metal-capable `llama-server` build.
- Provide one small local GGUF model suitable for repeatable smoke tests.
- Create an uncommitted `~/.nullcontext/config.toml` pointing to the local
  runtime and model.

### Verification

- Run `cargo fmt` and `cargo build`.
- Run `pnpm build` from `apps/web`.
- Start the local server and confirm the health endpoint and browser UI load.
- Exercise one-shot inference, cancellation, active chat, and active-chat end
  plus sanitization.
- Exercise persistent-session retention, cleanup, and reconciliation.
- Ingest a small txt/markdown corpus, query it, and use it for one-shot and
  active-chat grounding.
- Inspect generated reports for macOS `ps` and `vmmap -summary` evidence,
  lifecycle operations, and honest residual-risk wording.

### Expected Limitation

The direct marker scan and controlled-canary RAM-side result will report the
current-platform scan backend as unsupported. Treat this as a check that the
fallback reporting is honest, not as a failed macOS test.

## Priority 2: Add Portable Automated Coverage

The repository has no formal automated test suite. Add focused unit and
fixture-style tests that do not need a model, llama runtime, or Windows host.

### Highest-Value Targets

- Process-scan status aggregation and distinction among detected, clear in
  scanned regions, incomplete, unsupported, and process-not-observable.
- Validation-history aggregation, cleanup-stage ranking, evidence-support
  classes, and release-gate verdicts.
- Runtime introspection manifest parsing, signal alias normalization, and
  declared-versus-observed contract gaps.
- Model and session configuration validation.
- Corpus chunking, retrieval selection, and provenance shaping.
- Legacy and current privacy-report JSON compatibility.

### Completion Criteria

- Tests cover the security-relevant status transitions and edge cases that the
  report UI presents to operators.
- `cargo test` passes locally once the Rust toolchain is installed.
- Existing report fixtures continue to deserialize without silent weakening of
  claim boundaries.

## Priority 3: Focused Refactoring and Report Cleanup

Refactor only where it improves reviewability or prevents evidence semantics
from drifting.

### Candidates

- Extract repeated report-grid blocks from
  `apps/web/src/components/PrivacyReportViewer.tsx`.
- Keep status-to-operator-language helpers centralized in
  `apps/web/src/appUtils.ts`.
- Isolate pure evidence derivation from report serialization in `src/audit.rs`
  where practical.
- Add reusable sanitized report fixtures for UI and compatibility checks.

### Do Not Do

- Do not broadly rewrite `src/runtime.rs`, `src/web.rs`, or active-chat
  lifecycle code without a passing local smoke path.
- Do not add speculative macOS process-memory scanning as a substitute for the
  Windows prototype.
- Do not add more invasive cleanup stages unless repeated evidence identifies
  a specific missing experiment.

## Priority 4: Claim-Boundary Audit

Review user-facing text in the CLI, report JSON summaries, and UI for terms
such as `clean`, `cleared`, `sanitized`, `memory`, and `VRAM`.

Each claim must resolve to one of these evidence levels:

1. Configured markers were detected in scanned regions.
2. Configured markers were not detected in scanned readable regions.
3. The process was no longer observable after cleanup.
4. Only host-tool/process-level RAM or GPU evidence exists.
5. A direct runtime lifecycle signal was observed.
6. The capability is unsupported or the observation is inconclusive.

Avoid language that upgrades any of these into proof of full RAM, VRAM, or
allocator sanitization.

## Priority 5: Prepare the Windows Evidence Session

Update `WINDOWS_RUNTIME_VALIDATION.md` or add a companion checklist that
requires an evidence bundle for each scenario.

### Record Once Per Machine

- Windows version, GPU model, NVIDIA driver version, and llama-server build.
- Model ID/path class, quantization, and GPU-offload setting.
- NullContext commit SHA and configuration relevant to the run.

### Required Scenarios

1. Normal one-shot CPU or low-offload run.
2. GPU-offloaded one-shot run.
3. Intentional runtime-start failure.
4. Active-chat start, message, cancel, end, and report generation.
5. Controlled-canary validation run.

### Capture for Every Scenario

- Privacy report JSON.
- Runtime PID and relevant NullContext logs.
- `Get-Process` and `Win32_Process` output for the runtime PID.
- `nvidia-smi` summary, `compute-apps`, and `pmon` output when GPU offload is
  requested.
- The observed result beside the expected report status and wording.

### Windows Completion Criteria

- Reported RAM, process-presence, and GPU visibility values agree with the
  contemporaneous host-tool captures or disclose the discrepancy.
- Direct process-scan phase statuses are plausible for live, failed-start, and
  post-shutdown cases.
- No report suggests allocator-level or VRAM-sanitization truth when the raw
  evidence only supports process-level visibility.

## Suggested Order

1. Provision the macOS toolchain and local runtime.
2. Establish build and smoke-test baselines.
3. Add portable tests and report fixtures.
4. Make only targeted refactors revealed by those tests.
5. Complete the claim-boundary audit.
6. Freeze the Windows runbook and collect its required commands and templates.

## Success Condition Before Windows Access

The repository builds locally, portable evidence logic is covered by automated
tests, macOS lifecycle/report paths have been smoke-tested, and the Windows
session can produce comparable evidence for every required scenario without
ad-hoc decisions.
