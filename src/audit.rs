use crate::cleanup::CleanupReport;
use crate::config::SessionConfig;
use crate::memory_validation::build_memory_validation_report;
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
use std::collections::{BTreeMap, BTreeSet};
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
    #[serde(default = "default_memory_validation_report")]
    pub memory_validation: MemoryValidationReport,
    #[serde(default = "default_memory_validation_history_report")]
    pub memory_validation_history: MemoryValidationHistoryReport,
    #[serde(default = "default_platform_capability_matrix_report")]
    pub platform_capability_matrix: PlatformCapabilityMatrixReport,
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
pub struct MemoryValidationReport {
    pub validation_status: String,
    pub harness_scope: String,
    pub canary_execution_status: String,
    pub process_scan_signal_status: String,
    pub best_stage_id: Option<String>,
    pub best_stage_label: Option<String>,
    pub best_stage_kind: Option<String>,
    pub best_stage_score: u32,
    pub best_stage_verdict: String,
    pub summary: String,
    #[serde(default = "default_controlled_canary_validation_run_report")]
    pub controlled_canary_run: ControlledCanaryValidationRunReport,
    #[serde(default)]
    pub stage_scorecards: Vec<MemoryValidationStageScorecard>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlledCanaryValidationRunReport {
    pub execution_status: String,
    pub requested_passes: u32,
    pub completed_passes: u32,
    pub failed_passes: u32,
    pub aggregate_signal_status: String,
    pub aggregate_process_scan_status: String,
    pub canary_id: String,
    pub selected_pass_index: Option<u32>,
    pub selected_pass_canary_id: Option<String>,
    #[serde(default = "default_controlled_canary_selection_reason")]
    pub selection_reason: String,
    pub runtime_pid: Option<u32>,
    pub runtime_endpoint: Option<String>,
    pub response_bytes: Option<usize>,
    pub summary: String,
    pub process_scan: ProcessScanReport,
    #[serde(default)]
    pub passes: Vec<ControlledCanaryValidationPassReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlledCanaryValidationPassReport {
    pub pass_index: u32,
    pub execution_status: String,
    pub canary_id: String,
    pub runtime_pid: Option<u32>,
    pub runtime_endpoint: Option<String>,
    pub response_bytes: Option<usize>,
    pub summary: String,
    pub process_scan: ProcessScanReport,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryValidationStageScorecard {
    pub stage_id: String,
    pub stage_label: String,
    pub stage_kind: String,
    pub action_status: String,
    pub vram_evidence_status: String,
    #[serde(default = "default_vram_cleanup_marker_evidence_status")]
    pub marker_evidence_status: String,
    pub process_scan_context_status: String,
    #[serde(default = "default_process_scan_context_scope")]
    pub process_scan_context_scope: String,
    #[serde(default = "default_cleanup_signal_support_status")]
    pub cleanup_signal_support_status: String,
    #[serde(default = "default_cleanup_signal_support_summary")]
    pub cleanup_signal_support_summary: String,
    #[serde(default = "default_controlled_canary_signal_status")]
    pub controlled_canary_signal_status: String,
    pub validation_score: u32,
    pub validation_verdict: String,
    pub summary: String,
    pub strengths: Vec<String>,
    pub gaps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryValidationHistoryReport {
    pub history_status: String,
    pub scope_key: String,
    pub scope_model_id: Option<String>,
    pub scope_platform: Option<String>,
    pub scope_gpu_offload_requested: Option<bool>,
    pub runs_recorded: u32,
    pub marker_detection_runs: u32,
    pub clear_canary_runs: u32,
    pub inconclusive_or_failed_runs: u32,
    pub strong_or_moderate_runs: u32,
    pub best_stage_score_min: Option<u32>,
    pub best_stage_score_max: Option<u32>,
    pub best_stage_score_avg: Option<f64>,
    pub last_recorded_at: Option<String>,
    #[serde(default)]
    pub stage_trends: Vec<MemoryValidationStageTrendReport>,
    #[serde(default = "default_controlled_canary_history_report")]
    pub controlled_canary_history: ControlledCanaryHistoryReport,
    #[serde(default = "default_memory_validation_stage_recommendation_report")]
    pub cleanup_stage_recommendation: MemoryValidationStageRecommendationReport,
    #[serde(default = "default_validation_release_gate_report")]
    pub release_gate: ValidationReleaseGateReport,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReleaseGateReport {
    pub gate_status: String,
    pub cleanup_stage_gate_status: String,
    pub controlled_canary_gate_status: String,
    pub min_stage_runs_required: u32,
    pub min_clear_canary_runs_required: u32,
    pub max_marker_detection_runs_allowed_for_clean_claim: u32,
    pub max_worsened_runs_allowed_for_clean_stage: u32,
    pub max_inconclusive_runs_allowed_for_clean_stage: u32,
    pub stage_gate_passed: bool,
    pub controlled_canary_gate_passed: bool,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlledCanaryHistoryReport {
    pub history_status: String,
    pub recommendation_status: String,
    pub runs_with_canary_requested: u32,
    pub runs_with_completed_passes: u32,
    pub total_requested_passes: u32,
    pub total_completed_passes: u32,
    pub total_failed_passes: u32,
    pub clear_runs: u32,
    pub marker_detection_runs: u32,
    pub mixed_or_inconclusive_runs: u32,
    pub backend_unsupported_runs: u32,
    pub latest_execution_status: String,
    pub latest_aggregate_signal_status: String,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryValidationStageRecommendationReport {
    pub recommendation_status: String,
    pub clean_claim_status: String,
    pub stage_id: Option<String>,
    pub stage_label: Option<String>,
    pub stage_kind: Option<String>,
    pub runner_up_stage_id: Option<String>,
    pub runner_up_stage_label: Option<String>,
    pub runner_up_stage_kind: Option<String>,
    pub compared_stage_count: u32,
    pub runs_recorded: u32,
    pub avg_validation_score: Option<f64>,
    pub effectiveness_score: Option<f64>,
    pub runner_up_effectiveness_score: Option<f64>,
    pub effectiveness_gap: Option<f64>,
    pub avg_validation_score_gap: Option<f64>,
    pub marker_detection_gap: Option<i32>,
    pub improved_runs: u32,
    pub unchanged_runs: u32,
    pub worsened_runs: u32,
    pub inconclusive_runs: u32,
    pub marker_detection_runs: u32,
    pub summary: String,
    pub clean_claim_summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryValidationStageTrendReport {
    pub stage_id: String,
    pub stage_label: String,
    pub stage_kind: String,
    pub runs_recorded: u32,
    pub avg_validation_score: f64,
    pub best_validation_score: u32,
    pub improved_runs: u32,
    pub unchanged_runs: u32,
    pub worsened_runs: u32,
    pub inconclusive_runs: u32,
    pub strong_or_moderate_runs: u32,
    pub marker_detection_runs: u32,
    pub clear_marker_support_runs: u32,
    pub helper_scan_runs: u32,
    pub helper_scan_clear_runs: u32,
    pub helper_scan_marker_detection_runs: u32,
    #[serde(default)]
    pub cleanup_signal_strong_runs: u32,
    #[serde(default)]
    pub cleanup_signal_partial_runs: u32,
    #[serde(default)]
    pub cleanup_signal_limited_runs: u32,
    #[serde(default)]
    pub stage_local_scan_runs: u32,
    #[serde(default)]
    pub stage_local_scan_clear_runs: u32,
    #[serde(default)]
    pub stage_local_scan_marker_detection_runs: u32,
    #[serde(default)]
    pub stage_local_scan_limited_runs: u32,
    #[serde(default)]
    pub session_fallback_scan_runs: u32,
    pub latest_vram_evidence_status: String,
    pub latest_validation_verdict: String,
    pub latest_marker_evidence_status: String,
    #[serde(default = "default_cleanup_signal_support_status")]
    pub latest_cleanup_signal_support_status: String,
    #[serde(default = "default_process_scan_context_status")]
    pub latest_process_scan_context_status: String,
    #[serde(default = "default_process_scan_context_scope")]
    pub latest_process_scan_context_scope: String,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilityMatrixReport {
    pub matrix_status: String,
    pub scope_platform: String,
    pub scope_model_id: Option<String>,
    pub runtime_build_profile: Option<String>,
    pub gpu_offload_requested: Option<bool>,
    pub summary: String,
    #[serde(default)]
    pub capabilities: Vec<PlatformCapabilityEntryReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapabilityEntryReport {
    pub capability_id: String,
    pub capability_label: String,
    pub roadmap_track: String,
    pub current_status: String,
    pub evidence_level: String,
    pub v1_blocker: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub notes: Vec<String>,
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
    #[serde(default = "default_vram_cleanup_strategy_report")]
    pub vram_cleanup: VramCleanupStrategyReport,
    pub memory_domains: Vec<LlamaMemoryDomainReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramCleanupStrategyReport {
    pub strategy_id: String,
    pub strategy_label: String,
    pub strategy_kind: String,
    pub implementation_status: String,
    pub support_status: String,
    pub attempt_status: String,
    pub activation_timing: String,
    pub evidence_outcome: String,
    pub expected_effect_scope: String,
    pub summary: String,
    #[serde(default = "default_vram_cleanup_comparison_report")]
    pub comparison: VramCleanupComparisonReport,
    #[serde(default)]
    pub stages: Vec<VramCleanupStrategyStageReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramCleanupComparisonReport {
    pub comparison_status: String,
    pub current_run_role: String,
    pub evidence_improvement_status: String,
    #[serde(default = "default_vram_cleanup_marker_evidence_status")]
    pub marker_evidence_status: String,
    #[serde(default = "default_vram_cleanup_marker_evidence_summary")]
    pub marker_evidence_summary: String,
    pub baseline_snapshot: VramCleanupEvidenceSnapshot,
    pub current_snapshot: VramCleanupEvidenceSnapshot,
    #[serde(default)]
    pub selected_stage_id: Option<String>,
    #[serde(default)]
    pub selected_stage_label: Option<String>,
    #[serde(default)]
    pub selected_stage_kind: Option<String>,
    #[serde(default = "default_vram_cleanup_selection_reason")]
    pub selection_reason: String,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramCleanupEvidenceSnapshot {
    pub vram_inspection_status: String,
    pub post_shutdown_gpu_visibility_status: String,
    pub gpu_entry_observed: Option<bool>,
    pub gpu_memory_bytes: Option<u64>,
    pub gpu_peak_memory_bytes: Option<u64>,
    pub gpu_samples_collected: u32,
    pub gpu_samples_with_pid_observed: u32,
    pub gpu_last_pid_observed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramCleanupStrategyStageReport {
    pub stage_id: String,
    pub stage_label: String,
    pub stage_kind: String,
    pub cooldown_ms_before_stage: u64,
    pub verification_window_ms: u64,
    pub action_status: String,
    pub evidence_improvement_status: String,
    #[serde(default)]
    pub process_scan_phase: Option<ProcessScanPhaseReport>,
    #[serde(default)]
    pub helper_process_scan_report: Option<ProcessScanReport>,
    #[serde(default = "default_vram_cleanup_marker_evidence_status")]
    pub marker_evidence_status: String,
    #[serde(default = "default_vram_cleanup_marker_evidence_summary")]
    pub marker_evidence_summary: String,
    pub evidence_snapshot: VramCleanupEvidenceSnapshot,
    pub summary: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeIntrospectionReport {
    pub capability_source: String,
    pub manifest_path: Option<String>,
    pub runtime_build_profile: String,
    pub instrumentation_backend: String,
    pub declared_signal_ids: Vec<String>,
    pub declared_cleanup_signal_ids: Vec<String>,
    pub lifecycle_signal_evidence_tier: String,
    pub signal_contract_status: String,
    pub signal_contract_summary: String,
    pub instrumentation_evidence_status: String,
    pub instrumentation_evidence_summary: String,
    pub declared_signal_count: u32,
    pub observed_signal_unique_count: u32,
    pub missing_declared_signal_count: u32,
    pub undeclared_observed_signal_count: u32,
    pub cleanup_path_evidence_status: String,
    pub setup_signal_coverage_status: String,
    pub cleanup_signal_coverage_status: String,
    pub cleanup_signal_contract_status: String,
    pub cleanup_signal_contract_summary: String,
    pub declared_cleanup_signal_count: u32,
    pub observed_cleanup_signal_count: u32,
    pub missing_declared_cleanup_signal_count: u32,
    pub undeclared_observed_cleanup_signal_count: u32,
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
    pub model_unload_observed: bool,
    pub model_unload_signal_status: String,
    pub allocator_reset_signal_status: String,
    pub summary: String,
    pub observed_signal_count: u32,
    pub observed_signal_sources: Vec<String>,
    pub cleanup_signal_matrix: Vec<LlamaRuntimeCleanupSignalEntryReport>,
    pub observed_events: Vec<LlamaRuntimeIntrospectionEventReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeCleanupSignalEntryReport {
    pub signal_id: String,
    pub signal_label: String,
    pub declared_support_status: String,
    pub observation_status: String,
    pub evidence_status: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeIntrospectionEventReport {
    pub event: String,
    pub status: String,
    pub source: String,
    pub lifecycle_phase: String,
    pub evidence_scope: String,
    pub cleanup_relevance: String,
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
            memory_validation: default_memory_validation_report(),
            memory_validation_history: default_memory_validation_history_report(),
            platform_capability_matrix: default_platform_capability_matrix_report(),
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
        self.refresh_derived_sections();
        self
    }

    pub fn with_process_scan(mut self, process_scan: ProcessScanReport) -> Self {
        self.process_scan = Some(process_scan);
        self.refresh_derived_sections();
        self
    }

    pub fn with_controlled_canary_run(
        mut self,
        controlled_canary_run: ControlledCanaryValidationRunReport,
    ) -> Self {
        self.memory_validation.controlled_canary_run = controlled_canary_run;
        self.refresh_derived_sections();
        self
    }

    pub fn with_memory_validation_history(
        mut self,
        memory_validation_history: MemoryValidationHistoryReport,
    ) -> Self {
        self.memory_validation_history = memory_validation_history;
        self.refresh_derived_sections();
        self
    }

    pub fn with_retrieval(mut self, retrieval: RetrievalReport) -> Self {
        self.retrieval = Some(retrieval);
        self
    }

    pub fn to_pretty_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    fn refresh_derived_sections(&mut self) {
        contextualize_vram_cleanup_marker_evidence(self);
        self.memory_validation = build_memory_validation_report(self);
        self.platform_capability_matrix = build_platform_capability_matrix_report(self);
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
    report.memory_validation = build_memory_validation_report(&report);
    report.platform_capability_matrix = build_platform_capability_matrix_report(&report);
    fs::write(report_path, report.to_pretty_json()?)?;

    Ok(())
}

fn build_platform_capability_matrix_report(
    report: &PrivacyReport,
) -> PlatformCapabilityMatrixReport {
    let scope_platform = report
        .process_scan
        .as_ref()
        .map(|scan| scan.platform.clone())
        .or_else(|| {
            let platform = report
                .memory_validation
                .controlled_canary_run
                .process_scan
                .platform
                .clone();
            if platform.is_empty() {
                None
            } else {
                Some(platform)
            }
        })
        .unwrap_or_else(|| std::env::consts::OS.to_string());

    let Some(llama_runtime) = report.llama_runtime.as_ref() else {
        return PlatformCapabilityMatrixReport {
            matrix_status: "matrix_waiting_for_runtime_report".to_string(),
            scope_platform,
            scope_model_id: None,
            runtime_build_profile: None,
            gpu_offload_requested: None,
            summary:
                "NullContext reserved the platform capability matrix, but this report did not include a runtime section yet."
                    .to_string(),
            capabilities: vec![],
            notes: vec![
                "The matrix is derived after runtime/process/canary evidence is attached to the report."
                    .to_string(),
            ],
        };
    };

    let process_scan_entry = build_process_scan_capability_entry(report);
    let allocator_entry = build_allocator_kv_capability_entry(llama_runtime);
    let gpu_inspection_entry =
        build_gpu_inspection_capability_entry(&scope_platform, llama_runtime);
    let cleanup_entry =
        build_vram_cleanup_capability_entry(llama_runtime, &report.memory_validation);
    let validation_entry = build_validation_harness_capability_entry(
        &report.memory_validation,
        &report.memory_validation_history,
    );

    let capabilities = vec![
        process_scan_entry,
        allocator_entry,
        gpu_inspection_entry,
        cleanup_entry,
        validation_entry,
    ];

    let supported_or_active = capabilities
        .iter()
        .filter(|entry| {
            entry.current_status.contains("supported")
                || entry.current_status.contains("available")
                || entry.current_status.contains("active")
        })
        .count();
    let limited_or_unavailable = capabilities
        .iter()
        .filter(|entry| {
            entry.current_status.contains("limited")
                || entry.current_status.contains("unavailable")
                || entry.current_status.contains("unsupported")
                || entry.current_status.contains("not_exercised")
                || entry.current_status.contains("not_applicable")
        })
        .count();
    let blocker_gaps = capabilities
        .iter()
        .filter(|entry| {
            entry.v1_blocker
                && !entry.current_status.contains("supported")
                && !entry.current_status.contains("available")
                && !entry.current_status.contains("active")
        })
        .count();

    PlatformCapabilityMatrixReport {
        matrix_status: if blocker_gaps == 0 {
            "matrix_ready_for_current_scope".to_string()
        } else {
            "matrix_blockers_still_open".to_string()
        },
        scope_platform,
        scope_model_id: Some(llama_runtime.model_id.clone()),
        runtime_build_profile: Some(llama_runtime.introspection.runtime_build_profile.clone()),
        gpu_offload_requested: Some(llama_runtime.gpu_offload_requested),
        summary: format!(
            "NullContext derived a v1 roadmap capability matrix for this {} report scope. {} track(s) are currently active or supported, {} remain limited or unavailable, and {} v1-blocking track(s) still have open gaps.",
            llama_runtime.model_id,
            supported_or_active,
            limited_or_unavailable,
            blocker_gaps
        ),
        capabilities,
        notes: vec![
            "This matrix is a scope-level readiness summary for the current report, not a claim that full RAM or VRAM sanitization has been achieved."
                .to_string(),
            "Track C remains intentionally platform-specific: Windows/NVIDIA API inspection work should not be overstated on macOS or non-CUDA paths."
                .to_string(),
        ],
    }
}

fn contextualize_vram_cleanup_marker_evidence(report: &mut PrivacyReport) {
    let process_scan_signal_status = report
        .process_scan
        .as_ref()
        .map(derive_process_scan_signal_status_from_report)
        .unwrap_or_else(|| "process_scan_context_unavailable".to_string());
    let controlled_canary_signal_status = report
        .memory_validation
        .controlled_canary_run
        .aggregate_signal_status
        .clone();

    let Some(llama_runtime) = report.llama_runtime.as_mut() else {
        return;
    };

    let selected_stage_process_scan_signal_status = llama_runtime
        .vram_cleanup
        .comparison
        .selected_stage_id
        .as_ref()
        .and_then(|selected_stage_id| {
            llama_runtime
                .vram_cleanup
                .stages
                .iter()
                .find(|stage| &stage.stage_id == selected_stage_id)
                .and_then(derive_stage_process_scan_signal_status)
        });
    let comparison_process_scan_signal_status = selected_stage_process_scan_signal_status
        .as_deref()
        .unwrap_or(process_scan_signal_status.as_str());
    let comparison_marker_evidence_status = derive_vram_cleanup_marker_evidence_status(
        &llama_runtime
            .vram_cleanup
            .comparison
            .evidence_improvement_status,
        comparison_process_scan_signal_status,
        &controlled_canary_signal_status,
    );
    llama_runtime
        .vram_cleanup
        .comparison
        .marker_evidence_summary = vram_cleanup_marker_evidence_summary(
        &comparison_marker_evidence_status,
        comparison_process_scan_signal_status,
        &controlled_canary_signal_status,
    );
    llama_runtime.vram_cleanup.comparison.marker_evidence_status =
        comparison_marker_evidence_status;

    for stage in &mut llama_runtime.vram_cleanup.stages {
        let stage_process_scan_signal_status = derive_stage_process_scan_signal_status(stage)
            .unwrap_or_else(|| process_scan_signal_status.clone());
        let marker_evidence_status = derive_vram_cleanup_marker_evidence_status(
            &stage.evidence_improvement_status,
            &stage_process_scan_signal_status,
            &controlled_canary_signal_status,
        );
        stage.marker_evidence_summary = vram_cleanup_marker_evidence_summary(
            &marker_evidence_status,
            &stage_process_scan_signal_status,
            &controlled_canary_signal_status,
        );
        stage.marker_evidence_status = marker_evidence_status;
    }
}

fn derive_process_scan_signal_status_from_report(report: &ProcessScanReport) -> String {
    match report.overall_status.as_str() {
        "markers_detected_in_scanned_memory" => "marker_persistence_detected".to_string(),
        "no_markers_detected_in_scanned_regions" => {
            "marker_scan_clear_in_scanned_regions".to_string()
        }
        "scan_attempt_failed" => "marker_scan_inconclusive".to_string(),
        "scan_backend_unsupported_on_platform" => "marker_scan_backend_unsupported".to_string(),
        "scan_skipped" | "scan_not_completed" => "marker_scan_not_completed".to_string(),
        _ => "marker_scan_context_mixed".to_string(),
    }
}

fn derive_process_scan_signal_status_from_phase(phase: &ProcessScanPhaseReport) -> String {
    if phase
        .patterns
        .iter()
        .any(|pattern| pattern.status == "detected_in_scanned_memory")
    {
        return "marker_persistence_detected".to_string();
    }

    match phase.status.as_str() {
        "scan_completed" => "marker_scan_clear_in_scanned_regions".to_string(),
        "scan_attempt_failed" | "scan_attempt_incomplete" => "marker_scan_inconclusive".to_string(),
        "scan_backend_unsupported_on_platform" => "marker_scan_backend_unsupported".to_string(),
        "process_not_observable_for_scan"
        | "post_shutdown_observation_inconclusive"
        | "pattern_empty" => "marker_scan_not_completed".to_string(),
        _ => "marker_scan_context_mixed".to_string(),
    }
}

fn derive_stage_process_scan_signal_status(
    stage: &VramCleanupStrategyStageReport,
) -> Option<String> {
    stage
        .helper_process_scan_report
        .as_ref()
        .map(derive_process_scan_signal_status_from_report)
        .or_else(|| {
            stage
                .process_scan_phase
                .as_ref()
                .map(derive_process_scan_signal_status_from_phase)
        })
}

fn derive_vram_cleanup_marker_evidence_status(
    vram_evidence_status: &str,
    process_scan_signal_status: &str,
    controlled_canary_signal_status: &str,
) -> String {
    let has_strong_gpu_improvement = matches!(
        vram_evidence_status,
        "evidence_improved_pid_no_longer_observed_after_strategy"
            | "evidence_unchanged_not_observed"
            | "evidence_improved_bytes_no_longer_visible_but_pid_still_observed"
            | "evidence_improved_peak_bytes_lower_but_residency_still_observed"
    );
    let session_clear = process_scan_signal_status == "marker_scan_clear_in_scanned_regions";
    let canary_clear =
        controlled_canary_signal_status == "controlled_canary_all_completed_passes_clear";
    let marker_detected = process_scan_signal_status == "marker_persistence_detected"
        || controlled_canary_signal_status == "controlled_canary_markers_detected_across_passes";
    let marker_limited = matches!(
        process_scan_signal_status,
        "marker_scan_inconclusive"
            | "marker_scan_backend_unsupported"
            | "marker_scan_not_completed"
            | "process_scan_context_unavailable"
            | "marker_scan_context_mixed"
    ) || matches!(
        controlled_canary_signal_status,
        "controlled_canary_backend_unsupported_across_passes"
            | "controlled_canary_mixed_clear_and_inconclusive"
            | "controlled_canary_inconclusive_across_passes"
            | "controlled_canary_completed_with_failures"
            | "controlled_canary_request_failed"
            | "controlled_canary_shutdown_failed"
            | "controlled_canary_helper_failed"
            | "controlled_canary_all_passes_failed"
            | "controlled_canary_not_run_yet"
    );

    if marker_detected {
        if has_strong_gpu_improvement {
            "gpu_evidence_improved_but_marker_persistence_detected".to_string()
        } else {
            "marker_persistence_detected_without_supporting_gpu_improvement".to_string()
        }
    } else if session_clear && canary_clear {
        "gpu_evidence_supported_by_clear_session_and_canary_scans".to_string()
    } else if session_clear || canary_clear {
        "gpu_evidence_supported_by_partial_marker_clearance".to_string()
    } else if marker_limited {
        "gpu_evidence_without_clear_marker_confirmation".to_string()
    } else {
        "marker_evidence_context_mixed".to_string()
    }
}

fn vram_cleanup_marker_evidence_summary(
    marker_evidence_status: &str,
    process_scan_signal_status: &str,
    controlled_canary_signal_status: &str,
) -> String {
    match marker_evidence_status {
        "gpu_evidence_improved_but_marker_persistence_detected" => format!(
            "GPU visibility improved, but direct marker evidence still remained negative: session scan {}, controlled canary {}.",
            process_scan_signal_status.replace('_', " "),
            controlled_canary_signal_status.replace('_', " ")
        ),
        "marker_persistence_detected_without_supporting_gpu_improvement" => format!(
            "Neither GPU visibility nor marker evidence produced a clean outcome: session scan {}, controlled canary {}.",
            process_scan_signal_status.replace('_', " "),
            controlled_canary_signal_status.replace('_', " ")
        ),
        "gpu_evidence_supported_by_clear_session_and_canary_scans" => {
            "This GPU-evidence result is reinforced by both a clear session process scan and clear repeated controlled canary passes."
                .to_string()
        }
        "gpu_evidence_supported_by_partial_marker_clearance" => format!(
            "This GPU-evidence result has partial RAM-side support: session scan {}, controlled canary {}.",
            process_scan_signal_status.replace('_', " "),
            controlled_canary_signal_status.replace('_', " ")
        ),
        "gpu_evidence_without_clear_marker_confirmation" => format!(
            "GPU visibility evidence exists, but RAM-side marker confirmation remained limited or unavailable: session scan {}, controlled canary {}.",
            process_scan_signal_status.replace('_', " "),
            controlled_canary_signal_status.replace('_', " ")
        ),
        _ => format!(
            "Marker-evidence context remained mixed for this cleanup result: session scan {}, controlled canary {}.",
            process_scan_signal_status.replace('_', " "),
            controlled_canary_signal_status.replace('_', " ")
        ),
    }
}

fn build_process_scan_capability_entry(report: &PrivacyReport) -> PlatformCapabilityEntryReport {
    let process_scan = report
        .process_scan
        .as_ref()
        .unwrap_or(&report.memory_validation.controlled_canary_run.process_scan);
    let current_status = if process_scan.implementation_status
        == "direct_process_scan_not_implemented_on_platform"
    {
        "direct_process_scan_unsupported_on_current_platform".to_string()
    } else if process_scan.implementation_status == "controlled_canary_not_run_yet" {
        "direct_process_scan_not_exercised_in_this_report".to_string()
    } else {
        "direct_process_scan_supported_on_current_platform".to_string()
    };
    let evidence_level = match process_scan.overall_status.as_str() {
        "markers_detected_in_scanned_memory" => "direct_marker_detection_observed",
        "no_markers_detected_in_scanned_regions" => "direct_marker_miss_observed",
        "scan_attempt_failed" => "direct_scan_attempt_inconclusive",
        "scan_backend_unsupported_on_platform" => "direct_scan_backend_unavailable",
        _ => "direct_scan_not_completed_for_scope",
    }
    .to_string();

    PlatformCapabilityEntryReport {
        capability_id: "track_a_process_memory_scan".to_string(),
        capability_label: "Direct Process Memory Scanning".to_string(),
        roadmap_track: "track_a".to_string(),
        current_status,
        evidence_level,
        v1_blocker: true,
        claim_boundary:
            "Scans configured markers in readable llama-server regions; it is not yet full forensic process-memory coverage."
                .to_string(),
        summary: process_scan.summary.clone(),
        notes: {
            let mut notes = vec![process_scan.residual_risk_summary.clone()];
            notes.extend(process_scan.notes.clone());
            notes
        },
    }
}

fn build_allocator_kv_capability_entry(
    llama_runtime: &LlamaRuntimeReport,
) -> PlatformCapabilityEntryReport {
    let introspection = &llama_runtime.introspection;
    let current_status =
        if introspection.lifecycle_signal_evidence_tier == "direct_cleanup_path_signals_observed" {
            "allocator_and_kv_cleanup_path_signals_active".to_string()
        } else {
            match introspection.instrumentation_evidence_status.as_str() {
                "manifest_declared_instrumentation_fully_exercised" => {
                    "allocator_and_kv_manifest_instrumentation_fully_exercised".to_string()
                }
                "manifest_declared_instrumentation_partially_exercised" => {
                    "allocator_and_kv_manifest_instrumentation_partially_exercised".to_string()
                }
                "manifest_declared_instrumentation_unobserved_in_run" => {
                    "allocator_and_kv_declared_instrumentation_unobserved_in_run".to_string()
                }
                "runtime_signals_observed_without_manifest_declared_instrumentation" => {
                    "allocator_and_kv_observed_without_declared_instrumentation".to_string()
                }
                "instrumentation_evidence_interrupted_by_startup_failure" => {
                    "allocator_and_kv_signal_collection_blocked_by_startup_failure".to_string()
                }
                "stock_runtime_without_instrumented_signal_support" => {
                    "allocator_and_kv_signal_support_limited".to_string()
                }
                _ => match introspection.lifecycle_signal_evidence_tier.as_str() {
                    "declared_and_observed_runtime_signals"
                    | "observed_runtime_signals_without_declared_manifest" => {
                        "allocator_and_kv_signal_support_partial".to_string()
                    }
                    "declared_support_without_observed_session_signals" => {
                        "allocator_and_kv_declared_support_without_session_evidence".to_string()
                    }
                    "startup_failed_without_direct_runtime_signals" => {
                        "allocator_and_kv_signal_collection_blocked_by_startup_failure".to_string()
                    }
                    _ => "allocator_and_kv_signal_support_limited".to_string(),
                },
            }
        };
    let evidence_level = introspection.lifecycle_signal_evidence_tier.clone();

    PlatformCapabilityEntryReport {
        capability_id: "track_b_allocator_kv_introspection".to_string(),
        capability_label: "Allocator / KV Introspection".to_string(),
        roadmap_track: "track_b".to_string(),
        current_status,
        evidence_level,
        v1_blocker: true,
        claim_boundary:
            "Observed lifecycle signals strengthen allocator/KV evidence, but they still do not prove freed-page overwrites or zeroization."
                .to_string(),
        summary: introspection.summary.clone(),
        notes: vec![
            introspection.allocator_summary.clone(),
            introspection.kv_cache_summary.clone(),
            introspection.instrumentation_evidence_summary.clone(),
            introspection.cleanup_signal_contract_summary.clone(),
            format!(
                "Instrumentation evidence: {}. Setup-signal coverage: {}. Cleanup-path evidence status: {}. Cleanup-signal coverage: {}. Cleanup-signal contract: {}. Observed signal count: {}.",
                introspection.instrumentation_evidence_status.replace('_', " "),
                introspection.setup_signal_coverage_status.replace('_', " "),
                introspection.cleanup_path_evidence_status.replace('_', " "),
                introspection.cleanup_signal_coverage_status.replace('_', " "),
                introspection.cleanup_signal_contract_status.replace('_', " "),
                introspection.observed_signal_count
            ),
        ],
    }
}

fn build_gpu_inspection_capability_entry(
    scope_platform: &str,
    llama_runtime: &LlamaRuntimeReport,
) -> PlatformCapabilityEntryReport {
    let current_status = if scope_platform != "windows" {
        "windows_nvidia_track_not_applicable_on_current_platform".to_string()
    } else if !llama_runtime.gpu_offload_requested {
        "windows_nvidia_gpu_inspection_not_exercised_in_this_report".to_string()
    } else if llama_runtime
        .gpu_observation_backend
        .as_deref()
        .map(|backend| backend.contains("nvidia"))
        .unwrap_or(false)
        || llama_runtime
            .gpu_check_backend
            .as_deref()
            .map(|backend| backend.contains("nvidia"))
            .unwrap_or(false)
    {
        "windows_nvidia_gpu_inspection_supported_with_host_tool_evidence".to_string()
    } else {
        "windows_nvidia_gpu_inspection_limited".to_string()
    };
    let evidence_level = if scope_platform != "windows" {
        "non_windows_scope".to_string()
    } else if !llama_runtime.gpu_offload_requested {
        "gpu_offload_not_exercised".to_string()
    } else {
        llama_runtime.vram_inspection_status.clone()
    };

    PlatformCapabilityEntryReport {
        capability_id: "track_c_cuda_nvidia_api_inspection".to_string(),
        capability_label: "CUDA / NVIDIA API Inspection".to_string(),
        roadmap_track: "track_c".to_string(),
        current_status,
        evidence_level,
        v1_blocker: true,
        claim_boundary:
            "Current GPU evidence may still come from host tools rather than true CUDA/NVML allocator-level inspection."
                .to_string(),
        summary: llama_runtime.inspection_summary.clone(),
        notes: vec![
            llama_runtime.cleanup_summary.clone(),
            llama_runtime.residual_risk_summary.clone(),
        ],
    }
}

fn build_vram_cleanup_capability_entry(
    llama_runtime: &LlamaRuntimeReport,
    memory_validation: &MemoryValidationReport,
) -> PlatformCapabilityEntryReport {
    let current_status = if !llama_runtime.gpu_offload_requested {
        "experimental_cleanup_not_exercised_in_this_report".to_string()
    } else if !llama_runtime.vram_cleanup.stages.is_empty() {
        "experimental_cleanup_stages_available".to_string()
    } else {
        "experimental_cleanup_stages_unavailable".to_string()
    };
    let evidence_level = if !llama_runtime.gpu_offload_requested {
        "gpu_offload_not_exercised".to_string()
    } else if !llama_runtime.vram_cleanup.stages.is_empty() {
        memory_validation.best_stage_verdict.clone()
    } else {
        llama_runtime.vram_cleanup.evidence_outcome.clone()
    };

    PlatformCapabilityEntryReport {
        capability_id: "track_d_experimental_vram_cleanup".to_string(),
        capability_label: "Experimental VRAM Cleanup".to_string(),
        roadmap_track: "track_d".to_string(),
        current_status,
        evidence_level,
        v1_blocker: true,
        claim_boundary:
            "Cleanup stages are measured by post-shutdown evidence changes; they are experiments, not proof of sanitized VRAM contents."
                .to_string(),
        summary: llama_runtime.vram_cleanup.summary.clone(),
        notes: llama_runtime.vram_cleanup.notes.clone(),
    }
}

fn build_validation_harness_capability_entry(
    memory_validation: &MemoryValidationReport,
    memory_validation_history: &MemoryValidationHistoryReport,
) -> PlatformCapabilityEntryReport {
    let current_status = if memory_validation_history
        .cleanup_stage_recommendation
        .clean_claim_status
        == "clean_claim_eligible_under_current_thresholds"
    {
        "validation_harness_active_with_clean_stage_candidate".to_string()
    } else if memory_validation_history
        .controlled_canary_history
        .recommendation_status
        == "controlled_canary_repeated_clear_history"
    {
        "validation_harness_active_with_repeated_clear_canary_history".to_string()
    } else if memory_validation_history
        .cleanup_stage_recommendation
        .recommendation_status
        == "recommendation_available"
    {
        "validation_harness_active_with_repeated_stage_guidance".to_string()
    } else if memory_validation.controlled_canary_run.requested_passes > 0
        && memory_validation_history.runs_recorded > 0
    {
        "validation_harness_active_with_cross_session_history".to_string()
    } else if memory_validation.controlled_canary_run.requested_passes > 0 {
        "validation_harness_active_for_current_report_only".to_string()
    } else {
        "validation_harness_not_exercised".to_string()
    };
    let evidence_level = if memory_validation.controlled_canary_run.requested_passes > 0 {
        memory_validation
            .controlled_canary_run
            .aggregate_signal_status
            .clone()
    } else {
        memory_validation.validation_status.clone()
    };

    PlatformCapabilityEntryReport {
        capability_id: "track_e_validation_release_gating".to_string(),
        capability_label: "Validation / Release Gating".to_string(),
        roadmap_track: "track_e".to_string(),
        current_status,
        evidence_level,
        v1_blocker: true,
        claim_boundary:
            "Validation history and repeated canary passes improve confidence, but they are still comparative evidence rather than release-proof forensic guarantees."
                .to_string(),
        summary: memory_validation.summary.clone(),
        notes: vec![
            memory_validation_history.summary.clone(),
            memory_validation_history
                .controlled_canary_history
                .summary
                .clone(),
            memory_validation_history
                .cleanup_stage_recommendation
                .summary
                .clone(),
            memory_validation_history
                .cleanup_stage_recommendation
                .clean_claim_summary
                .clone(),
            format!(
                "Controlled canary aggregate signal: {}.",
                memory_validation
                    .controlled_canary_run
                    .aggregate_signal_status
                    .replace('_', " ")
            ),
        ],
    }
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
    let vram_cleanup = build_vram_cleanup_strategy_report(
        gpu_offload_requested,
        &vram_inspection_status,
        post_shutdown,
    );
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
        vram_cleanup,
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
    let vram_cleanup = build_failed_start_vram_cleanup_strategy_report(gpu_offload_requested);
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
        vram_cleanup,
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
                declared_signal_ids: vec![],
                declared_cleanup_signal_ids: vec![],
                lifecycle_signal_evidence_tier: "introspection_capability_load_failed"
                    .to_string(),
                signal_contract_status: if startup_failed {
                    "signal_contract_interrupted_by_startup_failure".to_string()
                } else {
                    "signal_contract_unavailable".to_string()
                },
                signal_contract_summary: if startup_failed {
                    "Runtime startup failed before NullContext could compare declared runtime-signal support with observed runtime-signal evidence."
                        .to_string()
                } else {
                    "NullContext could not derive a runtime-signal contract comparison because runtime capability loading failed."
                        .to_string()
                },
                instrumentation_evidence_status: if startup_failed {
                    "instrumentation_evidence_interrupted_by_startup_failure".to_string()
                } else {
                    "instrumentation_evidence_unavailable".to_string()
                },
                instrumentation_evidence_summary: if startup_failed {
                    "Runtime startup failed before NullContext could determine whether the current run exercised a trustworthy instrumented runtime path."
                        .to_string()
                } else {
                    "NullContext could not determine whether this run used a trustworthy instrumented runtime path because capability loading failed."
                        .to_string()
                },
                declared_signal_count: 0,
                observed_signal_unique_count: 0,
                missing_declared_signal_count: 0,
                undeclared_observed_signal_count: 0,
                cleanup_path_evidence_status: if startup_failed {
                    "cleanup_path_unavailable_due_to_startup_failure".to_string()
                } else {
                    "cleanup_path_not_observed_directly".to_string()
                },
                setup_signal_coverage_status: if startup_failed {
                    "setup_signal_collection_interrupted_by_startup_failure".to_string()
                } else {
                    "no_setup_or_reuse_signals_observed".to_string()
                },
                cleanup_signal_coverage_status: if startup_failed {
                    "cleanup_signal_collection_interrupted_by_startup_failure".to_string()
                } else {
                    "no_cleanup_signals_observed".to_string()
                },
                cleanup_signal_contract_status: if startup_failed {
                    "cleanup_signal_contract_interrupted_by_startup_failure".to_string()
                } else {
                    "cleanup_signal_contract_unavailable".to_string()
                },
                cleanup_signal_contract_summary: if startup_failed {
                    "Runtime startup failed before NullContext could compare declared cleanup-signal support with observed cleanup-signal evidence."
                        .to_string()
                } else {
                    "NullContext could not derive a cleanup-signal contract comparison because runtime capability loading failed."
                        .to_string()
                },
                declared_cleanup_signal_count: 0,
                observed_cleanup_signal_count: 0,
                missing_declared_cleanup_signal_count: 0,
                undeclared_observed_cleanup_signal_count: 0,
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
                model_unload_observed: false,
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
                observed_signal_count: observed_signals.len() as u32,
                observed_signal_sources: observed_signal_sources(observed_signals),
                cleanup_signal_matrix: vec![
                    fallback_cleanup_signal_entry(
                        "allocator_reset",
                        "Allocator Reset",
                        startup_failed,
                    ),
                    fallback_cleanup_signal_entry(
                        "kv_cache_clear",
                        "KV Cache Clear",
                        startup_failed,
                    ),
                    fallback_cleanup_signal_entry("model_unload", "Model Unload", startup_failed),
                ],
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
    let observed_signal_count = observed_events.len() as u32;
    let declared_signal_ids = capabilities
        .declared_signal_ids
        .iter()
        .chain(capabilities.declared_cleanup_signal_ids.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed_signal_ids = observed_signals
        .iter()
        .filter(|signal| {
            signal.status != "failed" && signal.event != "introspection_signal_parse_failed"
        })
        .map(|signal| signal.event.clone())
        .collect::<BTreeSet<_>>();
    let declared_signal_count = declared_signal_ids.len() as u32;
    let observed_signal_unique_count = observed_signal_ids.len() as u32;
    let missing_declared_signal_count =
        declared_signal_ids.difference(&observed_signal_ids).count() as u32;
    let undeclared_observed_signal_count =
        observed_signal_ids.difference(&declared_signal_ids).count() as u32;
    let observed_signal_sources = observed_signal_sources(observed_signals);
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
    let observed_setup_or_reuse_signal =
        observed_allocator_initialized || observed_kv_initialized || observed_kv_reused;
    let observed_cleanup_signal = observed_allocator_teardown
        || observed_allocator_reset_signal
        || observed_kv_clear
        || observed_model_unload_signal;
    let declared_direct_support = declared_allocator_introspection_status.contains("available")
        || declared_kv_cache_introspection_status.contains("available")
        || capabilities
            .model_unload_signal_status
            .contains("available")
        || capabilities
            .allocator_reset_signal_status
            .contains("available");
    let cleanup_path_evidence_status = if startup_failed {
        "cleanup_path_unavailable_due_to_startup_failure".to_string()
    } else if observed_allocator_reset_signal && observed_kv_clear && observed_model_unload_signal {
        "allocator_reset_kv_clear_and_model_unload_observed".to_string()
    } else if observed_allocator_reset_signal || observed_kv_clear || observed_model_unload_signal {
        "partial_cleanup_path_signals_observed".to_string()
    } else if observed_allocator_initialized
        || observed_allocator_teardown
        || observed_kv_initialized
        || observed_kv_reused
    {
        "setup_or_reuse_signals_observed_without_cleanup_signals".to_string()
    } else {
        "cleanup_path_not_observed_directly".to_string()
    };
    let signal_contract_status = if startup_failed && observed_signal_unique_count == 0 {
        "signal_contract_interrupted_by_startup_failure".to_string()
    } else if declared_signal_count == 0 && observed_signal_unique_count == 0 {
        "no_declared_runtime_signal_contract".to_string()
    } else if declared_signal_count == 0 && undeclared_observed_signal_count > 0 {
        "observed_runtime_signals_without_declared_contract".to_string()
    } else if missing_declared_signal_count == 0
        && undeclared_observed_signal_count == 0
        && declared_signal_count > 0
    {
        "all_declared_runtime_signals_observed".to_string()
    } else if observed_signal_unique_count > 0 && missing_declared_signal_count > 0 {
        "partial_declared_runtime_signals_observed".to_string()
    } else if missing_declared_signal_count == declared_signal_count && declared_signal_count > 0 {
        "declared_runtime_signals_unobserved".to_string()
    } else if undeclared_observed_signal_count > 0 {
        "mixed_declared_and_undeclared_runtime_signal_observation".to_string()
    } else {
        "runtime_signal_contract_mixed".to_string()
    };
    let signal_contract_summary = match signal_contract_status.as_str() {
        "signal_contract_interrupted_by_startup_failure" => {
            "Runtime startup failed before NullContext could compare declared runtime-signal support with observed runtime-signal evidence."
                .to_string()
        }
        "all_declared_runtime_signals_observed" => format!(
            "This runtime declared {} unique runtime signal(s), and NullContext observed all of them in this run.",
            declared_signal_count
        ),
        "partial_declared_runtime_signals_observed" => format!(
            "This runtime declared {} unique runtime signal(s); NullContext observed {} unique runtime signal(s) and still missed {} declared signal(s) in this run.",
            declared_signal_count,
            observed_signal_unique_count,
            missing_declared_signal_count
        ),
        "declared_runtime_signals_unobserved" => format!(
            "This runtime declared {} unique runtime signal(s), but NullContext did not observe any of them in this run.",
            declared_signal_count
        ),
        "observed_runtime_signals_without_declared_contract" => format!(
            "NullContext observed {} unique runtime signal(s) even though this runtime did not declare a runtime-signal contract for them.",
            undeclared_observed_signal_count
        ),
        "mixed_declared_and_undeclared_runtime_signal_observation" => format!(
            "This run mixed declared and undeclared runtime-signal evidence: {} unique observed signal(s), {} missing declared signal(s), and {} undeclared observed signal(s).",
            observed_signal_unique_count,
            missing_declared_signal_count,
            undeclared_observed_signal_count
        ),
        "no_declared_runtime_signal_contract" => {
            "This runtime did not declare a runtime-signal contract, and NullContext did not observe direct runtime lifecycle signals in this run."
                .to_string()
        }
        _ => format!(
            "Runtime-signal contract evidence remained mixed: declared={}, observed unique={}, missing declared={}, undeclared observed={}.",
            declared_signal_count,
            observed_signal_unique_count,
            missing_declared_signal_count,
            undeclared_observed_signal_count
        ),
    };
    let instrumentation_evidence_status = if startup_failed && observed_signal_unique_count == 0 {
        "instrumentation_evidence_interrupted_by_startup_failure".to_string()
    } else if capabilities.capability_source == "sidecar_manifest"
        && signal_contract_status == "all_declared_runtime_signals_observed"
    {
        "manifest_declared_instrumentation_fully_exercised".to_string()
    } else if capabilities.capability_source == "sidecar_manifest"
        && observed_signal_unique_count > 0
    {
        "manifest_declared_instrumentation_partially_exercised".to_string()
    } else if capabilities.capability_source == "sidecar_manifest" {
        "manifest_declared_instrumentation_unobserved_in_run".to_string()
    } else if observed_signal_unique_count > 0 {
        "runtime_signals_observed_without_manifest_declared_instrumentation".to_string()
    } else if capabilities.capability_source == "stock_runtime_fallback" {
        "stock_runtime_without_instrumented_signal_support".to_string()
    } else {
        "instrumentation_evidence_mixed".to_string()
    };
    let instrumentation_evidence_summary = match instrumentation_evidence_status.as_str() {
        "instrumentation_evidence_interrupted_by_startup_failure" => {
            "Runtime startup failed before NullContext could determine whether the current run exercised a trustworthy instrumented runtime path."
                .to_string()
        }
        "manifest_declared_instrumentation_fully_exercised" => format!(
            "This run used a manifest-declared instrumented runtime path and exercised all {} declared runtime signal(s).",
            declared_signal_count
        ),
        "manifest_declared_instrumentation_partially_exercised" => format!(
            "This run used a manifest-declared instrumented runtime path, but only {} of {} declared runtime signal(s) were observed in the current run.",
            observed_signal_unique_count,
            declared_signal_count
        ),
        "manifest_declared_instrumentation_unobserved_in_run" => {
            "This runtime declared an instrumented signal contract, but the current run did not observe direct runtime signals from it."
                .to_string()
        }
        "runtime_signals_observed_without_manifest_declared_instrumentation" => {
            "NullContext observed direct runtime lifecycle signals in this run, but they were not backed by a manifest-declared instrumented runtime contract."
                .to_string()
        }
        "stock_runtime_without_instrumented_signal_support" => {
            "This run is being treated as a stock external runtime path without declared instrumented signal support."
                .to_string()
        }
        _ => {
            "Instrumentation evidence remained mixed, so manifest-declared support and observed runtime signals should both be treated cautiously."
                .to_string()
        }
    };
    let instrumentation_evidence_status_label = instrumentation_evidence_status.replace('_', " ");
    let lifecycle_signal_evidence_tier = if startup_failed && observed_signal_count == 0 {
        "startup_failed_without_direct_runtime_signals".to_string()
    } else if observed_allocator_reset_signal && observed_kv_clear && observed_model_unload_signal {
        "direct_cleanup_path_signals_observed".to_string()
    } else if signal_contract_status == "all_declared_runtime_signals_observed" {
        "declared_runtime_signal_contract_fully_exercised".to_string()
    } else if signal_contract_status == "partial_declared_runtime_signals_observed" {
        "declared_runtime_signal_contract_partially_exercised".to_string()
    } else if signal_contract_status == "observed_runtime_signals_without_declared_contract"
        || signal_contract_status == "mixed_declared_and_undeclared_runtime_signal_observation"
    {
        "observed_runtime_signals_without_declared_manifest".to_string()
    } else if observed_signal_count > 0 && capabilities.capability_source == "sidecar_manifest" {
        "declared_and_observed_runtime_signals".to_string()
    } else if observed_signal_count > 0 {
        "observed_runtime_signals_without_declared_manifest".to_string()
    } else if declared_direct_support {
        "declared_support_without_observed_session_signals".to_string()
    } else {
        "no_direct_runtime_signal_evidence".to_string()
    };
    let setup_signal_coverage_status = if startup_failed && !observed_setup_or_reuse_signal {
        "setup_signal_collection_interrupted_by_startup_failure".to_string()
    } else if observed_allocator_initialized && observed_kv_initialized {
        "allocator_and_kv_setup_signals_observed".to_string()
    } else if observed_setup_or_reuse_signal {
        "partial_setup_or_reuse_signals_observed".to_string()
    } else {
        "no_setup_or_reuse_signals_observed".to_string()
    };
    let cleanup_signal_coverage_status = if startup_failed && !observed_cleanup_signal {
        "cleanup_signal_collection_interrupted_by_startup_failure".to_string()
    } else if observed_allocator_reset_signal && observed_kv_clear && observed_model_unload_signal {
        "allocator_reset_kv_clear_and_model_unload_signals_observed".to_string()
    } else if observed_cleanup_signal {
        "partial_cleanup_signals_observed".to_string()
    } else {
        "no_cleanup_signals_observed".to_string()
    };
    let cleanup_signal_matrix = vec![
        cleanup_signal_entry(
            "allocator_reset",
            "Allocator Reset",
            signal_declared(
                &capabilities.declared_cleanup_signal_ids,
                &capabilities.declared_signal_ids,
                "allocator_reset_observed",
            ) || capabilities
                .allocator_reset_signal_status
                .contains("available"),
            observed_allocator_reset_signal,
            startup_failed,
        ),
        cleanup_signal_entry(
            "kv_cache_clear",
            "KV Cache Clear",
            signal_declared(
                &capabilities.declared_cleanup_signal_ids,
                &capabilities.declared_signal_ids,
                "kv_cache_clear_observed",
            ) || declared_kv_cache_introspection_status.contains("available"),
            observed_kv_clear,
            startup_failed,
        ),
        cleanup_signal_entry(
            "model_unload",
            "Model Unload",
            signal_declared(
                &capabilities.declared_cleanup_signal_ids,
                &capabilities.declared_signal_ids,
                "model_unload_observed",
            ) || capabilities
                .model_unload_signal_status
                .contains("available"),
            observed_model_unload_signal,
            startup_failed,
        ),
    ];
    let declared_cleanup_signal_count = cleanup_signal_matrix
        .iter()
        .filter(|entry| entry.declared_support_status == "declared_supported")
        .count() as u32;
    let observed_cleanup_signal_count = cleanup_signal_matrix
        .iter()
        .filter(|entry| entry.observation_status == "signal_observed")
        .count() as u32;
    let missing_declared_cleanup_signal_count = cleanup_signal_matrix
        .iter()
        .filter(|entry| {
            entry.declared_support_status == "declared_supported"
                && entry.observation_status != "signal_observed"
        })
        .count() as u32;
    let undeclared_observed_cleanup_signal_count = cleanup_signal_matrix
        .iter()
        .filter(|entry| {
            entry.declared_support_status != "declared_supported"
                && entry.observation_status == "signal_observed"
        })
        .count() as u32;
    let cleanup_signal_contract_status = if startup_failed {
        "cleanup_signal_contract_interrupted_by_startup_failure".to_string()
    } else if declared_cleanup_signal_count == 0 && observed_cleanup_signal_count == 0 {
        "no_declared_cleanup_signal_contract".to_string()
    } else if declared_cleanup_signal_count == 0 && undeclared_observed_cleanup_signal_count > 0 {
        "observed_cleanup_signals_without_declared_contract".to_string()
    } else if missing_declared_cleanup_signal_count == 0
        && undeclared_observed_cleanup_signal_count == 0
        && declared_cleanup_signal_count > 0
    {
        "all_declared_cleanup_signals_observed".to_string()
    } else if observed_cleanup_signal_count > 0 && missing_declared_cleanup_signal_count > 0 {
        "partial_declared_cleanup_signals_observed".to_string()
    } else if missing_declared_cleanup_signal_count == declared_cleanup_signal_count
        && declared_cleanup_signal_count > 0
    {
        "declared_cleanup_signals_unobserved".to_string()
    } else if undeclared_observed_cleanup_signal_count > 0 {
        "mixed_declared_and_undeclared_cleanup_signal_observation".to_string()
    } else {
        "cleanup_signal_contract_mixed".to_string()
    };
    let cleanup_signal_contract_summary = match cleanup_signal_contract_status.as_str() {
        "cleanup_signal_contract_interrupted_by_startup_failure" => {
            "Runtime startup failed before NullContext could compare declared cleanup-signal support with observed cleanup-signal evidence."
                .to_string()
        }
        "all_declared_cleanup_signals_observed" => format!(
            "This runtime declared {} cleanup signal(s), and NullContext observed all of them in this run.",
            declared_cleanup_signal_count
        ),
        "partial_declared_cleanup_signals_observed" => format!(
            "This runtime declared {} cleanup signal(s); NullContext observed {} of them and still missed {} declared cleanup signal(s) in this run.",
            declared_cleanup_signal_count,
            observed_cleanup_signal_count,
            missing_declared_cleanup_signal_count
        ),
        "declared_cleanup_signals_unobserved" => format!(
            "This runtime declared {} cleanup signal(s), but NullContext did not observe any of them in this run.",
            declared_cleanup_signal_count
        ),
        "observed_cleanup_signals_without_declared_contract" => format!(
            "NullContext observed {} cleanup signal(s) even though this runtime did not declare a cleanup-signal contract for them.",
            undeclared_observed_cleanup_signal_count
        ),
        "mixed_declared_and_undeclared_cleanup_signal_observation" => format!(
            "This run mixed declared and undeclared cleanup-signal evidence: {} declared signal(s) were observed, {} declared signal(s) were missed, and {} undeclared cleanup signal(s) were still observed.",
            observed_cleanup_signal_count.saturating_sub(undeclared_observed_cleanup_signal_count),
            missing_declared_cleanup_signal_count,
            undeclared_observed_cleanup_signal_count
        ),
        "no_declared_cleanup_signal_contract" => {
            "This runtime did not declare a cleanup-signal contract, and NullContext did not observe direct cleanup signals in this run."
                .to_string()
        }
        _ => format!(
            "Cleanup-signal contract evidence remained mixed: declared={}, observed={}, missing declared={}, undeclared observed={}.",
            declared_cleanup_signal_count,
            observed_cleanup_signal_count,
            missing_declared_cleanup_signal_count,
            undeclared_observed_cleanup_signal_count
        ),
    };

    LlamaRuntimeIntrospectionReport {
        capability_source: capabilities.capability_source.clone(),
        manifest_path: capabilities.manifest_path,
        runtime_build_profile: capabilities.runtime_build_profile.clone(),
        instrumentation_backend: capabilities.instrumentation_backend.clone(),
        declared_signal_ids: capabilities.declared_signal_ids.clone(),
        declared_cleanup_signal_ids: capabilities.declared_cleanup_signal_ids.clone(),
        lifecycle_signal_evidence_tier,
        signal_contract_status,
        signal_contract_summary,
        instrumentation_evidence_status,
        instrumentation_evidence_summary,
        declared_signal_count,
        observed_signal_unique_count,
        missing_declared_signal_count,
        undeclared_observed_signal_count,
        cleanup_path_evidence_status,
        setup_signal_coverage_status,
        cleanup_signal_coverage_status,
        cleanup_signal_contract_status,
        cleanup_signal_contract_summary,
        declared_cleanup_signal_count,
        observed_cleanup_signal_count,
        missing_declared_cleanup_signal_count,
        undeclared_observed_cleanup_signal_count,
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
        model_unload_observed: observed_model_unload_signal,
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
                "This runtime used '{}' capability detection on a build profiled as '{}', but startup failed before any allocator/KV signals could be observed directly. Instrumentation evidence status: {}. NullContext still lacks direct visibility into allocator reset, KV/cache teardown, or model-unload behavior on this path.",
                capabilities.capability_source,
                capabilities.runtime_build_profile,
                instrumentation_evidence_status_label
            )
        } else if capabilities.capability_source == "sidecar_manifest" {
            format!(
                "NullContext loaded runtime introspection capabilities from a sidecar manifest for build profile '{}'. Instrumentation evidence status: {}. Host-tool memory observation is still in use, and {} lifecycle signal(s) were captured from runtime output for this session.",
                capabilities.runtime_build_profile,
                instrumentation_evidence_status_label,
                observed_signal_count
            )
        } else if !observed_events.is_empty() {
            format!(
                "NullContext captured {} runtime lifecycle signal(s) from llama-server output, even though this runtime is otherwise being treated as a stock external build. Instrumentation evidence status: {}.",
                observed_signal_count,
                instrumentation_evidence_status_label
            )
        } else {
            format!(
                "This runtime is being treated as a stock external llama-server build. Instrumentation evidence status: {}. NullContext can currently observe process- and host-tool-level evidence, but it does not yet have direct allocator, KV/cache, or model-unload introspection inside llama.cpp.",
                instrumentation_evidence_status_label
            )
        },
        observed_signal_count,
        observed_signal_sources,
        cleanup_signal_matrix,
        observed_events,
        notes,
    }
}

fn fallback_cleanup_signal_entry(
    signal_id: &str,
    signal_label: &str,
    startup_failed: bool,
) -> LlamaRuntimeCleanupSignalEntryReport {
    let observation_status = if startup_failed {
        "signal_collection_interrupted_by_startup_failure"
    } else {
        "signal_not_observed"
    };
    let evidence_status = if startup_failed {
        "signal_evidence_unavailable_due_to_startup_failure"
    } else {
        "signal_support_unknown_in_fallback_path"
    };

    LlamaRuntimeCleanupSignalEntryReport {
        signal_id: signal_id.to_string(),
        signal_label: signal_label.to_string(),
        declared_support_status: "support_unknown_in_fallback_path".to_string(),
        observation_status: observation_status.to_string(),
        evidence_status: evidence_status.to_string(),
        summary: if startup_failed {
            format!(
                "{} could not be evaluated because runtime startup failed before normal signal collection.",
                signal_label
            )
        } else {
            format!(
                "{} remained on the stock fallback path, so NullContext could not verify direct support for this cleanup signal.",
                signal_label
            )
        },
    }
}

fn cleanup_signal_entry(
    signal_id: &str,
    signal_label: &str,
    declared_support: bool,
    observed: bool,
    startup_failed: bool,
) -> LlamaRuntimeCleanupSignalEntryReport {
    let declared_support_status = if declared_support {
        "declared_signal_support_available"
    } else {
        "declared_signal_support_unavailable"
    };
    let observation_status = if startup_failed {
        "signal_collection_interrupted_by_startup_failure"
    } else if observed {
        "signal_observed"
    } else {
        "signal_not_observed"
    };
    let evidence_status = if startup_failed {
        "signal_evidence_unavailable_due_to_startup_failure"
    } else if observed {
        "direct_signal_observed"
    } else if declared_support {
        "declared_support_but_signal_not_observed"
    } else {
        "no_declared_support_and_no_signal_observed"
    };
    let summary = if startup_failed {
        format!(
            "{} could not be evaluated because runtime startup failed before normal signal collection.",
            signal_label
        )
    } else if observed {
        format!(
            "{} was observed directly during this runtime lifecycle.",
            signal_label
        )
    } else if declared_support {
        format!(
            "{} was declared as supported by this runtime path, but no direct signal was observed for this session.",
            signal_label
        )
    } else {
        format!(
            "{} was neither declared as supported nor observed directly for this session.",
            signal_label
        )
    };

    LlamaRuntimeCleanupSignalEntryReport {
        signal_id: signal_id.to_string(),
        signal_label: signal_label.to_string(),
        declared_support_status: declared_support_status.to_string(),
        observation_status: observation_status.to_string(),
        evidence_status: evidence_status.to_string(),
        summary,
    }
}

fn signal_declared(
    declared_cleanup_signal_ids: &[String],
    declared_signal_ids: &[String],
    signal_id: &str,
) -> bool {
    declared_cleanup_signal_ids
        .iter()
        .any(|value| value == signal_id)
        || declared_signal_ids.iter().any(|value| value == signal_id)
}

fn observed_signal_sources(signals: &[RuntimeIntrospectionSignal]) -> Vec<String> {
    let mut sources = signals
        .iter()
        .map(|signal| signal.source_stream.clone())
        .collect::<Vec<_>>();
    sources.sort();
    sources.dedup();
    sources
}

fn map_runtime_introspection_signal(
    signal: &RuntimeIntrospectionSignal,
) -> LlamaRuntimeIntrospectionEventReport {
    LlamaRuntimeIntrospectionEventReport {
        event: signal.event.clone(),
        status: signal.status.clone(),
        source: signal.source_stream.clone(),
        lifecycle_phase: introspection_event_phase(&signal.event).to_string(),
        evidence_scope: introspection_event_scope(&signal.event).to_string(),
        cleanup_relevance: introspection_event_cleanup_relevance(&signal.event).to_string(),
        details: signal.details.clone(),
    }
}

fn introspection_event_phase(event: &str) -> &'static str {
    match event {
        "allocator_initialized" | "kv_cache_initialized" => "setup",
        "kv_cache_reused" => "reuse",
        "allocator_teardown_observed"
        | "allocator_reset_observed"
        | "kv_cache_clear_observed"
        | "model_unload_observed" => "cleanup",
        "introspection_signal_parse_failed" => "parse_failure",
        _ => "unknown",
    }
}

fn introspection_event_scope(event: &str) -> &'static str {
    match event {
        "allocator_initialized" | "allocator_teardown_observed" | "allocator_reset_observed" => {
            "allocator"
        }
        "kv_cache_initialized" | "kv_cache_reused" | "kv_cache_clear_observed" => "kv_cache",
        "model_unload_observed" => "model_lifecycle",
        "introspection_signal_parse_failed" => "parser",
        _ => "unknown",
    }
}

fn introspection_event_cleanup_relevance(event: &str) -> &'static str {
    match event {
        "allocator_reset_observed"
        | "kv_cache_clear_observed"
        | "model_unload_observed"
        | "allocator_teardown_observed" => "direct_cleanup_path_signal",
        "allocator_initialized" | "kv_cache_initialized" | "kv_cache_reused" => {
            "setup_or_reuse_signal_only"
        }
        "introspection_signal_parse_failed" => "signal_capture_failure",
        _ => "unknown_cleanup_relevance",
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
    post_shutdown_gpu_visibility_status_from_gpu_window(
        gpu_offload_requested,
        post_shutdown.gpu_entry_present_after_shutdown,
        post_shutdown.gpu_memory_bytes_after_shutdown,
        post_shutdown.gpu_check_backend.as_deref(),
    )
}

fn post_shutdown_gpu_visibility_status_from_gpu_window(
    gpu_offload_requested: bool,
    gpu_entry_present: Option<bool>,
    gpu_memory_bytes: Option<u64>,
    gpu_check_backend: Option<&str>,
) -> String {
    if !gpu_offload_requested {
        return "gpu_offload_not_requested".to_string();
    }

    match gpu_entry_present {
        Some(true) => {
            if gpu_memory_bytes.is_some() {
                "post_shutdown_gpu_pid_and_allocation_bytes_observed".to_string()
            } else {
                "post_shutdown_gpu_pid_observed_but_allocation_bytes_unavailable".to_string()
            }
        }
        Some(false) => {
            if gpu_post_shutdown_visibility_limited_from_backend(
                gpu_entry_present,
                gpu_check_backend,
            ) {
                "post_shutdown_gpu_pid_not_observed_but_visibility_limited".to_string()
            } else {
                "post_shutdown_gpu_pid_not_observed".to_string()
            }
        }
        None => "post_shutdown_gpu_visibility_unavailable".to_string(),
    }
}

fn build_vram_cleanup_strategy_report(
    gpu_offload_requested: bool,
    vram_inspection_status: &str,
    post_shutdown: &RuntimePostShutdownObservation,
) -> VramCleanupStrategyReport {
    if !gpu_offload_requested {
        return VramCleanupStrategyReport {
            strategy_id: "gpu_offload_not_requested".to_string(),
            strategy_label: "GPU Offload Not Requested".to_string(),
            strategy_kind: "not_applicable".to_string(),
            implementation_status: "not_applicable".to_string(),
            support_status: "not_applicable".to_string(),
            attempt_status: "not_applicable".to_string(),
            activation_timing: "none".to_string(),
            evidence_outcome: "not_applicable".to_string(),
            expected_effect_scope: "No VRAM cleanup strategy was relevant because llama.cpp GPU offload was not requested for this session."
                .to_string(),
            summary: "NullContext did not need a VRAM cleanup strategy because the session did not request GPU offload."
                .to_string(),
            comparison: build_not_applicable_vram_cleanup_comparison_report(),
            stages: vec![],
            notes: vec![],
        };
    }

    let baseline_snapshot =
        build_vram_cleanup_evidence_snapshot(vram_inspection_status, post_shutdown);

    if !post_shutdown.vram_cleanup_strategy_windows.is_empty() {
        let mut stage_reports = Vec::new();

        for strategy_stage in &post_shutdown.vram_cleanup_strategy_windows {
            stage_reports.push(build_vram_cleanup_stage_report(
                gpu_offload_requested,
                &baseline_snapshot,
                strategy_stage,
            ));
        }

        let selected_stage = select_best_vram_cleanup_stage_report(&stage_reports)
            .cloned()
            .expect("strategy stages should exist when reporting experimental cleanup");
        let comparison = build_experimental_vram_cleanup_comparison_report(
            baseline_snapshot.clone(),
            &selected_stage,
            stage_reports.len(),
        );
        let evidence_outcome = strategy_evidence_outcome(&comparison.evidence_improvement_status);

        return VramCleanupStrategyReport {
            strategy_id: post_shutdown
                .vram_cleanup_strategy_id
                .clone()
                .unwrap_or_else(|| "multi_stage_cleanup_experiments".to_string()),
            strategy_label: "Multi-Stage Cleanup Experiments".to_string(),
            strategy_kind: "experimental_multi_stage_cleanup".to_string(),
            implementation_status: "experimental_strategy_implemented".to_string(),
            support_status: "supported".to_string(),
            attempt_status: "strategy_attempted".to_string(),
            activation_timing: "after_baseline_post_shutdown_window_in_multiple_stages"
                .to_string(),
            evidence_outcome,
            expected_effect_scope:
                "This experimental strategy runs multiple post-shutdown stages, including cooldown rechecks, self-owned host-RAM pressure, explicit host page discard/decommit pressure, self-owned CUDA memory pressure, and helper-runtime probes, to see whether driver-visible GPU residency changes after more invasive cleanup attempts."
                    .to_string(),
            summary: format!(
                "NullContext ran experimental VRAM cleanup strategy {} with {} staged cleanup experiment(s) after the baseline window. Selected strongest stage: {}. {}",
                post_shutdown
                    .vram_cleanup_strategy_id
                    .as_deref()
                    .unwrap_or("multi_stage_cleanup_experiments"),
                stage_reports.len(),
                selected_stage.stage_label,
                comparison.summary.as_str()
            ),
            comparison,
            stages: stage_reports,
            notes: vec![
                "These are experimental cleanup stages, not proof of allocator- or driver-level VRAM sanitization."
                    .to_string(),
                "Host-RAM pressure, host page discard/decommit pressure, and CUDA pressure stages do real overwrite/discard work in memory owned by NullContext, but they still do not prove that the exact prior llama.cpp pages or VRAM allocations were reclaimed and overwritten."
                    .to_string(),
                "A stronger future strategy may need explicit context teardown, allocator churn, direct process-memory evidence, or lower-level CUDA/NVML control."
                    .to_string(),
            ],
        };
    }

    VramCleanupStrategyReport {
        strategy_id: "baseline_no_special_vram_cleanup".to_string(),
        strategy_label: "Baseline Observation Only".to_string(),
        strategy_kind: "baseline".to_string(),
        implementation_status: "strategy_model_defined".to_string(),
        support_status: "supported".to_string(),
        attempt_status: "baseline_only_no_special_strategy".to_string(),
        activation_timing: "post_shutdown_observation_only".to_string(),
        evidence_outcome: baseline_vram_cleanup_evidence_outcome(vram_inspection_status),
        expected_effect_scope:
            "This baseline records what VRAM evidence looked like after normal runtime shutdown without any extra cleanup strategy such as forced context teardown, allocator churn, or device reset."
                .to_string(),
        summary: format!(
            "NullContext recorded baseline VRAM evidence over a {} ms post-shutdown window without applying a special VRAM cleanup strategy.",
            post_shutdown.verification_window_ms
        ),
        comparison: build_baseline_only_vram_cleanup_comparison_report(
            baseline_snapshot.clone(),
        ),
        stages: vec![],
        notes: vec![
            "Process termination and post-shutdown inspection were recorded, but no experimental VRAM cleanup action was attempted yet."
                .to_string(),
            "This baseline entry exists so later strategies can be compared against the same report contract."
                .to_string(),
        ],
    }
}

fn default_vram_cleanup_strategy_report() -> VramCleanupStrategyReport {
    VramCleanupStrategyReport {
        strategy_id: "legacy_report_no_vram_cleanup_section".to_string(),
        strategy_label: "Legacy Report".to_string(),
        strategy_kind: "unknown".to_string(),
        implementation_status: "section_missing_in_legacy_report".to_string(),
        support_status: "unknown".to_string(),
        attempt_status: "unknown".to_string(),
        activation_timing: "unknown".to_string(),
        evidence_outcome: "legacy_report_unavailable".to_string(),
        expected_effect_scope:
            "This report was created before NullContext recorded structured VRAM cleanup strategy data."
                .to_string(),
        summary:
            "Structured VRAM cleanup strategy reporting was not present in this older report."
                .to_string(),
        comparison: default_vram_cleanup_comparison_report(),
        stages: vec![],
        notes: vec![
            "Open a newer session report to compare baseline or experimental VRAM cleanup outcomes."
                .to_string(),
        ],
    }
}

fn default_memory_validation_report() -> MemoryValidationReport {
    MemoryValidationReport {
        validation_status: "validation_not_derived".to_string(),
        harness_scope: "session_evidence_scorecard".to_string(),
        canary_execution_status: "controlled_canary_not_run_yet".to_string(),
        process_scan_signal_status: "process_scan_context_unavailable".to_string(),
        best_stage_id: None,
        best_stage_label: None,
        best_stage_kind: None,
        best_stage_score: 0,
        best_stage_verdict: "validation_not_derived".to_string(),
        summary:
            "NullContext had not yet derived a structured memory-validation scorecard for this report."
                .to_string(),
        controlled_canary_run: default_controlled_canary_validation_run_report(),
        stage_scorecards: vec![],
        notes: vec![
            "Older reports may not include the derived memory-validation harness section."
                .to_string(),
        ],
    }
}

fn default_memory_validation_history_report() -> MemoryValidationHistoryReport {
    MemoryValidationHistoryReport {
        history_status: "history_not_recorded".to_string(),
        scope_key: "unavailable".to_string(),
        scope_model_id: None,
        scope_platform: None,
        scope_gpu_offload_requested: None,
        runs_recorded: 0,
        marker_detection_runs: 0,
        clear_canary_runs: 0,
        inconclusive_or_failed_runs: 0,
        strong_or_moderate_runs: 0,
        best_stage_score_min: None,
        best_stage_score_max: None,
        best_stage_score_avg: None,
        last_recorded_at: None,
        stage_trends: vec![],
        controlled_canary_history: default_controlled_canary_history_report(),
        cleanup_stage_recommendation: default_memory_validation_stage_recommendation_report(),
        release_gate: default_validation_release_gate_report(),
        summary:
            "NullContext had not yet derived or persisted cross-session memory-validation history for this report."
                .to_string(),
        notes: vec![
            "Older reports may not include the cross-session memory-validation history section."
                .to_string(),
        ],
    }
}

fn default_validation_release_gate_report() -> ValidationReleaseGateReport {
    ValidationReleaseGateReport {
        gate_status: "release_gate_not_derived".to_string(),
        cleanup_stage_gate_status: "cleanup_stage_gate_not_derived".to_string(),
        controlled_canary_gate_status: "controlled_canary_gate_not_derived".to_string(),
        min_stage_runs_required: 2,
        min_clear_canary_runs_required: 2,
        max_marker_detection_runs_allowed_for_clean_claim: 0,
        max_worsened_runs_allowed_for_clean_stage: 0,
        max_inconclusive_runs_allowed_for_clean_stage: 0,
        stage_gate_passed: false,
        controlled_canary_gate_passed: false,
        summary:
            "NullContext had not yet derived explicit release-gating thresholds for this report."
                .to_string(),
        notes: vec![
            "Older reports may not include repeated-evidence release-gating guidance.".to_string(),
        ],
    }
}

fn default_controlled_canary_history_report() -> ControlledCanaryHistoryReport {
    ControlledCanaryHistoryReport {
        history_status: "controlled_canary_history_not_derived".to_string(),
        recommendation_status: "controlled_canary_not_exercised".to_string(),
        runs_with_canary_requested: 0,
        runs_with_completed_passes: 0,
        total_requested_passes: 0,
        total_completed_passes: 0,
        total_failed_passes: 0,
        clear_runs: 0,
        marker_detection_runs: 0,
        mixed_or_inconclusive_runs: 0,
        backend_unsupported_runs: 0,
        latest_execution_status: "controlled_canary_not_run_yet".to_string(),
        latest_aggregate_signal_status: "controlled_canary_not_run_yet".to_string(),
        summary:
            "NullContext had not yet derived repeated dedicated controlled canary history for this report."
                .to_string(),
        notes: vec![
            "Older reports may not include repeated controlled canary history guidance."
                .to_string(),
        ],
    }
}

fn default_memory_validation_stage_recommendation_report(
) -> MemoryValidationStageRecommendationReport {
    MemoryValidationStageRecommendationReport {
        recommendation_status: "recommendation_not_derived".to_string(),
        clean_claim_status: "clean_claim_not_derived".to_string(),
        stage_id: None,
        stage_label: None,
        stage_kind: None,
        runner_up_stage_id: None,
        runner_up_stage_label: None,
        runner_up_stage_kind: None,
        compared_stage_count: 0,
        runs_recorded: 0,
        avg_validation_score: None,
        effectiveness_score: None,
        runner_up_effectiveness_score: None,
        effectiveness_gap: None,
        avg_validation_score_gap: None,
        marker_detection_gap: None,
        improved_runs: 0,
        unchanged_runs: 0,
        worsened_runs: 0,
        inconclusive_runs: 0,
        marker_detection_runs: 0,
        summary:
            "NullContext had not yet derived a repeated-evidence cleanup-stage recommendation for this report."
                .to_string(),
        clean_claim_summary:
            "NullContext had not yet separated 'best repeated stage' from 'clean enough stage' for this report."
                .to_string(),
        notes: vec![
            "Older reports may not include cleanup-stage recommendation guidance.".to_string(),
        ],
    }
}

fn default_platform_capability_matrix_report() -> PlatformCapabilityMatrixReport {
    PlatformCapabilityMatrixReport {
        matrix_status: "matrix_not_derived".to_string(),
        scope_platform: std::env::consts::OS.to_string(),
        scope_model_id: None,
        runtime_build_profile: None,
        gpu_offload_requested: None,
        summary: "NullContext had not yet derived a platform capability matrix for this report."
            .to_string(),
        capabilities: vec![],
        notes: vec![
            "Older reports may not include the platform capability matrix section.".to_string(),
        ],
    }
}

fn default_controlled_canary_signal_status() -> String {
    "controlled_canary_not_run_yet".to_string()
}

fn default_controlled_canary_selection_reason() -> String {
    "No representative controlled canary pass was selected.".to_string()
}

fn default_vram_cleanup_selection_reason() -> String {
    "This older report did not record stage-selection metadata.".to_string()
}

fn default_vram_cleanup_marker_evidence_status() -> String {
    "marker_evidence_not_yet_contextualized".to_string()
}

fn default_vram_cleanup_marker_evidence_summary() -> String {
    "This report had not yet attached RAM-side marker-persistence context to the VRAM cleanup comparison."
        .to_string()
}

fn default_process_scan_context_status() -> String {
    "process_scan_context_unavailable".to_string()
}

fn default_process_scan_context_scope() -> String {
    "process_scan_context_unavailable".to_string()
}

fn default_cleanup_signal_support_status() -> String {
    "cleanup_signal_support_unavailable".to_string()
}

fn default_cleanup_signal_support_summary() -> String {
    "This report did not attach allocator/KV cleanup-signal support to the stage scorecard."
        .to_string()
}

fn default_controlled_canary_validation_run_report() -> ControlledCanaryValidationRunReport {
    ControlledCanaryValidationRunReport {
        execution_status: "controlled_canary_not_run_yet".to_string(),
        requested_passes: 0,
        completed_passes: 0,
        failed_passes: 0,
        aggregate_signal_status: "controlled_canary_not_run_yet".to_string(),
        aggregate_process_scan_status: "scan_not_completed".to_string(),
        canary_id: "none".to_string(),
        selected_pass_index: None,
        selected_pass_canary_id: None,
        selection_reason: default_controlled_canary_selection_reason(),
        runtime_pid: None,
        runtime_endpoint: None,
        response_bytes: None,
        summary:
            "NullContext did not run the dedicated controlled canary validation helper for this report."
                .to_string(),
        process_scan: ProcessScanReport {
            overall_status: "scan_not_completed".to_string(),
            implementation_status: "controlled_canary_not_run_yet".to_string(),
            platform: std::env::consts::OS.to_string(),
            target_process_kind: "llama-server".to_string(),
            target_runtime_pid: None,
            planned_platforms: vec![
                "windows".to_string(),
                "linux".to_string(),
                "macos".to_string(),
            ],
            summary:
                "No controlled canary helper run was executed, so no dedicated canary process scan was recorded."
                    .to_string(),
            residual_risk_summary:
                "Without a controlled canary helper run, this report cannot compare dedicated canary marker persistence against the session's cleanup evidence."
                    .to_string(),
            phases: vec![],
            notes: vec![
                "The dedicated controlled canary validation helper had not run for this report."
                    .to_string(),
            ],
        },
        passes: vec![],
        notes: vec![
            "Future Track E slices will execute a dedicated helper runtime with known canary markers."
                .to_string(),
        ],
    }
}

fn build_failed_start_vram_cleanup_strategy_report(
    gpu_offload_requested: bool,
) -> VramCleanupStrategyReport {
    if !gpu_offload_requested {
        return VramCleanupStrategyReport {
            strategy_id: "gpu_offload_not_requested".to_string(),
            strategy_label: "GPU Offload Not Requested".to_string(),
            strategy_kind: "not_applicable".to_string(),
            implementation_status: "not_applicable".to_string(),
            support_status: "not_applicable".to_string(),
            attempt_status: "not_applicable".to_string(),
            activation_timing: "none".to_string(),
            evidence_outcome: "not_applicable".to_string(),
            expected_effect_scope: "No VRAM cleanup strategy was relevant because llama.cpp GPU offload was not requested for this failed-start session."
                .to_string(),
        summary: "No VRAM cleanup strategy was needed because GPU offload was not requested."
                .to_string(),
            comparison: build_not_applicable_vram_cleanup_comparison_report(),
            stages: vec![],
            notes: vec![],
        };
    }

    VramCleanupStrategyReport {
        strategy_id: "baseline_no_special_vram_cleanup".to_string(),
        strategy_label: "Baseline Observation Only".to_string(),
        strategy_kind: "baseline".to_string(),
        implementation_status: "strategy_model_defined".to_string(),
        support_status: "supported".to_string(),
        attempt_status: "unattempted_due_to_startup_failure".to_string(),
        activation_timing: "post_shutdown_observation_only".to_string(),
        evidence_outcome: "inconclusive_due_to_startup_failure".to_string(),
        expected_effect_scope:
            "This baseline would normally describe VRAM evidence after normal shutdown without an extra cleanup strategy, but startup failed before the runtime became healthy."
                .to_string(),
        summary: "NullContext defined the VRAM cleanup strategy model, but this run never reached a normal baseline cleanup stage because startup failed before readiness."
            .to_string(),
        comparison: build_startup_failed_vram_cleanup_comparison_report(),
        stages: vec![],
        notes: vec![
            "No special VRAM cleanup strategy was attempted.".to_string(),
            "Startup failure prevented a normal baseline comparison for post-shutdown VRAM evidence."
                .to_string(),
        ],
    }
}

fn baseline_vram_cleanup_evidence_outcome(vram_inspection_status: &str) -> String {
    match vram_inspection_status {
        "gpu_entry_not_observed_after_shutdown" => {
            "baseline_evidence_recorded_not_observed".to_string()
        }
        "gpu_entry_not_observed_after_shutdown_but_visibility_limited" => {
            "baseline_evidence_visibility_limited".to_string()
        }
        "gpu_entry_observed_during_post_shutdown_window" => {
            "baseline_evidence_explicit_vram_residency_observed".to_string()
        }
        "gpu_pid_observed_during_post_shutdown_window_but_memory_bytes_unavailable" => {
            "baseline_evidence_pid_visible_bytes_unavailable".to_string()
        }
        "gpu_inspection_unavailable" => "baseline_evidence_inspection_unavailable".to_string(),
        "gpu_offload_not_requested" => "not_applicable".to_string(),
        _ => "baseline_evidence_inconclusive".to_string(),
    }
}

fn strategy_evidence_outcome(evidence_improvement_status: &str) -> String {
    match evidence_improvement_status {
        "evidence_improved_pid_no_longer_observed_after_strategy" => {
            "strategy_evidence_improved".to_string()
        }
        "evidence_improved_peak_bytes_lower_but_residency_still_observed" => {
            "strategy_evidence_improved_but_residency_still_visible".to_string()
        }
        "evidence_improved_bytes_no_longer_visible_but_pid_still_observed" => {
            "strategy_evidence_improved_but_pid_still_visible".to_string()
        }
        "evidence_unchanged_pid_still_observed" | "evidence_unchanged_not_observed" => {
            "strategy_evidence_unchanged".to_string()
        }
        "evidence_worsened_gpu_visibility_increased_after_strategy" => {
            "strategy_evidence_worsened".to_string()
        }
        _ => "strategy_evidence_inconclusive".to_string(),
    }
}

fn build_vram_cleanup_evidence_snapshot(
    vram_inspection_status: &str,
    post_shutdown: &RuntimePostShutdownObservation,
) -> VramCleanupEvidenceSnapshot {
    VramCleanupEvidenceSnapshot {
        vram_inspection_status: vram_inspection_status.to_string(),
        post_shutdown_gpu_visibility_status: post_shutdown_gpu_visibility_status(
            true,
            post_shutdown,
        ),
        gpu_entry_observed: post_shutdown.gpu_entry_present_after_shutdown,
        gpu_memory_bytes: post_shutdown.gpu_memory_bytes_after_shutdown,
        gpu_peak_memory_bytes: post_shutdown.gpu_peak_memory_bytes_after_shutdown,
        gpu_samples_collected: post_shutdown.gpu_samples_collected_after_shutdown,
        gpu_samples_with_pid_observed: post_shutdown.gpu_samples_with_pid_observed_after_shutdown,
        gpu_last_pid_observed_at_ms: post_shutdown.gpu_last_pid_observed_at_ms,
    }
}

fn build_vram_cleanup_strategy_snapshot(
    gpu_offload_requested: bool,
    strategy_window: &crate::runtime::RuntimeGpuObservationWindow,
) -> VramCleanupEvidenceSnapshot {
    VramCleanupEvidenceSnapshot {
        vram_inspection_status: vram_inspection_status_from_gpu_window(
            gpu_offload_requested,
            strategy_window.gpu_entry_present,
            strategy_window.gpu_memory_bytes,
            strategy_window.gpu_check_backend.as_deref(),
        ),
        post_shutdown_gpu_visibility_status: post_shutdown_gpu_visibility_status_from_gpu_window(
            gpu_offload_requested,
            strategy_window.gpu_entry_present,
            strategy_window.gpu_memory_bytes,
            strategy_window.gpu_check_backend.as_deref(),
        ),
        gpu_entry_observed: strategy_window.gpu_entry_present,
        gpu_memory_bytes: strategy_window.gpu_memory_bytes,
        gpu_peak_memory_bytes: strategy_window.gpu_peak_memory_bytes,
        gpu_samples_collected: strategy_window.gpu_samples_collected,
        gpu_samples_with_pid_observed: strategy_window.gpu_samples_with_pid_observed,
        gpu_last_pid_observed_at_ms: strategy_window.gpu_last_pid_observed_at_ms,
    }
}

fn build_vram_cleanup_stage_report(
    gpu_offload_requested: bool,
    baseline_snapshot: &VramCleanupEvidenceSnapshot,
    strategy_stage: &crate::runtime::RuntimeGpuObservationStrategyStage,
) -> VramCleanupStrategyStageReport {
    let evidence_snapshot =
        build_vram_cleanup_strategy_snapshot(gpu_offload_requested, &strategy_stage.window);
    let evidence_improvement_status =
        compare_vram_cleanup_snapshots(baseline_snapshot, &evidence_snapshot);

    VramCleanupStrategyStageReport {
        stage_id: strategy_stage.stage_id.clone(),
        stage_label: strategy_stage.stage_label.clone(),
        stage_kind: strategy_stage.stage_kind.clone(),
        cooldown_ms_before_stage: strategy_stage.cooldown_ms_before_stage,
        verification_window_ms: strategy_stage.window.verification_window_ms,
        action_status: strategy_stage.action_status.clone(),
        summary: format!(
            "{} ({}) waited {} ms before collecting a {} ms GPU recheck window. {}",
            strategy_stage.stage_label,
            strategy_stage.action_status,
            strategy_stage.cooldown_ms_before_stage,
            strategy_stage.window.verification_window_ms,
            vram_cleanup_comparison_summary(&evidence_improvement_status)
        ),
        process_scan_phase: strategy_stage.process_scan_phase.clone(),
        helper_process_scan_report: strategy_stage.helper_process_scan_report.clone(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        notes: {
            let mut notes = strategy_stage.action_notes.clone();
            notes.extend(vram_cleanup_comparison_notes(
                baseline_snapshot,
                &evidence_snapshot,
            ));
            notes
        },
        evidence_improvement_status,
        evidence_snapshot,
    }
}

fn build_baseline_only_vram_cleanup_comparison_report(
    baseline_snapshot: VramCleanupEvidenceSnapshot,
) -> VramCleanupComparisonReport {
    VramCleanupComparisonReport {
        comparison_status: "baseline_reference_recorded_no_strategy_delta".to_string(),
        current_run_role: "baseline_reference".to_string(),
        evidence_improvement_status: "baseline_only_no_strategy_delta".to_string(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        baseline_snapshot: baseline_snapshot.clone(),
        current_snapshot: baseline_snapshot,
        selected_stage_id: None,
        selected_stage_label: None,
        selected_stage_kind: None,
        selection_reason: "No experimental cleanup stage was selected because this run only recorded the baseline reference path."
            .to_string(),
        summary:
            "This run establishes the baseline VRAM evidence reference. No experimental cleanup strategy was applied yet, so there is no strategy delta to compare."
                .to_string(),
        notes: vec![
            "Baseline and current snapshots are identical because this report records the control path only."
                .to_string(),
            "A future experimental strategy should reuse this comparison shape and report whether evidence improved, stayed unchanged, or remained inconclusive."
                .to_string(),
        ],
    }
}

fn compare_vram_cleanup_snapshots(
    baseline: &VramCleanupEvidenceSnapshot,
    current: &VramCleanupEvidenceSnapshot,
) -> String {
    if snapshot_is_inconclusive(current) || snapshot_is_inconclusive(baseline) {
        return "evidence_inconclusive_visibility_limited_or_unavailable".to_string();
    }

    match (baseline.gpu_entry_observed, current.gpu_entry_observed) {
        (Some(true), Some(false)) => {
            return "evidence_improved_pid_no_longer_observed_after_strategy".to_string();
        }
        (Some(false), Some(false)) => return "evidence_unchanged_not_observed".to_string(),
        (Some(false), Some(true)) | (None, Some(true)) => {
            return "evidence_worsened_gpu_visibility_increased_after_strategy".to_string();
        }
        _ => {}
    }

    if baseline.gpu_entry_observed == Some(true) && current.gpu_entry_observed == Some(true) {
        match (
            baseline.gpu_peak_memory_bytes,
            current.gpu_peak_memory_bytes,
        ) {
            (Some(before), Some(after)) if after < before => {
                return "evidence_improved_peak_bytes_lower_but_residency_still_observed"
                    .to_string();
            }
            (Some(before), Some(after)) if after > before => {
                return "evidence_worsened_peak_bytes_higher_after_strategy".to_string();
            }
            _ => {}
        }

        if baseline.gpu_memory_bytes.is_some() && current.gpu_memory_bytes.is_none() {
            return "evidence_improved_bytes_no_longer_visible_but_pid_still_observed".to_string();
        }

        return "evidence_unchanged_pid_still_observed".to_string();
    }

    "evidence_inconclusive".to_string()
}

fn select_best_vram_cleanup_stage_report(
    stage_reports: &[VramCleanupStrategyStageReport],
) -> Option<&VramCleanupStrategyStageReport> {
    stage_reports
        .iter()
        .max_by_key(|stage| vram_cleanup_stage_preference_key(stage))
}

fn vram_cleanup_stage_preference_key(stage: &VramCleanupStrategyStageReport) -> (u8, u8, u32, u64) {
    (
        vram_cleanup_stage_status_rank(&stage.evidence_improvement_status),
        vram_cleanup_stage_visibility_rank(&stage.evidence_snapshot),
        u32::MAX.saturating_sub(stage.evidence_snapshot.gpu_samples_with_pid_observed),
        stage
            .evidence_snapshot
            .gpu_peak_memory_bytes
            .map(|value| u64::MAX.saturating_sub(value))
            .unwrap_or(u64::MAX),
    )
}

fn vram_cleanup_stage_status_rank(evidence_improvement_status: &str) -> u8 {
    match evidence_improvement_status {
        "evidence_improved_pid_no_longer_observed_after_strategy" => 8,
        "evidence_unchanged_not_observed" => 7,
        "evidence_improved_bytes_no_longer_visible_but_pid_still_observed" => 6,
        "evidence_improved_peak_bytes_lower_but_residency_still_observed" => 5,
        "evidence_unchanged_pid_still_observed" => 4,
        "evidence_worsened_peak_bytes_higher_after_strategy" => 3,
        "evidence_worsened_gpu_visibility_increased_after_strategy" => 2,
        "evidence_inconclusive_visibility_limited_or_unavailable" | "evidence_inconclusive" => 1,
        _ => 0,
    }
}

fn vram_cleanup_stage_visibility_rank(snapshot: &VramCleanupEvidenceSnapshot) -> u8 {
    match (snapshot.gpu_entry_observed, snapshot.gpu_memory_bytes) {
        (Some(false), _) => 3,
        (Some(true), None) => 2,
        (Some(true), Some(_)) => 1,
        (None, _) => 0,
    }
}

fn vram_cleanup_stage_selection_reason(stage: &VramCleanupStrategyStageReport) -> String {
    match stage.evidence_improvement_status.as_str() {
        "evidence_improved_pid_no_longer_observed_after_strategy" => {
            "This stage was selected because its recheck no longer observed a matching GPU PID, which is the strongest driver-visible outcome currently available."
                .to_string()
        }
        "evidence_unchanged_not_observed" => {
            "This stage was selected because its recheck also showed no matching GPU PID, preserving the clearest observed post-shutdown visibility state."
                .to_string()
        }
        "evidence_improved_bytes_no_longer_visible_but_pid_still_observed" => {
            "This stage was selected because it still observed the GPU PID but no longer surfaced per-process GPU memory bytes."
                .to_string()
        }
        "evidence_improved_peak_bytes_lower_but_residency_still_observed" => {
            "This stage was selected because GPU residency was still visible, but with lower peak byte visibility than stronger competing stages."
                .to_string()
        }
        "evidence_unchanged_pid_still_observed" => {
            "This stage was selected because no stage produced a cleaner outcome, so this was the best available observed result among still-visible GPU residency states."
                .to_string()
        }
        "evidence_worsened_peak_bytes_higher_after_strategy"
        | "evidence_worsened_gpu_visibility_increased_after_strategy" => {
            "This stage was selected only because no stage produced a better observed outcome; the recorded evidence remained worsened or noisier than the baseline."
                .to_string()
        }
        _ => {
            "This stage was selected as the best available comparison point, but the evidence remained visibility-limited or inconclusive."
                .to_string()
        }
    }
}

fn snapshot_is_inconclusive(snapshot: &VramCleanupEvidenceSnapshot) -> bool {
    snapshot
        .vram_inspection_status
        .contains("visibility_limited")
        || snapshot.vram_inspection_status.contains("unavailable")
        || snapshot.vram_inspection_status.contains("inconclusive")
        || snapshot
            .post_shutdown_gpu_visibility_status
            .contains("visibility_limited")
        || snapshot
            .post_shutdown_gpu_visibility_status
            .contains("unavailable")
}

fn vram_cleanup_comparison_summary(evidence_improvement_status: &str) -> String {
    match evidence_improvement_status {
        "evidence_improved_pid_no_longer_observed_after_strategy" => {
            "Compared with the baseline window, the strategy recheck no longer observed a matching GPU PID. This is stronger post-shutdown evidence, but not proof of full VRAM sanitization."
                .to_string()
        }
        "evidence_improved_peak_bytes_lower_but_residency_still_observed" => {
            "The strategy recheck still observed GPU residency, but with lower peak byte visibility than the baseline window."
                .to_string()
        }
        "evidence_improved_bytes_no_longer_visible_but_pid_still_observed" => {
            "The strategy recheck still observed the GPU PID, but per-process GPU memory bytes were no longer visible."
                .to_string()
        }
        "evidence_unchanged_not_observed" => {
            "Neither the baseline nor the strategy recheck observed a matching GPU PID. The strategy did not improve the already-clear driver-visible evidence."
                .to_string()
        }
        "evidence_unchanged_pid_still_observed" => {
            "The strategy recheck still observed GPU residency with no meaningful improvement over the baseline window."
                .to_string()
        }
        "evidence_worsened_gpu_visibility_increased_after_strategy" => {
            "The strategy recheck surfaced more GPU visibility than the baseline window, so this experimental strategy did not improve the observed outcome."
                .to_string()
        }
        "evidence_worsened_peak_bytes_higher_after_strategy" => {
            "The strategy recheck still observed GPU residency and reported higher peak byte visibility than the baseline window."
                .to_string()
        }
        _ => "The strategy comparison remained inconclusive because GPU visibility stayed limited or the evidence did not support a clear improvement claim."
            .to_string(),
    }
}

fn vram_cleanup_comparison_notes(
    baseline: &VramCleanupEvidenceSnapshot,
    current: &VramCleanupEvidenceSnapshot,
) -> Vec<String> {
    vec![
        format!(
            "Baseline window: {} sample(s), {} GPU-positive sample(s), peak bytes {}.",
            baseline.gpu_samples_collected,
            baseline.gpu_samples_with_pid_observed,
            baseline
                .gpu_peak_memory_bytes
                .map(|value| format!("{value}"))
                .unwrap_or_else(|| "unknown".to_string())
        ),
        format!(
            "Strategy window: {} sample(s), {} GPU-positive sample(s), peak bytes {}.",
            current.gpu_samples_collected,
            current.gpu_samples_with_pid_observed,
            current
                .gpu_peak_memory_bytes
                .map(|value| format!("{value}"))
                .unwrap_or_else(|| "unknown".to_string())
        ),
    ]
}

fn build_experimental_vram_cleanup_comparison_report(
    baseline_snapshot: VramCleanupEvidenceSnapshot,
    selected_stage: &VramCleanupStrategyStageReport,
    total_stage_count: usize,
) -> VramCleanupComparisonReport {
    let current_snapshot = selected_stage.evidence_snapshot.clone();
    let evidence_improvement_status =
        compare_vram_cleanup_snapshots(&baseline_snapshot, &current_snapshot);

    VramCleanupComparisonReport {
        comparison_status: "baseline_compared_to_selected_experimental_stage".to_string(),
        current_run_role: "selected_experimental_strategy_stage_result".to_string(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        summary: format!(
            "Selected stage {} ({}). {}",
            selected_stage.stage_label,
            selected_stage.action_status,
            vram_cleanup_comparison_summary(&evidence_improvement_status)
        ),
        notes: {
            let mut notes = vec![
                format!(
                    "Selected stage {} ({}) was chosen as the strongest observed cleanup outcome from {} recorded stage(s).",
                    selected_stage.stage_label, selected_stage.stage_id, total_stage_count
                ),
                format!(
                    "Selection reason: {}",
                    vram_cleanup_stage_selection_reason(selected_stage)
                ),
            ];
            notes.extend(vram_cleanup_comparison_notes(
                &baseline_snapshot,
                &current_snapshot,
            ));
            notes
        },
        evidence_improvement_status,
        baseline_snapshot,
        current_snapshot,
        selected_stage_id: Some(selected_stage.stage_id.clone()),
        selected_stage_label: Some(selected_stage.stage_label.clone()),
        selected_stage_kind: Some(selected_stage.stage_kind.clone()),
        selection_reason: vram_cleanup_stage_selection_reason(selected_stage),
    }
}

fn build_not_applicable_vram_cleanup_comparison_report() -> VramCleanupComparisonReport {
    let snapshot = VramCleanupEvidenceSnapshot {
        vram_inspection_status: "gpu_offload_not_requested".to_string(),
        post_shutdown_gpu_visibility_status: "gpu_offload_not_requested".to_string(),
        gpu_entry_observed: None,
        gpu_memory_bytes: None,
        gpu_peak_memory_bytes: None,
        gpu_samples_collected: 0,
        gpu_samples_with_pid_observed: 0,
        gpu_last_pid_observed_at_ms: None,
    };

    VramCleanupComparisonReport {
        comparison_status: "not_applicable".to_string(),
        current_run_role: "not_applicable".to_string(),
        evidence_improvement_status: "not_applicable".to_string(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        baseline_snapshot: snapshot.clone(),
        current_snapshot: snapshot,
        selected_stage_id: None,
        selected_stage_label: None,
        selected_stage_kind: None,
        selection_reason:
            "No cleanup stage was selected because GPU offload was not requested.".to_string(),
        summary:
            "No baseline-versus-strategy comparison was relevant because GPU offload was not requested."
                .to_string(),
        notes: vec![],
    }
}

fn build_startup_failed_vram_cleanup_comparison_report() -> VramCleanupComparisonReport {
    let snapshot = VramCleanupEvidenceSnapshot {
        vram_inspection_status: "gpu_inspection_unavailable_due_to_startup_failure".to_string(),
        post_shutdown_gpu_visibility_status:
            "post_shutdown_gpu_visibility_unavailable_due_to_startup_failure".to_string(),
        gpu_entry_observed: None,
        gpu_memory_bytes: None,
        gpu_peak_memory_bytes: None,
        gpu_samples_collected: 0,
        gpu_samples_with_pid_observed: 0,
        gpu_last_pid_observed_at_ms: None,
    };

    VramCleanupComparisonReport {
        comparison_status: "inconclusive_due_to_startup_failure".to_string(),
        current_run_role: "startup_failed_before_baseline".to_string(),
        evidence_improvement_status: "inconclusive_due_to_startup_failure".to_string(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        baseline_snapshot: snapshot.clone(),
        current_snapshot: snapshot,
        selected_stage_id: None,
        selected_stage_label: None,
        selected_stage_kind: None,
        selection_reason:
            "No cleanup stage was selected because startup failed before the experimental strategy could run."
                .to_string(),
        summary:
            "No baseline-versus-strategy comparison was available because startup failed before NullContext could complete a normal VRAM observation baseline."
                .to_string(),
        notes: vec![
            "A future successful run is required before any cleanup strategy delta can be measured."
                .to_string(),
        ],
    }
}

fn default_vram_cleanup_comparison_report() -> VramCleanupComparisonReport {
    let snapshot = VramCleanupEvidenceSnapshot {
        vram_inspection_status: "legacy_report_unavailable".to_string(),
        post_shutdown_gpu_visibility_status: "legacy_report_unavailable".to_string(),
        gpu_entry_observed: None,
        gpu_memory_bytes: None,
        gpu_peak_memory_bytes: None,
        gpu_samples_collected: 0,
        gpu_samples_with_pid_observed: 0,
        gpu_last_pid_observed_at_ms: None,
    };

    VramCleanupComparisonReport {
        comparison_status: "legacy_report_unavailable".to_string(),
        current_run_role: "legacy_report_unavailable".to_string(),
        evidence_improvement_status: "legacy_report_unavailable".to_string(),
        marker_evidence_status: default_vram_cleanup_marker_evidence_status(),
        marker_evidence_summary: default_vram_cleanup_marker_evidence_summary(),
        baseline_snapshot: snapshot.clone(),
        current_snapshot: snapshot,
        selected_stage_id: None,
        selected_stage_label: None,
        selected_stage_kind: None,
        selection_reason:
            "This older report did not record stage-selection metadata.".to_string(),
        summary:
            "This older report did not include structured baseline-versus-strategy VRAM comparison data."
                .to_string(),
        notes: vec![
            "Open a newer report to inspect comparison snapshots and evidence-improvement status."
                .to_string(),
        ],
    }
}

fn vram_inspection_status(
    gpu_offload_requested: bool,
    post_shutdown: &RuntimePostShutdownObservation,
) -> String {
    vram_inspection_status_from_gpu_window(
        gpu_offload_requested,
        post_shutdown.gpu_entry_present_after_shutdown,
        post_shutdown.gpu_memory_bytes_after_shutdown,
        post_shutdown.gpu_check_backend.as_deref(),
    )
}

fn vram_inspection_status_from_gpu_window(
    gpu_offload_requested: bool,
    gpu_entry_present: Option<bool>,
    gpu_memory_bytes: Option<u64>,
    gpu_check_backend: Option<&str>,
) -> String {
    if !gpu_offload_requested {
        return "gpu_offload_not_requested".to_string();
    }

    match gpu_entry_present {
        Some(false) => {
            if gpu_post_shutdown_visibility_limited_from_backend(
                gpu_entry_present,
                gpu_check_backend,
            ) {
                "gpu_entry_not_observed_after_shutdown_but_visibility_limited".to_string()
            } else {
                "gpu_entry_not_observed_after_shutdown".to_string()
            }
        }
        Some(true) => {
            if gpu_memory_bytes.is_some() {
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
    gpu_post_shutdown_visibility_limited_from_backend(
        post_shutdown.gpu_entry_present_after_shutdown,
        post_shutdown.gpu_check_backend.as_deref(),
    )
}

fn gpu_post_shutdown_visibility_limited_from_backend(
    gpu_entry_present: Option<bool>,
    gpu_check_backend: Option<&str>,
) -> bool {
    cfg!(target_os = "windows")
        && gpu_entry_present == Some(false)
        && gpu_check_backend
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
