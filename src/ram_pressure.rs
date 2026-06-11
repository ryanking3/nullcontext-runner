#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "macos")]
use std::process::Command;

pub struct HostRamPressureProbeReport {
    pub status: String,
    pub notes: Vec<String>,
}

const HOST_RAM_PRESSURE_TARGET_FRACTION: u64 = 8;
const HOST_RAM_PRESSURE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const HOST_RAM_PRESSURE_MIN_BYTES: u64 = 64 * 1024 * 1024;
const HOST_RAM_PRESSURE_CHUNK_BYTES: usize = 32 * 1024 * 1024;

pub fn run_host_ram_pressure_probe() -> HostRamPressureProbeReport {
    let budget = detect_host_memory_budget();
    let mut notes = vec![format!(
        "Host RAM pressure probe memory budget source: {}.",
        budget.source
    )];

    if let Some(total_bytes) = budget.total_bytes {
        notes.push(format!(
            "Host RAM pressure probe observed total physical memory: {total_bytes} bytes."
        ));
    }

    if let Some(available_bytes) = budget.available_bytes {
        notes.push(format!(
            "Host RAM pressure probe observed available physical memory: {available_bytes} bytes."
        ));
    }

    if let Some(note) = budget.note {
        notes.push(note);
    }

    let Some(available_bytes) = budget.available_bytes else {
        notes.push(
            "Host RAM pressure probe could not determine available physical memory, so it skipped allocation pressure."
                .to_string(),
        );
        return HostRamPressureProbeReport {
            status: "host_ram_pressure_probe_unavailable".to_string(),
            notes,
        };
    };

    let target_bytes =
        (available_bytes / HOST_RAM_PRESSURE_TARGET_FRACTION).min(HOST_RAM_PRESSURE_MAX_BYTES);

    if target_bytes < HOST_RAM_PRESSURE_MIN_BYTES {
        notes.push(format!(
            "Host RAM pressure probe skipped because the computed target ({} bytes) was below the {}-byte minimum.",
            target_bytes, HOST_RAM_PRESSURE_MIN_BYTES
        ));
        return HostRamPressureProbeReport {
            status: "host_ram_pressure_probe_skipped_low_available_memory".to_string(),
            notes,
        };
    }

    let mut allocations: Vec<Vec<u8>> = Vec::new();
    let mut remaining = target_bytes;
    let mut total_allocated = 0_u64;
    let mut allocation_warnings = Vec::new();

    while remaining > 0 {
        let chunk_bytes = remaining.min(HOST_RAM_PRESSURE_CHUNK_BYTES as u64) as usize;
        let mut buffer = Vec::new();

        if let Err(error) = buffer.try_reserve_exact(chunk_bytes) {
            allocation_warnings.push(format!(
                "Host RAM pressure probe failed to reserve a {}-byte chunk: {}.",
                chunk_bytes, error
            ));
            break;
        }

        buffer.resize(chunk_bytes, 0_u8);
        buffer.fill(0xA5);
        buffer.fill(0x00);

        total_allocated = total_allocated.saturating_add(chunk_bytes as u64);
        remaining = remaining.saturating_sub(chunk_bytes as u64);
        allocations.push(buffer);
    }

    notes.push(format!(
        "Host RAM pressure probe targeted {} bytes and allocated/touched {} bytes.",
        target_bytes, total_allocated
    ));
    notes.push(
        "Each host-memory chunk was filled with 0xA5 and then overwritten with 0x00 before release."
            .to_string(),
    );
    notes.push(
        "This pressures host allocator/page reuse after shutdown, but it does not prove that the exact physical pages previously used by llama-server were overwritten."
            .to_string(),
    );

    notes.extend(allocation_warnings);

    drop(allocations);
    notes.push("Host RAM pressure probe released its temporary allocations.".to_string());

    let status = if total_allocated == 0 {
        "host_ram_pressure_probe_no_allocations_completed".to_string()
    } else if notes.iter().any(|note| note.contains("failed")) {
        "host_ram_pressure_probe_completed_with_warnings".to_string()
    } else {
        "host_ram_pressure_probe_completed".to_string()
    };

    HostRamPressureProbeReport { status, notes }
}

struct HostMemoryBudget {
    total_bytes: Option<u64>,
    available_bytes: Option<u64>,
    source: String,
    note: Option<String>,
}

#[cfg(target_os = "windows")]
fn detect_host_memory_budget() -> HostMemoryBudget {
    use windows_sys::Win32::System::Memory::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };

    let ok = unsafe { GlobalMemoryStatusEx(&mut status as *mut MEMORYSTATUSEX) };
    if ok == 0 {
        return HostMemoryBudget {
            total_bytes: None,
            available_bytes: None,
            source: "GlobalMemoryStatusEx".to_string(),
            note: Some(
                "GlobalMemoryStatusEx did not return host memory totals for the RAM pressure probe."
                    .to_string(),
            ),
        };
    }

    HostMemoryBudget {
        total_bytes: Some(status.ullTotalPhys),
        available_bytes: Some(status.ullAvailPhys),
        source: "GlobalMemoryStatusEx".to_string(),
        note: None,
    }
}

#[cfg(target_os = "linux")]
fn detect_host_memory_budget() -> HostMemoryBudget {
    match fs::read_to_string("/proc/meminfo") {
        Ok(raw) => {
            let total_kib = parse_meminfo_kib_value(&raw, "MemTotal");
            let available_kib = parse_meminfo_kib_value(&raw, "MemAvailable")
                .or_else(|| parse_meminfo_kib_value(&raw, "MemFree"));

            HostMemoryBudget {
                total_bytes: total_kib.map(|value| value * 1024),
                available_bytes: available_kib.map(|value| value * 1024),
                source: "/proc/meminfo".to_string(),
                note: None,
            }
        }
        Err(error) => HostMemoryBudget {
            total_bytes: None,
            available_bytes: None,
            source: "/proc/meminfo".to_string(),
            note: Some(format!(
                "Could not read /proc/meminfo for the RAM pressure probe: {error}."
            )),
        },
    }
}

#[cfg(target_os = "macos")]
fn detect_host_memory_budget() -> HostMemoryBudget {
    let total_bytes = detect_macos_total_memory_bytes();

    match Command::new("vm_stat").output() {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let page_size = parse_macos_vm_stat_page_size(&raw);
            let free_pages = parse_macos_vm_stat_pages(&raw, "Pages free");
            let inactive_pages = parse_macos_vm_stat_pages(&raw, "Pages inactive");
            let speculative_pages = parse_macos_vm_stat_pages(&raw, "Pages speculative");
            let available_pages = free_pages
                .unwrap_or(0)
                .saturating_add(inactive_pages.unwrap_or(0))
                .saturating_add(speculative_pages.unwrap_or(0));

            HostMemoryBudget {
                total_bytes,
                available_bytes: page_size.map(|size| available_pages.saturating_mul(size)),
                source: "vm_stat + sysctl hw.memsize".to_string(),
                note: Some(
                    "macOS available-memory estimate uses free + inactive + speculative pages from vm_stat, so it is an approximation rather than a kernel-owned forensic reading."
                        .to_string(),
                ),
            }
        }
        Ok(output) => HostMemoryBudget {
            total_bytes,
            available_bytes: None,
            source: "vm_stat".to_string(),
            note: Some(format!(
                "vm_stat returned non-zero status {} during host RAM pressure budgeting.",
                output.status
            )),
        },
        Err(error) => HostMemoryBudget {
            total_bytes,
            available_bytes: None,
            source: "vm_stat".to_string(),
            note: Some(format!(
                "vm_stat was unavailable during host RAM pressure budgeting: {error}."
            )),
        },
    }
}

#[cfg(target_os = "linux")]
fn parse_meminfo_kib_value(raw: &str, field: &str) -> Option<u64> {
    raw.lines().find_map(|line| {
        let (label, value) = line.split_once(':')?;
        if label.trim() != field {
            return None;
        }

        value.split_whitespace().next()?.parse::<u64>().ok()
    })
}

#[cfg(target_os = "macos")]
fn detect_macos_total_memory_bytes() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()
}

#[cfg(target_os = "macos")]
fn parse_macos_vm_stat_page_size(raw: &str) -> Option<u64> {
    let line = raw.lines().next()?;
    let marker = "page size of ";
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find(" bytes")? + start;
    line[start..end].trim().parse::<u64>().ok()
}

#[cfg(target_os = "macos")]
fn parse_macos_vm_stat_pages(raw: &str, label: &str) -> Option<u64> {
    raw.lines().find_map(|line| {
        let normalized = line.replace('.', "");
        let (line_label, value) = normalized.split_once(':')?;
        if line_label.trim() != label {
            return None;
        }

        value.trim().replace('.', "").parse::<u64>().ok()
    })
}
