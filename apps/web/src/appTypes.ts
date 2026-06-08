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
  model_unload_signal_status: string;
  allocator_reset_signal_status: string;
  summary: string;
  observed_events: LlamaRuntimeIntrospectionEventReport[];
  notes: string[];
};

export type LlamaRuntimeIntrospectionEventReport = {
  event: string;
  status: string;
  source: string;
  details: string;
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
  post_shutdown_gpu_visibility_status: string;
  gpu_check_backend?: string | null;
  gpu_check_source?: string | null;
  inspection_status: string;
  ram_inspection_status: string;
  vram_inspection_status: string;
  inspection_summary: string;
  observation_notes: string[];
  cleanup_summary: string;
  residual_risk_summary: string;
  introspection: LlamaRuntimeIntrospectionReport;
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
