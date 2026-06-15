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

That is strong progress.

It is not yet enough to call the security program complete.

---

## What Still Blocks v1

The main remaining blockers are:

1. Track B still needs deeper allocator / KV introspection.
2. Track C still needs stronger CUDA / NVIDIA API-level truth.
3. Track D and Track E still need repeated-run aggregation that tells us which cleanup stages actually help.
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

### Remaining v1 Work

- aggregate cleanup-stage scan outcomes across runs, not just per report
- make stage-level evidence easier to compare over time
- reduce places where fallback session-wide scan context is still used instead of truly stage-local evidence
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

Track B is not done.
It has structure, but not enough depth yet.
This remains one of the biggest true v1 blockers.

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
- explicit repeated controlled-canary history reporting

### Remaining v1 Work

- define what counts as a meaningful pass versus weak/inconclusive evidence
- tighten recommendation semantics so “best stage” is clearly separated from “clean stage”
- freeze v1 security claim wording

### v1 Exit Criteria

- validation is based on repeated results, not only single reports
- the UI can show which cleanup stages have the best repeated evidence
- the final wording for v1 claims is fixed and conservative

### Honest Status

Track E is advanced, but not finished.
This is the track that turns the other work into a shippable v1 security story.

---

## True Remaining Work, In Order

This is the order that best fits the actual blocker stack.

### 1. Finish Track E repeated-results aggregation

Must do:

- keep the existing cleanup-stage outcome history and recommendation path honest as more repeated runs accumulate
- define stronger release-gating thresholds for:
  - enough repeated runs
  - mixed versus acceptable evidence
  - marker persistence versus recommendation eligibility
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

## Bottom Line

The remaining v1 blocker stack is:

1. repeated cleanup-stage and helper-stage history aggregation
2. deeper allocator / KV introspection
3. deeper CUDA / NVIDIA inspection truth
4. frozen conservative v1 claim wording

That is the real pre-v1 roadmap from here.
