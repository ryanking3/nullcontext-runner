# NullContext Security-First Pre-v1 Roadmap

## Goal

Before `v1.0`, NullContext should make major progress on:

1. direct process memory scanning
2. llama.cpp allocator / KV introspection
3. CUDA / NVIDIA API-level inspection
4. experimental VRAM sanitization

This roadmap treats those as true release blockers rather than optional research.

---

## Guiding Rules

- Never claim sanitization when we only have loss of observability.
- Always distinguish:
  - `not observed`
  - `not scannable`
  - `scan unavailable`
  - `still found`
- Treat platform support explicitly:
  - `macos`
  - `windows_nvidia`
  - `linux`
- Every new security feature should produce:
  - report data
  - operator-visible UI
  - residual-risk wording
  - a validation procedure

---

## Phase 1: Direct Process Memory Scanning

### Goal

Scan the `llama-server` process memory for known prompt/response markers and report whether they are still observable:

- while runtime is alive
- after shutdown
- after failed startup cleanup

### Why First

This is the first serious step beyond RSS / VRAM summaries into actual content persistence evidence.

### Scope

Build a new module:

- `src/process_scan.rs`

Core concepts:

- scan target = `llama-server` PID
- scan patterns:
  - prompt marker
  - response marker
  - optional synthetic canary marker
- scan phases:
  - `live_runtime`
  - `post_shutdown`
  - `failed_start_cleanup`

### Data Model

Add report structs for:

- scan attempted
- platform
- method
- scope summary
- regions scanned
- regions skipped
- bytes scanned
- patterns searched
- pattern matches found / not found
- errors / limitations

Likely new report types:

- `ProcessScanReport`
- `ProcessScanPatternResult`
- `ProcessScanRegionSummary`

### Platform Order

Recommended order:

1. `windows_nvidia` first if that is the main dev/test environment
2. `linux` second because it may be technically cleaner
3. `macos` third if feasible

### Implementation Slices

1. Add process scan report model and report/UI placeholders
2. Add first real process scan backend for one platform
3. Scan live runtime for prompt marker
4. Scan post-shutdown window for prompt marker
5. Add response-marker scanning
6. Add failed-start cleanup scan path

### Acceptance Criteria

- One platform can scan the target process for a known marker.
- Reports distinguish:
  - marker found
  - marker not found
  - scan unavailable
  - scan incomplete
- UI displays the result clearly.
- Live vs post-shutdown evidence can be compared.

---

## Phase 2: llama.cpp Allocator / KV Introspection

### Goal

Understand and, where possible, expose internal llama.cpp memory lifecycle:

- KV cache creation/lifetime
- allocator arenas
- model weight residency boundaries
- teardown behavior

### Why Second

External scanning tells us whether content persists. Allocator/KV introspection helps explain why.

### Scope

This phase is likely a mix of:

- source research in llama.cpp
- possible patching/forking
- optional custom build path for introspection-enabled llama-server

### Likely Outputs

- documented allocator/KV map
- introspection-enabled build mode
- report fields for:
  - KV cache initialized
  - KV cache reused
  - KV/cache explicitly cleared or not
  - allocator reset hooks available/unavailable
  - model unload semantics observed/unobserved

### Likely Implementation Shape

Possibly:

- a patched llama.cpp build under a documented fork/patch set
- introspection hooks exposed via stdout/stderr parsing, local API, or patched endpoints
- `src/runtime.rs` and `src/audit.rs` consuming those signals

### Implementation Slices

1. Research spike
2. Add introspection capability flags
3. Expose KV/cache lifecycle events
4. Expose allocator teardown/reset signals
5. Report model unload / allocator reset evidence
6. Integrate with runtime memory-domain reporting

### Acceptance Criteria

- Reports can say more than “allocator unverified”.
- Reports distinguish:
  - stock runtime
  - instrumented runtime
  - KV/cache clear observed
  - allocator reset observed
  - no allocator evidence available

---

## Phase 3: CUDA / NVIDIA API-Level Inspection

### Goal

Move beyond `nvidia-smi` host-tool visibility into stronger GPU/runtime evidence.

### Why Third

Without API-level inspection, VRAM claims stay too weak and WDDM ambiguity remains a ceiling.

### Scope

Investigate:

- CUDA Driver API
- CUDA Runtime API
- NVML
- whether per-process / per-context memory visibility is possible
- what can be tied to llama-server PID or CUDA context

### Likely Outputs

- better GPU observation backend than `nvidia-smi` alone
- clearer distinction between:
  - PID visible
  - context visible
  - allocation bytes visible
  - allocator state visible
- platform caveat matrix for WDDM vs TCC if relevant

### Implementation Slices

1. Research spike
2. Add backend abstraction for GPU inspection
3. Add NVML/CUDA-backed observation path
4. Report context/allocation visibility separately
5. Improve post-shutdown GPU inspection
6. Update UI/report semantics for richer GPU evidence

### Acceptance Criteria

- GPU evidence is no longer solely `nvidia-smi`-based on Windows/NVIDIA.
- Reports distinguish:
  - PID-only visibility
  - allocation-byte visibility
  - context visibility
  - inspection unavailable

---

## Phase 4: Experimental VRAM Sanitization

### Goal

Try real cleanup strategies and measure whether they improve VRAM evidence.

### Why Fourth

Sanitization attempts without inspection are guesswork.

### Scope

This phase is explicitly experimental. Test strategies like:

- process termination alone
- different shutdown timing
- forced context teardown if feasible
- allocator churn / overwrite experiments if feasible
- device reset only if safe and realistic

### Important Constraint

The first goal is evidence of improvement, not immediate proof of full sanitization.

### Implementation Slices

1. Define cleanup strategy model
2. Run baseline observation with no special strategy
3. Add one experimental cleanup strategy
4. Compare pre/post evidence
5. Report strategy used and outcome
6. Only elevate claims if repeatably justified

### Acceptance Criteria

- At least one VRAM cleanup strategy is implemented experimentally.
- Reports say:
  - strategy attempted
  - evidence improved
  - evidence unchanged
  - evidence inconclusive
- No wording implies complete VRAM sanitization unless proven.

---

## Cross-Cutting Work Required in Every Phase

### 1. Report Model Expansion

Every phase needs updates in:

- `src/audit.rs`

Add structured sections for:

- process scan evidence
- allocator/KV evidence
- GPU API inspection evidence
- VRAM cleanup strategy outcomes

### 2. UI Surfaces

Likely updates to:

- `apps/web/src/components/PrivacyReportViewer.tsx`
- compact summary cards in the inspector
- per-platform capability notes

### 3. Capability Matrix

Add a per-platform capability model:

- `supported`
- `unsupported`
- `unavailable`
- `visibility_limited`

This should affect both backend report fields and UI display.

### 4. Validation Harness

Before `v1.0`, this needs at least a lightweight harness:

- known marker injection
- controlled prompt canaries
- scan before/after shutdown
- repeated runs
- recorded results by platform

---

## Suggested Commit-Sized Roadmap

### Track A: Process Scanning

1. `[*] Add process memory scan report schema`
2. `[*] Show process scan status in privacy reports`
3. `[*] Add Windows process memory scan prototype`
4. `[*] Scan live runtime for prompt markers`
5. `[*] Scan post-shutdown runtime memory for prompt markers`
6. `[*] Add response marker scanning and report comparison`

### Track B: llama Allocator / KV Introspection

7. `[*] Document llama allocator and KV introspection plan`
8. `[*] Add runtime capability flags for instrumented llama builds`
9. `[*] Expose KV cache lifecycle signals in reports`
10. `[*] Expose allocator reset signals in reports`

### Track C: CUDA / NVIDIA API Inspection

11. `[*] Add GPU inspection backend abstraction`
12. `[*] Add CUDA or NVML inspection spike implementation`
13. `[*] Report allocation visibility separately from PID visibility`
14. `[*] Improve Windows VRAM post-shutdown evidence reporting`

### Track D: Experimental Sanitization

15. `[*] Add VRAM cleanup strategy model`
16. `[ ] Add baseline versus strategy comparison reporting`
17. `[ ] Implement first experimental VRAM cleanup strategy`
18. `[ ] Report VRAM cleanup outcome evidence`

### Track E: Validation and Release Gating

19. `[ ] Add memory inspection validation harness`
20. `[ ] Document platform security capability matrix`
21. `[ ] Freeze security claim wording for v1`

---

## V1 Gates for This Security Program

### Must Be True Before v1

- At least one platform has real process memory scanning.
- NullContext can report whether prompt markers were found in scanned process memory.
- NullContext has some allocator/KV evidence, even if partial.
- NullContext has better-than-`nvidia-smi`-only GPU inspection on Windows/NVIDIA.
- NullContext has at least one experimental VRAM cleanup strategy with measured outcomes.
- Reports and UI clearly distinguish:
  - observed cleared
  - not found
  - not scannable
  - visibility limited
  - unsupported
  - inconclusive

### Does Not Have To Be True Before v1

- perfect forensic-grade RAM scan coverage
- universal cross-platform parity
- full VRAM sanitization proof
- deep allocator introspection on every platform

---

## Recommended Starting Point

Start with:

### Track A, Commit 1

`Add process memory scan report schema`

Then immediately:

### Track A, Commit 2

`Show process scan status in privacy reports`

Then:

### Track A, Commit 3

`Add Windows process memory scan prototype`

This sequence builds evidence before claims and creates the foundation for the deeper allocator and VRAM work.
