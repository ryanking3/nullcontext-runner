use crate::config::SessionConfig;
use crate::logging::stdout_line;
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
#[cfg(target_os = "windows")]
use serde_json::Value;
use std::io::Read;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ManagedRuntime {
    child: Child,
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeShutdownOutcome {
    pub stopped: bool,
    pub shutdown_method: String,
    pub exit_code: Option<i32>,
    pub graceful_shutdown_supported: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeUsageSnapshot {
    pub resident_bytes: Option<u64>,
    pub virtual_bytes: Option<u64>,
    pub process_memory_source: Option<String>,
    pub physical_footprint_bytes: Option<u64>,
    pub physical_footprint_peak_bytes: Option<u64>,
    pub vmmap_summary_source: Option<String>,
    pub resident_regions: Vec<RuntimeResidentRegion>,
    pub gpu_pid_observed: Option<bool>,
    pub gpu_memory_bytes: Option<u64>,
    pub gpu_memory_source: Option<String>,
    pub observation_notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeResidentRegion {
    pub region_type: String,
    pub virtual_bytes: u64,
    pub resident_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct RuntimePostShutdownObservation {
    pub process_present_after_shutdown: Option<bool>,
    pub process_check_source: Option<String>,
    pub process_resident_bytes_after_shutdown: Option<u64>,
    pub process_virtual_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_peak_bytes_after_shutdown: Option<u64>,
    pub vmmap_summary_source_after_shutdown: Option<String>,
    pub resident_regions_after_shutdown: Vec<RuntimeResidentRegion>,
    pub verification_window_ms: u64,
    pub gpu_entry_present_after_shutdown: Option<bool>,
    pub gpu_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_check_source: Option<String>,
    pub observation_notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuntimeGpuObservation {
    memory_bytes: Option<u64>,
    source: Option<String>,
    pid_observed: Option<bool>,
    note: Option<String>,
}

const POST_SHUTDOWN_VERIFICATION_WINDOW_MS: u64 = 1500;
const POST_SHUTDOWN_VERIFICATION_INTERVAL_MS: u64 = 150;

impl ManagedRuntime {
    pub fn launch(config: &SessionConfig) -> Result<Self> {
        stdout_line("Launching llama-server...");
        let port = reserve_local_runtime_port()?;
        let base_url = format!("http://127.0.0.1:{port}");

        let child = Command::new(&config.llama_path)
            .arg("-m")
            .arg(&config.model_path)
            .arg("-ngl")
            .arg(&config.gpu_layers)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to launch llama-server at {}", config.llama_path))?;

        let mut runtime = Self { child, base_url };

        if let Err(error) = runtime.wait_until_ready(Duration::from_secs(60)) {
            return Err(runtime.build_failed_launch_error(error));
        }

        stdout_line(format!("Runtime endpoint: {}", runtime.base_url));
        stdout_line("Runtime healthy.");

        Ok(runtime)
    }

    pub fn completion_url(&self) -> String {
        format!("{}/completion", self.base_url)
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn observe_usage(&self) -> RuntimeUsageSnapshot {
        let pid = self.pid();
        let mut snapshot = observe_process_memory(pid);

        let gpu = observe_gpu_memory(pid);
        snapshot.gpu_pid_observed = gpu.pid_observed;
        snapshot.gpu_memory_bytes = gpu.memory_bytes;
        snapshot.gpu_memory_source = gpu.source;

        if let Some(note) = gpu.note {
            snapshot.observation_notes.push(note);
        }

        snapshot
    }

    pub fn shutdown(&mut self) -> Result<RuntimeShutdownOutcome> {
        stdout_line("Shutting down runtime...");

        match self.child.try_wait()? {
            Some(status) => Ok(RuntimeShutdownOutcome {
                stopped: true,
                shutdown_method: "already_exited".to_string(),
                exit_code: status.code(),
                graceful_shutdown_supported: false,
            }),
            None => {
                self.child.kill()?;
                let status = self.child.wait()?;
                Ok(RuntimeShutdownOutcome {
                    stopped: true,
                    shutdown_method: "forced_kill_wait".to_string(),
                    exit_code: status.code(),
                    graceful_shutdown_supported: false,
                })
            }
        }
    }

    fn wait_until_ready(&mut self, timeout: Duration) -> Result<()> {
        stdout_line("Waiting for runtime readiness...");

        let client = Client::new();
        let health_url = format!("{}/health", self.base_url);
        let started_at = Instant::now();
        let mut last_probe_result = "no readiness probe completed".to_string();

        while started_at.elapsed() < timeout {
            if let Some(status) = self.child.try_wait()? {
                let stderr = read_child_stderr(&mut self.child);
                let stdout = read_child_stdout(&mut self.child);

                bail!(
                    "llama-server exited before becoming ready. status: {}\nstdout:\n{}\nstderr:\n{}",
                    status,
                    stdout,
                    stderr
                );
            }

            match client.get(&health_url).send() {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                }
                Ok(response) => {
                    last_probe_result =
                        format!("received HTTP {} from {}", response.status(), health_url);
                    thread::sleep(Duration::from_millis(250));
                }
                Err(error) => {
                    last_probe_result = format!("request to {} failed: {}", health_url, error);
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }

        bail!(
            "llama-server did not become ready within {:?}. Last readiness probe: {}.",
            timeout,
            last_probe_result
        )
    }

    fn build_failed_launch_error(&mut self, startup_error: anyhow::Error) -> anyhow::Error {
        let pid = self.pid();
        let cleanup_result = self.shutdown();
        let stdout = read_child_stdout(&mut self.child);
        let stderr = read_child_stderr(&mut self.child);

        let cleanup_summary = match cleanup_result {
            Ok(outcome) => format!(
                "NullContext cleaned up the failed startup runtime using {} (exit code {}).",
                outcome.shutdown_method,
                outcome
                    .exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            ),
            Err(error) => format!(
                "NullContext also failed to clean up the startup runtime automatically: {error}."
            ),
        };

        let mut details = format!(
            "llama-server failed to become ready on {} (pid {}). {} {}",
            self.base_url, pid, startup_error, cleanup_summary
        );

        if !stdout.trim().is_empty() {
            details.push_str(&format!("\nstdout:\n{stdout}"));
        }

        if !stderr.trim().is_empty() {
            details.push_str(&format!("\nstderr:\n{stderr}"));
        }

        anyhow::anyhow!(details)
    }
}

fn reserve_local_runtime_port() -> Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .context("Failed to reserve a localhost port for llama-server")?;
    let port = listener
        .local_addr()
        .context("Failed to read reserved localhost port for llama-server")?
        .port();
    drop(listener);
    Ok(port)
}

pub fn observe_post_shutdown(pid: u32) -> RuntimePostShutdownObservation {
    let started_at = Instant::now();
    let mut process_present_after_shutdown;
    let mut process_check_source;
    let mut process_note = None;

    loop {
        let (present, source, note) = observe_process_presence(pid);
        process_present_after_shutdown = present;
        process_check_source = source;

        if note.is_some() {
            process_note = note;
        }

        if present == Some(false)
            || started_at.elapsed() >= Duration::from_millis(POST_SHUTDOWN_VERIFICATION_WINDOW_MS)
        {
            break;
        }

        thread::sleep(Duration::from_millis(
            POST_SHUTDOWN_VERIFICATION_INTERVAL_MS,
        ));
    }

    let post_shutdown_process_sample = if process_present_after_shutdown == Some(true) {
        Some(observe_process_memory(pid))
    } else {
        None
    };

    let gpu = observe_gpu_memory(pid);
    let gpu_entry_present_after_shutdown = gpu.pid_observed;

    let mut observation_notes = Vec::new();

    if let Some(note) = process_note {
        observation_notes.push(note);
    }

    if let Some(note) = gpu.note {
        observation_notes.push(note);
    }

    RuntimePostShutdownObservation {
        process_present_after_shutdown,
        process_check_source,
        process_resident_bytes_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .and_then(|sample| sample.resident_bytes),
        process_virtual_bytes_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .and_then(|sample| sample.virtual_bytes),
        physical_footprint_bytes_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .and_then(|sample| sample.physical_footprint_bytes),
        physical_footprint_peak_bytes_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .and_then(|sample| sample.physical_footprint_peak_bytes),
        vmmap_summary_source_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .and_then(|sample| sample.vmmap_summary_source.clone()),
        resident_regions_after_shutdown: post_shutdown_process_sample
            .as_ref()
            .map(|sample| sample.resident_regions.clone())
            .unwrap_or_default(),
        verification_window_ms: POST_SHUTDOWN_VERIFICATION_WINDOW_MS,
        gpu_entry_present_after_shutdown,
        gpu_memory_bytes_after_shutdown: gpu.memory_bytes,
        gpu_check_source: gpu.source,
        observation_notes,
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn observe_process_memory(pid: u32) -> RuntimeUsageSnapshot {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-o", "vsz=", "-p", &pid.to_string()])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let mut parts = raw.split_whitespace();
            let resident_kb = parts.next().and_then(|value| value.parse::<u64>().ok());
            let virtual_kb = parts.next().and_then(|value| value.parse::<u64>().ok());

            RuntimeUsageSnapshot {
                resident_bytes: resident_kb.map(|value| value * 1024),
                virtual_bytes: virtual_kb.map(|value| value * 1024),
                process_memory_source: Some("ps rss/vsz".to_string()),
                physical_footprint_bytes: None,
                physical_footprint_peak_bytes: None,
                vmmap_summary_source: None,
                resident_regions: vec![],
                gpu_pid_observed: None,
                gpu_memory_bytes: None,
                gpu_memory_source: None,
                observation_notes: vec![],
            }
        }
        Ok(output) => RuntimeUsageSnapshot {
            resident_bytes: None,
            virtual_bytes: None,
            process_memory_source: None,
            physical_footprint_bytes: None,
            physical_footprint_peak_bytes: None,
            vmmap_summary_source: None,
            resident_regions: vec![],
            gpu_pid_observed: None,
            gpu_memory_bytes: None,
            gpu_memory_source: None,
            observation_notes: vec![format!(
                "Process memory observation via ps failed with status {}.",
                output.status
            )],
        },
        Err(error) => RuntimeUsageSnapshot {
            resident_bytes: None,
            virtual_bytes: None,
            process_memory_source: None,
            physical_footprint_bytes: None,
            physical_footprint_peak_bytes: None,
            vmmap_summary_source: None,
            resident_regions: vec![],
            gpu_pid_observed: None,
            gpu_memory_bytes: None,
            gpu_memory_source: None,
            observation_notes: vec![format!(
                "Process memory observation via ps was unavailable: {error}."
            )],
        },
    }
}

#[cfg(target_os = "windows")]
fn observe_process_memory(_pid: u32) -> RuntimeUsageSnapshot {
    let pid = _pid;
    let script = format!(
        concat!(
            "$proc = Get-Process -Id {pid} -ErrorAction SilentlyContinue; ",
            "if (-not $proc) {{ exit 3 }}; ",
            "$cim = Get-CimInstance Win32_Process -Filter \"ProcessId = {pid}\" -ErrorAction SilentlyContinue; ",
            "$pagefileBytes = $null; ",
            "if ($cim -and $cim.PageFileUsage -ne $null) {{ $pagefileBytes = [uint64]$cim.PageFileUsage * 1024 }}; ",
            "[pscustomobject]@{{ ",
            "working_set_bytes = [uint64]$proc.WorkingSet64; ",
            "virtual_bytes = [uint64]$proc.VirtualMemorySize64; ",
            "private_bytes = [uint64]$proc.PrivateMemorySize64; ",
            "paged_memory_bytes = [uint64]$proc.PagedMemorySize64; ",
            "nonpaged_system_bytes = [uint64]$proc.NonpagedSystemMemorySize64; ",
            "pagefile_bytes = $pagefileBytes ",
            "}} | ConvertTo-Json -Compress"
        ),
        pid = pid
    );

    match run_windows_powershell(&script) {
        Ok(output) if output.status.success() => {
            match serde_json::from_slice::<Value>(&output.stdout) {
                Ok(value) => {
                    let resident_bytes = value.get("working_set_bytes").and_then(Value::as_u64);
                    let virtual_bytes = value.get("virtual_bytes").and_then(Value::as_u64);
                    let private_bytes = value.get("private_bytes").and_then(Value::as_u64);
                    let paged_memory_bytes =
                        value.get("paged_memory_bytes").and_then(Value::as_u64);
                    let nonpaged_system_bytes =
                        value.get("nonpaged_system_bytes").and_then(Value::as_u64);
                    let pagefile_bytes = value.get("pagefile_bytes").and_then(Value::as_u64);

                    let mut observation_notes = Vec::new();

                    if let Some(value) = private_bytes {
                        observation_notes.push(format!(
                            "Windows private bytes observed via Get-Process: {value} bytes."
                        ));
                    }

                    if let Some(value) = pagefile_bytes {
                        observation_notes.push(format!(
                        "Windows pagefile-backed usage observed via Win32_Process: {value} bytes."
                    ));
                    }

                    if let Some(value) = paged_memory_bytes {
                        observation_notes.push(format!(
                            "Windows paged memory observed via Get-Process: {value} bytes."
                        ));
                    }

                    if let Some(value) = nonpaged_system_bytes {
                        observation_notes.push(format!(
                        "Windows nonpaged system memory observed via Get-Process: {value} bytes."
                    ));
                    }

                    RuntimeUsageSnapshot {
                        resident_bytes,
                        virtual_bytes,
                        process_memory_source: Some(
                            "PowerShell Get-Process + CIM Win32_Process".to_string(),
                        ),
                        physical_footprint_bytes: None,
                        physical_footprint_peak_bytes: None,
                        vmmap_summary_source: None,
                        resident_regions: vec![],
                        gpu_pid_observed: None,
                        gpu_memory_bytes: None,
                        gpu_memory_source: None,
                        observation_notes,
                    }
                }
                Err(error) => RuntimeUsageSnapshot {
                    resident_bytes: None,
                    virtual_bytes: None,
                    process_memory_source: None,
                    physical_footprint_bytes: None,
                    physical_footprint_peak_bytes: None,
                    vmmap_summary_source: None,
                    resident_regions: vec![],
                    gpu_pid_observed: None,
                    gpu_memory_bytes: None,
                    gpu_memory_source: None,
                    observation_notes: vec![format!(
                        "Windows process memory observation returned unparsable JSON: {error}."
                    )],
                },
            }
        }
        Ok(output) => RuntimeUsageSnapshot {
            resident_bytes: None,
            virtual_bytes: None,
            process_memory_source: None,
            physical_footprint_bytes: None,
            physical_footprint_peak_bytes: None,
            vmmap_summary_source: None,
            resident_regions: vec![],
            gpu_pid_observed: None,
            gpu_memory_bytes: None,
            gpu_memory_source: None,
            observation_notes: vec![format!(
                "Windows process memory observation via PowerShell failed with status {}.",
                output.status
            )],
        },
        Err(error) => RuntimeUsageSnapshot {
            resident_bytes: None,
            virtual_bytes: None,
            process_memory_source: None,
            physical_footprint_bytes: None,
            physical_footprint_peak_bytes: None,
            vmmap_summary_source: None,
            resident_regions: vec![],
            gpu_pid_observed: None,
            gpu_memory_bytes: None,
            gpu_memory_source: None,
            observation_notes: vec![format!(
                "Windows process memory observation via PowerShell was unavailable: {error}."
            )],
        },
    }
}

#[cfg(not(any(unix, target_os = "windows")))]
fn observe_process_memory(_pid: u32) -> RuntimeUsageSnapshot {
    RuntimeUsageSnapshot {
        resident_bytes: None,
        virtual_bytes: None,
        process_memory_source: None,
        physical_footprint_bytes: None,
        physical_footprint_peak_bytes: None,
        vmmap_summary_source: None,
        resident_regions: vec![],
        gpu_pid_observed: None,
        gpu_memory_bytes: None,
        gpu_memory_source: None,
        observation_notes: vec![
            "Process memory observation is not yet implemented on this platform.".to_string(),
        ],
    }
}

fn observe_gpu_memory(pid: u32) -> RuntimeGpuObservation {
    let mut prior_notes = Vec::new();
    let mut prior_sources = Vec::new();

    if let Some(observation) = observe_gpu_memory_compute_apps(pid) {
        if observation.pid_observed == Some(true) && observation.memory_bytes.is_some() {
            return observation;
        }

        if let Some(source) = observation.source {
            prior_sources.push(source);
        }

        if let Some(note) = observation.note {
            prior_notes.push(note);
        }
    }

    if let Some(mut observation) = observe_gpu_memory_pmon(pid) {
        if !prior_sources.is_empty() {
            observation.source = Some(merge_gpu_sources(
                &prior_sources,
                observation.source.as_deref(),
            ));
        }

        if !prior_notes.is_empty() {
            observation.note = Some(match observation.note.take() {
                Some(note) => format!("{} {}", prior_notes.join(" "), note),
                None => prior_notes.join(" "),
            });
        }

        return observation;
    }

    RuntimeGpuObservation {
        memory_bytes: None,
        source: if prior_sources.is_empty() {
            None
        } else {
            Some(merge_gpu_sources(&prior_sources, None))
        },
        pid_observed: None,
        note: Some(if prior_notes.is_empty() {
            "GPU memory observation via nvidia-smi was unavailable or inconclusive.".to_string()
        } else {
            prior_notes.join(" ")
        }),
    }
}

fn observe_gpu_memory_compute_apps(pid: u32) -> Option<RuntimeGpuObservation> {
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
                    return Some(RuntimeGpuObservation {
                        memory_bytes: memory_mb.map(|value| value * 1024 * 1024),
                        source: Some("nvidia-smi compute-apps".to_string()),
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
        Ok(output) => Some(RuntimeGpuObservation {
            memory_bytes: None,
            source: None,
            pid_observed: None,
            note: Some(format!(
                "GPU memory observation via nvidia-smi compute-apps failed with status {}.",
                output.status
            )),
        }),
        Err(error) => Some(RuntimeGpuObservation {
            memory_bytes: None,
            source: None,
            pid_observed: None,
            note: Some(format!(
                "GPU memory observation via nvidia-smi compute-apps was unavailable: {error}."
            )),
        }),
    }
}

fn observe_gpu_memory_pmon(pid: u32) -> Option<RuntimeGpuObservation> {
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
                    return Some(RuntimeGpuObservation {
                        memory_bytes: None,
                        source: Some("nvidia-smi pmon".to_string()),
                        pid_observed: Some(true),
                        note: Some(
                            "llama-server PID was observed in nvidia-smi pmon, but per-process GPU memory bytes were unavailable from the current NVIDIA tooling path."
                                .to_string(),
                        ),
                    });
                }
            }

            Some(RuntimeGpuObservation {
                memory_bytes: None,
                source: Some("nvidia-smi compute-apps + pmon".to_string()),
                pid_observed: Some(false),
                note: Some(no_matching_gpu_pid_note()),
            })
        }
        Ok(output) => Some(RuntimeGpuObservation {
            memory_bytes: None,
            source: None,
            pid_observed: None,
            note: Some(format!(
                "GPU process observation via nvidia-smi pmon failed with status {}.",
                output.status
            )),
        }),
        Err(error) => Some(RuntimeGpuObservation {
            memory_bytes: None,
            source: None,
            pid_observed: None,
            note: Some(format!(
                "GPU process observation via nvidia-smi pmon was unavailable: {error}."
            )),
        }),
    }
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

fn merge_gpu_sources(prior_sources: &[String], current_source: Option<&str>) -> String {
    let mut merged = Vec::new();

    for source in prior_sources {
        if !merged.iter().any(|existing: &String| existing == source) {
            merged.push(source.clone());
        }
    }

    if let Some(source) = current_source {
        if !merged.iter().any(|existing| existing == source) {
            merged.push(source.to_string());
        }
    }

    merged.join(" + ")
}

fn observe_process_presence(pid: u32) -> (Option<bool>, Option<String>, Option<String>) {
    #[cfg(unix)]
    {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let present = stdout
                    .lines()
                    .any(|line| line.trim().parse::<u32>().ok() == Some(pid));

                (Some(present), Some("ps pid".to_string()), None)
            }
            Ok(output) => (
                None,
                None,
                Some(format!(
                    "Post-shutdown process presence check via ps failed with status {}.",
                    output.status
                )),
            ),
            Err(error) => (
                None,
                None,
                Some(format!(
                    "Post-shutdown process presence check via ps was unavailable: {error}."
                )),
            ),
        }
    }

    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'present' }} else {{ 'absent' }}",
            pid = pid
        );

        match run_windows_powershell(&script) {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let present = stdout.trim().eq_ignore_ascii_case("present");

                (
                    Some(present),
                    Some("PowerShell Get-Process".to_string()),
                    None,
                )
            }
            Ok(output) => (
                None,
                None,
                Some(format!(
                    "Post-shutdown process presence check via PowerShell failed with status {}.",
                    output.status
                )),
            ),
            Err(error) => (
                None,
                None,
                Some(format!(
                    "Post-shutdown process presence check via PowerShell was unavailable: {error}."
                )),
            ),
        }
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        (
            None,
            None,
            Some(
                "Post-shutdown process presence observation is not yet implemented on this platform."
                    .to_string(),
            ),
        )
    }
}

#[cfg(target_os = "macos")]
fn observe_process_memory(pid: u32) -> RuntimeUsageSnapshot {
    let mut snapshot = {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-o", "vsz=", "-p", &pid.to_string()])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let raw = String::from_utf8_lossy(&output.stdout);
                let mut parts = raw.split_whitespace();
                let resident_kb = parts.next().and_then(|value| value.parse::<u64>().ok());
                let virtual_kb = parts.next().and_then(|value| value.parse::<u64>().ok());

                RuntimeUsageSnapshot {
                    resident_bytes: resident_kb.map(|value| value * 1024),
                    virtual_bytes: virtual_kb.map(|value| value * 1024),
                    process_memory_source: Some("ps rss/vsz".to_string()),
                    physical_footprint_bytes: None,
                    physical_footprint_peak_bytes: None,
                    vmmap_summary_source: None,
                    resident_regions: vec![],
                    gpu_pid_observed: None,
                    gpu_memory_bytes: None,
                    gpu_memory_source: None,
                    observation_notes: vec![],
                }
            }
            Ok(output) => RuntimeUsageSnapshot {
                resident_bytes: None,
                virtual_bytes: None,
                process_memory_source: None,
                physical_footprint_bytes: None,
                physical_footprint_peak_bytes: None,
                vmmap_summary_source: None,
                resident_regions: vec![],
                gpu_pid_observed: None,
                gpu_memory_bytes: None,
                gpu_memory_source: None,
                observation_notes: vec![format!(
                    "Process memory observation via ps failed with status {}.",
                    output.status
                )],
            },
            Err(error) => RuntimeUsageSnapshot {
                resident_bytes: None,
                virtual_bytes: None,
                process_memory_source: None,
                physical_footprint_bytes: None,
                physical_footprint_peak_bytes: None,
                vmmap_summary_source: None,
                resident_regions: vec![],
                gpu_pid_observed: None,
                gpu_memory_bytes: None,
                gpu_memory_source: None,
                observation_notes: vec![format!(
                    "Process memory observation via ps was unavailable: {error}."
                )],
            },
        }
    };

    let output = Command::new("vmmap")
        .args(["-summary", &pid.to_string()])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            snapshot.physical_footprint_bytes = find_vmmap_value(&raw, "Physical footprint:");
            snapshot.physical_footprint_peak_bytes =
                find_vmmap_value(&raw, "Physical footprint (peak):");
            snapshot.vmmap_summary_source = Some("vmmap -summary".to_string());
            snapshot.resident_regions = parse_vmmap_resident_regions(&raw);

            if snapshot.process_memory_source.is_some() {
                snapshot.process_memory_source = Some("ps rss/vsz + vmmap -summary".to_string());
            } else {
                snapshot.process_memory_source = Some("vmmap -summary".to_string());
            }
        }
        Ok(output) => {
            snapshot.observation_notes.push(format!(
                "macOS vmmap summary observation failed with status {}.",
                output.status
            ));
        }
        Err(error) => {
            snapshot.observation_notes.push(format!(
                "macOS vmmap summary observation was unavailable: {error}."
            ));
        }
    }

    snapshot
}

#[cfg(target_os = "macos")]
fn find_vmmap_value(raw: &str, prefix: &str) -> Option<u64> {
    raw.lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .and_then(|line| line.split(':').nth(1))
        .and_then(|value| parse_vmmap_size(value.trim()))
}

#[cfg(target_os = "macos")]
fn parse_vmmap_resident_regions(raw: &str) -> Vec<RuntimeResidentRegion> {
    let mut in_table = false;
    let mut regions = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("REGION TYPE") {
            in_table = true;
            continue;
        }

        if !in_table {
            continue;
        }

        if trimmed.starts_with("TOTAL") || trimmed.starts_with("MALLOC ZONE") {
            break;
        }

        if trimmed.is_empty() || trimmed.starts_with("===========") {
            continue;
        }

        if let Some(region) = parse_vmmap_region_line(trimmed) {
            if region.resident_bytes > 0 {
                regions.push(region);
            }
        }
    }

    regions.sort_by(|a, b| b.resident_bytes.cmp(&a.resident_bytes));
    regions.truncate(6);
    regions
}

#[cfg(target_os = "macos")]
fn parse_vmmap_region_line(line: &str) -> Option<RuntimeResidentRegion> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let numeric_index = tokens
        .iter()
        .position(|token| parse_vmmap_size(token).is_some())?;

    if numeric_index == 0 || tokens.len() <= numeric_index + 1 {
        return None;
    }

    let region_type = tokens[..numeric_index].join(" ");
    let virtual_bytes = parse_vmmap_size(tokens[numeric_index])?;
    let resident_bytes = parse_vmmap_size(tokens[numeric_index + 1])?;

    Some(RuntimeResidentRegion {
        region_type,
        virtual_bytes,
        resident_bytes,
    })
}

#[cfg(target_os = "macos")]
fn parse_vmmap_size(value: &str) -> Option<u64> {
    let trimmed = value.trim().trim_end_matches('%');

    if trimmed.is_empty() {
        return None;
    }

    let (number_part, unit) = trimmed.chars().last().map(|last| {
        if last.is_ascii_alphabetic() {
            (&trimmed[..trimmed.len() - 1], Some(last))
        } else {
            (trimmed, None)
        }
    })?;

    let number = number_part.parse::<f64>().ok()?;
    let multiplier = match unit.map(|value| value.to_ascii_uppercase()) {
        Some('K') => 1024.0,
        Some('M') => 1024.0 * 1024.0,
        Some('G') => 1024.0 * 1024.0 * 1024.0,
        Some('T') => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        Some('B') | None => 1.0,
        _ => return None,
    };

    Some((number * multiplier) as u64)
}

#[cfg(target_os = "windows")]
fn run_windows_powershell(script: &str) -> std::io::Result<std::process::Output> {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
}

fn read_child_stderr(child: &mut Child) -> String {
    let Some(stderr) = child.stderr.as_mut() else {
        return String::new();
    };

    let mut output = String::new();
    let _ = stderr.read_to_string(&mut output);
    output
}

fn read_child_stdout(child: &mut Child) -> String {
    let Some(stdout) = child.stdout.as_mut() else {
        return String::new();
    };

    let mut output = String::new();
    let _ = stdout.read_to_string(&mut output);
    output
}
