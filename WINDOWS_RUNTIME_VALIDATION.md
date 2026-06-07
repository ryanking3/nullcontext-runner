# Windows Runtime Validation Checklist

This file tracks the Windows-specific validation and follow-up work that should happen when development resumes on the Windows/NVIDIA machine.

## Immediate validation goals

1. Validate the direct process-scan prototype against a real `llama-server.exe` PID.
2. Validate current RAM and VRAM report fields against live host tooling.
3. Tighten any misleading or weak report wording based on what the Windows machine actually exposes.

## Before running NullContext

- Confirm `C:\\Users\\<you>\\.nullcontext\\config.toml` points at the real Windows `llama-server.exe`.
- Confirm the configured model path exists.
- Confirm `gpu_layers` reflects the intended GPU offload mode.
- Confirm `nvidia-smi` is available in the shell used to launch NullContext.

## Validation commands

### PowerShell process checks

```powershell
Get-Process -Id <pid>
Get-CimInstance Win32_Process -Filter "ProcessId = <pid>"
```

What to compare:

- working set vs reported RSS/resident memory
- virtual memory fields
- pagefile/private memory fields
- whether the PID remains visible after shutdown

### NVIDIA checks

```powershell
nvidia-smi
nvidia-smi --query-compute-apps=pid,used_gpu_memory --format=csv,noheader,nounits
nvidia-smi pmon -c 1
```

What to compare:

- whether `compute-apps` shows the `llama-server.exe` PID
- whether VRAM bytes are reported for that PID
- whether `pmon` sees the PID when `compute-apps` does not
- whether the PID remains observable after shutdown

## Direct process-scan checks

Current prototype scope:

- Windows only
- one-shot CLI and one-shot streamed web path
- scans live runtime memory
- scans failed-start cleanup memory when the PID is still observable
- scans post-shutdown memory only when the PID is still observable

Things to validate:

1. A normal one-shot run produces a `process_scan` section in the privacy report.
2. The `live_runtime` phase records:
   - regions scanned
   - regions skipped
   - bytes scanned
   - marker results
3. The `post_shutdown` phase behaves correctly when:
   - the PID disappears
   - the PID remains observable
4. Startup-failure reports show a `failed_start_cleanup` process-scan phase.
5. Marker statuses are reasonable:
   - `detected_in_scanned_memory`
   - `not_detected_in_scanned_regions`
   - `process_not_observable_for_scan`
   - `scan_attempt_incomplete`
   - `scan_attempt_failed`

## Manual scenarios to run

### Scenario 1: normal one-shot run

- Start NullContext server
- Run one-shot inference
- Capture the privacy report
- Compare:
  - process scan status
  - RAM snapshot values
  - GPU visibility values
  - post-shutdown PID visibility

### Scenario 2: one-shot with GPU offload

- Ensure a GPU-offloaded model is selected
- Run one-shot inference
- Compare report GPU fields to `nvidia-smi` output
- Record whether WDDM visibility limits show up in practice

### Scenario 3: induced startup failure

- Temporarily break the model path or runtime launch condition
- Trigger a failed startup
- Confirm:
  - failed-start cleanup report exists
  - process scan section exists
  - `failed_start_cleanup` phase is populated honestly

## Follow-up items likely to matter

- tighten Windows region filtering if too many unusable regions are scanned
- improve notes when `ReadProcessMemory` returns partial progress
- decide whether empty markers should be omitted or shown explicitly
- validate whether response markers are too large/noisy and should be truncated or replaced with synthetic canaries
- decide whether active chat end should reuse the same direct-scan backend next
- check if additional process rights are needed on some Windows setups

## Nice-to-have captures

- save one good report JSON from:
  - successful one-shot
  - GPU-offloaded one-shot
  - failed startup
- keep a short note on:
  - Windows version
  - NVIDIA driver version
  - GPU model
  - whether WDDM or other tooling limitations were observed
