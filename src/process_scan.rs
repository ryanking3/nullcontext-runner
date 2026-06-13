use crate::audit::{ProcessScanPatternReport, ProcessScanPhaseReport, ProcessScanReport};
use crate::runtime::RuntimePostShutdownObservation;

const PLANNED_PLATFORMS: [&str; 3] = ["windows", "linux", "macos"];

pub struct ProcessScanMarker<'a> {
    pub kind: &'a str,
    pub bytes: &'a [u8],
}

pub fn build_skipped_process_scan_report(
    runtime_pid: Option<u32>,
    summary: &str,
    residual_risk_summary: &str,
    notes: Vec<String>,
) -> ProcessScanReport {
    ProcessScanReport {
        overall_status: "scan_skipped".to_string(),
        implementation_status: if cfg!(target_os = "windows") {
            "windows_direct_process_scan_prototype".to_string()
        } else {
            "direct_process_scan_not_implemented_on_platform".to_string()
        },
        platform: std::env::consts::OS.to_string(),
        target_process_kind: "llama-server".to_string(),
        target_runtime_pid: runtime_pid,
        planned_platforms: PLANNED_PLATFORMS
            .iter()
            .map(|value| value.to_string())
            .collect(),
        summary: summary.to_string(),
        residual_risk_summary: residual_risk_summary.to_string(),
        phases: vec![],
        notes,
    }
}

pub fn build_process_scan_report(
    runtime_pid: Option<u32>,
    phases: Vec<ProcessScanPhaseReport>,
) -> ProcessScanReport {
    let platform = std::env::consts::OS.to_string();
    let implementation_status = if cfg!(target_os = "windows") {
        "windows_direct_process_scan_prototype".to_string()
    } else {
        "direct_process_scan_not_implemented_on_platform".to_string()
    };

    let saw_completed_scan = phases.iter().any(|phase| phase.status == "scan_completed");
    let saw_detected_marker = phases.iter().any(phase_has_detected_marker);
    let saw_scan_failure = phases.iter().any(|phase| {
        phase.status == "scan_attempt_failed" || phase.status == "scan_attempt_incomplete"
    });
    let saw_unsupported = phases
        .iter()
        .any(|phase| phase.status == "scan_backend_unsupported_on_platform");

    let overall_status = if saw_detected_marker {
        "markers_detected_in_scanned_memory".to_string()
    } else if saw_completed_scan {
        "no_markers_detected_in_scanned_regions".to_string()
    } else if saw_scan_failure {
        "scan_attempt_failed".to_string()
    } else if saw_unsupported {
        "scan_backend_unsupported_on_platform".to_string()
    } else {
        "scan_not_completed".to_string()
    };

    let summary = if saw_detected_marker {
        "NullContext found one or more configured marker patterns in scanned llama-server process memory. This is meaningful evidence that sensitive content remained observable in at least some readable regions."
            .to_string()
    } else if saw_completed_scan {
        "NullContext scanned readable llama-server process regions for configured marker patterns and did not find them in the scanned regions. This is useful evidence, but it does not prove full process-memory sanitization."
            .to_string()
    } else if saw_scan_failure {
        "NullContext attempted direct process-memory scanning, but the scan did not complete cleanly enough to support a strong marker-presence conclusion."
            .to_string()
    } else if saw_unsupported {
        "This report used a direct process-scan schema, but the current platform build does not implement direct llama-server memory scanning yet."
            .to_string()
    } else {
        "NullContext reserved a direct process-scan section for this report, but no conclusive process-memory scan ran."
            .to_string()
    };

    let residual_risk_summary = if saw_detected_marker {
        "Marker detection in readable process memory means prompt or response material may still have been observable in the external llama-server process at scan time."
            .to_string()
    } else if saw_completed_scan {
        "A clean marker miss only speaks to the configured patterns and scanned readable regions. It does not rule out persistence elsewhere in process memory, allocator slack, swapped pages, or unscanned regions."
            .to_string()
    } else {
        "Without a completed direct process-memory scan, NullContext cannot say whether prompt or response markers remained present in readable llama-server memory."
            .to_string()
    };

    let mut notes = vec![];

    if cfg!(target_os = "windows") {
        notes.push(
            "The current direct-scan prototype targets Windows first via VirtualQueryEx and ReadProcessMemory."
                .to_string(),
        );
    } else {
        notes.push(
            "This build still relies on host-tool RAM/VRAM observation outside Windows; direct process scanning is planned for later platform slices."
                .to_string(),
        );
    }

    ProcessScanReport {
        overall_status,
        implementation_status,
        platform,
        target_process_kind: "llama-server".to_string(),
        target_runtime_pid: runtime_pid,
        planned_platforms: PLANNED_PLATFORMS
            .iter()
            .map(|value| value.to_string())
            .collect(),
        summary,
        residual_risk_summary,
        phases,
        notes,
    }
}

pub fn scan_live_process_phase(
    pid: u32,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    scan_process_phase("live_runtime", pid, markers)
}

pub fn scan_post_shutdown_process_phase(
    pid: u32,
    post_shutdown: &RuntimePostShutdownObservation,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    scan_process_phase_with_presence(
        "post_shutdown",
        pid,
        post_shutdown.process_present_after_shutdown,
        markers,
    )
}

pub fn scan_failed_start_cleanup_phase(
    pid: u32,
    post_shutdown: &RuntimePostShutdownObservation,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    scan_process_phase_with_presence(
        "failed_start_cleanup",
        pid,
        post_shutdown.process_present_after_shutdown,
        markers,
    )
}

pub fn scan_process_phase_with_presence(
    phase_name: &str,
    pid: u32,
    process_present: Option<bool>,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    match process_present {
        Some(true) => scan_process_phase(phase_name, pid, markers),
        Some(false) => ProcessScanPhaseReport {
            phase: phase_name.to_string(),
            status: "process_not_observable_for_scan".to_string(),
            method: "not_applicable_process_not_observed".to_string(),
            target_pid: Some(pid),
            scope_summary:
                "The runtime PID was not observed after shutdown, so there was no target process left for direct post-shutdown scanning."
                    .to_string(),
            bytes_scanned: None,
            regions_scanned: None,
            regions_skipped: None,
            patterns: markers
                .iter()
                .map(|marker| ProcessScanPatternReport {
                    pattern_kind: marker.kind.to_string(),
                    status: "process_not_observable_for_scan".to_string(),
                    matches_found: None,
                    notes:
                        "No direct post-shutdown scan ran because the runtime PID was no longer observable."
                            .to_string(),
                })
                .collect(),
            notes: vec![
                format!(
                    "NullContext could not perform a direct {} process scan because the PID was not observable after cleanup.",
                    phase_name.replace('_', " ")
                ),
            ],
        },
        None => ProcessScanPhaseReport {
            phase: phase_name.to_string(),
            status: "post_shutdown_observation_inconclusive".to_string(),
            method: "not_attempted_inconclusive_target_state".to_string(),
            target_pid: Some(pid),
            scope_summary:
                "The runtime PID could not be conclusively classified as present or absent after shutdown, so a direct post-shutdown scan was not attempted."
                    .to_string(),
            bytes_scanned: None,
            regions_scanned: None,
            regions_skipped: None,
            patterns: markers
                .iter()
                .map(|marker| ProcessScanPatternReport {
                    pattern_kind: marker.kind.to_string(),
                    status: "post_shutdown_observation_inconclusive".to_string(),
                    matches_found: None,
                    notes:
                        "No direct post-shutdown scan ran because PID visibility after shutdown was inconclusive."
                            .to_string(),
                })
                .collect(),
            notes: vec![
                format!(
                    "NullContext skipped direct {} scanning because PID visibility after cleanup was inconclusive.",
                    phase_name.replace('_', " ")
                ),
            ],
        },
    }
}

fn phase_has_detected_marker(phase: &ProcessScanPhaseReport) -> bool {
    phase
        .patterns
        .iter()
        .any(|pattern| pattern.status == "detected_in_scanned_memory")
}

#[cfg(target_os = "windows")]
fn scan_process_phase(
    phase: &str,
    pid: u32,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    scan_process_phase_windows(phase, pid, markers)
}

#[cfg(not(target_os = "windows"))]
fn scan_process_phase(
    phase: &str,
    pid: u32,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    ProcessScanPhaseReport {
        phase: phase.to_string(),
        status: "scan_backend_unsupported_on_platform".to_string(),
        method: "not_implemented_for_current_platform".to_string(),
        target_pid: Some(pid),
        scope_summary:
            "This build does not implement direct llama-server process scanning on the current platform yet."
                .to_string(),
        bytes_scanned: None,
        regions_scanned: None,
        regions_skipped: None,
        patterns: markers
            .iter()
            .map(|marker| ProcessScanPatternReport {
                pattern_kind: marker.kind.to_string(),
                status: if marker.bytes.is_empty() {
                    "pattern_empty".to_string()
                } else {
                    "scan_backend_unsupported_on_platform".to_string()
                },
                matches_found: None,
                notes: if marker.bytes.is_empty() {
                    "The configured marker bytes were empty, so no pattern search was attempted."
                        .to_string()
                } else {
                    "No direct process-memory scan ran on this platform build.".to_string()
                },
            })
            .collect(),
        notes: vec![
            format!(
                "Direct process-memory scanning is currently only prototyped for Windows. Current platform: {}.",
                std::env::consts::OS
            ),
        ],
    }
}

#[cfg(target_os = "windows")]
fn scan_process_phase_windows(
    phase: &str,
    pid: u32,
    markers: &[ProcessScanMarker<'_>],
) -> ProcessScanPhaseReport {
    use std::ffi::c_void;
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Memory::{VirtualQueryEx, MEMORY_BASIC_INFORMATION};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    const CHUNK_SIZE: usize = 64 * 1024;

    let process_handle =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };

    if process_handle == 0 {
        return ProcessScanPhaseReport {
            phase: phase.to_string(),
            status: "scan_attempt_failed".to_string(),
            method: "win32_openprocess_virtualqueryex_readprocessmemory".to_string(),
            target_pid: Some(pid),
            scope_summary:
                "OpenProcess failed, so NullContext could not inspect readable regions of the target process."
                    .to_string(),
            bytes_scanned: None,
            regions_scanned: None,
            regions_skipped: None,
            patterns: markers
                .iter()
                .map(|marker| ProcessScanPatternReport {
                    pattern_kind: marker.kind.to_string(),
                    status: "scan_attempt_failed".to_string(),
                    matches_found: None,
                    notes:
                        "Opening the target process for memory reads failed before any marker scan could run."
                            .to_string(),
                })
                .collect(),
            notes: vec![
                "OpenProcess with PROCESS_QUERY_INFORMATION | PROCESS_VM_READ returned a null handle."
                    .to_string(),
            ],
        };
    }

    let mut notes = vec![];
    let max_pattern_len = markers
        .iter()
        .map(|marker| marker.bytes.len())
        .max()
        .unwrap_or(0);
    let mut matches_found = vec![0_u64; markers.len()];
    let mut bytes_scanned = 0_u64;
    let mut regions_scanned = 0_u64;
    let mut regions_skipped = 0_u64;
    let mut partial_reads = 0_u64;

    let mut address = 0usize;

    loop {
        let mut info = MEMORY_BASIC_INFORMATION {
            BaseAddress: std::ptr::null_mut(),
            AllocationBase: std::ptr::null_mut(),
            AllocationProtect: 0,
            RegionSize: 0,
            State: 0,
            Protect: 0,
            Type: 0,
        };

        let queried = unsafe {
            VirtualQueryEx(
                process_handle,
                address as *const c_void,
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if queried == 0 {
            break;
        }

        let base = info.BaseAddress as usize;
        let region_size = info.RegionSize;

        if region_size == 0 {
            if address == usize::MAX {
                break;
            }
            address = address.saturating_add(4096);
            continue;
        }

        if is_region_scannable(info.State, info.Protect) {
            let mut region_bytes_scanned = 0_u64;
            let mut region_scan_succeeded = false;
            let mut tail: Vec<u8> = vec![];
            let tail_len_limit = max_pattern_len.saturating_sub(1);

            let mut offset = 0usize;
            while offset < region_size {
                let to_read = CHUNK_SIZE.min(region_size - offset);
                let mut buffer = vec![0u8; to_read];
                let mut bytes_read = 0usize;

                let read_ok = unsafe {
                    ReadProcessMemory(
                        process_handle,
                        (base + offset) as *const c_void,
                        buffer.as_mut_ptr() as *mut c_void,
                        to_read,
                        &mut bytes_read,
                    )
                };

                if read_ok == 0 || bytes_read == 0 {
                    if region_scan_succeeded {
                        partial_reads += 1;
                        notes.push(format!(
                            "ReadProcessMemory stopped partway through a readable region at 0x{base:x}; partial region scan data was retained."
                        ));
                    }
                    break;
                }

                region_scan_succeeded = true;
                buffer.truncate(bytes_read);
                region_bytes_scanned += bytes_read as u64;

                let previous_tail_len = tail.len();
                let mut combined = Vec::with_capacity(previous_tail_len + buffer.len());
                combined.extend_from_slice(&tail);
                combined.extend_from_slice(&buffer);

                for (index, marker) in markers.iter().enumerate() {
                    if marker.bytes.is_empty() {
                        continue;
                    }

                    let min_start =
                        previous_tail_len.saturating_sub(marker.bytes.len().saturating_sub(1));
                    matches_found[index] +=
                        count_occurrences_from(&combined, marker.bytes, min_start);
                }

                if tail_len_limit == 0 {
                    tail.clear();
                } else if combined.len() > tail_len_limit {
                    tail = combined[combined.len() - tail_len_limit..].to_vec();
                } else {
                    tail = combined;
                }

                offset += bytes_read;
            }

            if region_scan_succeeded {
                regions_scanned += 1;
                bytes_scanned += region_bytes_scanned;
            } else {
                regions_skipped += 1;
            }
        } else {
            regions_skipped += 1;
        }

        match base.checked_add(region_size) {
            Some(next) if next > address => address = next,
            _ => break,
        }
    }

    unsafe {
        CloseHandle(process_handle);
    }

    let patterns = markers
        .iter()
        .enumerate()
        .map(|(index, marker)| {
            let status = if marker.bytes.is_empty() {
                "pattern_empty".to_string()
            } else if matches_found[index] > 0 {
                "detected_in_scanned_memory".to_string()
            } else if regions_scanned > 0 {
                "not_detected_in_scanned_regions".to_string()
            } else {
                "scan_attempt_failed".to_string()
            };

            let notes = if marker.bytes.is_empty() {
                "The configured marker bytes were empty, so no search was performed.".to_string()
            } else if matches_found[index] > 0 {
                format!(
                    "NullContext found {} occurrence(s) of this marker in scanned readable regions.",
                    matches_found[index]
                )
            } else if regions_scanned > 0 {
                "NullContext did not find this marker in the readable regions it successfully scanned."
                    .to_string()
            } else {
                "No readable process regions were scanned successfully, so this marker result is inconclusive."
                    .to_string()
            };

            ProcessScanPatternReport {
                pattern_kind: marker.kind.to_string(),
                status,
                matches_found: if marker.bytes.is_empty() {
                    None
                } else {
                    Some(matches_found[index])
                },
                notes,
            }
        })
        .collect();

    let status = if regions_scanned == 0 {
        "scan_attempt_failed".to_string()
    } else if partial_reads > 0 {
        "scan_attempt_incomplete".to_string()
    } else {
        "scan_completed".to_string()
    };

    if partial_reads > 0 {
        notes.push(
            "One or more readable regions could only be scanned partially, so the scan result is useful but incomplete."
                .to_string(),
        );
    }

    ProcessScanPhaseReport {
        phase: phase.to_string(),
        status,
        method: "win32_openprocess_virtualqueryex_readprocessmemory".to_string(),
        target_pid: Some(pid),
        scope_summary: format!(
            "Scanned readable committed regions of the target process using VirtualQueryEx and ReadProcessMemory. Regions scanned: {regions_scanned}. Regions skipped: {regions_skipped}."
        ),
        bytes_scanned: Some(bytes_scanned),
        regions_scanned: Some(regions_scanned),
        regions_skipped: Some(regions_skipped),
        patterns,
        notes,
    }
}

#[cfg(target_os = "windows")]
fn is_region_scannable(state: u32, protect: u32) -> bool {
    use windows_sys::Win32::System::Memory::{
        MEM_COMMIT, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
        PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
    };

    if state != MEM_COMMIT {
        return false;
    }

    if protect & PAGE_GUARD != 0 || protect & PAGE_NOACCESS != 0 {
        return false;
    }

    matches!(
        protect & 0xff,
        PAGE_READONLY
            | PAGE_READWRITE
            | PAGE_WRITECOPY
            | PAGE_EXECUTE_READ
            | PAGE_EXECUTE_READWRITE
            | PAGE_EXECUTE_WRITECOPY
    )
}

#[cfg(target_os = "windows")]
fn count_occurrences_from(haystack: &[u8], needle: &[u8], min_start: usize) -> u64 {
    if needle.is_empty() || needle.len() > haystack.len() {
        return 0;
    }

    let mut count = 0_u64;
    for start in min_start..=haystack.len() - needle.len() {
        if &haystack[start..start + needle.len()] == needle {
            count += 1;
        }
    }
    count
}
