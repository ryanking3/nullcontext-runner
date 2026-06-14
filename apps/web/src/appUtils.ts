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
    parsed.memory_validation_history.stage_trends =
      parsed.memory_validation_history.stage_trends.map((trend) => ({
        ...legacyMemoryValidationStageTrendReport(),
        ...trend,
      }));

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
    latest_vram_evidence_status: "legacy_status_unknown",
    latest_validation_verdict: "legacy_status_unknown",
    latest_marker_evidence_status: "legacy_status_unknown",
    summary: "This older report did not include cleanup-stage trend details.",
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
