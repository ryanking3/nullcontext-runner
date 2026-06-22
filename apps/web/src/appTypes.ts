export type SessionRegistry = {
  sessions: SessionIndexEntry[];
};

export type RegisteredModel = {
  id: string;
  name: string;
  description?: string | null;
  model_path: string;
  max_tokens: number;
  gpu_layers: number;
  chat_template: string;
  chat_context_token_budget: number;
  chat_context_turn_limit: number;
  selectable: boolean;
  validation_status: string;
  validation_message?: string | null;
  default_selected: boolean;
};

export type ModelRegistrySnapshot = {
  default_model_id: string;
  runtime: {
    llama_path: string;
    selectable: boolean;
    validation_status: string;
    validation_message?: string | null;
  };
  models: RegisteredModel[];
};

export type CorpusRegistrySnapshot = {
  corpora: CorpusIndexEntry[];
};

export type StartupReconciliationSnapshot = {
  scanned: number;
  changed: number;
  orphaned: number;
  abandoned_active: number;
  cleanup_consistent: number;
  unchanged: number;
  notes: string[];
};

export type StartupStatusResponse = {
  sessions: StartupReconciliationSnapshot;
  corpora: StartupReconciliationSnapshot;
};

export type CorpusLifecycleMetadata = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  state_note?: string | null;
  updated_at?: string | null;
};

export type CorpusIndexEntry = {
  corpus_id: string;
  name: string;
  created_at: string;
  persistent: boolean;
  root_path: string;
  manifest_path: string;
  report_path: string;
  source_count: number;
  chunk_count: number;
  embedding_backend?: string | null;
  embedding_model?: string | null;
  ocr_backend?: string | null;
  root_exists: boolean;
  manifest_exists: boolean;
  report_exists: boolean;
  report_available: boolean;
  report_storage: string;
  loadable_report_path?: string | null;
  lifecycle: CorpusLifecycleMetadata;
};

export type SessionLifecycleMetadata = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  state_note?: string | null;
  updated_at?: string | null;
};

export type SessionIndexEntry = {
  session_id: string;
  started_at: string;
  security_mode: string;
  prompt_source: string;
  history_stored: boolean;
  backend: string;
  model_id?: string;
  model_name?: string;
  model_path: string;
  workspace: string;
  report_path: string;
  artifacts_detected: number;
  cleanup_attempted: boolean;
  cleanup_successful: boolean;
  workspace_deleted: boolean;
  workspace_exists: boolean;
  report_exists: boolean;
  report_available: boolean;
  report_storage: string;
  loadable_report_path?: string | null;
  lifecycle: SessionLifecycleMetadata;
};

export type AuditOperation = {
  operation: string;
  status: string;
  details: string;
};

export type ArtifactRecord = {
  path: string;
  kind: string;
  size_bytes: number;
};

export type CleanupInfo = {
  attempted: boolean;
  successful: boolean;
  workspace_deleted: boolean;
  files_removed: number;
  directories_removed: number;
  artifacts_detected: ArtifactRecord[];
  sanitization_operations: AuditOperation[];
  error?: string | null;
};

export type TurnArtifact = {
  turn: number;
  prompt_path: string;
  response_path: string;
};

export type SessionProfile = {
  session_kind: string;
  runtime_lifetime: string;
  turn_count: number;
  runtime_duration_ms: number;
  history_policy: string;
  persistence_policy: string;
  prompt_source: string;
  turn_artifacts: TurnArtifact[];
  active_runtime_residual_risk: string;
  grounding_scope?: string | null;
  bound_corpus_id?: string | null;
  bound_corpus_name?: string | null;
  grounded_turn_count?: number;
};

export type LifecycleReport = {
  state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  cleanup_reason?: string | null;
  state_note?: string | null;
  updated_at?: string | null;
  policy_summary: string;
  decision_summary: string;
};

export type RetrievalReportData = {
  corpus_id: string;
  corpus_name: string;
  retrieval_mode: string;
  query: string;
  top_k: number;
  grounded_turns: number;
  retrieved_chunks: number;
  source_paths: string[];
  page_hits: string[];
  context_injected: boolean;
};

export type ProcessScanPatternReportData = {
  pattern_kind: string;
  status: string;
  matches_found?: number | null;
  notes: string;
};

export type ProcessScanPhaseReportData = {
  phase: string;
  status: string;
  method: string;
  target_pid?: number | null;
  scope_summary: string;
  bytes_scanned?: number | null;
  regions_scanned?: number | null;
  regions_skipped?: number | null;
  patterns: ProcessScanPatternReportData[];
  notes: string[];
};

export type ProcessScanReportData = {
  overall_status: string;
  implementation_status: string;
  platform: string;
  target_process_kind: string;
  target_runtime_pid?: number | null;
  planned_platforms: string[];
  summary: string;
  residual_risk_summary: string;
  phases: ProcessScanPhaseReportData[];
  notes: string[];
};

export type MemoryValidationStageScorecard = {
  stage_id: string;
  stage_label: string;
  stage_kind: string;
  action_status: string;
  vram_evidence_status: string;
  marker_evidence_status: string;
  process_scan_context_status: string;
  process_scan_context_scope: string;
  cleanup_signal_support_status: string;
  cleanup_signal_support_summary: string;
  controlled_canary_signal_status: string;
  validation_score: number;
  validation_verdict: string;
  summary: string;
  strengths: string[];
  gaps: string[];
};

export type ControlledCanaryValidationPassReportData = {
  pass_index: number;
  execution_status: string;
  canary_id: string;
  runtime_pid?: number | null;
  runtime_endpoint?: string | null;
  response_bytes?: number | null;
  summary: string;
  process_scan: ProcessScanReportData;
  notes: string[];
};

export type ControlledCanaryValidationRunReportData = {
  execution_status: string;
  requested_passes: number;
  completed_passes: number;
  failed_passes: number;
  aggregate_signal_status: string;
  aggregate_process_scan_status: string;
  canary_id: string;
  selected_pass_index?: number | null;
  selected_pass_canary_id?: string | null;
  selection_reason: string;
  runtime_pid?: number | null;
  runtime_endpoint?: string | null;
  response_bytes?: number | null;
  summary: string;
  process_scan: ProcessScanReportData;
  passes: ControlledCanaryValidationPassReportData[];
  notes: string[];
};

export type MemoryValidationReportData = {
  validation_status: string;
  harness_scope: string;
  canary_execution_status: string;
  process_scan_signal_status: string;
  best_stage_id?: string | null;
  best_stage_label?: string | null;
  best_stage_kind?: string | null;
  best_stage_score: number;
  best_stage_verdict: string;
  summary: string;
  controlled_canary_run: ControlledCanaryValidationRunReportData;
  stage_scorecards: MemoryValidationStageScorecard[];
  notes: string[];
};

export type MemoryValidationHistoryReportData = {
  history_status: string;
  scope_key: string;
  scope_model_id?: string | null;
  scope_platform?: string | null;
  scope_gpu_offload_requested?: boolean | null;
  runs_recorded: number;
  marker_detection_runs: number;
  clear_canary_runs: number;
  inconclusive_or_failed_runs: number;
  strong_or_moderate_runs: number;
  best_stage_score_min?: number | null;
  best_stage_score_max?: number | null;
  best_stage_score_avg?: number | null;
  last_recorded_at?: string | null;
  stage_trends: MemoryValidationStageTrendReportData[];
  controlled_canary_history: ControlledCanaryHistoryReportData;
  cleanup_stage_recommendation: MemoryValidationStageRecommendationReportData;
  release_gate: ValidationReleaseGateReportData;
  summary: string;
  notes: string[];
};

export type ValidationReleaseGateReportData = {
  gate_status: string;
  cleanup_stage_gate_status: string;
  controlled_canary_gate_status: string;
  min_stage_runs_required: number;
  min_clear_canary_runs_required: number;
  max_marker_detection_runs_allowed_for_clean_claim: number;
  max_worsened_runs_allowed_for_clean_stage: number;
  max_inconclusive_runs_allowed_for_clean_stage: number;
  required_stage_evidence_support_statuses: string[];
  observed_stage_evidence_support_status: string;
  stage_gate_passed: boolean;
  controlled_canary_gate_passed: boolean;
  summary: string;
  notes: string[];
};

export type ControlledCanaryHistoryReportData = {
  history_status: string;
  recommendation_status: string;
  runs_with_canary_requested: number;
  runs_with_completed_passes: number;
  total_requested_passes: number;
  total_completed_passes: number;
  total_failed_passes: number;
  clear_runs: number;
  marker_detection_runs: number;
  mixed_or_inconclusive_runs: number;
  backend_unsupported_runs: number;
  latest_execution_status: string;
  latest_aggregate_signal_status: string;
  summary: string;
  notes: string[];
};

export type MemoryValidationStageRecommendationReportData = {
  recommendation_status: string;
  clean_claim_status: string;
  evidence_support_status: string;
  evidence_support_summary: string;
  stage_id?: string | null;
  stage_label?: string | null;
  stage_kind?: string | null;
  runner_up_stage_id?: string | null;
  runner_up_stage_label?: string | null;
  runner_up_stage_kind?: string | null;
  compared_stage_count: number;
  runs_recorded: number;
  avg_validation_score?: number | null;
  effectiveness_score?: number | null;
  runner_up_effectiveness_score?: number | null;
  effectiveness_gap?: number | null;
  avg_validation_score_gap?: number | null;
  marker_detection_gap?: number | null;
  improved_runs: number;
  unchanged_runs: number;
  worsened_runs: number;
  inconclusive_runs: number;
  marker_detection_runs: number;
  summary: string;
  clean_claim_summary: string;
  notes: string[];
};

export type MemoryValidationStageTrendReportData = {
  stage_id: string;
  stage_label: string;
  stage_kind: string;
  runs_recorded: number;
  avg_validation_score: number;
  best_validation_score: number;
  improved_runs: number;
  unchanged_runs: number;
  worsened_runs: number;
  inconclusive_runs: number;
  strong_or_moderate_runs: number;
  marker_detection_runs: number;
  clear_marker_support_runs: number;
  helper_scan_runs: number;
  helper_scan_clear_runs: number;
  helper_scan_marker_detection_runs: number;
  cleanup_signal_strong_runs: number;
  cleanup_signal_partial_runs: number;
  cleanup_signal_limited_runs: number;
  stage_local_scan_runs: number;
  stage_local_scan_clear_runs: number;
  stage_local_scan_marker_detection_runs: number;
  stage_local_scan_limited_runs: number;
  session_fallback_scan_runs: number;
  latest_vram_evidence_status: string;
  latest_validation_verdict: string;
  latest_marker_evidence_status: string;
  latest_cleanup_signal_support_status: string;
  latest_process_scan_context_status: string;
  latest_process_scan_context_scope: string;
  evidence_support_status: string;
  evidence_support_summary: string;
  summary: string;
  notes: string[];
};

export type PlatformCapabilityEntryReportData = {
  capability_id: string;
  capability_label: string;
  roadmap_track: string;
  current_status: string;
  evidence_level: string;
  v1_blocker: boolean;
  claim_boundary: string;
  summary: string;
  notes: string[];
};

export type PlatformCapabilityMatrixReportData = {
  matrix_status: string;
  scope_platform: string;
  scope_model_id?: string | null;
  runtime_build_profile?: string | null;
  gpu_offload_requested?: boolean | null;
  summary: string;
  capabilities: PlatformCapabilityEntryReportData[];
  notes: string[];
};

export type LlamaMemoryDomainReport = {
  domain: string;
  exposure_scope: string;
  cleanup_status: string;
  notes: string;
};

export type LlamaResidentRegionReport = {
  region_type: string;
  virtual_bytes: number;
  resident_bytes: number;
};

export type LlamaResidentRegionDeltaReport = {
  region_type: string;
  before_resident_bytes: number;
  after_resident_bytes: number;
  resident_delta_bytes: number;
};

export type LlamaRuntimeIntrospectionReport = {
  capability_source: string;
  manifest_path?: string | null;
  runtime_build_profile: string;
  instrumentation_backend: string;
  declared_signal_ids: string[];
  declared_cleanup_signal_ids: string[];
  lifecycle_signal_evidence_tier: string;
  signal_contract_status: string;
  signal_contract_summary: string;
  instrumentation_evidence_status: string;
  instrumentation_evidence_summary: string;
  declared_signal_count: number;
  observed_signal_unique_count: number;
  missing_declared_signal_count: number;
  undeclared_observed_signal_count: number;
  cleanup_path_evidence_status: string;
  setup_signal_coverage_status: string;
  cleanup_signal_coverage_status: string;
  cleanup_signal_contract_status: string;
  cleanup_signal_contract_summary: string;
  declared_cleanup_signal_count: number;
  observed_cleanup_signal_count: number;
  missing_declared_cleanup_signal_count: number;
  undeclared_observed_cleanup_signal_count: number;
  allocator_introspection_status: string;
  allocator_initialized_observed: boolean;
  allocator_teardown_observed: boolean;
  allocator_reset_observed: boolean;
  allocator_summary: string;
  kv_cache_introspection_status: string;
  kv_cache_initialized_observed: boolean;
  kv_cache_reused_observed: boolean;
  kv_cache_clear_observed: boolean;
  kv_cache_summary: string;
  model_unload_observed: boolean;
  model_unload_signal_status: string;
  allocator_reset_signal_status: string;
  summary: string;
  observed_signal_count: number;
  observed_signal_sources: string[];
  runtime_signal_matrix: LlamaRuntimeCleanupSignalEntryReport[];
  cleanup_signal_matrix: LlamaRuntimeCleanupSignalEntryReport[];
  observed_events: LlamaRuntimeIntrospectionEventReport[];
  notes: string[];
};

export type LlamaRuntimeCleanupSignalEntryReport = {
  signal_id: string;
  signal_label: string;
  declared_support_status: string;
  observation_status: string;
  evidence_status: string;
  observed_count: number;
  observed_sources: string[];
  observed_phases: string[];
  sample_observed_status?: string | null;
  sample_observed_details?: string | null;
  summary: string;
};

export type LlamaRuntimeIntrospectionEventReport = {
  event: string;
  status: string;
  source: string;
  lifecycle_phase: string;
  evidence_scope: string;
  cleanup_relevance: string;
  details: string;
};

export type VramCleanupStrategyReport = {
  strategy_id: string;
  strategy_label: string;
  strategy_kind: string;
  implementation_status: string;
  support_status: string;
  attempt_status: string;
  activation_timing: string;
  evidence_outcome: string;
  expected_effect_scope: string;
  summary: string;
  comparison: VramCleanupComparisonReport;
  stages: VramCleanupStrategyStageReport[];
  notes: string[];
};

export type VramCleanupComparisonReport = {
  comparison_status: string;
  current_run_role: string;
  evidence_improvement_status: string;
  marker_evidence_status: string;
  marker_evidence_summary: string;
  baseline_snapshot: VramCleanupEvidenceSnapshot;
  current_snapshot: VramCleanupEvidenceSnapshot;
  selected_stage_id?: string | null;
  selected_stage_label?: string | null;
  selected_stage_kind?: string | null;
  cleanup_signal_support_status: string;
  cleanup_signal_support_summary: string;
  cleanup_signal_support_scope_status: string;
  cleanup_signal_support_scope_summary: string;
  contributing_cleanup_signals: string[];
  selection_reason: string;
  summary: string;
  notes: string[];
};

export type VramCleanupEvidenceSnapshot = {
  vram_inspection_status: string;
  post_shutdown_gpu_visibility_status: string;
  gpu_entry_observed?: boolean | null;
  gpu_memory_bytes?: number | null;
  gpu_peak_memory_bytes?: number | null;
  gpu_samples_collected: number;
  gpu_samples_with_pid_observed: number;
  gpu_last_pid_observed_at_ms?: number | null;
};

export type VramCleanupStrategyStageReport = {
  stage_id: string;
  stage_label: string;
  stage_kind: string;
  cooldown_ms_before_stage: number;
  verification_window_ms: number;
  action_status: string;
  evidence_improvement_status: string;
  process_scan_phase?: ProcessScanPhaseReportData | null;
  helper_process_scan_report?: ProcessScanReportData | null;
  selection_evidence_status: string;
  selection_evidence_summary: string;
  cleanup_signal_support_status: string;
  cleanup_signal_support_summary: string;
  cleanup_signal_support_scope_status: string;
  cleanup_signal_support_scope_summary: string;
  contributing_cleanup_signals: string[];
  marker_evidence_status: string;
  marker_evidence_summary: string;
  evidence_snapshot: VramCleanupEvidenceSnapshot;
  summary: string;
  notes: string[];
};

export type LlamaRuntimeReportData = {
  runtime_kind: string;
  runtime_pid?: number | null;
  runtime_endpoint?: string | null;
  model_id: string;
  model_name: string;
  model_path: string;
  gpu_layers_requested: number;
  gpu_offload_requested: boolean;
  shutdown_method: string;
  process_exit_code?: number | null;
  graceful_shutdown_supported: boolean;
  observed_resident_bytes?: number | null;
  observed_virtual_bytes?: number | null;
  process_memory_source?: string | null;
  physical_footprint_bytes?: number | null;
  physical_footprint_peak_bytes?: number | null;
  vmmap_summary_source?: string | null;
  resident_regions: LlamaResidentRegionReport[];
  observed_gpu_pid?: boolean | null;
  observed_gpu_memory_bytes?: number | null;
  live_gpu_visibility_status: string;
  live_gpu_evidence_class: string;
  live_gpu_limitation_status: string;
  gpu_observation_backend?: string | null;
  gpu_memory_source?: string | null;
  process_present_after_shutdown?: boolean | null;
  process_check_source?: string | null;
  process_resident_bytes_after_shutdown?: number | null;
  process_virtual_bytes_after_shutdown?: number | null;
  physical_footprint_bytes_after_shutdown?: number | null;
  physical_footprint_peak_bytes_after_shutdown?: number | null;
  vmmap_summary_source_after_shutdown?: string | null;
  resident_regions_after_shutdown: LlamaResidentRegionReport[];
  physical_footprint_delta_bytes?: number | null;
  resident_region_deltas: LlamaResidentRegionDeltaReport[];
  verification_window_ms: number;
  gpu_entry_present_after_shutdown?: boolean | null;
  gpu_memory_bytes_after_shutdown?: number | null;
  gpu_peak_memory_bytes_after_shutdown?: number | null;
  gpu_samples_collected_after_shutdown: number;
  gpu_samples_with_pid_observed_after_shutdown: number;
  gpu_last_pid_observed_at_ms?: number | null;
  post_shutdown_gpu_visibility_status: string;
  post_shutdown_gpu_evidence_class: string;
  gpu_evidence_summary: string;
  post_shutdown_gpu_limitation_status: string;
  gpu_limitation_summary: string;
  gpu_trust_boundary_status: string;
  gpu_trust_boundary_summary: string;
  gpu_backend_provenance_status: string;
  gpu_backend_provenance_summary: string;
  gpu_evidence_tier_status: string;
  gpu_evidence_tier_summary: string;
  gpu_claim_boundary_status: string;
  gpu_claim_boundary_summary: string;
  gpu_context_visibility_status: string;
  gpu_context_visibility_summary: string;
  gpu_check_backend?: string | null;
  gpu_check_source?: string | null;
  inspection_status: string;
  ram_inspection_status: string;
  vram_inspection_status: string;
  inspection_summary: string;
  observation_notes: string[];
  allocator_kv_cleanup_boundary_status: string;
  allocator_kv_cleanup_boundary_summary: string;
  cleanup_summary: string;
  residual_risk_summary: string;
  introspection: LlamaRuntimeIntrospectionReport;
  vram_cleanup: VramCleanupStrategyReport;
  memory_domains: LlamaMemoryDomainReport[];
};

export type PrivacyReportData = {
  session_id: string;
  started_at: string;
  history_stored: boolean;
  backend: string;
  security_mode: string;
  gpu_layers: string;
  process_exited_cleanly: boolean;
  cleanup: CleanupInfo;
  session_profile?: SessionProfile | null;
  lifecycle?: LifecycleReport | null;
  llama_runtime?: LlamaRuntimeReportData | null;
  process_scan?: ProcessScanReportData | null;
  memory_validation: MemoryValidationReportData;
  memory_validation_history: MemoryValidationHistoryReportData;
  platform_capability_matrix: PlatformCapabilityMatrixReportData;
  retrieval?: RetrievalReportData | null;
  residual_risk: string;
};

export type Theme = "dark" | "light";
export type RunStatus = "idle" | "running" | "success" | "failed";
export type RuntimeMode = "one-shot" | "active-chat";
export type ChatTemplateOption = "auto" | "generic" | "chatml" | "llama3-instruct";
export type InspectorView = "audit" | "runtime" | "report" | "stderr";
export type RegistryModeFilter = "all" | "secure" | "standard" | "air-gapped";
export type RegistryOutcomeFilter =
  | "all"
  | "cleanup-failed"
  | "workspace-retained"
  | "artifacts"
  | "history-stored";
export type RegistrySortOrder = "newest" | "oldest";

export type StreamPayload = {
  type: string;
  message?: string;
  text?: string;
  operation?: string;
  status?: string;
  details?: string;
  success?: boolean;
};

export type ChatStartResponse = {
  session_id: string;
  workspace: string;
  runtime_endpoint: string;
  security_mode: string;
  persistent: boolean;
  model_id: string;
  model_name: string;
  corpus_id?: string | null;
  corpus_name?: string | null;
  runtime_active: boolean;
  turns: number;
  grounded_turns: number;
  chat_template: string;
  chat_context_token_budget: number;
  chat_context_turn_limit: number;
  history_policy: string;
};

export type ChatStatusResponse = {
  session_id: string;
  workspace: string;
  runtime_endpoint: string;
  security_mode: string;
  persistent: boolean;
  model_id?: string;
  model_name?: string;
  corpus_id?: string | null;
  corpus_name?: string | null;
  runtime_active: boolean;
  turns: number;
  grounded_turns?: number;
  runtime_duration_ms?: number;
  chat_template?: string;
  chat_context_token_budget?: number;
  chat_context_turn_limit?: number;
  history_policy?: string;
  residual_risk?: string;
};

export type ChatEndResponse = {
  session_id: string;
  runtime_stopped: boolean;
  report: unknown;
};

export type ChatCancelResponse = {
  session_id: string;
  generation_active: boolean;
  cancel_requested: boolean;
  message: string;
};

export type SessionLifecycleActionResponse = {
  session_id: string;
  lifecycle_state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_reason?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  state_note?: string | null;
  updated_at?: string | null;
  cleanup_attempted: boolean;
  cleanup_successful: boolean;
  workspace_deleted: boolean;
  workspace_exists: boolean;
  report_exists: boolean;
  report_available: boolean;
  report_storage: string;
  loadable_report_path?: string | null;
  workspace: string;
  report_path: string;
  message: string;
};

export type ApiErrorResponse = {
  error?: string;
};

export type CorpusIngestionReport = {
  corpus_id: string;
  created_at: string;
  persistent: boolean;
  source_paths_requested: string[];
  files_discovered: number;
  files_ingested: number;
  files_failed: number;
  pdf_pages_seen: number;
  pdf_pages_ocrd: number;
  chunk_count: number;
  ocr_enabled: boolean;
  warnings: string[];
  lifecycle?: {
    state: string;
    retention_policy: string;
    retention_deadline?: string | null;
    cleanup_requested_at?: string | null;
    cleanup_completed_at?: string | null;
    cleanup_reason?: string | null;
    state_note?: string | null;
    updated_at?: string | null;
    policy_summary: string;
    decision_summary: string;
  } | null;
  upload_staging?: {
    staging_root: string;
    staged_files: number;
    staged_bytes: number;
    source_filenames: string[];
    cleaned_up: boolean;
    cleanup_error?: string | null;
  } | null;
  residual_risk: string;
};

export type IngestCorpusResponse = {
  corpus: CorpusIndexEntry;
  report: CorpusIngestionReport;
};

export type CorpusLifecycleActionResponse = {
  corpus_id: string;
  lifecycle_state: string;
  retention_policy: string;
  retention_deadline?: string | null;
  cleanup_reason?: string | null;
  cleanup_requested_at?: string | null;
  cleanup_completed_at?: string | null;
  state_note?: string | null;
  updated_at?: string | null;
  root_exists: boolean;
  manifest_exists?: boolean;
  report_exists: boolean;
  report_available: boolean;
  report_storage: string;
  loadable_report_path?: string | null;
  root_path: string;
  manifest_path: string;
  report_path: string;
  message: string;
};

export type ChatMessage = {
  role: "user" | "assistant";
  content: string;
};
