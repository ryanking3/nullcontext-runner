use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::registry::{
    CleanupReason, RetentionPolicy, SessionLifecycleMetadata, SessionLifecycleState,
};
use crate::runtime::{
    RuntimePostShutdownObservation, RuntimeResidentRegion, RuntimeShutdownOutcome,
    RuntimeUsageSnapshot,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyReport {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub history_stored: bool,
    pub backend: String,
    pub security_mode: String,
    pub gpu_layers: String,
    pub process_exited_cleanly: bool,
    pub cleanup: CleanupReport,
    pub session_profile: Option<SessionProfile>,
    pub lifecycle: Option<LifecycleReport>,
    pub llama_runtime: Option<LlamaRuntimeReport>,
    pub retrieval: Option<RetrievalReport>,
    pub residual_risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProfile {
    pub session_kind: String,
    pub runtime_lifetime: String,
    pub turn_count: usize,
    pub runtime_duration_ms: i64,
    pub history_policy: String,
    pub persistence_policy: String,
    pub prompt_source: String,
    pub turn_artifacts: Vec<TurnArtifact>,
    pub active_runtime_residual_risk: String,
    pub grounding_scope: Option<String>,
    pub bound_corpus_id: Option<String>,
    pub bound_corpus_name: Option<String>,
    pub grounded_turn_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnArtifact {
    pub turn: usize,
    pub prompt_path: String,
    pub response_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleReport {
    pub state: String,
    pub retention_policy: String,
    pub retention_deadline: Option<String>,
    pub cleanup_requested_at: Option<String>,
    pub cleanup_completed_at: Option<String>,
    pub cleanup_reason: Option<String>,
    pub updated_at: Option<String>,
    pub policy_summary: String,
    pub decision_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalReport {
    pub corpus_id: String,
    pub corpus_name: String,
    pub retrieval_mode: String,
    pub query: String,
    pub top_k: usize,
    pub grounded_turns: usize,
    pub retrieved_chunks: usize,
    pub source_paths: Vec<String>,
    pub page_hits: Vec<String>,
    pub context_injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeReport {
    pub runtime_kind: String,
    pub runtime_pid: Option<u32>,
    pub model_id: String,
    pub model_name: String,
    pub model_path: String,
    pub gpu_layers_requested: u32,
    pub gpu_offload_requested: bool,
    pub shutdown_method: String,
    pub process_exit_code: Option<i32>,
    pub graceful_shutdown_supported: bool,
    pub observed_resident_bytes: Option<u64>,
    pub observed_virtual_bytes: Option<u64>,
    pub process_memory_source: Option<String>,
    pub physical_footprint_bytes: Option<u64>,
    pub physical_footprint_peak_bytes: Option<u64>,
    pub vmmap_summary_source: Option<String>,
    pub resident_regions: Vec<LlamaResidentRegionReport>,
    pub observed_gpu_memory_bytes: Option<u64>,
    pub gpu_memory_source: Option<String>,
    pub process_present_after_shutdown: Option<bool>,
    pub process_check_source: Option<String>,
    pub process_resident_bytes_after_shutdown: Option<u64>,
    pub process_virtual_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_peak_bytes_after_shutdown: Option<u64>,
    pub vmmap_summary_source_after_shutdown: Option<String>,
    pub resident_regions_after_shutdown: Vec<LlamaResidentRegionReport>,
    pub verification_window_ms: u64,
    pub gpu_entry_present_after_shutdown: Option<bool>,
    pub gpu_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_check_source: Option<String>,
    pub inspection_status: String,
    pub ram_inspection_status: String,
    pub vram_inspection_status: String,
    pub inspection_summary: String,
    pub observation_notes: Vec<String>,
    pub cleanup_summary: String,
    pub residual_risk_summary: String,
    pub memory_domains: Vec<LlamaMemoryDomainReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaMemoryDomainReport {
    pub domain: String,
    pub exposure_scope: String,
    pub cleanup_status: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaResidentRegionReport {
    pub region_type: String,
    pub virtual_bytes: u64,
    pub resident_bytes: u64,
}

impl PrivacyReport {
    pub fn new(
        session_id: String,
        started_at: DateTime<Utc>,
        history_stored: bool,
        backend: String,
        security_mode: String,
        gpu_layers: String,
        process_exited_cleanly: bool,
        cleanup: CleanupReport,
    ) -> Self {
        Self {
            session_id,
            started_at,
            history_stored,
            backend,
            security_mode,
            gpu_layers,
            process_exited_cleanly,
            cleanup,
            session_profile: None,
            lifecycle: None,
            llama_runtime: None,
            retrieval: None,
            residual_risk:
                "OS memory, swap, shell history, and llama.cpp internal allocations are not yet sanitized."
                    .to_string(),
        }
    }

    pub fn with_session_profile(mut self, profile: SessionProfile) -> Self {
        self.session_profile = Some(profile);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: &SessionLifecycleMetadata) -> Self {
        self.lifecycle = Some(LifecycleReport::from_metadata(lifecycle));
        self
    }

    pub fn with_llama_runtime(mut self, llama_runtime: LlamaRuntimeReport) -> Self {
        self.llama_runtime = Some(llama_runtime);
        self
    }

    pub fn with_retrieval(mut self, retrieval: RetrievalReport) -> Self {
        self.retrieval = Some(retrieval);
        self
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl LifecycleReport {
    pub fn from_metadata(metadata: &SessionLifecycleMetadata) -> Self {
        Self {
            state: metadata.state.as_str().to_string(),
            retention_policy: metadata.retention_policy.as_str().to_string(),
            retention_deadline: metadata.retention_deadline.clone(),
            cleanup_requested_at: metadata.cleanup_requested_at.clone(),
            cleanup_completed_at: metadata.cleanup_completed_at.clone(),
            cleanup_reason: metadata
                .cleanup_reason
                .as_ref()
                .map(|reason| reason.as_str().to_string()),
            updated_at: metadata.updated_at.clone(),
            policy_summary: lifecycle_policy_summary(metadata),
            decision_summary: lifecycle_decision_summary(metadata),
        }
    }
}

pub fn sync_report_lifecycle(
    report_path: &Path,
    lifecycle: &SessionLifecycleMetadata,
) -> Result<()> {
    if !report_path.exists() {
        return Ok(());
    }

    let raw = fs::read_to_string(report_path)?;
    let mut report: PrivacyReport = serde_json::from_str(&raw)?;
    report.lifecycle = Some(LifecycleReport::from_metadata(lifecycle));
    fs::write(report_path, report.to_pretty_json()?)?;

    Ok(())
}

pub fn build_llama_runtime_report(
    config: &SessionConfig,
    runtime_pid: Option<u32>,
    shutdown: &RuntimeShutdownOutcome,
    usage: &RuntimeUsageSnapshot,
    post_shutdown: &RuntimePostShutdownObservation,
) -> LlamaRuntimeReport {
    let gpu_layers_requested = config.gpu_layers.parse::<u32>().unwrap_or(0);
    let gpu_offload_requested = gpu_layers_requested > 0;
    let process_exited_cleanly = shutdown.stopped;

    let mut memory_domains = vec![
        LlamaMemoryDomainReport {
            domain: "llama_process_runtime".to_string(),
            exposure_scope: "external child process memory and runtime state".to_string(),
            cleanup_status: if post_shutdown.process_present_after_shutdown == Some(false) {
                "successful".to_string()
            } else if post_shutdown.process_present_after_shutdown == Some(true) {
                "failed".to_string()
            } else if process_exited_cleanly {
                "warning".to_string()
            } else {
                "failed".to_string()
            },
            notes: if post_shutdown.process_present_after_shutdown == Some(false) {
                format!(
                    "No llama-server PID was observed during the {} ms verification window after shutdown. This is evidence that the external runtime process ended, but not proof that released RAM pages were zeroed.",
                    post_shutdown.verification_window_ms
                )
            } else if post_shutdown.process_present_after_shutdown == Some(true) {
                format!(
                    "The llama-server PID was still observable after the {} ms verification window. Post-shutdown RSS/VSZ remained at {} / {}, with physical footprint {}.",
                    post_shutdown.verification_window_ms,
                    post_shutdown
                        .process_resident_bytes_after_shutdown
                        .map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "unknown".to_string()),
                    post_shutdown
                        .process_virtual_bytes_after_shutdown
                        .map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "unknown".to_string()),
                    post_shutdown
                        .physical_footprint_bytes_after_shutdown
                        .map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "unknown".to_string())
                )
            } else if !process_exited_cleanly {
                "The llama-server child process was not confirmed to stop, so external runtime memory may have remained live longer than intended."
                    .to_string()
            } else if shutdown.shutdown_method == "already_exited" {
                "llama-server had already exited before NullContext ran its shutdown step. External llama.cpp memory ended with process exit, but no graceful cleanup hook was observed."
                    .to_string()
            } else {
                "NullContext stopped llama-server by killing the child process and waiting for exit. This is the current cleanup boundary for external llama.cpp-owned memory."
                    .to_string()
            },
        },
        LlamaMemoryDomainReport {
            domain: "llama_internal_allocator".to_string(),
            exposure_scope: "llama.cpp allocator state and freed runtime pages".to_string(),
            cleanup_status: "warning".to_string(),
            notes: "NullContext does not currently verify allocator-level clearing inside llama.cpp after shutdown.".to_string(),
        },
        LlamaMemoryDomainReport {
            domain: "model_weights_ram".to_string(),
            exposure_scope: "loaded GGUF weight residency in external process RAM".to_string(),
            cleanup_status: "warning".to_string(),
            notes: "Process termination ends normal access to model-weight memory, but NullContext does not verify whether released OS pages were zeroed or later reused.".to_string(),
        },
        LlamaMemoryDomainReport {
            domain: "kv_cache_state".to_string(),
            exposure_scope: "prompt context, KV/cache state, and decoded token history inside llama.cpp".to_string(),
            cleanup_status: "warning".to_string(),
            notes: "KV/cache lifetime is bounded by runtime lifetime in this build, but NullContext does not yet inspect or sanitize llama.cpp cache internals directly.".to_string(),
        },
    ];

    if gpu_offload_requested {
        memory_domains.push(LlamaMemoryDomainReport {
            domain: "gpu_vram".to_string(),
            exposure_scope: "GPU-offloaded model layers and possible prompt/cache-related buffers".to_string(),
            cleanup_status: if post_shutdown.gpu_entry_present_after_shutdown == Some(true) {
                "failed".to_string()
            } else if post_shutdown.gpu_entry_present_after_shutdown == Some(false) {
                "successful".to_string()
            } else {
                "warning".to_string()
            },
            notes: if post_shutdown.gpu_entry_present_after_shutdown == Some(true) {
                format!(
                    "A matching GPU-memory observation was still present after shutdown ({}). This is evidence of post-shutdown GPU residency visibility, not proof of allocator ownership or complete VRAM state.",
                    post_shutdown
                        .gpu_memory_bytes_after_shutdown
                        .map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "unknown usage".to_string())
                )
            } else if post_shutdown.gpu_entry_present_after_shutdown == Some(false) {
                "No matching GPU-memory entry was observed after shutdown. This is evidence that the runtime PID no longer had an observable nvidia-smi compute-apps allocation, but it is not proof of full VRAM sanitization."
                    .to_string()
            } else {
                "GPU offload was requested, but post-shutdown GPU inspection was unavailable or inconclusive. NullContext does not yet verify or sanitize VRAM contents after shutdown."
                    .to_string()
            },
        });
    } else {
        memory_domains.push(LlamaMemoryDomainReport {
            domain: "gpu_vram".to_string(),
            exposure_scope: "GPU-offloaded model layers and possible prompt/cache-related buffers".to_string(),
            cleanup_status: "not_attempted".to_string(),
            notes: "GPU offload was not requested for this session, so NullContext did not expect model residency in VRAM from llama.cpp.".to_string(),
        });
    }

    let mut observation_notes = usage.observation_notes.clone();
    observation_notes.extend(post_shutdown.observation_notes.clone());
    let resident_regions = usage
        .resident_regions
        .iter()
        .map(LlamaResidentRegionReport::from_runtime_region)
        .collect();
    let resident_regions_after_shutdown = post_shutdown
        .resident_regions_after_shutdown
        .iter()
        .map(LlamaResidentRegionReport::from_runtime_region)
        .collect();
    let inspection_status = runtime_inspection_status(post_shutdown);
    let ram_inspection_status = ram_inspection_status(post_shutdown);
    let vram_inspection_status = vram_inspection_status(gpu_offload_requested, post_shutdown);
    let inspection_summary = runtime_inspection_summary(
        &inspection_status,
        &ram_inspection_status,
        &vram_inspection_status,
        post_shutdown,
    );

    LlamaRuntimeReport {
        runtime_kind: "llama-server".to_string(),
        runtime_pid,
        model_id: config.model_id.clone(),
        model_name: config.model_name.clone(),
        model_path: config.model_path.clone(),
        gpu_layers_requested,
        gpu_offload_requested,
        shutdown_method: shutdown.shutdown_method.clone(),
        process_exit_code: shutdown.exit_code,
        graceful_shutdown_supported: shutdown.graceful_shutdown_supported,
        observed_resident_bytes: usage.resident_bytes,
        observed_virtual_bytes: usage.virtual_bytes,
        process_memory_source: usage.process_memory_source.clone(),
        physical_footprint_bytes: usage.physical_footprint_bytes,
        physical_footprint_peak_bytes: usage.physical_footprint_peak_bytes,
        vmmap_summary_source: usage.vmmap_summary_source.clone(),
        resident_regions,
        observed_gpu_memory_bytes: usage.gpu_memory_bytes,
        gpu_memory_source: usage.gpu_memory_source.clone(),
        process_present_after_shutdown: post_shutdown.process_present_after_shutdown,
        process_check_source: post_shutdown.process_check_source.clone(),
        process_resident_bytes_after_shutdown: post_shutdown.process_resident_bytes_after_shutdown,
        process_virtual_bytes_after_shutdown: post_shutdown.process_virtual_bytes_after_shutdown,
        physical_footprint_bytes_after_shutdown: post_shutdown
            .physical_footprint_bytes_after_shutdown,
        physical_footprint_peak_bytes_after_shutdown: post_shutdown
            .physical_footprint_peak_bytes_after_shutdown,
        vmmap_summary_source_after_shutdown: post_shutdown
            .vmmap_summary_source_after_shutdown
            .clone(),
        resident_regions_after_shutdown,
        verification_window_ms: post_shutdown.verification_window_ms,
        gpu_entry_present_after_shutdown: post_shutdown.gpu_entry_present_after_shutdown,
        gpu_memory_bytes_after_shutdown: post_shutdown.gpu_memory_bytes_after_shutdown,
        gpu_check_source: post_shutdown.gpu_check_source.clone(),
        inspection_status,
        ram_inspection_status,
        vram_inspection_status,
        inspection_summary,
        observation_notes,
        cleanup_summary: if !process_exited_cleanly {
            "NullContext could not confirm llama-server shutdown, so runtime-owned memory domains remain more weakly bounded than intended."
                .to_string()
        } else if shutdown.shutdown_method == "already_exited" {
            "The llama-server process had already exited before the final shutdown step. Process exit is still the strongest cleanup boundary currently available for llama.cpp-owned memory domains."
                .to_string()
        } else {
            "NullContext stopped llama-server by force-killing the child process and waiting for exit. Process termination is currently the strongest cleanup action applied to llama.cpp-owned memory domains."
                .to_string()
        },
        residual_risk_summary: if gpu_offload_requested {
            "Allocator state, KV/cache contents, model-weight residency, and possible VRAM-resident buffers remain unverified even after the recorded shutdown path."
                .to_string()
        } else {
            "Allocator state, KV/cache contents, and model-weight residency in the external llama.cpp process remain unverified even after the recorded shutdown path."
                .to_string()
        },
        memory_domains,
    }
}

impl LlamaResidentRegionReport {
    fn from_runtime_region(region: &RuntimeResidentRegion) -> Self {
        Self {
            region_type: region.region_type.clone(),
            virtual_bytes: region.virtual_bytes,
            resident_bytes: region.resident_bytes,
        }
    }
}

fn runtime_inspection_status(post_shutdown: &RuntimePostShutdownObservation) -> String {
    match post_shutdown.process_present_after_shutdown {
        Some(false) => "process_not_observed_after_shutdown".to_string(),
        Some(true) => "process_still_observable_after_shutdown".to_string(),
        None => "process_shutdown_observation_inconclusive".to_string(),
    }
}

fn ram_inspection_status(post_shutdown: &RuntimePostShutdownObservation) -> String {
    match post_shutdown.process_present_after_shutdown {
        Some(false) => "resident_memory_not_observed_after_shutdown".to_string(),
        Some(true) => "resident_memory_still_observable_after_shutdown".to_string(),
        None => "ram_inspection_inconclusive".to_string(),
    }
}

fn vram_inspection_status(
    gpu_offload_requested: bool,
    post_shutdown: &RuntimePostShutdownObservation,
) -> String {
    if !gpu_offload_requested {
        return "gpu_offload_not_requested".to_string();
    }

    match post_shutdown.gpu_entry_present_after_shutdown {
        Some(false) => "gpu_entry_not_observed_after_shutdown".to_string(),
        Some(true) => "gpu_entry_still_observable_after_shutdown".to_string(),
        None => "gpu_inspection_unavailable".to_string(),
    }
}

fn runtime_inspection_summary(
    inspection_status: &str,
    ram_inspection_status: &str,
    vram_inspection_status: &str,
    post_shutdown: &RuntimePostShutdownObservation,
) -> String {
    match (
        inspection_status,
        ram_inspection_status,
        vram_inspection_status,
    ) {
        (
            "process_not_observed_after_shutdown",
            "resident_memory_not_observed_after_shutdown",
            "gpu_entry_not_observed_after_shutdown",
        ) => format!(
            "Within the {} ms verification window, NullContext did not observe the llama-server PID, did not observe residual process RSS/VSZ, and did not observe a matching post-shutdown GPU entry.",
            post_shutdown.verification_window_ms
        ),
        (
            "process_not_observed_after_shutdown",
            "resident_memory_not_observed_after_shutdown",
            "gpu_offload_not_requested",
        ) => format!(
            "Within the {} ms verification window, NullContext did not observe the llama-server PID or residual process RSS/VSZ. GPU offload was not requested for this session.",
            post_shutdown.verification_window_ms
        ),
        ("process_still_observable_after_shutdown", _, _) => format!(
            "The llama-server PID was still observable after the {} ms verification window, so RAM cleanup evidence remains unfavorable{} and follow-up inspection is recommended.",
            post_shutdown.verification_window_ms
            ,
            post_shutdown
                .physical_footprint_bytes_after_shutdown
                .map(|value| format!("; post-shutdown physical footprint remained {}", value))
                .unwrap_or_default()
        ),
        (_, _, "gpu_entry_still_observable_after_shutdown") => format!(
            "A matching GPU entry was still observable after the {} ms verification window, so VRAM exposure remains explicitly visible after shutdown.",
            post_shutdown.verification_window_ms
        ),
        _ => format!(
            "Post-shutdown inspection completed with mixed or incomplete evidence over a {} ms verification window. Review the RAM and VRAM inspection statuses before making cleanup claims.",
            post_shutdown.verification_window_ms
        ),
    }
}

fn lifecycle_policy_summary(metadata: &SessionLifecycleMetadata) -> String {
    match metadata.retention_policy {
        RetentionPolicy::EphemeralImmediate => {
            "Ephemeral policy targeted immediate cleanup at session end.".to_string()
        }
        RetentionPolicy::RetainUntilManualCleanup => {
            "Session is retained until an explicit operator cleanup action is requested."
                .to_string()
        }
        RetentionPolicy::RetainForDuration => {
            if let Some(deadline) = &metadata.retention_deadline {
                format!(
                    "Session is retained until {deadline}, after which scheduled cleanup may run."
                )
            } else {
                "Session is configured for scheduled retention expiry, but no deadline is currently recorded."
                    .to_string()
            }
        }
    }
}

fn lifecycle_decision_summary(metadata: &SessionLifecycleMetadata) -> String {
    match metadata.state {
        SessionLifecycleState::CompletedRetained => {
            "Session completed and its retained artifacts remain available under the current lifecycle policy."
                .to_string()
        }
        SessionLifecycleState::CleanupPending => {
            "Cleanup has been requested but has not yet completed.".to_string()
        }
        SessionLifecycleState::CleanupSucceeded => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(cleanup_reason_summary)
                .unwrap_or("Cleanup completed successfully.");

            reason.to_string()
        }
        SessionLifecycleState::CleanupFailed => {
            let reason = metadata
                .cleanup_reason
                .as_ref()
                .map(cleanup_reason_summary)
                .unwrap_or("Cleanup attempted but did not complete successfully.");

            format!("{reason} Cleanup failed or requires operator follow-up.")
        }
        SessionLifecycleState::Orphaned => {
            "Lifecycle reconciliation detected an inconsistency between registry state and on-disk artifacts. Operator review is recommended."
                .to_string()
        }
        SessionLifecycleState::Active => {
            "Session is still marked active in lifecycle metadata.".to_string()
        }
    }
}

fn cleanup_reason_summary(reason: &CleanupReason) -> &'static str {
    match reason {
        CleanupReason::EphemeralPolicy => {
            "Cleanup ran because the session policy was ephemeral-at-end."
        }
        CleanupReason::ManualOperatorRequest => {
            "Cleanup ran because an operator explicitly requested lifecycle cleanup."
        }
        CleanupReason::ScheduledRetentionExpiry => {
            "Cleanup ran because the scheduled retention deadline expired."
        }
        CleanupReason::StartupOrphanReconciliation => {
            "Lifecycle reconciliation changed the session state during startup recovery."
        }
    }
}
