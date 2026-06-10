use crate::config::SessionConfig;
use crate::gpu_inspection::observe_gpu_process;
use crate::logging::stdout_line;
use crate::runtime_introspection::{
    parse_runtime_introspection_signals, RuntimeIntrospectionSignal,
};
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
#[cfg(target_os = "windows")]
use serde_json::Value;
use std::fmt;
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
    pub introspection_signals: Vec<RuntimeIntrospectionSignal>,
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
    pub gpu_observation_backend: Option<String>,
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
    pub gpu_peak_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_samples_collected_after_shutdown: u32,
    pub gpu_samples_with_pid_observed_after_shutdown: u32,
    pub gpu_last_pid_observed_at_ms: Option<u64>,
    pub gpu_check_backend: Option<String>,
    pub gpu_check_source: Option<String>,
    pub vram_cleanup_strategy_id: Option<String>,
    pub vram_cleanup_strategy_windows: Vec<RuntimeGpuObservationStrategyStage>,
    pub observation_notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeGpuObservationWindow {
    pub verification_window_ms: u64,
    pub gpu_entry_present: Option<bool>,
    pub gpu_memory_bytes: Option<u64>,
    pub gpu_peak_memory_bytes: Option<u64>,
    pub gpu_samples_collected: u32,
    pub gpu_samples_with_pid_observed: u32,
    pub gpu_last_pid_observed_at_ms: Option<u64>,
    pub gpu_check_backend: Option<String>,
    pub gpu_check_source: Option<String>,
    pub observation_notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeGpuObservationStrategyStage {
    pub stage_id: String,
    pub stage_label: String,
    pub stage_kind: String,
    pub cooldown_ms_before_stage: u64,
    pub action_status: String,
    pub action_notes: Vec<String>,
    pub window: RuntimeGpuObservationWindow,
}

#[derive(Debug, Clone, Default)]
struct RuntimePostShutdownGpuWindowObservation {
    any_pid_observed: bool,
    any_sample_collected: bool,
    last_memory_bytes: Option<u64>,
    peak_memory_bytes: Option<u64>,
    sample_count: u32,
    samples_with_pid_observed: u32,
    last_pid_observed_at_ms: Option<u64>,
    backends: Vec<String>,
    sources: Vec<String>,
    notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeLaunchFailure {
    pub runtime_pid: u32,
    pub runtime_endpoint: String,
    pub startup_error: String,
    pub cleanup_succeeded: bool,
    pub cleanup_shutdown_method: Option<String>,
    pub cleanup_exit_code: Option<i32>,
    pub cleanup_error: Option<String>,
    pub post_cleanup_observation: RuntimePostShutdownObservation,
    pub stdout: String,
    pub stderr: String,
    pub introspection_signals: Vec<RuntimeIntrospectionSignal>,
}

const POST_SHUTDOWN_VERIFICATION_WINDOW_MS: u64 = 1500;
const POST_SHUTDOWN_VERIFICATION_INTERVAL_MS: u64 = 150;
const VRAM_CLEANUP_STRATEGY_ID: &str = "multi_stage_cooldown_and_relaunch_probe";
const VRAM_CLEANUP_STRATEGY_VERIFICATION_WINDOW_MS: u64 = 1000;
const VRAM_CLEANUP_STRATEGY_STAGES: [VramCleanupStrategyStagePlan; 3] = [
    VramCleanupStrategyStagePlan {
        stage_id: "short_cooldown_recheck",
        stage_label: "Short Cooldown Recheck",
        stage_kind: "cooldown_recheck",
        cooldown_ms_before_stage: 1500,
        perform_helper_relaunch_probe: false,
    },
    VramCleanupStrategyStagePlan {
        stage_id: "extended_cooldown_recheck",
        stage_label: "Extended Cooldown Recheck",
        stage_kind: "cooldown_recheck",
        cooldown_ms_before_stage: 3000,
        perform_helper_relaunch_probe: false,
    },
    VramCleanupStrategyStagePlan {
        stage_id: "helper_runtime_relaunch_probe",
        stage_label: "Helper Runtime Relaunch Probe",
        stage_kind: "helper_runtime_relaunch_probe",
        cooldown_ms_before_stage: 500,
        perform_helper_relaunch_probe: true,
    },
];

#[derive(Clone, Copy)]
struct VramCleanupStrategyStagePlan {
    stage_id: &'static str,
    stage_label: &'static str,
    stage_kind: &'static str,
    cooldown_ms_before_stage: u64,
    perform_helper_relaunch_probe: bool,
}

impl RuntimePostShutdownGpuWindowObservation {
    fn record_sample(
        &mut self,
        elapsed_ms: u64,
        sample: crate::gpu_inspection::GpuInspectionObservation,
    ) {
        self.any_sample_collected = true;
        self.sample_count = self.sample_count.saturating_add(1);

        if sample.pid_observed == Some(true) {
            self.any_pid_observed = true;
            self.samples_with_pid_observed = self.samples_with_pid_observed.saturating_add(1);
            self.last_pid_observed_at_ms = Some(elapsed_ms);
            self.last_memory_bytes = sample.memory_bytes;

            if let Some(memory_bytes) = sample.memory_bytes {
                self.peak_memory_bytes = Some(
                    self.peak_memory_bytes
                        .map(|existing| existing.max(memory_bytes))
                        .unwrap_or(memory_bytes),
                );
            }
        }

        push_unique_label(&mut self.backends, sample.backend);
        push_unique_label(&mut self.sources, sample.source);
        push_unique_label(&mut self.notes, sample.note);
    }

    fn finalize(self, verification_window_ms: u64) -> RuntimeGpuObservationWindow {
        let mut observation_notes = self.notes;

        if self.sample_count > 0 {
            observation_notes.push(format!(
                "GPU inspection collected {} sample(s) over the {} ms window; {} sample(s) observed a matching GPU PID.",
                self.sample_count, verification_window_ms, self.samples_with_pid_observed
            ));
        }

        RuntimeGpuObservationWindow {
            verification_window_ms,
            gpu_entry_present: if self.any_sample_collected {
                Some(self.any_pid_observed)
            } else {
                None
            },
            gpu_memory_bytes: self.last_memory_bytes,
            gpu_peak_memory_bytes: self.peak_memory_bytes,
            gpu_samples_collected: self.sample_count,
            gpu_samples_with_pid_observed: self.samples_with_pid_observed,
            gpu_last_pid_observed_at_ms: self.last_pid_observed_at_ms,
            gpu_check_backend: if self.backends.is_empty() {
                None
            } else {
                Some(merge_observation_labels(&self.backends))
            },
            gpu_check_source: if self.sources.is_empty() {
                None
            } else {
                Some(merge_observation_labels(&self.sources))
            },
            observation_notes,
        }
    }
}

fn push_unique_label(target: &mut Vec<String>, value: Option<String>) {
    let Some(value) = value else {
        return;
    };

    if !target.iter().any(|existing| existing == &value) {
        target.push(value);
    }
}

fn merge_observation_labels(labels: &[String]) -> String {
    labels.join(" + ")
}

fn gpu_offload_requested(config: &SessionConfig) -> bool {
    config.gpu_layers.parse::<u32>().unwrap_or(0) > 0
}

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
            return Err(runtime.build_failed_launch_error(error, config));
        }

        stdout_line(format!("Runtime endpoint: {}", runtime.base_url));
        stdout_line("Runtime healthy.");

        Ok(runtime)
    }

    pub fn completion_url(&self) -> String {
        format!("{}/completion", self.base_url)
    }

    pub fn endpoint_url(&self) -> &str {
        &self.base_url
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn observe_usage(&self) -> RuntimeUsageSnapshot {
        let pid = self.pid();
        let mut snapshot = observe_process_memory(pid);

        let gpu = observe_gpu_process(pid);
        snapshot.gpu_pid_observed = gpu.pid_observed;
        snapshot.gpu_memory_bytes = gpu.memory_bytes;
        snapshot.gpu_observation_backend = gpu.backend;
        snapshot.gpu_memory_source = gpu.source;

        if let Some(note) = gpu.note {
            snapshot.observation_notes.push(note);
        }

        snapshot
    }

    pub fn shutdown(&mut self) -> Result<RuntimeShutdownOutcome> {
        stdout_line("Shutting down runtime...");

        match self.child.try_wait()? {
            Some(status) => {
                let stdout = read_child_stdout(&mut self.child);
                let stderr = read_child_stderr(&mut self.child);
                let introspection_signals = parse_runtime_introspection_signals(&stdout, &stderr);

                Ok(RuntimeShutdownOutcome {
                    stopped: true,
                    shutdown_method: "already_exited".to_string(),
                    exit_code: status.code(),
                    graceful_shutdown_supported: false,
                    introspection_signals,
                })
            }
            None => {
                self.child.kill()?;
                let status = self.child.wait()?;
                let stdout = read_child_stdout(&mut self.child);
                let stderr = read_child_stderr(&mut self.child);
                let introspection_signals = parse_runtime_introspection_signals(&stdout, &stderr);

                Ok(RuntimeShutdownOutcome {
                    stopped: true,
                    shutdown_method: "forced_kill_wait".to_string(),
                    exit_code: status.code(),
                    graceful_shutdown_supported: false,
                    introspection_signals,
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

    fn build_failed_launch_error(
        &mut self,
        startup_error: anyhow::Error,
        config: &SessionConfig,
    ) -> anyhow::Error {
        let pid = self.pid();
        let cleanup_result = self.shutdown();
        let post_cleanup_observation =
            observe_post_shutdown(pid, gpu_offload_requested(config), None);
        let stdout = read_child_stdout(&mut self.child);
        let stderr = read_child_stderr(&mut self.child);
        let introspection_signals = parse_runtime_introspection_signals(&stdout, &stderr);

        let failure = match cleanup_result {
            Ok(outcome) => RuntimeLaunchFailure {
                runtime_pid: pid,
                runtime_endpoint: self.base_url.clone(),
                startup_error: startup_error.to_string(),
                cleanup_succeeded: true,
                cleanup_shutdown_method: Some(outcome.shutdown_method),
                cleanup_exit_code: outcome.exit_code,
                cleanup_error: None,
                post_cleanup_observation,
                stdout,
                stderr,
                introspection_signals: outcome.introspection_signals,
            },
            Err(error) => RuntimeLaunchFailure {
                runtime_pid: pid,
                runtime_endpoint: self.base_url.clone(),
                startup_error: startup_error.to_string(),
                cleanup_succeeded: false,
                cleanup_shutdown_method: None,
                cleanup_exit_code: None,
                cleanup_error: Some(error.to_string()),
                post_cleanup_observation,
                stdout,
                stderr,
                introspection_signals,
            },
        };

        anyhow::Error::new(failure)
    }
}

impl fmt::Display for RuntimeLaunchFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "llama-server failed to become ready on {} (pid {}). {} ",
            self.runtime_endpoint, self.runtime_pid, self.startup_error
        )?;

        if self.cleanup_succeeded {
            write!(
                f,
                "NullContext cleaned up the failed startup runtime using {} (exit code {}).",
                self.cleanup_shutdown_method.as_deref().unwrap_or("unknown"),
                self.cleanup_exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            )?;
        } else {
            write!(
                f,
                "NullContext also failed to clean up the startup runtime automatically: {}.",
                self.cleanup_error
                    .as_deref()
                    .unwrap_or("unknown cleanup failure")
            )?;
        }

        if !self.stdout.trim().is_empty() {
            write!(f, "\nstdout:\n{}", self.stdout)?;
        }

        if !self.stderr.trim().is_empty() {
            write!(f, "\nstderr:\n{}", self.stderr)?;
        }

        Ok(())
    }
}

impl std::error::Error for RuntimeLaunchFailure {}

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

pub fn observe_post_shutdown(
    pid: u32,
    gpu_offload_requested: bool,
    strategy_config: Option<&SessionConfig>,
) -> RuntimePostShutdownObservation {
    let started_at = Instant::now();
    let verification_window = Duration::from_millis(POST_SHUTDOWN_VERIFICATION_WINDOW_MS);
    let mut process_present_after_shutdown = None;
    let mut process_check_source = None;
    let mut process_note = None;
    let mut gpu_window = RuntimePostShutdownGpuWindowObservation::default();

    while started_at.elapsed() <= verification_window {
        let (present, source, note) = observe_process_presence(pid);
        process_present_after_shutdown = present;
        process_check_source = source;

        if note.is_some() {
            process_note = note;
        }

        let elapsed_ms = started_at.elapsed().as_millis() as u64;
        let gpu = observe_gpu_process(pid);
        gpu_window.record_sample(elapsed_ms, gpu);

        if started_at.elapsed() >= verification_window {
            break;
        }

        thread::sleep(Duration::from_millis(
            POST_SHUTDOWN_VERIFICATION_INTERVAL_MS,
        ));
    }

    let baseline_gpu_window = gpu_window.finalize(POST_SHUTDOWN_VERIFICATION_WINDOW_MS);

    let vram_cleanup_strategy_windows = if gpu_offload_requested {
        let mut stages = Vec::new();

        for stage_plan in VRAM_CLEANUP_STRATEGY_STAGES {
            thread::sleep(Duration::from_millis(stage_plan.cooldown_ms_before_stage));
            let (action_status, action_notes) = if stage_plan.perform_helper_relaunch_probe {
                match strategy_config {
                    Some(config) => execute_helper_runtime_relaunch_probe(config),
                    None => (
                        "helper_runtime_relaunch_probe_unavailable".to_string(),
                        vec![
                            "Helper runtime relaunch probe was skipped because no session configuration was available during this shutdown observation."
                                .to_string(),
                        ],
                    ),
                }
            } else {
                ("cooldown_recheck_completed".to_string(), vec![])
            };

            stages.push(RuntimeGpuObservationStrategyStage {
                stage_id: stage_plan.stage_id.to_string(),
                stage_label: stage_plan.stage_label.to_string(),
                stage_kind: stage_plan.stage_kind.to_string(),
                cooldown_ms_before_stage: stage_plan.cooldown_ms_before_stage,
                action_status,
                action_notes,
                window: observe_gpu_window(
                    pid,
                    VRAM_CLEANUP_STRATEGY_VERIFICATION_WINDOW_MS,
                    POST_SHUTDOWN_VERIFICATION_INTERVAL_MS,
                ),
            });
        }

        stages
    } else {
        vec![]
    };

    let post_shutdown_process_sample = if process_present_after_shutdown == Some(true) {
        Some(observe_process_memory(pid))
    } else {
        None
    };

    let mut observation_notes = Vec::new();

    if let Some(note) = process_note {
        observation_notes.push(note);
    }

    observation_notes.extend(baseline_gpu_window.observation_notes.clone());

    for strategy_stage in &vram_cleanup_strategy_windows {
        observation_notes.push(format!(
            "Experimental VRAM cleanup stage {} ({}) waited {} ms, completed action status {}, then collected {} GPU sample(s) over a {} ms recheck window.",
            strategy_stage.stage_id,
            strategy_stage.stage_label,
            strategy_stage.cooldown_ms_before_stage,
            strategy_stage.action_status,
            strategy_stage.window.gpu_samples_collected,
            strategy_stage.window.verification_window_ms
        ));
        observation_notes.extend(strategy_stage.action_notes.clone());
        observation_notes.extend(strategy_stage.window.observation_notes.clone());
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
        gpu_entry_present_after_shutdown: baseline_gpu_window.gpu_entry_present,
        gpu_memory_bytes_after_shutdown: baseline_gpu_window.gpu_memory_bytes,
        gpu_peak_memory_bytes_after_shutdown: baseline_gpu_window.gpu_peak_memory_bytes,
        gpu_samples_collected_after_shutdown: baseline_gpu_window.gpu_samples_collected,
        gpu_samples_with_pid_observed_after_shutdown: baseline_gpu_window
            .gpu_samples_with_pid_observed,
        gpu_last_pid_observed_at_ms: baseline_gpu_window.gpu_last_pid_observed_at_ms,
        gpu_check_backend: baseline_gpu_window.gpu_check_backend.clone(),
        gpu_check_source: baseline_gpu_window.gpu_check_source.clone(),
        vram_cleanup_strategy_id: (!vram_cleanup_strategy_windows.is_empty())
            .then(|| VRAM_CLEANUP_STRATEGY_ID.to_string()),
        vram_cleanup_strategy_windows,
        observation_notes,
    }
}

fn observe_gpu_window(
    pid: u32,
    verification_window_ms: u64,
    interval_ms: u64,
) -> RuntimeGpuObservationWindow {
    let started_at = Instant::now();
    let verification_window = Duration::from_millis(verification_window_ms);
    let mut collector = RuntimePostShutdownGpuWindowObservation::default();

    while started_at.elapsed() <= verification_window {
        let elapsed_ms = started_at.elapsed().as_millis() as u64;
        let gpu = observe_gpu_process(pid);
        collector.record_sample(elapsed_ms, gpu);

        if started_at.elapsed() >= verification_window {
            break;
        }

        thread::sleep(Duration::from_millis(interval_ms));
    }

    collector.finalize(verification_window_ms)
}

fn execute_helper_runtime_relaunch_probe(config: &SessionConfig) -> (String, Vec<String>) {
    match ManagedRuntime::launch(config) {
        Ok(mut helper_runtime) => {
            let helper_pid = helper_runtime.pid();
            let helper_endpoint = helper_runtime.endpoint_url().to_string();
            let shutdown_result = helper_runtime.shutdown();

            let mut notes = vec![format!(
                "Helper runtime relaunch probe started a temporary llama-server at {} with pid {}.",
                helper_endpoint, helper_pid
            )];

            match shutdown_result {
                Ok(outcome) => {
                    notes.push(format!(
                        "Helper runtime relaunch probe shut the temporary runtime down using {} (exit code {}).",
                        outcome.shutdown_method,
                        outcome
                            .exit_code
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    ));

                    ("helper_runtime_relaunch_probe_completed".to_string(), notes)
                }
                Err(error) => {
                    notes.push(format!(
                        "Helper runtime relaunch probe started successfully but cleanup failed: {error}."
                    ));

                    (
                        "helper_runtime_relaunch_probe_cleanup_failed".to_string(),
                        notes,
                    )
                }
            }
        }
        Err(error) => (
            "helper_runtime_relaunch_probe_failed".to_string(),
            vec![format!(
                "Helper runtime relaunch probe could not start a temporary runtime: {error}."
            )],
        ),
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
                gpu_observation_backend: None,
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
            gpu_observation_backend: None,
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
            gpu_observation_backend: None,
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
                        gpu_observation_backend: None,
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
                    gpu_observation_backend: None,
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
            gpu_observation_backend: None,
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
            gpu_observation_backend: None,
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
        gpu_observation_backend: None,
        gpu_memory_source: None,
        observation_notes: vec![
            "Process memory observation is not yet implemented on this platform.".to_string(),
        ],
    }
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
                    gpu_observation_backend: None,
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
                gpu_observation_backend: None,
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
                gpu_observation_backend: None,
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
