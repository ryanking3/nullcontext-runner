use std::process::Command;

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
    pid_observed: Option<bool>,
    note: Option<String>,
}

trait GpuInspectionBackend {
    fn inspect_process(&self, pid: u32) -> Option<BackendObservation>;
}

pub fn observe_gpu_process(pid: u32) -> GpuInspectionObservation {
    let compute_apps = NvidiaSmiComputeAppsBackend;
    let pmon = NvidiaSmiPmonBackend;

    let mut prior_notes = Vec::new();
    let mut prior_sources = Vec::new();
    let mut compute_apps_attempted = false;

    if let Some(observation) = compute_apps.inspect_process(pid) {
        compute_apps_attempted = true;

        if observation.pid_observed == Some(true) && observation.memory_bytes.is_some() {
            return GpuInspectionObservation {
                memory_bytes: observation.memory_bytes,
                source: observation.source,
                backend: Some("nvidia_smi_compute_apps".to_string()),
                pid_observed: observation.pid_observed,
                note: observation.note,
            };
        }

        if let Some(source) = observation.source {
            prior_sources.push(source);
        }

        if let Some(note) = observation.note {
            prior_notes.push(note);
        }
    }

    if let Some(mut observation) = pmon.inspect_process(pid) {
        if !prior_sources.is_empty() {
            observation.source = Some(merge_labels(&prior_sources, observation.source.as_deref()));
        }

        if !prior_notes.is_empty() {
            observation.note = Some(match observation.note.take() {
                Some(note) => format!("{} {}", prior_notes.join(" "), note),
                None => prior_notes.join(" "),
            });
        }

        return GpuInspectionObservation {
            memory_bytes: observation.memory_bytes,
            source: observation.source,
            backend: Some(if compute_apps_attempted {
                "nvidia_smi_compute_apps_then_pmon".to_string()
            } else {
                "nvidia_smi_pmon".to_string()
            }),
            pid_observed: observation.pid_observed,
            note: observation.note,
        };
    }

    GpuInspectionObservation {
        memory_bytes: None,
        source: if prior_sources.is_empty() {
            None
        } else {
            Some(merge_labels(&prior_sources, None))
        },
        backend: if compute_apps_attempted {
            Some("nvidia_smi_compute_apps".to_string())
        } else {
            None
        },
        pid_observed: None,
        note: Some(if prior_notes.is_empty() {
            "GPU memory observation via nvidia-smi was unavailable or inconclusive.".to_string()
        } else {
            prior_notes.join(" ")
        }),
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
                pid_observed: None,
                note: Some(format!(
                    "GPU memory observation via nvidia-smi compute-apps failed with status {}.",
                    output.status
                )),
            }),
            Err(error) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
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
                    pid_observed: Some(false),
                    note: Some(no_matching_gpu_pid_note()),
                })
            }
            Ok(output) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                pid_observed: None,
                note: Some(format!(
                    "GPU process observation via nvidia-smi pmon failed with status {}.",
                    output.status
                )),
            }),
            Err(error) => Some(BackendObservation {
                memory_bytes: None,
                source: None,
                pid_observed: None,
                note: Some(format!(
                    "GPU process observation via nvidia-smi pmon was unavailable: {error}."
                )),
            }),
        }
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
