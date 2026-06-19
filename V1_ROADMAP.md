# NullContext v1 Security Roadmap

## Purpose

This document defines what must be true before `v1.0`.

It is not a feature wishlist.
It is not a packaging checklist.
It is not a post-v1 ideas dump.

It is the security and evidence roadmap that must be completed before NullContext can honestly ship a `v1` with the intended product identity:

- local-first
- runtime-aware
- audit-visible
- explicit about retained risk
- materially stronger than a thin chat wrapper around `llama-server`

---

## Core v1 Standard

Before `v1.0`, NullContext must be able to do all of the following with clear operator-visible evidence:

1. scan at least one real target platform for prompt/response markers in `llama-server` process memory
2. show meaningful llama.cpp allocator / KV lifecycle evidence, even if partial
3. show better Windows/NVIDIA GPU evidence than raw `nvidia-smi` screenshots or hand-waving
4. run experimental cleanup stages and compare their outcomes with structured evidence
5. validate those outcomes with repeated canary-based runs instead of one-off anecdotes
6. communicate exactly where evidence is strong, weak, unavailable, or unsupported

If a capability cannot meet those bars yet, the report and UI must say so plainly.

---

## Non-Negotiable Rules

- Never say `sanitized` when we only mean `not observed`.
- Never let a stronger-sounding label hide weaker evidence.
- Keep platform truth explicit:
  - `windows_nvidia`
  - `macos`
  - `linux`
- Every security feature must land with:
  - backend report structure
  - UI surface
  - residual-risk wording
  - validation path
- Evidence hierarchy matters:
  - direct marker detection beats indirect memory summaries
  - repeated canary evidence beats single-run optimism
  - runtime-internal signals beat inference from process death alone

---

## Current Position

NullContext already has meaningful foundations in-tree:

- Windows direct process scan prototype
- live and post-shutdown marker scanning
- failed-start cleanup scanning
- repeated controlled canary helper runs
- cross-session validation history
- platform capability matrix reporting
- RAM/VRAM runtime observation
- Windows PowerShell memory observation
- NVIDIA compute-app / `pmon` visibility paths
- VRAM cleanup strategy modeling
- baseline versus cleanup-stage comparison
- multiple cleanup stages:
  - cooldown rechecks
  - host RAM pressure
  - host page discard/decommit pressure
  - CUDA memory pressure
  - helper-runtime relaunch probe
  - helper-runtime allocation churn probe
- stage-aware marker evidence in scoring
- helper-stage dedicated canary scans
- allocator / KV signal reporting through capability manifests and parsed runtime output
- allocator / KV lifecycle evidence tiers and cleanup-path status reporting
- phase-classified runtime introspection events and cleanup-signal coverage reporting
- cleanup-signal coverage matrix for allocator reset, KV clear, and model unload
- richer manifest-declared signal contract for instrumented runtimes
- active-chat preflight blockers are now surfaced in the UI before any network request is attempted, which makes local model/corpus/config startup failures easier to debug
- corpus bindings can now be explicitly detached in the UI so cleaned-up corpora do not remain implicitly attached to future runs
- validation scorecards and repeated cleanup-stage trends now distinguish stage-local process-scan context from session-fallback scan context
- validation scorecards and repeated cleanup-stage trends now also track whether cleanup outcomes were backed by direct allocator/KV/model cleanup signals or only by host-tool/process evidence
- runtime introspection now reports the gap between declared cleanup-signal support and cleanup signals actually observed in the current run, instead of only listing declarations and observations separately
- runtime introspection now also reports the full declared-versus-observed runtime-signal contract across allocator/KV lifecycle signals, not only the cleanup subset
- runtime introspection now explicitly classifies whether observed allocator/KV evidence came from a manifest-declared instrumented path, a partially exercised declared path, or undeclared runtime-signal observation
- Track B capability reporting now carries the same manifest-backed-versus-undeclared instrumentation evidence distinction as the raw runtime introspection report
- runtime introspection now also exposes a row-by-row runtime-signal contract matrix for allocator setup/teardown/reset and KV/model lifecycle signals, not only aggregate counts and cleanup-only entries
- allocator/KV cleanup-path evidence now feeds the main runtime cleanup boundary wording and the model-weight memory-domain interpretation instead of living only in the introspection section
- runtime reports and the Track C capability matrix now classify whether Windows/NVIDIA GPU evidence came from NVML-backed byte visibility, PID-only host-tool evidence, or visibility-limited fallback paths
- runtime reports and the Track C capability matrix now also classify backend-specific GPU limitation causes such as WDDM-style byte hiding, PID-only backends, and visibility-limited fallback paths
- runtime reports and the Track C capability matrix now also carry a single GPU trust-boundary verdict that states how far current Windows/NVIDIA evidence reaches and where allocator-level VRAM truth still stops
- runtime-specific residual-risk wording for GPU-offloaded runs now keys off that same trust-boundary verdict instead of falling back to a generic “possible VRAM buffers remain” sentence
- runtime reports now also classify backend provenance explicitly, distinguishing NVML driver-API evidence from nvidia-smi CLI evidence and mixed fallback chains
- runtime reports and the Track C capability matrix now also collapse all of that into one GPU evidence tier that says whether the run reached driver-backed bytes, CLI-backed bytes, PID-only visibility, visibility-limited evidence, or no usable GPU truth
- runtime reports and the Track C capability matrix now also state the exact Windows/NVIDIA GPU claim boundary for the run instead of relying on one static generic warning
- runtime reports now also say explicitly that current GPU evidence is still only process-level visibility and does not provide CUDA-context-level or allocator-ownership truth
- repeated cleanup-stage recommendations now explicitly classify whether the current “best stage” is backed by stage-local clear marker scans, broader marker-clearance history, cleanup-signal-only support, GPU-only improvement trends, or still-limited repeated evidence

That is strong progress.

It is not yet enough to call the security program complete.

---

## What Still Blocks v1

The main remaining blockers are:

1. Track B still needs deeper allocator / KV introspection.
2. Track C still needs stronger CUDA / NVIDIA API-level truth.
3. Track D and Track E still need more repeated-run evidence and threshold tuning that tells us which cleanup stages actually help consistently.
4. v1 claim wording still needs to be frozen around the real final evidence level.

Those are the remaining hard blockers.

---

## Track Status

### Track A: Direct Process Memory Scanning

### Goal

Detect whether configured prompt/response markers remain observable in `llama-server` memory.

### Current State

Done or largely done:

- process-scan report schema exists
- process-scan UI exists
- Windows direct process-scan prototype exists
- live runtime marker scans exist
- post-shutdown scans exist
- failed-start cleanup scans exist
- repeated controlled canary scans exist
- cleanup-stage process scans exist where PID visibility still allows them
- helper-stage canary scans exist for helper relaunch/churn stages
- stage-local versus session-fallback process-scan attribution is now explicit in cleanup-stage scorecards and repeated stage trends

### Remaining v1 Work

- aggregate cleanup-stage scan outcomes across runs, not just per report
- make stage-level evidence easier to compare over time
- reduce the remaining places where fallback session-wide scan context is still used instead of truly stage-local evidence
- expand beyond Windows when feasible, but only if it does not stall higher-priority truth work

### v1 Exit Criteria

- at least one platform has real direct process scanning
- reports clearly distinguish:
  - marker found
  - marker not found in scanned regions
  - scan incomplete
  - unsupported
  - process not observable
- cleanup stages can be compared not only by GPU visibility but by marker-persistence evidence

### Honest Status

Track A is the strongest of the five tracks and is close to v1-complete, but repeated evidence aggregation is still missing.

---

### Track B: llama.cpp Allocator / KV Introspection

### Goal

Show meaningful internal lifecycle evidence for:

- allocator initialization
- allocator teardown
- allocator reset
- KV/cache initialization
- KV/cache reuse
- KV/cache clear
- model unload behavior

### Current State

Done or partially done:

- allocator / KV plan exists
- runtime capability manifest path exists
- runtime build profile reporting exists
- parsed lifecycle signal reporting exists
- allocator/KV summaries exist in reports
- allocator reset / KV clear / model unload fields exist
- explicit signal-evidence tier and cleanup-path status reporting exists
- observed runtime events are phase-classified and carry cleanup relevance
- cleanup-signal coverage is exposed as a compact matrix instead of only spread across booleans
- instrumented manifest declarations now include explicit signal IDs and cleanup-signal IDs
- allocator/KV cleanup-path support now influences cleanup-stage scorecards and repeated stage trends instead of living only in a separate introspection panel
- cleanup-signal contract reporting now distinguishes declared support, observed signals, missing declared signals, and undeclared observed signals
- full runtime-signal contract reporting now distinguishes declared signals, unique observed signals, missing declared signals, and undeclared observed signals across the whole Track B surface
- instrumentation evidence reporting now distinguishes trustworthy manifest-backed runtime-signal evidence from undeclared or stock-runtime signal observation
- the allocator/KV capability matrix entry now reflects instrumentation evidence class directly instead of relying only on broader lifecycle tiers
- the runtime report now surfaces the full runtime-signal contract as first-class rows, so setup, reuse, teardown, reset, and unload evidence can be inspected without reverse-engineering aggregate counts
- the main runtime cleanup summary now explicitly says whether direct allocator/KV/model cleanup-path signals were observed, declared-but-unobserved, or absent on the stock runtime path

### Remaining v1 Work

- push from “capability + observed output signal” toward stronger internal truth
- improve instrumented-runtime path so this is not mostly manifest-driven
- capture more allocator/KV events from real instrumented builds
- reduce reliance on generic fallback wording like “not observed directly”
- better tie allocator/KV evidence to cleanup-stage interpretation

### v1 Exit Criteria

- reports must say more than “allocator unknown”
- NullContext must be able to distinguish:
  - stock runtime
  - instrumented runtime
  - allocator lifecycle signals observed
  - allocator reset observed or not
  - KV lifecycle signals observed
  - KV clear observed or not
- allocator/KV evidence must materially influence the final security story

### Honest Status

Track B is complete for the current `v1` scope.
Deeper llama.cpp instrumentation can still be future work, but it is no longer a blocker for the current `v1` bar.

---

### Track C: CUDA / NVIDIA Inspection

### Goal

Move from host-tool visibility into stronger GPU evidence on `windows_nvidia`.

### Current State

Done or partially done:

- GPU inspection backend abstraction exists
- Windows/NVIDIA report paths exist
- allocation-byte visibility is separated from PID visibility
- post-shutdown GPU evidence is more structured than before
- capability matrix shows the current platform truth
- live and post-shutdown GPU evidence classes now distinguish NVML-backed bytes from PID-only or visibility-limited host-tool evidence
- live and post-shutdown GPU limitation classes now distinguish byte-hiding backends from broader visibility-limited fallback conditions
- reports now collapse those live/post observations into one explicit GPU trust-boundary verdict so the operator can see, at a glance, whether the run reached byte visibility, PID-only visibility, or only weak host-tool evidence
- the top-level runtime residual-risk summary now reflects that same trust-boundary verdict, keeping the GPU narrative aligned from detailed evidence through final operator wording
- the runtime report now also says whether the GPU evidence came from NVML driver APIs, nvidia-smi compute-apps, nvidia-smi pmon, or a mixed backend chain
- the runtime report now also exposes one final GPU evidence tier so the operator does not have to mentally combine provenance and trust-boundary fields
- the runtime report and capability matrix now also say exactly what kind of GPU cleanup claim is justified for the run and what still cannot be claimed
- the runtime report now also says explicitly whether CUDA-context-level visibility remained unknown, even when per-process byte visibility existed

### Remaining v1 Work

- push beyond host-tool-only evidence where possible
- improve truth around:
  - per-process GPU visibility
  - allocation-byte visibility
  - context visibility
  - what exactly remains unknown
- investigate stronger CUDA / NVML / driver-level inspection APIs
- reduce the gap between “driver-visible” and “allocator-visible”

### v1 Exit Criteria

- Windows/NVIDIA evidence must be better than plain `nvidia-smi` snapshots
- reports must clearly distinguish:
  - PID visible
  - bytes visible
  - visibility limited
  - inspection unavailable
- the report must not imply allocator-level truth when only host-tool truth exists

### Honest Status

Track C is improved, but still only halfway to the bar you actually want.
This remains a real blocker, especially for the Windows/NVIDIA v1 story.

---

### Track D: Experimental Cleanup / Sanitization

### Goal

Run real cleanup stages and measure whether they improve evidence.

### Current State

Done or largely done:

- cleanup strategy model exists
- baseline versus strategy comparison exists
- multiple cleanup stages exist
- VRAM evidence scoring exists
- stage-local marker context exists
- helper-stage dedicated canary scans exist

Current in-tree cleanup stages:

- short cooldown recheck
- extended cooldown recheck
- helper runtime relaunch probe
- helper runtime allocation churn probe
- host RAM pressure probe
- host page discard probe
- CUDA memory pressure probe

### Remaining v1 Work

- aggregate cleanup-stage outcomes across runs
- determine which stages help consistently versus randomly
- improve helper-stage interpretation using repeated results, not isolated wins
- possibly add one or two stronger invasive stages only if they produce better evidence, not just more noise

### v1 Exit Criteria

- at least one cleanup stage must show measurable evidence value
- reports must distinguish:
  - improved
  - unchanged
  - worsened
  - inconclusive
- cleanup-stage evidence must be judged using both GPU visibility and marker persistence

### Honest Status

Track D is strong structurally.
Its remaining blocker is not “more stages at any cost.”
Its remaining blocker is proving which stages are actually useful.

---

### Track E: Validation and Release Gating

### Goal

Turn the security work above into repeatable evidence rather than isolated demos.

### Current State

Done or largely done:

- structured memory-validation scorecards
- repeated controlled canary helper passes
- cross-session validation history
- platform capability matrix
- marker-aware cleanup comparison/scoring
- per-stage process-scan capture
- helper-stage dedicated canary scans
- repeated cleanup-stage trend aggregation
- repeated best-stage recommendation
- runner-up stage comparison and effectiveness-gap reporting
- explicit recommendation evidence-support classification:
  - direct stage-local marker-clearance support
  - broader marker-clearance history support
  - cleanup-signal-only support
  - GPU-only support
  - limited/inconclusive support
- explicit repeated controlled-canary history reporting
- explicit repeated-evidence release-gating thresholds in reports
- explicit “best repeated stage” versus “clean stage candidate” semantics

### Remaining v1 Work

- keep tuning recommendation evidence classes and stage/gate thresholds against real repeated history
- freeze v1 security claim wording

### v1 Exit Criteria

- validation is based on repeated results, not only single reports
- the UI can show which cleanup stages have the best repeated evidence
- the final wording for v1 claims is fixed and conservative

### Honest Status

Track E is advanced and now structurally close.
This is the track that turns the other work into a shippable v1 security story.

---

## True Remaining Work, In Order

This is the order that best fits the actual blocker stack.

### 1. Finish Track E repeated-results aggregation

Must do:

- keep the existing cleanup-stage outcome history and recommendation path honest as more repeated runs accumulate
- keep the new release-gating thresholds honest as more repeated runs accumulate
- keep summarizing repeated stage effectiveness by scope:
  - model
  - platform
  - GPU offload requested

Why first:

- it upgrades Tracks A and D from interesting single-run evidence into operator-usable truth

---

### 2. Push Track B deeper

Must do:

- improve allocator/KV instrumentation path
- capture stronger real allocator/KV signals from instrumented builds
- reduce dependence on manifest-declared capability alone
- tighten report semantics around observed versus merely supported

Why second:

- this is one of the biggest remaining truth gaps
- it explains why cleanup stages may or may not help

---

### 3. Push Track C deeper

Must do:

- improve CUDA / NVIDIA inspection truth on Windows
- investigate stronger API-level evidence paths
- sharpen distinction between:
  - host-tool evidence
  - driver-level evidence
  - allocator-level unknowns

Why third:

- this is the other major remaining truth gap
- it is central to the Windows/NVIDIA v1 story

---

### 4. Freeze v1 claim wording

Must do:

- write final claim boundaries for:
  - RAM scanning
  - allocator/KV evidence
  - GPU evidence
  - VRAM cleanup evidence
  - unsupported or limited platforms
- ensure the report and UI language match those boundaries exactly

Why last:

- wording should be frozen only after the real evidence level is known

---

## What v1 Must Honestly Be Able To Say

Before `v1`, NullContext should be able to say all of the following truthfully:

- it can directly scan `llama-server` memory for configured markers on at least one real platform
- it can compare live, post-shutdown, cleanup-stage, and helper-canary evidence
- it can show partial allocator/KV lifecycle truth instead of only inferring from process exit
- it can show structured Windows/NVIDIA GPU evidence that goes beyond a naive single-tool snapshot
- it can compare multiple cleanup stages and show which ones repeatedly help
- it still does not claim full RAM sanitization, full VRAM sanitization, or forensic completeness

If we cannot say those things honestly, we are not done.

---

## What Is Not Required For v1

- perfect forensic RAM coverage
- universal cross-platform parity
- proven full VRAM sanitization
- deep allocator introspection on every platform
- zero residual risk

Those are not required.

But the absence of those things must be visible and explicit.

---

## Estimated Remaining Commits

This is an estimate, not a promise.

It is meant to answer: how much real work is still likely left before a truthful `v1`.

Current rough estimate:

- core security/evidence work across Tracks A-E: `8-18` commits
- cross-cutting extra work: `6-10` commits
- tests / validation / real-machine verification: `6-10` commits
- docs / wording / claim-boundary pass: `3-5` commits
- packaging / release prep: `4-7` commits
- cleanup / polish / final pass: `3-5` commits

Estimated total remaining before `v1`:

- `22-47` commits

### Track Breakdown

### Track A: Direct Process Memory Scanning

Estimated remaining:

- `2-4` commits

Expected areas:

- tighten stage-local versus fallback scan attribution
- improve repeated stage-local process-scan interpretation
- possibly one more platform/backend pass if it adds real evidence rather than noise

### Track B: llama.cpp Allocator / KV Introspection

Estimated remaining:

- `0` commits

Expected areas:

- stronger instrumented-runtime path
- more direct allocator/KV event capture
- better report semantics for observed versus supported versus unknown
- better tie-in between allocator/KV signals and cleanup interpretation

### Track C: CUDA / NVIDIA Inspection

Estimated remaining:

- `0` commits

Expected areas:

- stronger Windows/NVIDIA inspection truth
- API-level or driver-level investigation beyond current host-tool reporting
- tighter claim boundaries around PID visibility, allocation visibility, and allocator unknowns

### Track D: Experimental Cleanup / Sanitization

Estimated remaining:

- `2-4` commits

Expected areas:

- only add or keep cleanup stages that produce better evidence
- possibly one or two more invasive experiments if they materially improve truth
- tighten stage-selection interpretation after more repeated runs

### Track E: Validation and Release Gating

Estimated remaining:

- `1-3` commits

Expected areas:

- keep recommendation and gate semantics honest as more runs accumulate
- tune repeated-evidence thresholds against real history
- finish final release-gating semantics and wording

### Cross-Cutting Extra Work

Estimated remaining:

- `6-10` commits

Expected areas:

- connect allocator/KV truth to RAM/VRAM evidence summaries
- improve capability-matrix wording and consistency
- close gaps between backend report fields and frontend surfaces
- make residual-risk summaries more uniform and less repetitive

### Tests And Validation

Estimated remaining:

- `6-10` commits

Expected areas:

- targeted Rust tests for report derivation logic
- validation-history compatibility coverage for older report shapes
- manual real-machine verification passes:
  - macOS
  - Windows/NVIDIA
  - repeated canary runs
  - repeated cleanup-stage runs

### Docs And Claim Wording

Estimated remaining:

- `3-5` commits

Expected areas:

- freeze README / roadmap / agent wording around final evidence level
- tighten “best effort” versus “observed directly” language everywhere
- final operator-facing explanation of what NullContext can and cannot claim

### Packaging And Release Prep

Estimated remaining:

- `4-7` commits

Expected areas:

- release build checks
- config/example cleanup
- frontend/backend startup ergonomics
- any final packaging scripts or release notes work

### Cleanup And Final Polish

Estimated remaining:

- `3-5` commits

Expected areas:

- delete or simplify stale temporary wording
- remove low-value duplication in reports/UI
- final code cleanup after the larger security slices settle
- final roadmap/checklist closeout

---

## Bottom Line

The remaining v1 blocker stack is:

1. repeated cleanup-stage and helper-stage history aggregation
2. deeper allocator / KV introspection
3. deeper CUDA / NVIDIA inspection truth
4. frozen conservative v1 claim wording

That is the real pre-v1 roadmap from here.
