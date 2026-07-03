#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use core::ffi::c_void;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

pub struct HostRamPressureProbeReport {
    pub status: String,
    pub notes: Vec<String>,
}

const HOST_RAM_PRESSURE_TARGET_FRACTION: u64 = 8;
const HOST_RAM_PRESSURE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const HOST_RAM_PRESSURE_MIN_BYTES: u64 = 64 * 1024 * 1024;
const HOST_RAM_PRESSURE_CHUNK_BYTES: usize = 32 * 1024 * 1024;
const HOST_PAGE_DISCARD_TARGET_FRACTION: u64 = 8;
const HOST_PAGE_DISCARD_MAX_BYTES: u64 = 512 * 1024 * 1024;
const HOST_PAGE_DISCARD_MIN_BYTES: u64 = 64 * 1024 * 1024;
const HOST_PAGE_DISCARD_CHUNK_BYTES: usize = 32 * 1024 * 1024;
const HOST_PAGE_DISCARD_CHURN_ROUNDS: u32 = 3;

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

pub fn run_host_page_discard_probe() -> HostRamPressureProbeReport {
    run_host_page_discard_probe_impl("Host page discard probe", "host_page_discard_probe", 1)
}

pub fn run_host_page_discard_churn_probe() -> HostRamPressureProbeReport {
    run_host_page_discard_probe_impl(
        "Host page discard churn probe",
        "host_page_discard_churn_probe",
        HOST_PAGE_DISCARD_CHURN_ROUNDS,
    )
}

fn run_host_page_discard_probe_impl(
    probe_label: &str,
    status_prefix: &str,
    rounds: u32,
) -> HostRamPressureProbeReport {
    let budget = detect_host_memory_budget();
    let mut notes = vec![format!(
        "{probe_label} memory budget source: {}.",
        budget.source
    )];

    if let Some(total_bytes) = budget.total_bytes {
        notes.push(format!(
            "{probe_label} observed total physical memory: {total_bytes} bytes."
        ));
    }

    if let Some(available_bytes) = budget.available_bytes {
        notes.push(format!(
            "{probe_label} observed available physical memory: {available_bytes} bytes."
        ));
    }

    if let Some(note) = budget.note {
        notes.push(note);
    }

    let page_size = detect_host_page_size();
    notes.push(format!(
        "{probe_label} page-size source: {}.",
        page_size.source
    ));

    if let Some(note) = page_size.note {
        notes.push(note);
    }

    let Some(available_bytes) = budget.available_bytes else {
        notes.push(
            format!(
                "{probe_label} could not determine available physical memory, so it skipped page mapping pressure."
            )
                .to_string(),
        );
        return HostRamPressureProbeReport {
            status: format!("{status_prefix}_unavailable"),
            notes,
        };
    };

    let Some(page_bytes) = page_size.bytes else {
        notes.push(
            format!(
                "{probe_label} could not determine the host page size, so it skipped page mapping pressure."
            )
                .to_string(),
        );
        return HostRamPressureProbeReport {
            status: format!("{status_prefix}_unavailable"),
            notes,
        };
    };

    notes.push(format!(
        "{probe_label} observed page size: {page_bytes} bytes."
    ));

    let target_bytes = align_down_u64(
        (available_bytes / HOST_PAGE_DISCARD_TARGET_FRACTION).min(HOST_PAGE_DISCARD_MAX_BYTES),
        page_bytes as u64,
    );

    if target_bytes < HOST_PAGE_DISCARD_MIN_BYTES {
        notes.push(format!(
            "{probe_label} skipped because the computed target ({} bytes) was below the {}-byte minimum.",
            target_bytes, HOST_PAGE_DISCARD_MIN_BYTES
        ));
        return HostRamPressureProbeReport {
            status: format!("{status_prefix}_skipped_low_available_memory"),
            notes,
        };
    }

    let chunk_bytes = align_down_u64(HOST_PAGE_DISCARD_CHUNK_BYTES as u64, page_bytes as u64)
        .max(page_bytes as u64) as usize;
    let mut total_mapped = 0_u64;
    let mut completed_rounds = 0_u32;
    let mut warnings = Vec::new();

    for round in 0..rounds {
        let mut remaining = target_bytes;
        let mut round_mapped = 0_u64;

        while remaining > 0 {
            let mapping_bytes = remaining.min(chunk_bytes as u64) as usize;
            let mapping_result = unsafe { allocate_page_probe_mapping(mapping_bytes) };

            let mapping = match mapping_result {
                Ok(mapping) => mapping,
                Err(error) => {
                    warnings.push(format!(
                        "{probe_label} failed to map a {}-byte region during round {} of {}: {error}.",
                        mapping_bytes,
                        round + 1,
                        rounds
                    ));
                    break;
                }
            };

            let fill_result = unsafe { fill_page_probe_mapping(mapping.ptr, mapping_bytes) };
            if let Err(error) = fill_result {
                warnings.push(format!(
                    "{probe_label} failed while writing a {}-byte region during round {} of {}: {error}.",
                    mapping_bytes,
                    round + 1,
                    rounds
                ));
            } else {
                total_mapped = total_mapped.saturating_add(mapping_bytes as u64);
                round_mapped = round_mapped.saturating_add(mapping_bytes as u64);
            }

            if let Some(error) = unsafe { discard_page_probe_mapping(mapping.ptr, mapping_bytes) } {
                warnings.push(format!(
                    "{probe_label} discard step reported an issue during round {} of {}: {error}",
                    round + 1,
                    rounds
                ));
            }

            if let Some(error) = unsafe { release_page_probe_mapping(mapping.ptr, mapping_bytes) } {
                warnings.push(format!(
                    "{probe_label} release step reported an issue during round {} of {}: {error}",
                    round + 1,
                    rounds
                ));
            }

            remaining = remaining.saturating_sub(mapping_bytes as u64);
        }

        if round_mapped > 0 {
            completed_rounds = completed_rounds.saturating_add(1);
        }

        notes.push(format!(
            "{probe_label} round {} of {} mapped/touched {} bytes.",
            round + 1,
            rounds,
            round_mapped
        ));

        if round_mapped == 0 {
            notes.push(format!(
                "{probe_label} stopped after round {} because it could not complete any page-mapping work in that round.",
                round + 1
            ));
            break;
        }
    }

    notes.push(format!(
        "{probe_label} targeted {} bytes per round, completed {} round(s), and mapped/touched {} bytes total.",
        target_bytes, completed_rounds, total_mapped
    ));
    if rounds > 1 {
        notes.push(format!(
            "{probe_label} repeatedly mapped fresh regions, filled them with 0xA5, overwrote them with 0x00, then explicitly discarded/decommitted and released them across {} rounds.",
            rounds
        ));
        notes.push(
            "This is more invasive than a single discard pass because it creates repeated OS-page reuse pressure after shutdown, but it still does not prove that the exact prior llama.cpp pages were reclaimed and overwritten."
                .to_string(),
        );
    } else {
        notes.push(
            "Each mapped region was filled with 0xA5, overwritten with 0x00, then explicitly discarded/decommitted before release."
                .to_string(),
        );
        notes.push(
            "This moves closer to OS-page-level cleanup behavior than allocator-only churn, but it still does not prove that the exact prior llama.cpp pages were reclaimed and overwritten."
                .to_string(),
        );
    }
    let had_warnings = !warnings.is_empty();
    notes.extend(warnings);

    let status = if total_mapped == 0 || completed_rounds == 0 {
        format!("{status_prefix}_no_mappings_completed")
    } else if had_warnings {
        format!("{status_prefix}_completed_with_warnings")
    } else {
        format!("{status_prefix}_completed")
    };

    HostRamPressureProbeReport { status, notes }
}

struct HostMemoryBudget {
    total_bytes: Option<u64>,
    available_bytes: Option<u64>,
    source: String,
    note: Option<String>,
}

struct HostPageSize {
    bytes: Option<usize>,
    source: String,
    note: Option<String>,
}

struct PageProbeMapping {
    ptr: *mut u8,
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

#[cfg(target_os = "windows")]
fn detect_host_page_size() -> HostPageSize {
    use windows_sys::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

    let mut system_info = unsafe { std::mem::zeroed::<SYSTEM_INFO>() };
    unsafe { GetSystemInfo(&mut system_info as *mut SYSTEM_INFO) };

    HostPageSize {
        bytes: Some(system_info.dwPageSize as usize),
        source: "GetSystemInfo".to_string(),
        note: None,
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn detect_host_page_size() -> HostPageSize {
    match Command::new("getconf").arg("PAGESIZE").output() {
        Ok(output) if output.status.success() => {
            let page_size = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<usize>()
                .ok();

            HostPageSize {
                bytes: page_size,
                source: "getconf PAGESIZE".to_string(),
                note: if page_size.is_some() {
                    None
                } else {
                    Some(
                        "getconf PAGESIZE succeeded, but the page size output could not be parsed."
                            .to_string(),
                    )
                },
            }
        }
        Ok(output) => HostPageSize {
            bytes: None,
            source: "getconf PAGESIZE".to_string(),
            note: Some(format!(
                "getconf PAGESIZE returned non-zero status {} during page-size detection.",
                output.status
            )),
        },
        Err(error) => HostPageSize {
            bytes: None,
            source: "getconf PAGESIZE".to_string(),
            note: Some(format!(
                "getconf PAGESIZE was unavailable during page-size detection: {error}."
            )),
        },
    }
}

#[cfg(target_os = "windows")]
unsafe fn allocate_page_probe_mapping(size: usize) -> Result<PageProbeMapping, String> {
    use windows_sys::Win32::System::Memory::{
        VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
    };

    let ptr = unsafe {
        VirtualAlloc(
            std::ptr::null(),
            size,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };

    if ptr.is_null() {
        Err("VirtualAlloc returned a null pointer".to_string())
    } else {
        Ok(PageProbeMapping { ptr: ptr.cast() })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe fn allocate_page_probe_mapping(size: usize) -> Result<PageProbeMapping, String> {
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        )
    };

    if ptr == map_failed() {
        Err("mmap returned MAP_FAILED".to_string())
    } else {
        Ok(PageProbeMapping { ptr: ptr.cast() })
    }
}

unsafe fn fill_page_probe_mapping(ptr: *mut u8, size: usize) -> Result<(), String> {
    if ptr.is_null() {
        return Err("page probe mapping pointer was null".to_string());
    }

    let buffer = unsafe { std::slice::from_raw_parts_mut(ptr, size) };
    buffer.fill(0xA5);
    buffer.fill(0x00);
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn discard_page_probe_mapping(ptr: *mut u8, size: usize) -> Option<String> {
    use windows_sys::Win32::System::Memory::{VirtualFree, MEM_DECOMMIT};

    let ok = unsafe { VirtualFree(ptr.cast::<c_void>(), size, MEM_DECOMMIT) };
    if ok == 0 {
        Some(format!(
            "Host page discard probe VirtualFree(MEM_DECOMMIT) failed for a {}-byte region.",
            size
        ))
    } else {
        None
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe fn discard_page_probe_mapping(ptr: *mut u8, size: usize) -> Option<String> {
    let advice = unix_page_discard_advice();
    let advice_label = unix_page_discard_advice_label();
    let ok = unsafe { madvise(ptr.cast::<c_void>(), size, advice) };
    if ok != 0 {
        Some(format!(
            "Host page discard probe madvise({advice_label}) failed for a {}-byte region.",
            size
        ))
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
unsafe fn release_page_probe_mapping(ptr: *mut u8, _size: usize) -> Option<String> {
    use windows_sys::Win32::System::Memory::{VirtualFree, MEM_RELEASE};

    let ok = unsafe { VirtualFree(ptr.cast::<c_void>(), 0, MEM_RELEASE) };
    if ok == 0 {
        Some("Host page discard probe VirtualFree(MEM_RELEASE) failed.".to_string())
    } else {
        None
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe fn release_page_probe_mapping(ptr: *mut u8, size: usize) -> Option<String> {
    let ok = unsafe { munmap(ptr.cast::<c_void>(), size) };
    if ok != 0 {
        Some(format!(
            "Host page discard probe munmap failed for a {}-byte region.",
            size
        ))
    } else {
        None
    }
}

fn align_down_u64(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        value
    } else {
        value - (value % alignment)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
const PROT_READ: i32 = 0x01;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PROT_WRITE: i32 = 0x02;
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAP_PRIVATE: i32 = 0x02;
#[cfg(target_os = "linux")]
const MAP_ANON: i32 = 0x20;
#[cfg(target_os = "macos")]
const MAP_ANON: i32 = 0x1000;
#[cfg(target_os = "linux")]
const MADV_DONTNEED: i32 = 4;
#[cfg(target_os = "macos")]
const MADV_FREE: i32 = 5;

#[cfg(any(target_os = "linux", target_os = "macos"))]
unsafe extern "C" {
    fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> i32;
    fn madvise(addr: *mut c_void, len: usize, advice: i32) -> i32;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn map_failed() -> *mut c_void {
    (-1_isize) as *mut c_void
}

#[cfg(target_os = "linux")]
fn unix_page_discard_advice() -> i32 {
    MADV_DONTNEED
}

#[cfg(target_os = "linux")]
fn unix_page_discard_advice_label() -> &'static str {
    "MADV_DONTNEED"
}

#[cfg(target_os = "macos")]
fn unix_page_discard_advice() -> i32 {
    MADV_FREE
}

#[cfg(target_os = "macos")]
fn unix_page_discard_advice_label() -> &'static str {
    "MADV_FREE"
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
