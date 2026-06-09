use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::registry::{
    CleanupReason, RetentionPolicy, SessionLifecycleMetadata, SessionLifecycleState,
};
use crate::runtime::{
    RuntimeLaunchFailure, RuntimePostShutdownObservation, RuntimeResidentRegion,
    RuntimeShutdownOutcome, RuntimeUsageSnapshot,
};
use crate::runtime_capabilities::detect_runtime_introspection_capabilities;
use crate::runtime_introspection::RuntimeIntrospectionSignal;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub process_scan: Option<ProcessScanReport>,
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
    pub state_note: Option<String>,
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
pub struct ProcessScanReport {
    pub overall_status: String,
    pub implementation_status: String,
    pub platform: String,
    pub target_process_kind: String,
    pub target_runtime_pid: Option<u32>,
    pub planned_platforms: Vec<String>,
    pub summary: String,
    pub residual_risk_summary: String,
    pub phases: Vec<ProcessScanPhaseReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessScanPhaseReport {
    pub phase: String,
    pub status: String,
    pub method: String,
    pub target_pid: Option<u32>,
    pub scope_summary: String,
    pub bytes_scanned: Option<u64>,
    pub regions_scanned: Option<u64>,
    pub regions_skipped: Option<u64>,
    pub patterns: Vec<ProcessScanPatternReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessScanPatternReport {
    pub pattern_kind: String,
    pub status: String,
    pub matches_found: Option<u64>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeReport {
    pub runtime_kind: String,
    pub runtime_pid: Option<u32>,
    pub runtime_endpoint: Option<String>,
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
    pub observed_gpu_pid: Option<bool>,
    pub observed_gpu_memory_bytes: Option<u64>,
    pub live_gpu_visibility_status: String,
    pub gpu_observation_backend: Option<String>,
    pub gpu_memory_source: Option<String>,
    pub process_present_after_shutdown: Option<bool>,
    pub process_check_source: Option<String>,
    pub process_resident_bytes_after_shutdown: Option<u64>,
    pub process_virtual_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_bytes_after_shutdown: Option<u64>,
    pub physical_footprint_peak_bytes_after_shutdown: Option<u64>,
    pub vmmap_summary_source_after_shutdown: Option<String>,
    pub resident_regions_after_shutdown: Vec<LlamaResidentRegionReport>,
    pub physical_footprint_delta_bytes: Option<i64>,
    pub resident_region_deltas: Vec<LlamaResidentRegionDeltaReport>,
    pub verification_window_ms: u64,
    pub gpu_entry_present_after_shutdown: Option<bool>,
    pub gpu_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_peak_memory_bytes_after_shutdown: Option<u64>,
    pub gpu_samples_collected_after_shutdown: u32,
    pub gpu_samples_with_pid_observed_after_shutdown: u32,
    pub gpu_last_pid_observed_at_ms: Option<u64>,
    pub post_shutdown_gpu_visibility_status: String,
    pub gpu_check_backend: Option<String>,
    pub gpu_check_source: Option<String>,
    pub inspection_status: String,
    pub ram_inspection_status: String,
    pub vram_inspection_status: String,
    pub inspection_summary: String,
    pub observation_notes: Vec<String>,
    pub cleanup_summary: String,
    pub residual_risk_summary: String,
    pub introspection: LlamaRuntimeIntrospectionReport,
    pub memory_domains: Vec<LlamaMemoryDomainReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeIntrospectionReport {
    pub capability_source: String,
    pub manifest_path: Option<String>,
    pub runtime_build_profile: String,
    pub instrumentation_backend: String,
    pub allocator_introspection_status: String,
    pub allocator_initialized_observed: bool,
    pub allocator_teardown_observed: bool,
    pub allocator_reset_observed: bool,
    pub allocator_summary: String,
    pub kv_cache_introspection_status: String,
    pub kv_cache_initialized_observed: bool,
    pub kv_cache_reused_observed: bool,
    pub kv_cache_clear_observed: bool,
    pub kv_cache_summary: String,
    pub model_unload_signal_status: String,
    pub allocator_reset_signal_status: String,
    pub summary: String,
    pub observed_events: Vec<LlamaRuntimeIntrospectionEventReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeIntrospectionEventReport {
    pub event: String,
    pub status: String,
    pub source: String,
    pub details: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaResidentRegionDeltaReport {
    pub region_type: String,
    pub before_resident_bytes: u64,
    pub after_resident_bytes: u64,
    pub resident_delta_bytes: i64,
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
            process_scan: None,
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

    pub fn with_process_scan(mut self, process_scan: ProcessScanReport) -> Self {
        self.process_scan = Some(process_scan);
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
            state_note: metadata.state_note.clone(),
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
    runtime_endpoint: Option<&str>,
    shutdown: &RuntimeShutdownOutcome,
    usage: &RuntimeUsageSnapshot,
    post_shutdown: &RuntimePostShutdownObservation,
) -> LlamaRuntimeReport {
    let gpu_layers_requested = config.gpu_layers.parse::<u32>().unwrap_or(0);
    let gpu_offload_requested = gpu_layers_requested > 0;
    let process_exited_cleanly = shutdown.stopped;
    let introspection = build_llama_runtime_introspection_report(
        &config.llama_path,
        false,
        &shutdown.introspection_signals,
    );

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
            cleanup_status: if introspection.allocator_reset_observed {
                "successful".to_string()
            } else if introspection.allocator_initialized_observed
                || introspection.allocator_teardown_observed
            {
                "warning".to_string()
            } else {
                "warning".to_string()
            },
            notes: introspection.allocator_summary.clone(),
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
            cleanup_status: if introspection.kv_cache_clear_observed {
                "successful".to_string()
            } else if introspection.kv_cache_initialized_observed
                || introspection.kv_cache_reused_observed
            {
                "warning".to_string()
            } else {
                "warning".to_string()
            },
            notes: introspection.kv_cache_summary.clone(),
        },
    ];

    if gpu_offload_requested {
        memory_domains.push(LlamaMemoryDomainReport {
            domain: "gpu_vram".to_string(),
            exposure_scope: "GPU-offloaded model layers and possible prompt/cache-related buffers".to_string(),
            cleanup_status: if post_shutdown.gpu_entry_present_after_shutdown == Some(true) {
                "failed".to_string()
            } else if post_shutdown.gpu_entry_present_after_shutdown == Some(false)
                && !gpu_post_shutdown_visibility_limited(post_shutdown)
            {
                "successful".to_string()
            } else {
                "warning".to_string()
            },
            notes: if post_shutdown.gpu_entry_present_after_shutdown == Some(true) {
                format!(
                    "A matching GPU-memory observation was seen during the {} ms post-shutdown verification window (peak observed usage: {}; samples with matching PID: {}). This is evidence of post-shutdown GPU residency visibility, not proof of allocator ownership or complete VRAM state.",
                    post_shutdown.verification_window_ms,
                    post_shutdown
                        .gpu_peak_memory_bytes_after_shutdown
                        .map(|value| format!("{value} bytes"))
                        .unwrap_or_else(|| "unknown usage".to_string())
                    ,
                    post_shutdown.gpu_samples_with_pid_observed_after_shutdown
                )
            } else if post_shutdown.gpu_entry_present_after_shutdown == Some(false)
                && !gpu_post_shutdown_visibility_limited(post_shutdown)
            {
                "No matching GPU-memory entry was observed during the post-shutdown verification window. This is evidence that the runtime PID no longer had an observable GPU allocation through the current backend path, but it is not proof of full VRAM sanitization."
                    .to_string()
            } else if post_shutdown.gpu_entry_present_after_shutdown == Some(false) {
                "No matching GPU PID was observed during the post-shutdown verification window, but the current Windows/NVIDIA visibility path remained limited. Treat post-shutdown VRAM evidence as inconclusive rather than as proof that GPU-resident state was cleared."
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
    let resident_region_deltas = build_resident_region_deltas(
        &usage.resident_regions,
        &post_shutdown.resident_regions_after_shutdown,
    );
    let physical_footprint_delta_bytes = match (
        usage.physical_footprint_bytes,
        post_shutdown.physical_footprint_bytes_after_shutdown,
    ) {
        (Some(before), Some(after)) => Some(after as i64 - before as i64),
        _ => None,
    };
    let inspection_status = runtime_inspection_status(post_shutdown);
    let ram_inspection_status = ram_inspection_status(post_shutdown);
    let live_gpu_visibility_status = live_gpu_visibility_status(gpu_offload_requested, usage);
    let post_shutdown_gpu_visibility_status =
        post_shutdown_gpu_visibility_status(gpu_offload_requested, post_shutdown);
    let vram_inspection_status = vram_inspection_status(gpu_offload_requested, post_shutdown);
    let inspection_summary = runtime_inspection_summary(
        &inspection_status,
        &ram_inspection_status,
        &vram_inspection_status,
        post_shutdown,
        physical_footprint_delta_bytes,
    );

    LlamaRuntimeReport {
        runtime_kind: "llama-server".to_string(),
        runtime_pid,
        runtime_endpoint: runtime_endpoint.map(str::to_string),
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
        observed_gpu_pid: usage.gpu_pid_observed,
        observed_gpu_memory_bytes: usage.gpu_memory_bytes,
        live_gpu_visibility_status,
        gpu_observation_backend: usage.gpu_observation_backend.clone(),
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
        physical_footprint_delta_bytes,
        resident_region_deltas,
        verification_window_ms: post_shutdown.verification_window_ms,
        gpu_entry_present_after_shutdown: post_shutdown.gpu_entry_present_after_shutdown,
        gpu_memory_bytes_after_shutdown: post_shutdown.gpu_memory_bytes_after_shutdown,
        gpu_peak_memory_bytes_after_shutdown: post_shutdown.gpu_peak_memory_bytes_after_shutdown,
        gpu_samples_collected_after_shutdown: post_shutdown.gpu_samples_collected_after_shutdown,
        gpu_samples_with_pid_observed_after_shutdown: post_shutdown
            .gpu_samples_with_pid_observed_after_shutdown,
        gpu_last_pid_observed_at_ms: post_shutdown.gpu_last_pid_observed_at_ms,
        post_shutdown_gpu_visibility_status,
        gpu_check_backend: post_shutdown.gpu_check_backend.clone(),
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
        introspection,
        memory_domains,
    }
}

pub fn build_failed_launch_llama_runtime_report(
    config: &SessionConfig,
    failure: &RuntimeLaunchFailure,
) -> LlamaRuntimeReport {
    let gpu_layers_requested = config.gpu_layers.parse::<u32>().unwrap_or(0);
    let gpu_offload_requested = gpu_layers_requested > 0;
    let cleanup_status = if failure.cleanup_succeeded {
        "warning"
    } else {
        "failed"
    };
    let introspection = build_llama_runtime_introspection_report(
        &config.llama_path,
        true,
        &failure.introspection_signals,
    );

    LlamaRuntimeReport {
        runtime_kind: "llama-server".to_string(),
        runtime_pid: Some(failure.runtime_pid),
        runtime_endpoint: Some(failure.runtime_endpoint.clone()),
        model_id: config.model_id.clone(),
        model_name: config.model_name.clone(),
        model_path: config.model_path.clone(),
        gpu_layers_requested,
        gpu_offload_requested,
        shutdown_method: failure
            .cleanup_shutdown_method
            .clone()
            .unwrap_or_else(|| "startup_failed_before_ready".to_string()),
        process_exit_code: failure.cleanup_exit_code,
        graceful_shutdown_supported: false,
        observed_resident_bytes: None,
        observed_virtual_bytes: None,
        process_memory_source: None,
        physical_footprint_bytes: None,
        physical_footprint_peak_bytes: None,
        vmmap_summary_source: None,
        resident_regions: vec![],
        observed_gpu_pid: None,
        observed_gpu_memory_bytes: None,
        live_gpu_visibility_status: if gpu_offload_requested {
            "gpu_visibility_unavailable_due_to_startup_failure".to_string()
        } else {
            "gpu_offload_not_requested".to_string()
        },
        gpu_observation_backend: None,
        gpu_memory_source: None,
        process_present_after_shutdown: failure
            .post_cleanup_observation
            .process_present_after_shutdown,
        process_check_source: failure.post_cleanup_observation.process_check_source.clone(),
        process_resident_bytes_after_shutdown: failure
            .post_cleanup_observation
            .process_resident_bytes_after_shutdown,
        process_virtual_bytes_after_shutdown: failure
            .post_cleanup_observation
            .process_virtual_bytes_after_shutdown,
        physical_footprint_bytes_after_shutdown: failure
            .post_cleanup_observation
            .physical_footprint_bytes_after_shutdown,
        physical_footprint_peak_bytes_after_shutdown: failure
            .post_cleanup_observation
            .physical_footprint_peak_bytes_after_shutdown,
        vmmap_summary_source_after_shutdown: failure
            .post_cleanup_observation
            .vmmap_summary_source_after_shutdown
            .clone(),
        resident_regions_after_shutdown: failure
            .post_cleanup_observation
            .resident_regions_after_shutdown
            .iter()
            .map(LlamaResidentRegionReport::from_runtime_region)
            .collect(),
        physical_footprint_delta_bytes: None,
        resident_region_deltas: vec![],
        verification_window_ms: failure.post_cleanup_observation.verification_window_ms,
        gpu_entry_present_after_shutdown: failure
            .post_cleanup_observation
            .gpu_entry_present_after_shutdown,
        gpu_memory_bytes_after_shutdown: failure
            .post_cleanup_observation
            .gpu_memory_bytes_after_shutdown,
        gpu_peak_memory_bytes_after_shutdown: failure
            .post_cleanup_observation
            .gpu_peak_memory_bytes_after_shutdown,
        gpu_samples_collected_after_shutdown: failure
            .post_cleanup_observation
            .gpu_samples_collected_after_shutdown,
        gpu_samples_with_pid_observed_after_shutdown: failure
            .post_cleanup_observation
            .gpu_samples_with_pid_observed_after_shutdown,
        gpu_last_pid_observed_at_ms: failure
            .post_cleanup_observation
            .gpu_last_pid_observed_at_ms,
        post_shutdown_gpu_visibility_status: if gpu_offload_requested {
            "post_shutdown_gpu_visibility_unavailable_due_to_startup_failure".to_string()
        } else {
            "gpu_offload_not_requested".to_string()
        },
        gpu_check_backend: failure.post_cleanup_observation.gpu_check_backend.clone(),
        gpu_check_source: failure.post_cleanup_observation.gpu_check_source.clone(),
        inspection_status: "runtime_startup_failed_before_ready".to_string(),
        ram_inspection_status: "ram_inspection_unavailable_due_to_startup_failure".to_string(),
        vram_inspection_status: if gpu_offload_requested {
            "gpu_inspection_unavailable_due_to_startup_failure".to_string()
        } else {
            "gpu_offload_not_requested".to_string()
        },
        inspection_summary: format!(
            "llama-server never reached a healthy runtime state on {} (pid {}), so NullContext could not run normal post-shutdown RAM/VRAM inspection. Startup failure: {}",
            failure.runtime_endpoint, failure.runtime_pid, failure.startup_error
        ),
        observation_notes: {
            let mut notes = vec![
                format!("Startup failure: {}", failure.startup_error),
                format!(
                    "Failed launch targeted runtime endpoint {} with child pid {}.",
                    failure.runtime_endpoint, failure.runtime_pid
                ),
            ];

            if let Some(error) = &failure.cleanup_error {
                notes.push(format!(
                    "Automatic cleanup of the failed startup runtime also failed: {error}"
                ));
            }

            notes.extend(failure.post_cleanup_observation.observation_notes.clone());

            if !failure.stdout.trim().is_empty() {
                notes.push("llama-server stdout was captured during failed startup.".to_string());
            }

            if !failure.stderr.trim().is_empty() {
                notes.push("llama-server stderr was captured during failed startup.".to_string());
            }

            notes
        },
        cleanup_summary: if failure.cleanup_succeeded {
            format!(
                "NullContext terminated the failed startup process using {} before inference began, but no normal post-shutdown observation window was completed.",
                failure
                    .cleanup_shutdown_method
                    .as_deref()
                    .unwrap_or("unknown shutdown method")
            )
        } else {
            "NullContext could not confirm automatic cleanup of the failed startup runtime, so process-owned memory boundaries remain weakly bounded.".to_string()
        },
        residual_risk_summary: if gpu_offload_requested {
            "Because runtime startup failed before readiness, allocator state, RAM residency, and any possible GPU-offloaded setup state were not inspected through the normal shutdown path.".to_string()
        } else {
            "Because runtime startup failed before readiness, allocator state and RAM residency were not inspected through the normal shutdown path.".to_string()
        },
        introspection,
        memory_domains: vec![
            LlamaMemoryDomainReport {
                domain: "llama_process_runtime".to_string(),
                exposure_scope: "external child process memory and runtime state".to_string(),
                cleanup_status: cleanup_status.to_string(),
                notes: if failure.cleanup_succeeded {
                    "NullContext forced the failed startup process to exit, but no normal post-shutdown process observation was completed.".to_string()
                } else {
                    "NullContext could not confirm automatic cleanup of the failed startup process.".to_string()
                },
            },
            LlamaMemoryDomainReport {
                domain: "gpu_vram".to_string(),
                exposure_scope: "GPU-offloaded model layers and possible prompt/cache-related buffers".to_string(),
                cleanup_status: if gpu_offload_requested {
                    cleanup_status.to_string()
                } else {
                    "not_attempted".to_string()
                },
                notes: if gpu_offload_requested {
                    "GPU offload was requested, but startup failed before readiness and no normal post-shutdown GPU inspection ran.".to_string()
                } else {
                    "GPU offload was not requested for this session.".to_string()
                },
            },
        ],
    }
}

fn build_llama_runtime_introspection_report(
    llama_path: &str,
    startup_failed: bool,
    observed_signals: &[RuntimeIntrospectionSignal],
) -> LlamaRuntimeIntrospectionReport {
    let capabilities = match detect_runtime_introspection_capabilities(llama_path) {
        Ok(capabilities) => capabilities,
        Err(error) => {
            return LlamaRuntimeIntrospectionReport {
                capability_source: "manifest_load_failed_fallback".to_string(),
                manifest_path: None,
                runtime_build_profile: "stock_external_llama_server".to_string(),
                instrumentation_backend: "none".to_string(),
                allocator_introspection_status: "allocator_introspection_capability_load_failed"
                    .to_string(),
                allocator_initialized_observed: false,
                allocator_teardown_observed: false,
                allocator_reset_observed: false,
                allocator_summary:
                    "NullContext could not load runtime introspection capabilities, so allocator lifecycle evidence remained unavailable for this session."
                        .to_string(),
                kv_cache_introspection_status:
                    "kv_cache_introspection_capability_load_failed".to_string(),
                kv_cache_initialized_observed: false,
                kv_cache_reused_observed: false,
                kv_cache_clear_observed: false,
                kv_cache_summary:
                    "NullContext could not load runtime introspection capabilities, so KV/cache lifecycle evidence remained unavailable for this session."
                        .to_string(),
                model_unload_signal_status: if startup_failed {
                    "model_unload_signal_unavailable_due_to_startup_failure".to_string()
                } else {
                    "model_unload_not_observed_directly".to_string()
                },
                allocator_reset_signal_status: if startup_failed {
                    "allocator_reset_signal_unavailable_due_to_startup_failure".to_string()
                } else {
                    "allocator_reset_not_observed_directly".to_string()
                },
                summary: format!(
                    "NullContext tried to load runtime introspection capabilities for this llama-server path, but capability loading failed: {}. Falling back to stock-runtime assumptions.",
                    error
                ),
                observed_events: observed_signals
                    .iter()
                    .map(map_runtime_introspection_signal)
                    .collect(),
                notes: vec![
                    format!("Capability load failure: {}", error),
                    "Future allocator/KV work should fill this section with explicit runtime capability evidence rather than freeform caveats.".to_string(),
                ],
            };
        }
    };

    let mut notes = capabilities.notes;
    notes.push(
        "Future allocator/KV work should fill this section with explicit runtime capability evidence rather than freeform caveats."
            .to_string(),
    );
    let observed_events = observed_signals
        .iter()
        .map(map_runtime_introspection_signal)
        .collect::<Vec<_>>();
    let observed_kv_initialized = observed_signals
        .iter()
        .any(|signal| signal.event == "kv_cache_initialized" && signal.status != "failed");
    let observed_kv_reused = observed_signals
        .iter()
        .any(|signal| signal.event == "kv_cache_reused" && signal.status != "failed");
    let observed_kv_clear = observed_signals
        .iter()
        .any(|signal| signal.event == "kv_cache_clear_observed" && signal.status != "failed");
    let observed_kv_signal = observed_signals.iter().any(|signal| {
        matches!(
            signal.event.as_str(),
            "kv_cache_initialized" | "kv_cache_reused" | "kv_cache_clear_observed"
        ) && signal.status != "failed"
    });
    let observed_allocator_signal = observed_signals.iter().any(|signal| {
        matches!(
            signal.event.as_str(),
            "allocator_reset_observed" | "allocator_initialized" | "allocator_teardown_observed"
        ) && signal.status != "failed"
    });
    let observed_allocator_initialized = observed_signals
        .iter()
        .any(|signal| signal.event == "allocator_initialized" && signal.status != "failed");
    let observed_allocator_teardown = observed_signals
        .iter()
        .any(|signal| signal.event == "allocator_teardown_observed" && signal.status != "failed");
    let observed_model_unload_signal = observed_signals
        .iter()
        .any(|signal| signal.event == "model_unload_observed" && signal.status != "failed");
    let observed_allocator_reset_signal = observed_signals
        .iter()
        .any(|signal| signal.event == "allocator_reset_observed" && signal.status != "failed");
    let declared_allocator_introspection_status =
        capabilities.allocator_introspection_status.clone();
    let declared_kv_cache_introspection_status = capabilities.kv_cache_introspection_status.clone();

    LlamaRuntimeIntrospectionReport {
        capability_source: capabilities.capability_source.clone(),
        manifest_path: capabilities.manifest_path,
        runtime_build_profile: capabilities.runtime_build_profile.clone(),
        instrumentation_backend: capabilities.instrumentation_backend.clone(),
        allocator_introspection_status: if observed_allocator_signal {
            "allocator_lifecycle_signals_observed".to_string()
        } else {
            capabilities.allocator_introspection_status
        },
        allocator_initialized_observed: observed_allocator_initialized,
        allocator_teardown_observed: observed_allocator_teardown,
        allocator_reset_observed: observed_allocator_reset_signal,
        allocator_summary: if startup_failed {
            "Runtime startup failed before normal teardown, so any observed allocator lifecycle signals should be treated as partial setup evidence rather than proof of allocator reset."
                .to_string()
        } else if observed_allocator_reset_signal {
            "NullContext observed an explicit allocator reset signal from the runtime for this session. That is stronger evidence than process-lifetime inference alone, but it is still not proof that freed pages were overwritten or zeroized."
                .to_string()
        } else if observed_allocator_initialized || observed_allocator_teardown {
            "NullContext observed allocator lifecycle setup or teardown signals for this session, but it did not observe an allocator reset signal before shutdown. Allocator cleanup should still be treated as only partially evidenced."
                .to_string()
        } else if declared_allocator_introspection_status.contains("available") {
            "This runtime declared allocator lifecycle signal support, but no allocator events were captured for this session."
                .to_string()
        } else {
            "Allocator state is still primarily bounded by runtime lifetime in this build, and NullContext did not capture any direct allocator lifecycle signals for this session."
                .to_string()
        },
        kv_cache_introspection_status: if observed_kv_signal {
            "kv_cache_lifecycle_signals_observed".to_string()
        } else {
            capabilities.kv_cache_introspection_status
        },
        kv_cache_initialized_observed: observed_kv_initialized,
        kv_cache_reused_observed: observed_kv_reused,
        kv_cache_clear_observed: observed_kv_clear,
        kv_cache_summary: if startup_failed {
            "Runtime startup failed before normal inference lifecycle completion, so any observed KV/cache signals should be treated as partial setup evidence rather than full teardown evidence.".to_string()
        } else if observed_kv_clear {
            "NullContext observed an explicit KV/cache clear signal from the runtime for this session. That is stronger evidence than process-lifetime inference alone, but it is still not proof of allocator zeroization or freed-page clearing."
                .to_string()
        } else if observed_kv_initialized || observed_kv_reused {
            "NullContext observed KV/cache lifecycle setup or reuse signals for this session, but it did not observe a KV/cache clear signal before shutdown. KV/cache teardown should still be treated as only indirectly bounded by runtime exit."
                .to_string()
        } else if declared_kv_cache_introspection_status.contains("available") {
            "This runtime declared KV/cache lifecycle signal support, but no KV/cache events were captured for this session."
                .to_string()
        } else {
            "KV/cache lifetime is still primarily bounded by runtime lifetime in this build, and NullContext did not capture any direct KV/cache lifecycle signals for this session."
                .to_string()
        },
        model_unload_signal_status: if startup_failed
            && capabilities.model_unload_signal_status == "model_unload_not_observed_directly"
        {
            "model_unload_signal_unavailable_due_to_startup_failure".to_string()
        } else if observed_model_unload_signal {
            "model_unload_signal_observed".to_string()
        } else {
            capabilities.model_unload_signal_status
        },
        allocator_reset_signal_status: if startup_failed
            && capabilities.allocator_reset_signal_status == "allocator_reset_not_observed_directly"
        {
            "allocator_reset_signal_unavailable_due_to_startup_failure".to_string()
        } else if observed_allocator_reset_signal {
            "allocator_reset_signal_observed".to_string()
        } else {
            capabilities.allocator_reset_signal_status
        },
        summary: if startup_failed {
            format!(
                "This runtime used '{}' capability detection on a build profiled as '{}', but startup failed before any allocator/KV signals could be observed directly. NullContext still lacks direct visibility into allocator reset, KV/cache teardown, or model-unload behavior on this path.",
                capabilities.capability_source, capabilities.runtime_build_profile
            )
        } else if capabilities.capability_source == "sidecar_manifest" {
            format!(
                "NullContext loaded runtime introspection capabilities from a sidecar manifest for build profile '{}'. Host-tool memory observation is still in use, and {} lifecycle signal(s) were captured from runtime output for this session.",
                capabilities.runtime_build_profile,
                observed_events.len()
            )
        } else if !observed_events.is_empty() {
            format!(
                "NullContext captured {} runtime lifecycle signal(s) from llama-server output, even though this runtime is otherwise being treated as a stock external build.",
                observed_events.len()
            )
        } else {
            "This runtime is being treated as a stock external llama-server build. NullContext can currently observe process- and host-tool-level evidence, but it does not yet have direct allocator, KV/cache, or model-unload introspection inside llama.cpp.".to_string()
        },
        observed_events,
        notes,
    }
}

fn map_runtime_introspection_signal(
    signal: &RuntimeIntrospectionSignal,
) -> LlamaRuntimeIntrospectionEventReport {
    LlamaRuntimeIntrospectionEventReport {
        event: signal.event.clone(),
        status: signal.status.clone(),
        source: signal.source_stream.clone(),
        details: signal.details.clone(),
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

fn live_gpu_visibility_status(gpu_offload_requested: bool, usage: &RuntimeUsageSnapshot) -> String {
    if !gpu_offload_requested {
        return "gpu_offload_not_requested".to_string();
    }

    match usage.gpu_pid_observed {
        Some(true) => {
            if usage.gpu_memory_bytes.is_some() {
                "gpu_pid_and_allocation_bytes_observed".to_string()
            } else {
                "gpu_pid_observed_but_allocation_bytes_unavailable".to_string()
            }
        }
        Some(false) => "gpu_pid_not_observed".to_string(),
        None => "gpu_visibility_unavailable".to_string(),
    }
}

fn post_shutdown_gpu_visibility_status(
    gpu_offload_requested: bool,
    post_shutdown: &RuntimePostShutdownObservation,
) -> String {
    if !gpu_offload_requested {
        return "gpu_offload_not_requested".to_string();
    }

    match post_shutdown.gpu_entry_present_after_shutdown {
        Some(true) => {
            if post_shutdown.gpu_memory_bytes_after_shutdown.is_some() {
                "post_shutdown_gpu_pid_and_allocation_bytes_observed".to_string()
            } else {
                "post_shutdown_gpu_pid_observed_but_allocation_bytes_unavailable".to_string()
            }
        }
        Some(false) => {
            if gpu_post_shutdown_visibility_limited(post_shutdown) {
                "post_shutdown_gpu_pid_not_observed_but_visibility_limited".to_string()
            } else {
                "post_shutdown_gpu_pid_not_observed".to_string()
            }
        }
        None => "post_shutdown_gpu_visibility_unavailable".to_string(),
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
        Some(false) => {
            if gpu_post_shutdown_visibility_limited(post_shutdown) {
                "gpu_entry_not_observed_after_shutdown_but_visibility_limited".to_string()
            } else {
                "gpu_entry_not_observed_after_shutdown".to_string()
            }
        }
        Some(true) => {
            if post_shutdown.gpu_memory_bytes_after_shutdown.is_some() {
                "gpu_entry_observed_during_post_shutdown_window".to_string()
            } else {
                "gpu_pid_observed_during_post_shutdown_window_but_memory_bytes_unavailable"
                    .to_string()
            }
        }
        None => "gpu_inspection_unavailable".to_string(),
    }
}

fn runtime_inspection_summary(
    inspection_status: &str,
    ram_inspection_status: &str,
    vram_inspection_status: &str,
    post_shutdown: &RuntimePostShutdownObservation,
    physical_footprint_delta_bytes: Option<i64>,
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
        (_, _, "gpu_pid_observed_during_post_shutdown_window_but_memory_bytes_unavailable") => format!(
            "A matching GPU PID was observed during the {} ms verification window, but the current GPU backends did not expose per-process GPU memory bytes. VRAM exposure therefore remains explicitly visible, even though byte-level usage stayed unavailable.",
            post_shutdown.verification_window_ms
        ),
        (_, _, "gpu_entry_not_observed_after_shutdown_but_visibility_limited") => format!(
            "Post-shutdown inspection did not observe a matching GPU PID over a {} ms verification window, but the current Windows/NVIDIA visibility path remained limited. Review observation notes before treating VRAM cleanup as successful.",
            post_shutdown.verification_window_ms
        ),
        ("process_still_observable_after_shutdown", _, _) => format!(
            "The llama-server PID was still observable after the {} ms verification window, so RAM cleanup evidence remains unfavorable{} and follow-up inspection is recommended.",
            post_shutdown.verification_window_ms
            ,
            physical_footprint_delta_bytes
                .map(format_signed_bytes_delta)
                .map(|value| format!("; physical footprint delta was {value}"))
                .unwrap_or_default()
        ),
        (_, _, "gpu_entry_observed_during_post_shutdown_window") => format!(
            "A matching GPU entry was observed during the {} ms verification window, so VRAM exposure remained explicitly visible after shutdown.",
            post_shutdown.verification_window_ms
        ),
        _ => format!(
            "Post-shutdown inspection completed with mixed or incomplete evidence over a {} ms verification window. Review the RAM and VRAM inspection statuses before making cleanup claims.",
            post_shutdown.verification_window_ms
        ),
    }
}

fn gpu_post_shutdown_visibility_limited(post_shutdown: &RuntimePostShutdownObservation) -> bool {
    cfg!(target_os = "windows")
        && post_shutdown.gpu_entry_present_after_shutdown == Some(false)
        && post_shutdown
            .gpu_check_backend
            .as_deref()
            .map(|backend| backend.contains("pmon"))
            .unwrap_or(false)
}

fn build_resident_region_deltas(
    before: &[RuntimeResidentRegion],
    after: &[RuntimeResidentRegion],
) -> Vec<LlamaResidentRegionDeltaReport> {
    let mut before_map = BTreeMap::new();
    let mut after_map = BTreeMap::new();

    for region in before {
        *before_map
            .entry(region.region_type.clone())
            .or_insert(0_u64) += region.resident_bytes;
    }

    for region in after {
        *after_map.entry(region.region_type.clone()).or_insert(0_u64) += region.resident_bytes;
    }

    let mut keys: Vec<String> = before_map.keys().chain(after_map.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    let mut deltas: Vec<LlamaResidentRegionDeltaReport> = keys
        .into_iter()
        .map(|region_type| {
            let before_resident_bytes = before_map.get(&region_type).copied().unwrap_or(0);
            let after_resident_bytes = after_map.get(&region_type).copied().unwrap_or(0);

            LlamaResidentRegionDeltaReport {
                region_type,
                before_resident_bytes,
                after_resident_bytes,
                resident_delta_bytes: after_resident_bytes as i64 - before_resident_bytes as i64,
            }
        })
        .collect();

    deltas.sort_by(|a, b| {
        b.resident_delta_bytes
            .abs()
            .cmp(&a.resident_delta_bytes.abs())
            .then_with(|| a.region_type.cmp(&b.region_type))
    });
    deltas.truncate(8);
    deltas
}

fn format_signed_bytes_delta(value: i64) -> String {
    if value > 0 {
        format!("+{value} bytes")
    } else {
        format!("{value} bytes")
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
    if let Some(note) = &metadata.state_note {
        return note.clone();
    }

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
        SessionLifecycleState::AbandonedActive => {
            "Startup recovery found a retained chat that had still been marked active before the previous process exited. The live in-memory runtime could not be recovered, so operator review is recommended."
                .to_string()
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
