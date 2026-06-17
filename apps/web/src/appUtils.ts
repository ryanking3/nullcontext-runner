import type {
  ApiErrorResponse,
  CorpusIngestionReport,
  PrivacyReportData,
  StreamPayload,
  VramCleanupStrategyReport,
} from "./appTypes";

export function statusClass(status: string): string {
  if (status === "successful") return "pill success";
  if (status === "failed") return "pill failed";
  if (status === "warning") return "pill warning";
  if (status === "not_attempted") return "pill muted";
  return "pill neutral";
}

export function inspectionStatusClass(status: string): string {
  if (
    status.includes("marker_persistence_detected") ||
    status.includes("without_supporting_gpu_improvement")
  ) {
    return "pill failed";
  }

  if (
    status.includes("supported_by_clear") ||
    status.includes("direct_marker_miss_observed")
  ) {
    return "pill success";
  }

  if (
    status.includes("partial_marker_clearance") ||
    status.includes("without_clear_marker_confirmation") ||
    status.includes("marker_evidence_context_mixed") ||
    status.includes("not_yet_contextualized")
  ) {
    return "pill warning";
  }

  if (
    status.includes("unsupported") ||
    status.includes("unavailable") ||
    status.includes("partial") ||
    status.includes("not_exercised")
  ) {
    return "pill warning";
  }

  if (
    status.includes("supported") ||
    status.includes("available") ||
    status.includes("active")
  ) {
    return "pill success";
  }

  if (
    status.includes("strong_improvement_signal") ||
    status.includes("moderate_improvement_signal")
  ) {
    return "pill success";
  }

  if (status.includes("mixed_signal") || status.includes("limited_signal")) {
    return "pill warning";
  }

  if (status.includes("negative_or_inconclusive_signal")) {
    return "pill failed";
  }

  if (status.includes("canary_not_run")) {
    return "pill warning";
  }

  if (status === "markers_detected_in_scanned_memory") {
    return "pill failed";
  }

  if (status.includes("markers_detected_across_passes")) {
    return "pill failed";
  }

  if (status.includes("all_completed_passes_clear")) {
    return "pill success";
  }

  if (status.includes("mixed_clear_and_inconclusive")) {
    return "pill warning";
  }

  if (status === "no_markers_detected_in_scanned_regions") {
    return "pill success";
  }

  if (status.includes("startup_failed")) {
    return "pill failed";
  }

  if (
    status.includes("visibility_limited") ||
    status.includes("memory_bytes_unavailable")
  ) {
    return "pill warning";
  }

  if (
    status.includes("not_observed_after_shutdown") ||
    status === "gpu_offload_not_requested"
  ) {
    return "pill success";
  }

  if (status.includes("still_observable_after_shutdown")) {
    return "pill failed";
  }

  if (
    status.includes("inconclusive") ||
    status.includes("unavailable") ||
    status.includes("unsupported") ||
    status.includes("skipped") ||
    status.includes("not_observable") ||
    status.includes("not_completed") ||
    status.includes("not_implemented")
  ) {
    return "pill warning";
  }

  if (status.includes("failed")) {
    return "pill failed";
  }

  return "pill neutral";
}

export function shortId(id: string): string {
  return id.slice(0, 8);
}

export function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  return `${minutes}m ${seconds.toString().padStart(2, "0")}s`;
}

export function formatTimestamp(value: string): string {
  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return parsed.toLocaleString();
}

export function formatBoolean(value: boolean): string {
  return value ? "yes" : "no";
}

export function formatBytes(value: number): string {
  if (!Number.isFinite(value)) {
    return String(value);
  }

  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatSignedBytesDelta(value: number): string {
  const prefix = value > 0 ? "+" : "";
  return `${prefix}${formatBytes(Math.abs(value))}${value === 0 ? "" : value > 0 ? " increase" : " decrease"}`;
}

export function minutesUntil(timestamp: string | null | undefined): string {
  if (!timestamp) {
    return "60";
  }

  const parsed = new Date(timestamp);

  if (Number.isNaN(parsed.getTime())) {
    return "60";
  }

  const minutes = Math.max(1, Math.ceil((parsed.getTime() - Date.now()) / 60000));
  return String(minutes);
}

export function humanizeSnakeCase(value: string): string {
  return value.replaceAll("_", " ");
}

export function lifecycleStateClass(state: string): string {
  if (state === "cleanup_succeeded") return "pill success";
  if (state === "cleanup_failed") return "pill failed";
  if (state === "cleanup_pending") return "pill warning";
  if (state === "abandoned_active") return "pill warning";
  if (state === "orphaned") return "pill warning";
  if (state === "active") return "pill warning";
  return "pill neutral";
}

export function modelValidationClass(status: string): string {
  if (status === "ready") return "pill success";
  if (status === "missing" || status === "not_file") return "pill failed";
  return "pill warning";
}

export function parsePositiveInteger(value: string): number | null {
  const parsed = Number(value);

  if (!Number.isInteger(parsed) || parsed <= 0) {
    return null;
  }

  return parsed;
}

export async function readApiError(response: Response, fallback: string): Promise<string> {
  try {
    const text = await response.text();

    if (!text.trim()) {
      return fallback;
    }

    try {
      const data = JSON.parse(text) as ApiErrorResponse;

      if (typeof data.error === "string" && data.error.trim()) {
        return data.error;
      }
    } catch {
      return text;
    }

    return text;
  } catch {
    return fallback;
  }
}

export function formatActiveChatApiError(
  action: "start" | "message" | "cancel" | "end",
  message: string
): string {
  if (
    action === "start" &&
    message.includes("Active chat startup failed before a live session could be created")
  ) {
    return `${message} NullContext did not leave an active chat session running. Review the startup diagnostics and retry after correcting the runtime issue.`;
  }

  if (message.includes("generation is still in progress")) {
    return "The active chat runtime is still finishing the current generation. Wait for streaming to settle, or use Stop and then retry End + Sanitize once the session is idle.";
  }

  if (message.includes("already generating")) {
    return "An active chat generation is already in progress for this session. Wait for it to finish before sending another message.";
  }

  if (message.includes("session is ending")) {
    return "This active chat session is already ending. Wait for End + Sanitize to complete before sending another message.";
  }

  if (message.includes("No active chat generation is currently running")) {
    return "There is no active chat generation running right now, so there was nothing to cancel.";
  }

  if (message.includes("Active chat session not found")) {
    if (action === "end") {
      return "This active chat session is no longer available. Refresh the UI state or start a new session.";
    }

    return "This active chat session is no longer available. Start a new session and try again.";
  }

  return message;
}

export function parseSseBlock(block: string): StreamPayload | null {
  const dataLines = block
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.replace(/^data:\s?/, ""));

  if (dataLines.length === 0) {
    return null;
  }

  try {
    return JSON.parse(dataLines.join("\n")) as StreamPayload;
  } catch {
    return {
      type: "error",
      message: `Failed to parse stream event: ${dataLines.join("\n")}`,
    };
  }
}

export function parsePrivacyReport(raw: string): PrivacyReportData | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as PrivacyReportData;

    if (parsed.llama_runtime && !parsed.llama_runtime.vram_cleanup) {
      parsed.llama_runtime.vram_cleanup = legacyVramCleanupStrategyReport();
    }

    if (parsed.llama_runtime && !parsed.llama_runtime.introspection) {
      parsed.llama_runtime.introspection = legacyLlamaRuntimeIntrospectionReport();
    }

    if (parsed.llama_runtime) {
      parsed.llama_runtime.live_gpu_evidence_class ??=
        "gpu_evidence_class_unavailable_in_legacy_report";
      parsed.llama_runtime.live_gpu_limitation_status ??=
        "gpu_limitation_status_unavailable_in_legacy_report";
      parsed.llama_runtime.post_shutdown_gpu_evidence_class ??=
        "post_shutdown_gpu_evidence_class_unavailable_in_legacy_report";
      parsed.llama_runtime.gpu_evidence_summary ??=
        "This older report did not classify the exact GPU evidence class behind the recorded NVIDIA visibility results.";
      parsed.llama_runtime.post_shutdown_gpu_limitation_status ??=
        "post_shutdown_gpu_limitation_status_unavailable_in_legacy_report";
      parsed.llama_runtime.gpu_limitation_summary ??=
        "This older report did not classify backend-specific GPU visibility limitations.";
      parsed.llama_runtime.gpu_trust_boundary_status ??=
        "gpu_trust_boundary_unavailable_in_legacy_report";
      parsed.llama_runtime.gpu_trust_boundary_summary ??=
        "This older report did not classify how far the recorded GPU evidence reached beyond host-tool visibility.";
    }

    if (parsed.llama_runtime?.introspection) {
      parsed.llama_runtime.introspection = {
        ...legacyLlamaRuntimeIntrospectionReport(),
        ...parsed.llama_runtime.introspection,
      };
      parsed.llama_runtime.introspection.cleanup_signal_matrix =
        parsed.llama_runtime.introspection.cleanup_signal_matrix.map((entry) => ({
          ...legacyLlamaRuntimeCleanupSignalEntryReport(),
          ...entry,
        }));
    }

    if (parsed.llama_runtime && !parsed.llama_runtime.vram_cleanup.comparison) {
      parsed.llama_runtime.vram_cleanup.comparison = legacyVramCleanupComparisonReport();
    }

    if (parsed.llama_runtime && !parsed.llama_runtime.vram_cleanup.stages) {
      parsed.llama_runtime.vram_cleanup.stages = [];
    }

    if (!parsed.memory_validation) {
      parsed.memory_validation = legacyMemoryValidationReport();
    }

    if (!parsed.memory_validation_history) {
      parsed.memory_validation_history = legacyMemoryValidationHistoryReport();
    }
    if (parsed.memory_validation_history.stage_trends === undefined) {
      parsed.memory_validation_history.stage_trends =
        legacyMemoryValidationHistoryReport().stage_trends;
    }
    if (parsed.memory_validation_history.controlled_canary_history === undefined) {
      parsed.memory_validation_history.controlled_canary_history =
        legacyMemoryValidationHistoryReport().controlled_canary_history;
    }
    if (parsed.memory_validation_history.cleanup_stage_recommendation === undefined) {
      parsed.memory_validation_history.cleanup_stage_recommendation =
        legacyMemoryValidationHistoryReport().cleanup_stage_recommendation;
    }
    if (parsed.memory_validation_history.release_gate === undefined) {
      parsed.memory_validation_history.release_gate =
        legacyMemoryValidationHistoryReport().release_gate;
    }
    parsed.memory_validation_history.stage_trends =
      parsed.memory_validation_history.stage_trends.map((trend) => ({
        ...legacyMemoryValidationStageTrendReport(),
        ...trend,
      }));
    parsed.memory_validation_history.cleanup_stage_recommendation = {
      ...legacyMemoryValidationStageRecommendationReport(),
      ...parsed.memory_validation_history.cleanup_stage_recommendation,
    };
    parsed.memory_validation_history.controlled_canary_history = {
      ...legacyControlledCanaryHistoryReport(),
      ...parsed.memory_validation_history.controlled_canary_history,
    };
    parsed.memory_validation_history.release_gate = {
      ...legacyValidationReleaseGateReport(),
      ...parsed.memory_validation_history.release_gate,
    };

    if (!parsed.platform_capability_matrix) {
      parsed.platform_capability_matrix = legacyPlatformCapabilityMatrixReport();
    }

    if (parsed.memory_validation && !parsed.memory_validation.controlled_canary_run) {
      parsed.memory_validation.controlled_canary_run =
        legacyMemoryValidationReport().controlled_canary_run;
    }

    if (
      parsed.memory_validation?.controlled_canary_run &&
      parsed.memory_validation.controlled_canary_run.requested_passes === undefined
    ) {
      const legacy = legacyMemoryValidationReport().controlled_canary_run;
      parsed.memory_validation.controlled_canary_run.requested_passes = legacy.requested_passes;
      parsed.memory_validation.controlled_canary_run.completed_passes = legacy.completed_passes;
      parsed.memory_validation.controlled_canary_run.failed_passes = legacy.failed_passes;
      parsed.memory_validation.controlled_canary_run.aggregate_signal_status =
        legacy.aggregate_signal_status;
      parsed.memory_validation.controlled_canary_run.aggregate_process_scan_status =
        legacy.aggregate_process_scan_status;
      parsed.memory_validation.controlled_canary_run.selected_pass_index ??=
        legacy.selected_pass_index;
      parsed.memory_validation.controlled_canary_run.selected_pass_canary_id ??=
        legacy.selected_pass_canary_id;
      parsed.memory_validation.controlled_canary_run.selection_reason ??=
        legacy.selection_reason;
      parsed.memory_validation.controlled_canary_run.passes ??= legacy.passes;
    }

    if (parsed.memory_validation?.stage_scorecards) {
      for (const scorecard of parsed.memory_validation.stage_scorecards) {
        if (scorecard.controlled_canary_signal_status === undefined) {
          scorecard.controlled_canary_signal_status = "controlled_canary_not_run_yet";
        }
        if (scorecard.marker_evidence_status === undefined) {
          scorecard.marker_evidence_status = "marker_evidence_not_yet_contextualized";
        }
        if (scorecard.process_scan_context_scope === undefined) {
          scorecard.process_scan_context_scope = "process_scan_context_unavailable";
        }
        if (scorecard.cleanup_signal_support_status === undefined) {
          scorecard.cleanup_signal_support_status = "cleanup_signal_support_unavailable";
        }
        if (scorecard.cleanup_signal_support_summary === undefined) {
          scorecard.cleanup_signal_support_summary =
            "This older report did not attach allocator/KV cleanup-signal support to the stage scorecard.";
        }
      }
    }

    if (
      parsed.llama_runtime &&
      parsed.llama_runtime.vram_cleanup?.comparison &&
      parsed.llama_runtime.vram_cleanup.comparison.selection_reason === undefined
    ) {
      parsed.llama_runtime.vram_cleanup.comparison.selection_reason =
        legacyVramCleanupComparisonReport().selection_reason;
      parsed.llama_runtime.vram_cleanup.comparison.selected_stage_id ??= null;
      parsed.llama_runtime.vram_cleanup.comparison.selected_stage_label ??= null;
      parsed.llama_runtime.vram_cleanup.comparison.selected_stage_kind ??= null;
    }

    if (parsed.llama_runtime?.vram_cleanup?.comparison) {
      const legacy = legacyVramCleanupComparisonReport();
      parsed.llama_runtime.vram_cleanup.comparison.marker_evidence_status ??=
        legacy.marker_evidence_status;
      parsed.llama_runtime.vram_cleanup.comparison.marker_evidence_summary ??=
        legacy.marker_evidence_summary;
    }

    if (parsed.llama_runtime?.vram_cleanup?.stages) {
      for (const stage of parsed.llama_runtime.vram_cleanup.stages) {
        stage.process_scan_phase ??= null;
        stage.helper_process_scan_report ??= null;
        stage.marker_evidence_status ??= "marker_evidence_not_yet_contextualized";
        stage.marker_evidence_summary ??=
          "This older report did not attach RAM-side marker context to this cleanup stage.";
      }
    }

    return parsed;
  } catch {
    return null;
  }
}

function legacyVramCleanupStrategyReport(): VramCleanupStrategyReport {
  return {
    strategy_id: "legacy_report_no_vram_cleanup_section",
    strategy_label: "Legacy Report",
    strategy_kind: "unknown",
    implementation_status: "section_missing_in_legacy_report",
    support_status: "unknown",
    attempt_status: "unknown",
    activation_timing: "unknown",
    evidence_outcome: "legacy_report_unavailable",
    expected_effect_scope:
      "This report was created before NullContext recorded structured VRAM cleanup strategy data.",
    summary:
      "Structured VRAM cleanup strategy reporting was not present in this older report.",
    comparison: legacyVramCleanupComparisonReport(),
    stages: [],
    notes: [
      "Open a newer session report to compare baseline or experimental VRAM cleanup outcomes.",
    ],
  };
}

function legacyLlamaRuntimeIntrospectionReport() {
  return {
    capability_source: "stock_runtime_fallback",
    manifest_path: null,
    runtime_build_profile: "stock_external_llama_server",
    instrumentation_backend: "none",
    declared_signal_ids: [],
    declared_cleanup_signal_ids: [],
    lifecycle_signal_evidence_tier: "no_direct_runtime_signal_evidence",
    signal_contract_status: "signal_contract_unavailable",
    signal_contract_summary:
      "This older report did not compare declared runtime-signal support with observed runtime-signal evidence.",
    instrumentation_evidence_status: "instrumentation_evidence_unavailable",
    instrumentation_evidence_summary:
      "This older report did not classify whether runtime-signal evidence came from a trustworthy instrumented path or only from undeclared observations.",
    declared_signal_count: 0,
    observed_signal_unique_count: 0,
    missing_declared_signal_count: 0,
    undeclared_observed_signal_count: 0,
    cleanup_path_evidence_status: "cleanup_path_not_observed_directly",
    setup_signal_coverage_status: "no_setup_or_reuse_signals_observed",
    cleanup_signal_coverage_status: "no_cleanup_signals_observed",
    cleanup_signal_contract_status: "cleanup_signal_contract_unavailable",
    cleanup_signal_contract_summary:
      "This older report did not compare declared cleanup-signal support with observed cleanup-signal evidence.",
    declared_cleanup_signal_count: 0,
    observed_cleanup_signal_count: 0,
    missing_declared_cleanup_signal_count: 0,
    undeclared_observed_cleanup_signal_count: 0,
    allocator_introspection_status: "allocator_introspection_unavailable",
    allocator_initialized_observed: false,
    allocator_teardown_observed: false,
    allocator_reset_observed: false,
    allocator_summary:
      "This older report did not include direct allocator lifecycle introspection.",
    kv_cache_introspection_status: "kv_cache_introspection_unavailable",
    kv_cache_initialized_observed: false,
    kv_cache_reused_observed: false,
    kv_cache_clear_observed: false,
    kv_cache_summary:
      "This older report did not include direct KV/cache lifecycle introspection.",
    model_unload_observed: false,
    model_unload_signal_status: "model_unload_not_observed_directly",
    allocator_reset_signal_status: "allocator_reset_not_observed_directly",
    summary:
      "This older report did not include the newer runtime introspection evidence-tier summary.",
    observed_signal_count: 0,
    observed_signal_sources: [],
    cleanup_signal_matrix: [],
    observed_events: [],
    notes: [],
  };
}

function legacyLlamaRuntimeCleanupSignalEntryReport() {
  return {
    signal_id: "legacy_cleanup_signal",
    signal_label: "Legacy Cleanup Signal",
    declared_support_status: "support_unknown_in_legacy_report",
    observation_status: "signal_not_observed",
    evidence_status: "legacy_cleanup_signal_evidence_unavailable",
    summary:
      "This older report did not include structured cleanup-signal coverage entries.",
  };
}

function legacyVramCleanupComparisonReport() {
  return {
    comparison_status: "legacy_report_unavailable",
    current_run_role: "legacy_report_unavailable",
    evidence_improvement_status: "legacy_report_unavailable",
    marker_evidence_status: "marker_evidence_not_yet_contextualized",
    marker_evidence_summary:
      "This older report did not attach RAM-side marker context to the VRAM cleanup comparison.",
    baseline_snapshot: {
      vram_inspection_status: "legacy_report_unavailable",
      post_shutdown_gpu_visibility_status: "legacy_report_unavailable",
      gpu_entry_observed: null,
      gpu_memory_bytes: null,
      gpu_peak_memory_bytes: null,
      gpu_samples_collected: 0,
      gpu_samples_with_pid_observed: 0,
      gpu_last_pid_observed_at_ms: null,
    },
    current_snapshot: {
      vram_inspection_status: "legacy_report_unavailable",
      post_shutdown_gpu_visibility_status: "legacy_report_unavailable",
      gpu_entry_observed: null,
      gpu_memory_bytes: null,
      gpu_peak_memory_bytes: null,
      gpu_samples_collected: 0,
      gpu_samples_with_pid_observed: 0,
      gpu_last_pid_observed_at_ms: null,
    },
    selected_stage_id: null,
    selected_stage_label: null,
    selected_stage_kind: null,
    selection_reason: "This older report did not record stage-selection metadata.",
    summary:
      "This older report did not include structured baseline-versus-strategy VRAM comparison data.",
    notes: [
      "Open a newer report to inspect comparison snapshots and evidence-improvement status.",
    ],
  };
}

function legacyMemoryValidationReport() {
  return {
    validation_status: "validation_not_derived",
    harness_scope: "session_evidence_scorecard",
    canary_execution_status: "controlled_canary_not_run_yet",
    process_scan_signal_status: "process_scan_context_unavailable",
    best_stage_id: null,
    best_stage_label: null,
    best_stage_kind: null,
    best_stage_score: 0,
    best_stage_verdict: "validation_not_derived",
    summary:
      "This older report did not include the derived memory-validation harness section.",
    controlled_canary_run: {
      execution_status: "controlled_canary_not_run_yet",
      requested_passes: 0,
      completed_passes: 0,
      failed_passes: 0,
      aggregate_signal_status: "controlled_canary_not_run_yet",
      aggregate_process_scan_status: "scan_not_completed",
      canary_id: "none",
      selected_pass_index: null,
      selected_pass_canary_id: null,
      selection_reason: "No representative controlled canary pass was selected.",
      runtime_pid: null,
      runtime_endpoint: null,
      response_bytes: null,
      summary:
        "This older report did not include a dedicated controlled canary helper run.",
      process_scan: {
        overall_status: "scan_not_completed",
        implementation_status: "controlled_canary_not_run_yet",
        platform: "unknown",
        target_process_kind: "llama-server",
        target_runtime_pid: null,
        planned_platforms: ["windows", "linux", "macos"],
        summary:
          "No controlled canary helper run was recorded in this older report.",
        residual_risk_summary:
          "Without a dedicated canary helper run, this report cannot compare known canary marker persistence against the session evidence.",
        phases: [],
        notes: [
          "Open a newer report to inspect dedicated controlled canary validation results.",
        ],
      },
      passes: [],
      notes: [
        "This older report predates the dedicated controlled canary helper validation slice.",
      ],
    },
    stage_scorecards: [],
    notes: [
      "Open a newer report to inspect stage scorecards and memory-validation evidence summaries.",
    ],
  };
}

function legacyMemoryValidationHistoryReport() {
  return {
    history_status: "history_not_recorded_in_legacy_report",
    scope_key: "legacy_report_scope_unknown",
    scope_model_id: null,
    scope_platform: null,
    scope_gpu_offload_requested: null,
    runs_recorded: 0,
    marker_detection_runs: 0,
    clear_canary_runs: 0,
    inconclusive_or_failed_runs: 0,
    strong_or_moderate_runs: 0,
    best_stage_score_min: null,
    best_stage_score_max: null,
    best_stage_score_avg: null,
    last_recorded_at: null,
    stage_trends: [],
    controlled_canary_history: legacyControlledCanaryHistoryReport(),
    cleanup_stage_recommendation: legacyMemoryValidationStageRecommendationReport(),
    release_gate: legacyValidationReleaseGateReport(),
    summary:
      "This older report did not include cross-session memory-validation history.",
    notes: [
      "Open a newer report to inspect locally persisted validation history for the current model/platform scope.",
    ],
  };
}

function legacyMemoryValidationStageTrendReport() {
  return {
    stage_id: "legacy_stage",
    stage_label: "Legacy Stage",
    stage_kind: "legacy",
    runs_recorded: 0,
    avg_validation_score: 0,
    best_validation_score: 0,
    improved_runs: 0,
    unchanged_runs: 0,
    worsened_runs: 0,
    inconclusive_runs: 0,
    strong_or_moderate_runs: 0,
    marker_detection_runs: 0,
    clear_marker_support_runs: 0,
    helper_scan_runs: 0,
    helper_scan_clear_runs: 0,
    helper_scan_marker_detection_runs: 0,
    cleanup_signal_strong_runs: 0,
    cleanup_signal_partial_runs: 0,
    cleanup_signal_limited_runs: 0,
    stage_local_scan_runs: 0,
    stage_local_scan_clear_runs: 0,
    stage_local_scan_marker_detection_runs: 0,
    stage_local_scan_limited_runs: 0,
    session_fallback_scan_runs: 0,
    latest_vram_evidence_status: "legacy_status_unknown",
    latest_validation_verdict: "legacy_status_unknown",
    latest_marker_evidence_status: "legacy_status_unknown",
    latest_cleanup_signal_support_status: "cleanup_signal_support_unavailable",
    latest_process_scan_context_status: "process_scan_context_unavailable",
    latest_process_scan_context_scope: "process_scan_context_unavailable",
    summary: "This older report did not include cleanup-stage trend details.",
    notes: [],
  };
}

function legacyMemoryValidationStageRecommendationReport() {
  return {
    recommendation_status: "recommendation_not_derived",
    clean_claim_status: "clean_claim_not_derived",
    stage_id: null,
    stage_label: null,
    stage_kind: null,
    runner_up_stage_id: null,
    runner_up_stage_label: null,
    runner_up_stage_kind: null,
    compared_stage_count: 0,
    runs_recorded: 0,
    avg_validation_score: null,
    effectiveness_score: null,
    runner_up_effectiveness_score: null,
    effectiveness_gap: null,
    avg_validation_score_gap: null,
    marker_detection_gap: null,
    improved_runs: 0,
    unchanged_runs: 0,
    worsened_runs: 0,
    inconclusive_runs: 0,
    marker_detection_runs: 0,
    summary: "This older report did not include cleanup-stage recommendation guidance.",
    clean_claim_summary:
      "This older report did not separate 'best stage' from 'clean stage' guidance.",
    notes: [],
  };
}

function legacyControlledCanaryHistoryReport() {
  return {
    history_status: "controlled_canary_history_not_derived",
    recommendation_status: "controlled_canary_not_exercised",
    runs_with_canary_requested: 0,
    runs_with_completed_passes: 0,
    total_requested_passes: 0,
    total_completed_passes: 0,
    total_failed_passes: 0,
    clear_runs: 0,
    marker_detection_runs: 0,
    mixed_or_inconclusive_runs: 0,
    backend_unsupported_runs: 0,
    latest_execution_status: "controlled_canary_not_run_yet",
    latest_aggregate_signal_status: "controlled_canary_not_run_yet",
    summary: "This older report did not include repeated controlled canary history.",
    notes: [],
  };
}

function legacyValidationReleaseGateReport() {
  return {
    gate_status: "release_gate_not_derived",
    cleanup_stage_gate_status: "cleanup_stage_gate_not_derived",
    controlled_canary_gate_status: "controlled_canary_gate_not_derived",
    min_stage_runs_required: 2,
    min_clear_canary_runs_required: 2,
    max_marker_detection_runs_allowed_for_clean_claim: 0,
    max_worsened_runs_allowed_for_clean_stage: 0,
    max_inconclusive_runs_allowed_for_clean_stage: 0,
    stage_gate_passed: false,
    controlled_canary_gate_passed: false,
    summary: "This older report did not include explicit release-gating thresholds.",
    notes: [],
  };
}

function legacyPlatformCapabilityMatrixReport() {
  return {
    matrix_status: "matrix_not_derived",
    scope_platform: "unknown",
    scope_model_id: null,
    runtime_build_profile: null,
    gpu_offload_requested: null,
    summary: "This older report did not include a platform capability matrix.",
    capabilities: [],
    notes: [
      "Open a newer report to inspect Track A-E readiness for the current platform scope.",
    ],
  };
}

export function parseCorpusReport(raw: string): CorpusIngestionReport | null {
  if (!raw.trim()) {
    return null;
  }

  try {
    return JSON.parse(raw) as CorpusIngestionReport;
  } catch {
    return null;
  }
}
