use crate::config::SessionConfig;
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use std::io::Read;
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
    pub gpu_memory_bytes: Option<u64>,
    pub gpu_memory_source: Option<String>,
    pub observation_notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimePostShutdownObservation {
    pub process_present_after_shutdown: Option<bool>,
    pub process_check_source: Option<String>,
    pub process_resident_bytes_after_shutdown: Option<u64>,
    pub process_virtual_bytes_after_shutdown: Option<u64>,
    pub verification_window_ms: u64,
    pub gpu_entry_present_after_shutdown: Option<bool>,
    pub gpu_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_check_source: Option<String>,
    pub observation_notes: Vec<String>,
}

const POST_SHUTDOWN_VERIFICATION_WINDOW_MS: u64 = 1500;
const POST_SHUTDOWN_VERIFICATION_INTERVAL_MS: u64 = 150;

impl ManagedRuntime {
    pub fn launch(config: &SessionConfig) -> Result<Self> {
        println!("Launching llama-server...");

        let child = Command::new(&config.llama_path)
            .arg("-m")
            .arg(&config.model_path)
            .arg("-ngl")
            .arg(&config.gpu_layers)
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg("8080")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to launch llama-server at {}", config.llama_path))?;

        let mut runtime = Self {
            child,
            base_url: "http://127.0.0.1:8080".to_string(),
        };

        runtime.wait_until_ready(Duration::from_secs(60))?;

        println!("Runtime healthy.");

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

        let (gpu_memory_bytes, gpu_memory_source, gpu_note) = observe_gpu_memory(pid);
        snapshot.gpu_memory_bytes = gpu_memory_bytes;
        snapshot.gpu_memory_source = gpu_memory_source;

        if let Some(note) = gpu_note {
            snapshot.observation_notes.push(note);
        }

        snapshot
    }

    pub fn shutdown(&mut self) -> Result<RuntimeShutdownOutcome> {
        println!("Shutting down runtime...");

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
        println!("Waiting for runtime readiness...");

        let client = Client::new();
        let health_url = format!("{}/health", self.base_url);
        let started_at = Instant::now();

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
                _ => {
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }

        bail!("llama-server did not become ready within {:?}", timeout)
    }
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

    let (gpu_memory_bytes_after_shutdown, gpu_check_source, gpu_note) = observe_gpu_memory(pid);
    let gpu_entry_present_after_shutdown = gpu_check_source
        .as_ref()
        .map(|_| gpu_memory_bytes_after_shutdown.is_some());

    let mut observation_notes = Vec::new();

    if let Some(note) = process_note {
        observation_notes.push(note);
    }

    if let Some(note) = gpu_note {
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
        verification_window_ms: POST_SHUTDOWN_VERIFICATION_WINDOW_MS,
        gpu_entry_present_after_shutdown,
        gpu_memory_bytes_after_shutdown,
        gpu_check_source,
        observation_notes,
    }
}

fn observe_process_memory(pid: u32) -> RuntimeUsageSnapshot {
    #[cfg(unix)]
    {
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
                    gpu_memory_bytes: None,
                    gpu_memory_source: None,
                    observation_notes: vec![],
                }
            }
            Ok(output) => RuntimeUsageSnapshot {
                resident_bytes: None,
                virtual_bytes: None,
                process_memory_source: None,
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
                gpu_memory_bytes: None,
                gpu_memory_source: None,
                observation_notes: vec![format!(
                    "Process memory observation via ps was unavailable: {error}."
                )],
            },
        }
    }

    #[cfg(not(unix))]
    {
        RuntimeUsageSnapshot {
            resident_bytes: None,
            virtual_bytes: None,
            process_memory_source: None,
            gpu_memory_bytes: None,
            gpu_memory_source: None,
            observation_notes: vec![
                "Process memory observation is not yet implemented on this platform.".to_string(),
            ],
        }
    }
}

fn observe_gpu_memory(pid: u32) -> (Option<u64>, Option<String>, Option<String>) {
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
                let memory_mb = parts.next().and_then(|value| value.parse::<u64>().ok());

                if observed_pid == Some(pid) {
                    return (
                        memory_mb.map(|value| value * 1024 * 1024),
                        Some("nvidia-smi compute-apps".to_string()),
                        None,
                    );
                }
            }

            (
                None,
                Some("nvidia-smi compute-apps".to_string()),
                Some(
                    "No matching nvidia-smi compute-apps entry was found for the llama-server PID at observation time."
                        .to_string(),
                ),
            )
        }
        Ok(output) => (
            None,
            None,
            Some(format!(
                "GPU memory observation via nvidia-smi failed with status {}.",
                output.status
            )),
        ),
        Err(error) => (
            None,
            None,
            Some(format!(
                "GPU memory observation via nvidia-smi was unavailable: {error}."
            )),
        ),
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

    #[cfg(not(unix))]
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
