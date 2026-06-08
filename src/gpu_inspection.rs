use std::process::Command;

#[cfg(any(target_os = "linux", target_os = "windows"))]
use core::ffi::{c_char, c_uint, c_ulonglong, c_void};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use libloading::Library;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::ffi::CStr;

#[derive(Debug, Clone)]
pub struct GpuInspectionObservation {
    pub memory_bytes: Option<u64>,
    pub source: Option<String>,
    pub backend: Option<String>,
    pub pid_observed: Option<bool>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
struct BackendObservation {
    memory_bytes: Option<u64>,
    source: Option<String>,
    backend: String,
    pid_observed: Option<bool>,
    note: Option<String>,
}

trait GpuInspectionBackend {
    fn inspect_process(&self, pid: u32) -> Option<BackendObservation>;
}

pub fn observe_gpu_process(pid: u32) -> GpuInspectionObservation {
    let backends: [&dyn GpuInspectionBackend; 3] = [
        &NvmlProcessBackend,
        &NvidiaSmiComputeAppsBackend,
        &NvidiaSmiPmonBackend,
    ];

    let mut prior_notes = Vec::new();
    let mut prior_sources = Vec::new();
    let mut prior_backends = Vec::new();

    for backend in backends {
        let Some(mut observation) = backend.inspect_process(pid) else {
            continue;
        };

        if !prior_sources.is_empty() {
            observation.source = Some(merge_labels(&prior_sources, observation.source.as_deref()));
        }

        if !prior_backends.is_empty() {
            observation.backend = merge_labels(&prior_backends, Some(&observation.backend));
        }

        if !prior_notes.is_empty() {
            observation.note = Some(match observation.note.take() {
                Some(note) => format!("{} {}", prior_notes.join(" "), note),
                None => prior_notes.join(" "),
            });
        }

        if observation.pid_observed == Some(true) && observation.memory_bytes.is_some() {
            return GpuInspectionObservation {
                memory_bytes: observation.memory_bytes,
                source: observation.source,
                backend: Some(observation.backend),
                pid_observed: observation.pid_observed,
                note: observation.note,
            };
        }

        if let Some(source) = observation.source {
            prior_sources.push(source);
        }

        prior_backends.push(observation.backend);

        if let Some(note) = observation.note {
            prior_notes.push(note);
        }

        if observation.pid_observed == Some(true) {
            return GpuInspectionObservation {
                memory_bytes: observation.memory_bytes,
                source: if prior_sources.is_empty() {
                    None
                } else {
                    Some(merge_labels(&prior_sources, None))
                },
                backend: if prior_backends.is_empty() {
                    None
                } else {
                    Some(merge_labels(&prior_backends, None))
                },
                pid_observed: Some(true),
                note: if prior_notes.is_empty() {
                    None
                } else {
                    Some(prior_notes.join(" "))
                },
            };
        }
    }

    GpuInspectionObservation {
        memory_bytes: None,
        source: if prior_sources.is_empty() {
            None
        } else {
            Some(merge_labels(&prior_sources, None))
        },
        backend: if prior_backends.is_empty() {
            None
        } else {
            Some(merge_labels(&prior_backends, None))
        },
        pid_observed: if prior_backends.is_empty() {
            None
        } else if prior_backends.iter().any(|value| value.contains("pmon")) {
            Some(false)
        } else {
            None
        },
        note: Some(if prior_notes.is_empty() {
            "GPU memory observation backends were unavailable or inconclusive.".to_string()
        } else {
            prior_notes.join(" ")
        }),
    }
}

struct NvmlProcessBackend;

impl GpuInspectionBackend for NvmlProcessBackend {
    fn inspect_process(&self, pid: u32) -> Option<BackendObservation> {
        inspect_process_via_nvml(pid)
    }
}

struct NvidiaSmiComputeAppsBackend;

impl GpuInspectionBackend for NvidiaSmiComputeAppsBackend {
    fn inspect_process(&self, pid: u32) -> Option<BackendObservation> {
        let output = Command::new("nvidia-smi")
            .args([
                "--query-compute-apps=pid,used_gpu_memory",
                "--format=csv,noheader,nounits",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);

                for line in stdout.lines() {
                    let mut parts = line.split(',').map(|part| part.trim());
                    let observed_pid = parts.next().and_then(|value| value.parse::<u32>().ok());
                    let memory_raw = parts.next().map(str::trim);
                    let memory_mb = memory_raw.and_then(|value| value.parse::<u64>().ok());

                    if observed_pid == Some(pid) {
                        return Some(BackendObservation {
                            memory_bytes: memory_mb.map(|value| value * 1024 * 1024),
                            source: Some("nvidia-smi compute-apps".to_string()),
                            backend: "nvidia_smi_compute_apps".to_string(),
                            pid_observed: Some(true),
                            note: if memory_mb.is_none() {
                                Some(compute_apps_memory_unavailable_note(
                                    memory_raw.unwrap_or("unknown"),
                                ))
                            } else {
                                None
                            },
                        });
                    }
                }

                None
            }
            Ok(output) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                backend: "nvidia_smi_compute_apps".to_string(),
                pid_observed: None,
                note: Some(format!(
                    "GPU memory observation via nvidia-smi compute-apps failed with status {}.",
                    output.status
                )),
            }),
            Err(error) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                backend: "nvidia_smi_compute_apps".to_string(),
                pid_observed: None,
                note: Some(format!(
                    "GPU memory observation via nvidia-smi compute-apps was unavailable: {error}."
                )),
            }),
        }
    }
}

struct NvidiaSmiPmonBackend;

impl GpuInspectionBackend for NvidiaSmiPmonBackend {
    fn inspect_process(&self, pid: u32) -> Option<BackendObservation> {
        let output = Command::new("nvidia-smi")
            .args(["pmon", "-c", "1"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);

                for line in stdout.lines() {
                    let trimmed = line.trim();

                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }

                    let parts: Vec<&str> = trimmed.split_whitespace().collect();

                    if parts.len() < 2 {
                        continue;
                    }

                    let observed_pid = parts.get(1).and_then(|value| value.parse::<u32>().ok());

                    if observed_pid == Some(pid) {
                        return Some(BackendObservation {
                            memory_bytes: None,
                            source: Some("nvidia-smi pmon".to_string()),
                            backend: "nvidia_smi_pmon".to_string(),
                            pid_observed: Some(true),
                            note: Some(
                                "llama-server PID was observed in nvidia-smi pmon, but per-process GPU memory bytes were unavailable from the current NVIDIA tooling path."
                                    .to_string(),
                            ),
                        });
                    }
                }

                Some(BackendObservation {
                    memory_bytes: None,
                    source: Some("nvidia-smi compute-apps + pmon".to_string()),
                    backend: "nvidia_smi_pmon".to_string(),
                    pid_observed: Some(false),
                    note: Some(no_matching_gpu_pid_note()),
                })
            }
            Ok(output) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                backend: "nvidia_smi_pmon".to_string(),
                pid_observed: None,
                note: Some(format!(
                    "GPU process observation via nvidia-smi pmon failed with status {}.",
                    output.status
                )),
            }),
            Err(error) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                backend: "nvidia_smi_pmon".to_string(),
                pid_observed: None,
                note: Some(format!(
                    "GPU process observation via nvidia-smi pmon was unavailable: {error}."
                )),
            }),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlReturn = i32;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlDevice = *mut c_void;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlInitV2 = unsafe extern "C" fn() -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlShutdown = unsafe extern "C" fn() -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlErrorString = unsafe extern "C" fn(result: NvmlReturn) -> *const c_char;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlSystemGetDriverVersion =
    unsafe extern "C" fn(version: *mut c_char, length: c_uint) -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlDeviceGetCountV2 = unsafe extern "C" fn(device_count: *mut c_uint) -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlDeviceGetHandleByIndexV2 =
    unsafe extern "C" fn(index: c_uint, device: *mut NvmlDevice) -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlDeviceGetComputeRunningProcessesV3 = unsafe extern "C" fn(
    device: NvmlDevice,
    info_count: *mut c_uint,
    infos: *mut NvmlProcessInfoV3,
) -> NvmlReturn;
#[cfg(any(target_os = "linux", target_os = "windows"))]
type NvmlDeviceGetGraphicsRunningProcessesV3 = unsafe extern "C" fn(
    device: NvmlDevice,
    info_count: *mut c_uint,
    infos: *mut NvmlProcessInfoV3,
) -> NvmlReturn;

#[cfg(any(target_os = "linux", target_os = "windows"))]
const NVML_SUCCESS: NvmlReturn = 0;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const NVML_ERROR_INSUFFICIENT_SIZE: NvmlReturn = 7;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const NVML_VALUE_NOT_AVAILABLE: u64 = u64::MAX;
#[cfg(any(target_os = "linux", target_os = "windows"))]
const NVML_DRIVER_VERSION_BUFFER_SIZE: usize = 80;

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct NvmlProcessInfoV3 {
    pid: c_uint,
    used_gpu_memory: c_ulonglong,
    gpu_instance_id: c_uint,
    compute_instance_id: c_uint,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
struct NvmlApi {
    library: Library,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl NvmlApi {
    fn load() -> Result<Self, String> {
        let candidates = nvml_library_candidates();
        let mut errors = Vec::new();

        for candidate in candidates {
            let library = unsafe { Library::new(candidate) };

            match library {
                Ok(library) => return Ok(Self { library }),
                Err(error) => errors.push(format!("{candidate}: {error}")),
            }
        }

        Err(format!("no NVML library loaded ({})", errors.join("; ")))
    }

    fn init(&self) -> Result<(), String> {
        let init = unsafe { self.library.get::<NvmlInitV2>(b"nvmlInit_v2\0") }
            .map_err(|error| format!("missing nvmlInit_v2: {error}"))?;
        let result = unsafe { init() };

        if result == NVML_SUCCESS {
            Ok(())
        } else {
            Err(format!(
                "nvmlInit_v2 failed with {}",
                self.error_description(result)
            ))
        }
    }

    fn shutdown(&self) {
        if let Ok(shutdown) = unsafe { self.library.get::<NvmlShutdown>(b"nvmlShutdown\0") } {
            let _ = unsafe { shutdown() };
        }
    }

    fn error_description(&self, code: NvmlReturn) -> String {
        let error_string = unsafe { self.library.get::<NvmlErrorString>(b"nvmlErrorString\0") };

        match error_string {
            Ok(error_string) => {
                let ptr = unsafe { error_string(code) };

                if ptr.is_null() {
                    format!("code {code}")
                } else {
                    unsafe { CStr::from_ptr(ptr) }
                        .to_str()
                        .map(|value| format!("{value} (code {code})"))
                        .unwrap_or_else(|_| format!("code {code}"))
                }
            }
            Err(_) => format!("code {code}"),
        }
    }

    fn driver_version(&self) -> Option<String> {
        let get_driver_version = unsafe {
            self.library
                .get::<NvmlSystemGetDriverVersion>(b"nvmlSystemGetDriverVersion\0")
        }
        .ok()?;

        let mut buffer = [0_i8; NVML_DRIVER_VERSION_BUFFER_SIZE];
        let result = unsafe { get_driver_version(buffer.as_mut_ptr(), buffer.len() as c_uint) };

        if result != NVML_SUCCESS {
            return None;
        }

        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_str()
            .ok()
            .map(str::to_string)
    }

    fn device_count(&self) -> Result<u32, String> {
        let get_device_count = unsafe {
            self.library
                .get::<NvmlDeviceGetCountV2>(b"nvmlDeviceGetCount_v2\0")
        }
        .map_err(|error| format!("missing nvmlDeviceGetCount_v2: {error}"))?;
        let mut count = 0_u32;
        let result = unsafe { get_device_count(&mut count as *mut u32) };

        if result == NVML_SUCCESS {
            Ok(count)
        } else {
            Err(format!(
                "nvmlDeviceGetCount_v2 failed with {}",
                self.error_description(result)
            ))
        }
    }

    fn device_handle(&self, index: u32) -> Result<NvmlDevice, String> {
        let get_handle = unsafe {
            self.library
                .get::<NvmlDeviceGetHandleByIndexV2>(b"nvmlDeviceGetHandleByIndex_v2\0")
        }
        .map_err(|error| format!("missing nvmlDeviceGetHandleByIndex_v2: {error}"))?;
        let mut device = std::ptr::null_mut();
        let result = unsafe { get_handle(index, &mut device as *mut NvmlDevice) };

        if result == NVML_SUCCESS {
            Ok(device)
        } else {
            Err(format!(
                "nvmlDeviceGetHandleByIndex_v2({index}) failed with {}",
                self.error_description(result)
            ))
        }
    }

    fn query_compute_processes(
        &self,
        device: NvmlDevice,
    ) -> Result<Vec<NvmlProcessRecord>, String> {
        let query = unsafe {
            self.library.get::<NvmlDeviceGetComputeRunningProcessesV3>(
                b"nvmlDeviceGetComputeRunningProcesses_v3\0",
            )
        }
        .map_err(|error| format!("missing nvmlDeviceGetComputeRunningProcesses_v3: {error}"))?;

        self.query_processes_with_symbol(device, *query)
    }

    fn query_graphics_processes(
        &self,
        device: NvmlDevice,
    ) -> Result<Vec<NvmlProcessRecord>, String> {
        let query = unsafe {
            self.library.get::<NvmlDeviceGetGraphicsRunningProcessesV3>(
                b"nvmlDeviceGetGraphicsRunningProcesses_v3\0",
            )
        }
        .map_err(|error| format!("missing nvmlDeviceGetGraphicsRunningProcesses_v3: {error}"))?;

        self.query_processes_with_symbol(device, *query)
    }

    fn query_processes_with_symbol(
        &self,
        device: NvmlDevice,
        query: unsafe extern "C" fn(NvmlDevice, *mut c_uint, *mut NvmlProcessInfoV3) -> NvmlReturn,
    ) -> Result<Vec<NvmlProcessRecord>, String> {
        let mut info_count = 16_u32;
        let mut infos = vec![NvmlProcessInfoV3::default(); info_count as usize];

        for _ in 0..3 {
            let mut requested = info_count;
            let result = unsafe { query(device, &mut requested as *mut u32, infos.as_mut_ptr()) };

            if result == NVML_SUCCESS {
                infos.truncate(requested as usize);
                return Ok(infos.into_iter().map(NvmlProcessRecord::from).collect());
            }

            if result == NVML_ERROR_INSUFFICIENT_SIZE {
                info_count = requested.max(info_count.saturating_mul(2)).max(1);
                infos.resize(info_count as usize, NvmlProcessInfoV3::default());
                continue;
            }

            return Err(self.error_description(result));
        }

        Err("NVML process query exceeded resize retries.".to_string())
    }

    fn query_target_process(&self, pid: u32) -> Result<NvmlProcessQuerySummary, String> {
        let device_count = self.device_count()?;
        let mut total_memory_bytes = 0_u64;
        let mut any_memory_visible = false;
        let mut matched_process_entries = 0_u32;
        let mut notes = Vec::new();

        for index in 0..device_count {
            let device = self.device_handle(index)?;

            match self.query_compute_processes(device) {
                Ok(processes) => {
                    for process in processes {
                        if process.pid == pid {
                            matched_process_entries += 1;
                            if let Some(memory_bytes) = process.memory_bytes {
                                total_memory_bytes =
                                    total_memory_bytes.saturating_add(memory_bytes);
                                any_memory_visible = true;
                            }
                        }
                    }
                }
                Err(error) => notes.push(format!(
                    "NVML compute-process query on GPU {index} failed: {error}."
                )),
            }

            match self.query_graphics_processes(device) {
                Ok(processes) => {
                    for process in processes {
                        if process.pid == pid {
                            matched_process_entries += 1;
                            if let Some(memory_bytes) = process.memory_bytes {
                                total_memory_bytes =
                                    total_memory_bytes.saturating_add(memory_bytes);
                                any_memory_visible = true;
                            }
                        }
                    }
                }
                Err(error) => notes.push(format!(
                    "NVML graphics-process query on GPU {index} failed: {error}."
                )),
            }
        }

        Ok(NvmlProcessQuerySummary {
            driver_version: self.driver_version(),
            device_count,
            pid_observed: matched_process_entries > 0,
            memory_bytes: if any_memory_visible {
                Some(total_memory_bytes)
            } else {
                None
            },
            notes,
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Debug)]
struct NvmlProcessQuerySummary {
    driver_version: Option<String>,
    device_count: u32,
    pid_observed: bool,
    memory_bytes: Option<u64>,
    notes: Vec<String>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn inspect_process_via_nvml(pid: u32) -> Option<BackendObservation> {
    let api = match NvmlApi::load() {
        Ok(api) => api,
        Err(error) => {
            return Some(BackendObservation {
                memory_bytes: None,
                source: None,
                backend: "nvml_process_listing".to_string(),
                pid_observed: None,
                note: Some(format!("NVML inspection backend was unavailable: {error}.")),
            });
        }
    };

    if let Err(error) = api.init() {
        return Some(BackendObservation {
            memory_bytes: None,
            source: None,
            backend: "nvml_process_listing".to_string(),
            pid_observed: None,
            note: Some(format!(
                "NVML inspection backend failed to initialize: {error}."
            )),
        });
    }

    let summary = api.query_target_process(pid);
    api.shutdown();

    match summary {
        Ok(summary) => {
            let mut notes = Vec::new();

            if let Some(driver_version) = summary.driver_version {
                notes.push(format!("NVML driver version: {driver_version}."));
            }

            notes.push(format!(
                "NVML queried {} GPU device(s) for compute and graphics process entries.",
                summary.device_count
            ));

            notes.extend(summary.notes);

            if summary.pid_observed {
                if summary.memory_bytes.is_none() {
                    notes.push(nvml_memory_unavailable_note());
                }

                Some(BackendObservation {
                    memory_bytes: summary.memory_bytes,
                    source: Some("NVML running-process APIs".to_string()),
                    backend: "nvml_process_listing".to_string(),
                    pid_observed: Some(true),
                    note: Some(notes.join(" ")),
                })
            } else {
                notes.push(no_matching_nvml_pid_note(pid));

                Some(BackendObservation {
                    memory_bytes: None,
                    source: Some("NVML running-process APIs".to_string()),
                    backend: "nvml_process_listing".to_string(),
                    pid_observed: Some(false),
                    note: Some(notes.join(" ")),
                })
            }
        }
        Err(error) => Some(BackendObservation {
            memory_bytes: None,
            source: Some("NVML running-process APIs".to_string()),
            backend: "nvml_process_listing".to_string(),
            pid_observed: None,
            note: Some(format!("NVML process inspection failed: {error}.")),
        }),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn inspect_process_via_nvml(_pid: u32) -> Option<BackendObservation> {
    None
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn nvml_library_candidates() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["nvml.dll", "nvml64.dll"]
    }

    #[cfg(target_os = "linux")]
    {
        &["libnvidia-ml.so.1", "libnvidia-ml.so"]
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
impl From<NvmlProcessInfoV3> for NvmlProcessRecord {
    fn from(info: NvmlProcessInfoV3) -> Self {
        Self {
            pid: info.pid,
            memory_bytes: if info.used_gpu_memory == NVML_VALUE_NOT_AVAILABLE {
                None
            } else {
                Some(info.used_gpu_memory)
            },
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Debug, Clone, Copy)]
struct NvmlProcessRecord {
    pid: u32,
    memory_bytes: Option<u64>,
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn no_matching_nvml_pid_note(pid: u32) -> String {
    format!("NVML did not report a matching GPU process entry for llama-server PID {pid}.")
}

#[cfg(target_os = "windows")]
fn nvml_memory_unavailable_note() -> String {
    "NVML observed a matching GPU PID, but memory bytes were unavailable. On Windows WDDM systems, NVML usedGpuMemory is often not exposed even when the process is real and GPU-offloaded."
        .to_string()
}

#[cfg(all(
    any(target_os = "linux", target_os = "windows"),
    not(target_os = "windows")
))]
fn nvml_memory_unavailable_note() -> String {
    "NVML observed a matching GPU PID, but per-process GPU memory bytes were unavailable from the current driver/runtime path."
        .to_string()
}

#[cfg(target_os = "windows")]
fn no_matching_gpu_pid_note() -> String {
    "No matching NVIDIA GPU process entry was found for the llama-server PID. On Windows WDDM systems, per-process VRAM visibility can be incomplete even when GPU offload was requested."
        .to_string()
}

#[cfg(not(target_os = "windows"))]
fn no_matching_gpu_pid_note() -> String {
    "No matching NVIDIA GPU process entry was found for the llama-server PID at observation time."
        .to_string()
}

#[cfg(target_os = "windows")]
fn compute_apps_memory_unavailable_note(memory_raw: &str) -> String {
    format!(
        "llama-server PID was observed in nvidia-smi compute-apps, but per-process GPU memory bytes were unavailable ({memory_raw}). On Windows WDDM systems, used_gpu_memory may remain hidden even when GPU offload is active."
    )
}

#[cfg(not(target_os = "windows"))]
fn compute_apps_memory_unavailable_note(memory_raw: &str) -> String {
    format!(
        "llama-server PID was observed in nvidia-smi compute-apps, but per-process GPU memory bytes were unavailable ({memory_raw})."
    )
}

fn merge_labels(prior_labels: &[String], current_label: Option<&str>) -> String {
    let mut merged = Vec::new();

    for label in prior_labels {
        if !merged.iter().any(|existing: &String| existing == label) {
            merged.push(label.clone());
        }
    }

    if let Some(label) = current_label {
        if !merged.iter().any(|existing| existing == label) {
            merged.push(label.to_string());
        }
    }

    merged.join(" + ")
}
