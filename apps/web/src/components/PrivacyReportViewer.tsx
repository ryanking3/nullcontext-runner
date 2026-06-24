import { ReportGrid } from "./ReportGrid";
import {
  formatBoolean,
  formatBytes,
  formatDuration,
  formatSignedBytesDelta,
  formatTimestamp,
  humanizeSnakeCase,
  inspectionStatusClass,
  parsePrivacyReport,
  shortId,
  statusClass,
} from "../appUtils";

function inspectionMetricValue(value?: number | null) {
  if (value === undefined || value === null) {
    return "none";
  }

  return formatBytes(value);
}

function gpuObservationValue(pidObserved?: boolean | null, bytes?: number | null) {
  if (pidObserved === undefined || pidObserved === null) {
    return "unknown";
  }

  if (!pidObserved) {
    return "not observed";
  }

  if (bytes === undefined || bytes === null) {
    return "pid observed, bytes unavailable";
  }

  return `pid observed, ${formatBytes(bytes)}`;
}

function runtimeInspectionTakeaway(
  overallStatus: string,
  ramStatus: string,
  vramStatus: string
) {
  if (overallStatus === "process_still_observable_after_shutdown") {
    return "The runtime process was still visible after shutdown, so cleanup evidence is currently unfavorable.";
  }

  if (vramStatus === "gpu_entry_observed_during_post_shutdown_window") {
    return "GPU residency was observed during the post-shutdown window, so VRAM exposure remains explicitly observable.";
  }

  if (
    vramStatus ===
    "gpu_pid_observed_during_post_shutdown_window_but_memory_bytes_unavailable"
  ) {
    return "A matching GPU PID was observed during the post-shutdown window, but the current GPU backend did not expose per-process VRAM bytes.";
  }

  if (vramStatus === "gpu_entry_not_observed_after_shutdown_but_visibility_limited") {
    return "The runtime GPU PID was not observed after shutdown, but Windows/NVIDIA tooling remained visibility-limited, so VRAM cleanup evidence is still inconclusive.";
  }

  if (ramStatus === "resident_memory_not_observed_after_shutdown") {
    return "NullContext did not observe the runtime PID or residual process memory after shutdown, which is good evidence but not proof of sanitization.";
  }

  return "Inspection completed, but some memory domains remain inconclusive or host-tooling dependent.";
}

export function PrivacyReportViewer({
  rawReport,
  showRawReport,
  onToggleRaw,
}: {
  rawReport: string;
  showRawReport: boolean;
  onToggleRaw: () => void;
}) {
  if (!rawReport) {
    return <p className="muted-text">no report selected</p>;
  }

  const currentReport = parsePrivacyReport(rawReport);

  if (!currentReport) {
    return (
      <div className="report-viewer">
        <div className="report-toolbar">
          <div className="report-toolbar-copy">
            <strong>privacy report</strong>
            <span>raw output</span>
          </div>
        </div>
        <pre>{rawReport}</pre>
      </div>
    );
  }

  return (
    <div className="report-viewer">
      <div className="report-toolbar">
        <div className="report-toolbar-copy">
          <strong>privacy report</strong>
          <span>
            session {shortId(currentReport.session_id)} |{" "}
            {formatTimestamp(currentReport.started_at)}
          </span>
        </div>

        <button className={showRawReport ? "selected" : ""} onClick={onToggleRaw}>
          {showRawReport ? "hide raw json" : "view raw json"}
        </button>
      </div>

      <section className="report-section">
        <div className="panel-title">summary</div>
        <ReportGrid
          entries={[
            { label: "session id", value: currentReport.session_id },
            { label: "started", value: formatTimestamp(currentReport.started_at) },
            { label: "security mode", value: currentReport.security_mode },
            { label: "backend", value: currentReport.backend },
            { label: "gpu layers", value: currentReport.gpu_layers },
            { label: "history stored", value: formatBoolean(currentReport.history_stored) },
            {
              label: "process exited cleanly",
              value: formatBoolean(currentReport.process_exited_cleanly),
            },
          ]}
        />
      </section>

      {currentReport.lifecycle && (
        <section className="report-section">
          <div className="panel-title">lifecycle policy</div>
          <ReportGrid
            entries={[
              { label: "state", value: humanizeSnakeCase(currentReport.lifecycle.state) },
              {
                label: "retention policy",
                value: humanizeSnakeCase(currentReport.lifecycle.retention_policy),
              },
              {
                label: "retention deadline",
                value: currentReport.lifecycle.retention_deadline
                  ? formatTimestamp(currentReport.lifecycle.retention_deadline)
                  : "none",
              },
              {
                label: "cleanup requested",
                value: currentReport.lifecycle.cleanup_requested_at
                  ? formatTimestamp(currentReport.lifecycle.cleanup_requested_at)
                  : "none",
              },
              {
                label: "cleanup completed",
                value: currentReport.lifecycle.cleanup_completed_at
                  ? formatTimestamp(currentReport.lifecycle.cleanup_completed_at)
                  : "none",
              },
              {
                label: "cleanup reason",
                value: currentReport.lifecycle.cleanup_reason
                  ? humanizeSnakeCase(currentReport.lifecycle.cleanup_reason)
                  : "none",
              },
              {
                label: "state note",
                value: currentReport.lifecycle.state_note || "none",
              },
              {
                label: "updated",
                value: currentReport.lifecycle.updated_at
                  ? formatTimestamp(currentReport.lifecycle.updated_at)
                  : "none",
              },
            ]}
          />

          <div className="report-risk-block">
            <p>
              <strong>policy summary:</strong> {currentReport.lifecycle.policy_summary}
            </p>
            <p>
              <strong>decision summary:</strong> {currentReport.lifecycle.decision_summary}
            </p>
          </div>
        </section>
      )}

      {currentReport.session_profile && (
        <section className="report-section">
          <div className="panel-title">session profile</div>
          <ReportGrid
            entries={[
              { label: "session kind", value: currentReport.session_profile.session_kind },
              { label: "runtime lifetime", value: currentReport.session_profile.runtime_lifetime },
              { label: "turn count", value: String(currentReport.session_profile.turn_count) },
              {
                label: "runtime duration",
                value: formatDuration(currentReport.session_profile.runtime_duration_ms),
              },
              {
                label: "history policy",
                value: currentReport.session_profile.history_policy,
              },
              {
                label: "persistence policy",
                value: currentReport.session_profile.persistence_policy,
              },
              { label: "prompt source", value: currentReport.session_profile.prompt_source },
              {
                label: "grounding scope",
                value: currentReport.session_profile.grounding_scope || "none",
              },
              {
                label: "bound corpus",
                value:
                  currentReport.session_profile.bound_corpus_name ||
                  currentReport.session_profile.bound_corpus_id ||
                  "none",
              },
              {
                label: "grounded turns",
                value: String(currentReport.session_profile.grounded_turn_count ?? 0),
              },
            ]}
          />

          <details className="report-detail" open>
            <summary>
              <span>turn artifacts</span>
              <span className="pill neutral">
                {currentReport.session_profile.turn_artifacts.length}
              </span>
            </summary>
            {currentReport.session_profile.turn_artifacts.length === 0 ? (
              <p className="muted-text">no turn artifacts recorded</p>
            ) : (
              <div className="report-list">
                {currentReport.session_profile.turn_artifacts.map((artifact) => (
                  <div className="report-item" key={`${artifact.turn}-${artifact.prompt_path}`}>
                    <div className="report-item-header">
                      <strong>turn {artifact.turn}</strong>
                    </div>
                    <div className="report-path-list">
                      <div>prompt: {artifact.prompt_path}</div>
                      <div>response: {artifact.response_path}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>
        </section>
      )}

      {currentReport.llama_runtime && (
        <section className="report-section">
          <div className="panel-title">llama runtime exposure</div>

          <section className="runtime-summary-card">
            <div className="runtime-summary-header">
              <div>
                <strong>inspection verdict</strong>
                <p>
                  {runtimeInspectionTakeaway(
                    currentReport.llama_runtime.inspection_status,
                    currentReport.llama_runtime.ram_inspection_status,
                    currentReport.llama_runtime.vram_inspection_status
                  )}
                </p>
              </div>
              <div className="registry-lifecycle-summary">
                <span
                  className={inspectionStatusClass(currentReport.llama_runtime.inspection_status)}
                >
                  overall {humanizeSnakeCase(currentReport.llama_runtime.inspection_status)}
                </span>
                <span
                  className={inspectionStatusClass(
                    currentReport.llama_runtime.ram_inspection_status
                  )}
                >
                  ram {humanizeSnakeCase(currentReport.llama_runtime.ram_inspection_status)}
                </span>
                <span
                  className={inspectionStatusClass(
                    currentReport.llama_runtime.vram_inspection_status
                  )}
                >
                  vram {humanizeSnakeCase(currentReport.llama_runtime.vram_inspection_status)}
                </span>
              </div>
            </div>

            <div className="runtime-summary-metrics">
              <div className="runtime-summary-metric">
                <span className="runtime-summary-label">live rss</span>
                <strong>
                  {inspectionMetricValue(currentReport.llama_runtime.observed_resident_bytes)}
                </strong>
              </div>
              <div className="runtime-summary-metric">
                <span className="runtime-summary-label">footprint</span>
                <strong>
                  {inspectionMetricValue(currentReport.llama_runtime.physical_footprint_bytes)}
                </strong>
              </div>
              <div className="runtime-summary-metric">
                <span className="runtime-summary-label">gpu memory</span>
                <strong>
                  {gpuObservationValue(
                    currentReport.llama_runtime.observed_gpu_pid,
                    currentReport.llama_runtime.observed_gpu_memory_bytes
                  )}
                </strong>
              </div>
              <div className="runtime-summary-metric">
                <span className="runtime-summary-label">after shutdown</span>
                <strong>
                  {currentReport.llama_runtime.process_present_after_shutdown === undefined ||
                  currentReport.llama_runtime.process_present_after_shutdown === null
                    ? "unknown"
                    : currentReport.llama_runtime.process_present_after_shutdown
                      ? "still present"
                      : "not observed"}
                </strong>
              </div>
            </div>

            <div className="runtime-summary-meta">
              <span>
                source: {currentReport.llama_runtime.process_memory_source || "none"}
              </span>
              <span>
                gpu backend: {currentReport.llama_runtime.gpu_observation_backend || "none"}
              </span>
              <span>
                gpu live:{" "}
                {humanizeSnakeCase(currentReport.llama_runtime.live_gpu_visibility_status)}
              </span>
              <span>
                gpu evidence:{" "}
                {humanizeSnakeCase(currentReport.llama_runtime.live_gpu_evidence_class)}
              </span>
              <span>
                gpu limit:{" "}
                {humanizeSnakeCase(currentReport.llama_runtime.live_gpu_limitation_status)}
              </span>
              <span>
                gpu boundary:{" "}
                {humanizeSnakeCase(currentReport.llama_runtime.gpu_trust_boundary_status)}
              </span>
              <span>window: {currentReport.llama_runtime.verification_window_ms} ms</span>
              <span>
                shutdown: {humanizeSnakeCase(currentReport.llama_runtime.shutdown_method)}
              </span>
            </div>
          </section>

          <div className="report-risk-block">
            <p>{currentReport.llama_runtime.inspection_summary}</p>
          </div>

          <div className="registry-lifecycle-summary">
            <span className={inspectionStatusClass(currentReport.llama_runtime.inspection_status)}>
              overall {humanizeSnakeCase(currentReport.llama_runtime.inspection_status)}
            </span>
            <span
              className={inspectionStatusClass(currentReport.llama_runtime.ram_inspection_status)}
            >
              ram {humanizeSnakeCase(currentReport.llama_runtime.ram_inspection_status)}
            </span>
            <span
              className={inspectionStatusClass(currentReport.llama_runtime.vram_inspection_status)}
            >
              vram {humanizeSnakeCase(currentReport.llama_runtime.vram_inspection_status)}
            </span>
          </div>

          <ReportGrid
            entries={[
              { label: "runtime kind", value: currentReport.llama_runtime.runtime_kind },
              {
                label: "runtime pid",
                value: currentReport.llama_runtime.runtime_pid?.toString() || "none",
              },
              {
                label: "runtime endpoint",
                value: currentReport.llama_runtime.runtime_endpoint || "none",
              },
              {
                label: "model",
                value: `${currentReport.llama_runtime.model_name} (${currentReport.llama_runtime.model_id})`,
              },
              {
                label: "gpu layers requested",
                value: String(currentReport.llama_runtime.gpu_layers_requested),
              },
              {
                label: "gpu offload requested",
                value: formatBoolean(currentReport.llama_runtime.gpu_offload_requested),
              },
              {
                label: "shutdown method",
                value: currentReport.llama_runtime.shutdown_method,
              },
              {
                label: "process exit code",
                value: currentReport.llama_runtime.process_exit_code?.toString() || "none",
              },
              {
                label: "graceful shutdown supported",
                value: formatBoolean(currentReport.llama_runtime.graceful_shutdown_supported),
              },
              {
                label: "observed resident memory",
                value: currentReport.llama_runtime.observed_resident_bytes
                  ? formatBytes(currentReport.llama_runtime.observed_resident_bytes)
                  : "none",
              },
              {
                label: "observed virtual memory",
                value: currentReport.llama_runtime.observed_virtual_bytes
                  ? formatBytes(currentReport.llama_runtime.observed_virtual_bytes)
                  : "none",
              },
              {
                label: "physical footprint",
                value: currentReport.llama_runtime.physical_footprint_bytes
                  ? formatBytes(currentReport.llama_runtime.physical_footprint_bytes)
                  : "none",
              },
              {
                label: "peak footprint",
                value: currentReport.llama_runtime.physical_footprint_peak_bytes
                  ? formatBytes(currentReport.llama_runtime.physical_footprint_peak_bytes)
                  : "none",
              },
              {
                label: "gpu pid observed",
                value:
                  currentReport.llama_runtime.observed_gpu_pid === undefined ||
                  currentReport.llama_runtime.observed_gpu_pid === null
                    ? "unknown"
                    : formatBoolean(currentReport.llama_runtime.observed_gpu_pid),
              },
              {
                label: "observed gpu memory",
                value: currentReport.llama_runtime.observed_gpu_memory_bytes
                  ? formatBytes(currentReport.llama_runtime.observed_gpu_memory_bytes)
                  : "none",
              },
              {
                label: "live gpu visibility",
                value: humanizeSnakeCase(currentReport.llama_runtime.live_gpu_visibility_status),
              },
              {
                label: "live gpu evidence class",
                value: humanizeSnakeCase(currentReport.llama_runtime.live_gpu_evidence_class),
              },
              {
                label: "live gpu limitation",
                value: humanizeSnakeCase(currentReport.llama_runtime.live_gpu_limitation_status),
              },
              {
                label: "gpu trust boundary",
                value: humanizeSnakeCase(currentReport.llama_runtime.gpu_trust_boundary_status),
              },
              {
                label: "gpu evidence tier",
                value: humanizeSnakeCase(currentReport.llama_runtime.gpu_evidence_tier_status),
              },
              {
                label: "gpu claim boundary",
                value: humanizeSnakeCase(currentReport.llama_runtime.gpu_claim_boundary_status),
              },
              {
                label: "gpu context visibility",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.gpu_context_visibility_status
                ),
              },
              {
                label: "gpu backend provenance",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.gpu_backend_provenance_status
                ),
              },
              {
                label: "allocator/kv cleanup boundary",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.allocator_kv_cleanup_boundary_status
                ),
              },
              {
                label: "process memory source",
                value: currentReport.llama_runtime.process_memory_source || "none",
              },
              {
                label: "gpu memory source",
                value: currentReport.llama_runtime.gpu_memory_source || "none",
              },
              {
                label: "live gpu backend",
                value: currentReport.llama_runtime.gpu_observation_backend || "none",
              },
              {
                label: "vmmap summary source",
                value: currentReport.llama_runtime.vmmap_summary_source || "none",
              },
              {
                label: "process present after shutdown",
                value:
                  currentReport.llama_runtime.process_present_after_shutdown === undefined ||
                  currentReport.llama_runtime.process_present_after_shutdown === null
                    ? "unknown"
                    : formatBoolean(currentReport.llama_runtime.process_present_after_shutdown),
              },
              {
                label: "verification window",
                value: `${currentReport.llama_runtime.verification_window_ms} ms`,
              },
              {
                label: "post-shutdown rss",
                value: currentReport.llama_runtime.process_resident_bytes_after_shutdown
                  ? formatBytes(currentReport.llama_runtime.process_resident_bytes_after_shutdown)
                  : "none",
              },
              {
                label: "post-shutdown virtual",
                value: currentReport.llama_runtime.process_virtual_bytes_after_shutdown
                  ? formatBytes(currentReport.llama_runtime.process_virtual_bytes_after_shutdown)
                  : "none",
              },
              {
                label: "post-shutdown footprint",
                value: currentReport.llama_runtime.physical_footprint_bytes_after_shutdown
                  ? formatBytes(currentReport.llama_runtime.physical_footprint_bytes_after_shutdown)
                  : "none",
              },
              {
                label: "post-shutdown peak footprint",
                value: currentReport.llama_runtime.physical_footprint_peak_bytes_after_shutdown
                  ? formatBytes(
                      currentReport.llama_runtime.physical_footprint_peak_bytes_after_shutdown
                    )
                  : "none",
              },
              {
                label: "footprint delta",
                value:
                  currentReport.llama_runtime.physical_footprint_delta_bytes === undefined ||
                  currentReport.llama_runtime.physical_footprint_delta_bytes === null
                    ? "none"
                    : formatSignedBytesDelta(
                        currentReport.llama_runtime.physical_footprint_delta_bytes
                      ),
              },
              {
                label: "gpu entry present after shutdown",
                value:
                  currentReport.llama_runtime.gpu_entry_present_after_shutdown === undefined ||
                  currentReport.llama_runtime.gpu_entry_present_after_shutdown === null
                    ? "unknown"
                    : formatBoolean(currentReport.llama_runtime.gpu_entry_present_after_shutdown),
              },
              {
                label: "gpu memory after shutdown",
                value: currentReport.llama_runtime.gpu_memory_bytes_after_shutdown
                  ? formatBytes(currentReport.llama_runtime.gpu_memory_bytes_after_shutdown)
                  : "none",
              },
              {
                label: "peak gpu memory after shutdown",
                value: currentReport.llama_runtime.gpu_peak_memory_bytes_after_shutdown
                  ? formatBytes(currentReport.llama_runtime.gpu_peak_memory_bytes_after_shutdown)
                  : "none",
              },
              {
                label: "post-shutdown gpu samples",
                value: String(currentReport.llama_runtime.gpu_samples_collected_after_shutdown),
              },
              {
                label: "gpu-positive samples",
                value: String(
                  currentReport.llama_runtime.gpu_samples_with_pid_observed_after_shutdown
                ),
              },
              {
                label: "last gpu pid seen",
                value:
                  currentReport.llama_runtime.gpu_last_pid_observed_at_ms === undefined ||
                  currentReport.llama_runtime.gpu_last_pid_observed_at_ms === null
                    ? "none"
                    : `${currentReport.llama_runtime.gpu_last_pid_observed_at_ms} ms`,
              },
              {
                label: "post-shutdown gpu visibility",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.post_shutdown_gpu_visibility_status
                ),
              },
              {
                label: "post-shutdown gpu evidence class",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.post_shutdown_gpu_evidence_class
                ),
              },
              {
                label: "post-shutdown gpu limitation",
                value: humanizeSnakeCase(
                  currentReport.llama_runtime.post_shutdown_gpu_limitation_status
                ),
              },
              {
                label: "process check source",
                value: currentReport.llama_runtime.process_check_source || "none",
              },
              {
                label: "gpu check source",
                value: currentReport.llama_runtime.gpu_check_source || "none",
              },
              {
                label: "post-shutdown gpu backend",
                value: currentReport.llama_runtime.gpu_check_backend || "none",
              },
              {
                label: "post-shutdown vmmap source",
                value: currentReport.llama_runtime.vmmap_summary_source_after_shutdown || "none",
              },
              {
                label: "model path",
                value: currentReport.llama_runtime.model_path,
              },
            ]}
          />

          <div className="report-risk-block">
            <p>
              <strong>gpu evidence:</strong>{" "}
              {currentReport.llama_runtime.gpu_evidence_summary}
            </p>
            <p>
              <strong>gpu limitation:</strong>{" "}
              {currentReport.llama_runtime.gpu_limitation_summary}
            </p>
            <p>
              <strong>gpu trust boundary:</strong>{" "}
              {currentReport.llama_runtime.gpu_trust_boundary_summary}
            </p>
            <p>
              <strong>gpu evidence tier:</strong>{" "}
              {currentReport.llama_runtime.gpu_evidence_tier_summary}
            </p>
            <p>
              <strong>gpu claim boundary:</strong>{" "}
              {currentReport.llama_runtime.gpu_claim_boundary_summary}
            </p>
            <p>
              <strong>gpu context visibility:</strong>{" "}
              {currentReport.llama_runtime.gpu_context_visibility_summary}
            </p>
            <p>
              <strong>gpu backend provenance:</strong>{" "}
              {currentReport.llama_runtime.gpu_backend_provenance_summary}
            </p>
            <p>
              <strong>allocator/kv cleanup boundary:</strong>{" "}
              {currentReport.llama_runtime.allocator_kv_cleanup_boundary_summary}
            </p>
            <p>
              <strong>cleanup boundary:</strong> {currentReport.llama_runtime.cleanup_summary}
            </p>
            <p>
              <strong>runtime-specific residual risk:</strong>{" "}
              {currentReport.llama_runtime.residual_risk_summary}
            </p>
          </div>

          <details className="report-detail" open>
            <summary>
              <span>vram cleanup strategy</span>
              <span className="pill neutral">
                {humanizeSnakeCase(currentReport.llama_runtime.vram_cleanup.evidence_outcome)}
              </span>
            </summary>

            <div className="report-risk-block">
              <p>{currentReport.llama_runtime.vram_cleanup.summary}</p>
            </div>

            <ReportGrid
              entries={[
                {
                  label: "strategy",
                  value: currentReport.llama_runtime.vram_cleanup.strategy_label,
                },
                {
                  label: "strategy id",
                  value: currentReport.llama_runtime.vram_cleanup.strategy_id,
                },
                {
                  label: "kind",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.strategy_kind
                  ),
                },
                {
                  label: "implementation",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.implementation_status
                  ),
                },
                {
                  label: "support",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.support_status
                  ),
                },
                {
                  label: "attempt",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.attempt_status
                  ),
                },
                {
                  label: "activation timing",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.activation_timing
                  ),
                },
                {
                  label: "evidence outcome",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.evidence_outcome
                  ),
                },
                {
                  label: "expected scope",
                  value: currentReport.llama_runtime.vram_cleanup.expected_effect_scope,
                },
              ]}
            />

            <div className="report-risk-block">
              <p>
                <strong>comparison summary:</strong>{" "}
                {currentReport.llama_runtime.vram_cleanup.comparison.summary}
              </p>
            </div>

            <ReportGrid
              entries={[
                {
                  label: "comparison status",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison.comparison_status
                  ),
                },
                {
                  label: "run role",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison.current_run_role
                  ),
                },
                {
                  label: "selected stage",
                  value:
                    currentReport.llama_runtime.vram_cleanup.comparison.selected_stage_label ??
                    "none",
                },
                {
                  label: "improvement status",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison
                      .evidence_improvement_status
                  ),
                },
                {
                  label: "selected cleanup signal support",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison
                      .cleanup_signal_support_status
                  ),
                },
                {
                  label: "selected cleanup signal scope",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison
                      .cleanup_signal_support_scope_status
                  ),
                },
              ]}
            />

            <div className="report-risk-block">
              <p>
                <strong>selection reason:</strong>{" "}
                {currentReport.llama_runtime.vram_cleanup.comparison.selection_reason}
              </p>
              <p>
                <strong>selected stage cleanup signal support:</strong>{" "}
                {currentReport.llama_runtime.vram_cleanup.comparison
                  .cleanup_signal_support_summary}
              </p>
              <p>
                <strong>selected stage cleanup signal scope:</strong>{" "}
                {currentReport.llama_runtime.vram_cleanup.comparison
                  .cleanup_signal_support_scope_summary}
              </p>
              <p>
                <strong>selected stage contributing cleanup signals:</strong>{" "}
                {currentReport.llama_runtime.vram_cleanup.comparison
                  .contributing_cleanup_signals.length > 0
                  ? currentReport.llama_runtime.vram_cleanup.comparison.contributing_cleanup_signals.join(
                      ", "
                    )
                  : "none"}
              </p>
            </div>

            <details className="report-detail" open>
              <summary>
                <span>baseline vs selected evidence</span>
                <span className="pill neutral">
                  {humanizeSnakeCase(
                    currentReport.llama_runtime.vram_cleanup.comparison
                      .evidence_improvement_status
                  )}
                </span>
              </summary>

              <ReportGrid
                entries={[
                  {
                    label: "baseline vram status",
                    value: humanizeSnakeCase(
                      currentReport.llama_runtime.vram_cleanup.comparison.baseline_snapshot
                        .vram_inspection_status
                    ),
                  },
                  {
                    label: "current vram status",
                    value: humanizeSnakeCase(
                      currentReport.llama_runtime.vram_cleanup.comparison.current_snapshot
                        .vram_inspection_status
                    ),
                  },
                  {
                    label: "baseline gpu visibility",
                    value: humanizeSnakeCase(
                      currentReport.llama_runtime.vram_cleanup.comparison.baseline_snapshot
                        .post_shutdown_gpu_visibility_status
                    ),
                  },
                  {
                    label: "current gpu visibility",
                    value: humanizeSnakeCase(
                      currentReport.llama_runtime.vram_cleanup.comparison.current_snapshot
                        .post_shutdown_gpu_visibility_status
                    ),
                  },
                  {
                    label: "marker evidence",
                    value: humanizeSnakeCase(
                      currentReport.llama_runtime.vram_cleanup.comparison
                        .marker_evidence_status
                    ),
                  },
                  {
                    label: "selected stage id",
                    value:
                      currentReport.llama_runtime.vram_cleanup.comparison.selected_stage_id ??
                      "none",
                  },
                  {
                    label: "selected stage kind",
                    value: currentReport.llama_runtime.vram_cleanup.comparison.selected_stage_kind
                      ? humanizeSnakeCase(
                          currentReport.llama_runtime.vram_cleanup.comparison
                            .selected_stage_kind
                        )
                      : "none",
                  },
                  {
                    label: "baseline peak bytes",
                    value:
                      currentReport.llama_runtime.vram_cleanup.comparison.baseline_snapshot
                        .gpu_peak_memory_bytes
                        ? formatBytes(
                            currentReport.llama_runtime.vram_cleanup.comparison
                              .baseline_snapshot.gpu_peak_memory_bytes
                          )
                        : "none",
                  },
                  {
                    label: "current peak bytes",
                    value:
                      currentReport.llama_runtime.vram_cleanup.comparison.current_snapshot
                        .gpu_peak_memory_bytes
                        ? formatBytes(
                            currentReport.llama_runtime.vram_cleanup.comparison.current_snapshot
                              .gpu_peak_memory_bytes
                          )
                        : "none",
                  },
                  {
                    label: "baseline gpu-positive samples",
                    value: String(
                      currentReport.llama_runtime.vram_cleanup.comparison.baseline_snapshot
                        .gpu_samples_with_pid_observed
                    ),
                  },
                  {
                    label: "current gpu-positive samples",
                    value: String(
                      currentReport.llama_runtime.vram_cleanup.comparison.current_snapshot
                        .gpu_samples_with_pid_observed
                    ),
                  },
                ]}
              />

              {currentReport.llama_runtime.vram_cleanup.comparison.notes.length > 0 && (
                <div className="report-risk-block">
                  <p>
                    <strong>marker evidence summary:</strong>{" "}
                    {
                      currentReport.llama_runtime.vram_cleanup.comparison
                        .marker_evidence_summary
                    }
                  </p>
                  {currentReport.llama_runtime.vram_cleanup.comparison.notes.map((note) => (
                    <p key={note}>{note}</p>
                  ))}
                </div>
              )}
            </details>

            {currentReport.llama_runtime.vram_cleanup.stages.length > 0 && (
              <details className="report-detail" open>
                <summary>
                  <span>strategy stages</span>
                  <span className="pill neutral">
                    {currentReport.llama_runtime.vram_cleanup.stages.length}
                  </span>
                </summary>

                <div className="report-list">
                  {currentReport.llama_runtime.vram_cleanup.stages.map((stage) => (
                    <div className="report-item" key={stage.stage_id}>
                      <div className="report-item-header">
                        <strong>{stage.stage_label}</strong>
                        <span className={inspectionStatusClass(stage.evidence_improvement_status)}>
                          {humanizeSnakeCase(stage.evidence_improvement_status)}
                        </span>
                      </div>
                      <div className="report-path-list">
                        <div>stage id: {stage.stage_id}</div>
                        <div>kind: {humanizeSnakeCase(stage.stage_kind)}</div>
                        <div>cooldown: {stage.cooldown_ms_before_stage} ms</div>
                        <div>window: {stage.verification_window_ms} ms</div>
                        <div>action: {humanizeSnakeCase(stage.action_status)}</div>
                        <div>
                          selection evidence:{" "}
                          {humanizeSnakeCase(stage.selection_evidence_status)}
                        </div>
                        <div>
                          cleanup signal support:{" "}
                          {humanizeSnakeCase(stage.cleanup_signal_support_status)}
                        </div>
                        <div>
                          cleanup signal scope:{" "}
                          {humanizeSnakeCase(stage.cleanup_signal_support_scope_status)}
                        </div>
                        <div>
                          contributing cleanup signals:{" "}
                          {stage.contributing_cleanup_signals.length > 0
                            ? stage.contributing_cleanup_signals.join(", ")
                            : "none"}
                        </div>
                        <div>
                          marker evidence: {humanizeSnakeCase(stage.marker_evidence_status)}
                        </div>
                        <div>
                          stage process scan:{" "}
                          {stage.process_scan_phase
                            ? humanizeSnakeCase(stage.process_scan_phase.status)
                            : "none"}
                        </div>
                        <div>
                          helper canary scan:{" "}
                          {stage.helper_process_scan_report
                            ? humanizeSnakeCase(stage.helper_process_scan_report.overall_status)
                            : "none"}
                        </div>
                        <div>
                          peak gpu bytes:{" "}
                          {stage.evidence_snapshot.gpu_peak_memory_bytes
                            ? formatBytes(stage.evidence_snapshot.gpu_peak_memory_bytes)
                            : "none"}
                        </div>
                        <div>
                          gpu-positive samples:{" "}
                          {stage.evidence_snapshot.gpu_samples_with_pid_observed}
                        </div>
                        {stage.process_scan_phase && (
                          <div>
                            stage scan method:{" "}
                            {humanizeSnakeCase(stage.process_scan_phase.method)}
                          </div>
                        )}
                        {stage.helper_process_scan_report && (
                          <div>
                            helper scan summary: {stage.helper_process_scan_report.summary}
                          </div>
                        )}
                        <div>{stage.selection_evidence_summary}</div>
                        <div>{stage.cleanup_signal_support_summary}</div>
                        <div>{stage.cleanup_signal_support_scope_summary}</div>
                        <div>{stage.marker_evidence_summary}</div>
                        <div>{stage.summary}</div>
                        {stage.notes.map((note) => (
                          <div key={note}>{note}</div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </details>
            )}

            {currentReport.llama_runtime.vram_cleanup.notes.length > 0 && (
              <div className="report-risk-block">
                {currentReport.llama_runtime.vram_cleanup.notes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </div>
            )}
          </details>

          <details className="report-detail" open>
            <summary>
              <span>runtime introspection capabilities</span>
              <span className="pill warning">
                {humanizeSnakeCase(
                  currentReport.llama_runtime.introspection.allocator_introspection_status
                )}
              </span>
            </summary>

            <div className="report-risk-block">
              <p>{currentReport.llama_runtime.introspection.summary}</p>
            </div>

            <ReportGrid
              entries={[
                {
                  label: "capability source",
                  value: currentReport.llama_runtime.introspection.capability_source,
                },
                {
                  label: "manifest path",
                  value: currentReport.llama_runtime.introspection.manifest_path || "none",
                },
                {
                  label: "runtime build profile",
                  value: currentReport.llama_runtime.introspection.runtime_build_profile,
                },
                {
                  label: "instrumentation backend",
                  value: currentReport.llama_runtime.introspection.instrumentation_backend,
                },
                {
                  label: "declared signals",
                  value:
                    currentReport.llama_runtime.introspection.declared_signal_ids.length === 0
                      ? "none"
                      : currentReport.llama_runtime.introspection.declared_signal_ids.join(", "),
                },
                {
                  label: "declared cleanup signals",
                  value:
                    currentReport.llama_runtime.introspection.declared_cleanup_signal_ids
                      .length === 0
                      ? "none"
                      : currentReport.llama_runtime.introspection.declared_cleanup_signal_ids.join(
                          ", "
                        ),
                },
                {
                  label: "signal evidence tier",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.lifecycle_signal_evidence_tier
                  ),
                },
                {
                  label: "instrumentation evidence",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.instrumentation_evidence_status
                  ),
                },
                {
                  label: "signal contract",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.signal_contract_status
                  ),
                },
                {
                  label: "declared runtime signals",
                  value: String(
                    currentReport.llama_runtime.introspection.declared_signal_count
                  ),
                },
                {
                  label: "observed unique runtime signals",
                  value: String(
                    currentReport.llama_runtime.introspection.observed_signal_unique_count
                  ),
                },
                {
                  label: "missing declared runtime signals",
                  value: String(
                    currentReport.llama_runtime.introspection
                      .missing_declared_signal_count
                  ),
                },
                {
                  label: "undeclared observed runtime signals",
                  value: String(
                    currentReport.llama_runtime.introspection
                      .undeclared_observed_signal_count
                  ),
                },
                {
                  label: "cleanup-path evidence",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.cleanup_path_evidence_status
                  ),
                },
                {
                  label: "setup-signal coverage",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.setup_signal_coverage_status
                  ),
                },
                {
                  label: "cleanup-signal coverage",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.cleanup_signal_coverage_status
                  ),
                },
                {
                  label: "cleanup-signal contract",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.cleanup_signal_contract_status
                  ),
                },
                {
                  label: "declared cleanup signals",
                  value: String(
                    currentReport.llama_runtime.introspection.declared_cleanup_signal_count
                  ),
                },
                {
                  label: "observed cleanup signals",
                  value: String(
                    currentReport.llama_runtime.introspection.observed_cleanup_signal_count
                  ),
                },
                {
                  label: "missing declared signals",
                  value: String(
                    currentReport.llama_runtime.introspection
                      .missing_declared_cleanup_signal_count
                  ),
                },
                {
                  label: "undeclared observed signals",
                  value: String(
                    currentReport.llama_runtime.introspection
                      .undeclared_observed_cleanup_signal_count
                  ),
                },
                {
                  label: "allocator introspection",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.allocator_introspection_status
                  ),
                },
                {
                  label: "allocator initialized",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.allocator_initialized_observed
                  ),
                },
                {
                  label: "allocator teardown observed",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.allocator_teardown_observed
                  ),
                },
                {
                  label: "allocator reset observed",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.allocator_reset_observed
                  ),
                },
                {
                  label: "kv/cache introspection",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.kv_cache_introspection_status
                  ),
                },
                {
                  label: "kv cache initialized",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.kv_cache_initialized_observed
                  ),
                },
                {
                  label: "kv cache reused",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.kv_cache_reused_observed
                  ),
                },
                {
                  label: "kv cache clear observed",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.kv_cache_clear_observed
                  ),
                },
                {
                  label: "model unload observed",
                  value: formatBoolean(
                    currentReport.llama_runtime.introspection.model_unload_observed
                  ),
                },
                {
                  label: "model unload signal",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.model_unload_signal_status
                  ),
                },
                {
                  label: "allocator reset signal",
                  value: humanizeSnakeCase(
                    currentReport.llama_runtime.introspection.allocator_reset_signal_status
                  ),
                },
                {
                  label: "observed signal count",
                  value: String(
                    currentReport.llama_runtime.introspection.observed_signal_count
                  ),
                },
                {
                  label: "observed signal sources",
                  value:
                    currentReport.llama_runtime.introspection.observed_signal_sources.length === 0
                      ? "none"
                      : currentReport.llama_runtime.introspection.observed_signal_sources.join(
                          ", "
                        ),
                },
              ]}
            />

            {currentReport.llama_runtime.introspection.notes.length > 0 && (
              <div className="report-risk-block">
                <p>
                  <strong>allocator summary:</strong>{" "}
                  {currentReport.llama_runtime.introspection.allocator_summary}
                </p>
                <p>
                  <strong>kv/cache summary:</strong>{" "}
                  {currentReport.llama_runtime.introspection.kv_cache_summary}
                </p>
                <p>
                  <strong>instrumentation evidence:</strong>{" "}
                  {currentReport.llama_runtime.introspection.instrumentation_evidence_summary}
                </p>
                <p>
                  <strong>signal contract:</strong>{" "}
                  {currentReport.llama_runtime.introspection.signal_contract_summary}
                </p>
                <p>
                  <strong>cleanup-signal contract:</strong>{" "}
                  {currentReport.llama_runtime.introspection.cleanup_signal_contract_summary}
                </p>
                {currentReport.llama_runtime.introspection.notes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </div>
            )}

            <details className="report-detail" open>
              <summary>
                <span>runtime signal contract matrix</span>
                <span className="pill neutral">
                  {currentReport.llama_runtime.introspection.runtime_signal_matrix.length}
                </span>
              </summary>
              {currentReport.llama_runtime.introspection.runtime_signal_matrix.length === 0 ? (
                <p className="muted-text">no runtime-signal contract entries were recorded</p>
              ) : (
                <div className="report-list">
                  {currentReport.llama_runtime.introspection.runtime_signal_matrix.map((entry) => (
                    <div className="report-item" key={entry.signal_id}>
                      <div className="report-item-header">
                        <strong>{entry.signal_label}</strong>
                        <span className={inspectionStatusClass(entry.evidence_status)}>
                          {humanizeSnakeCase(entry.evidence_status)}
                        </span>
                      </div>
                      <div className="report-path-list">
                        <div>
                          declared support:{" "}
                          {humanizeSnakeCase(entry.declared_support_status)}
                        </div>
                        <div>
                          observation: {humanizeSnakeCase(entry.observation_status)}
                        </div>
                        <div>observed count: {entry.observed_count}</div>
                        <div>
                          observed sources:{" "}
                          {entry.observed_sources.length > 0
                            ? entry.observed_sources.join(", ")
                            : "none"}
                        </div>
                        <div>
                          observed phases:{" "}
                          {entry.observed_phases.length > 0
                            ? entry.observed_phases
                                .map((phase) => humanizeSnakeCase(phase))
                                .join(", ")
                            : "none"}
                        </div>
                        <div>
                          sample status:{" "}
                          {entry.sample_observed_status
                            ? humanizeSnakeCase(entry.sample_observed_status)
                            : "none"}
                        </div>
                        {entry.sample_observed_details && (
                          <div>sample details: {entry.sample_observed_details}</div>
                        )}
                        <div>{entry.summary}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </details>

            <details className="report-detail" open>
              <summary>
                <span>cleanup signal coverage matrix</span>
                <span className="pill neutral">
                  {currentReport.llama_runtime.introspection.cleanup_signal_matrix.length}
                </span>
              </summary>
              {currentReport.llama_runtime.introspection.cleanup_signal_matrix.length === 0 ? (
                <p className="muted-text">no cleanup-signal coverage entries were recorded</p>
              ) : (
                <div className="report-list">
                  {currentReport.llama_runtime.introspection.cleanup_signal_matrix.map((entry) => (
                    <div className="report-item" key={entry.signal_id}>
                      <div className="report-item-header">
                        <strong>{entry.signal_label}</strong>
                        <span className={inspectionStatusClass(entry.evidence_status)}>
                          {humanizeSnakeCase(entry.evidence_status)}
                        </span>
                      </div>
                      <div className="report-path-list">
                        <div>
                          declared support:{" "}
                          {humanizeSnakeCase(entry.declared_support_status)}
                        </div>
                        <div>
                          observation: {humanizeSnakeCase(entry.observation_status)}
                        </div>
                        <div>observed count: {entry.observed_count}</div>
                        <div>
                          observed sources:{" "}
                          {entry.observed_sources.length > 0
                            ? entry.observed_sources.join(", ")
                            : "none"}
                        </div>
                        <div>
                          observed phases:{" "}
                          {entry.observed_phases.length > 0
                            ? entry.observed_phases
                                .map((phase) => humanizeSnakeCase(phase))
                                .join(", ")
                            : "none"}
                        </div>
                        <div>
                          sample status:{" "}
                          {entry.sample_observed_status
                            ? humanizeSnakeCase(entry.sample_observed_status)
                            : "none"}
                        </div>
                        {entry.sample_observed_details && (
                          <div>sample details: {entry.sample_observed_details}</div>
                        )}
                        <div>{entry.summary}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </details>

            <details className="report-detail" open>
              <summary>
                <span>observed lifecycle signals</span>
                <span className="pill neutral">
                  {currentReport.llama_runtime.introspection.observed_events.length}
                </span>
              </summary>
              {currentReport.llama_runtime.introspection.observed_events.length === 0 ? (
                <p className="muted-text">no allocator or kv lifecycle signals were captured</p>
              ) : (
                <div className="report-list">
                  {currentReport.llama_runtime.introspection.observed_events.map((event) => (
                    <div
                      className="report-item"
                      key={`${event.source}-${event.event}-${event.details}`}
                    >
                      <div className="report-item-header">
                        <strong>{humanizeSnakeCase(event.event)}</strong>
                        <span className={statusClass(event.status)}>{event.status}</span>
                      </div>
                      <div className="report-path-list">
                        <div>source: {event.source}</div>
                        <div>phase: {humanizeSnakeCase(event.lifecycle_phase)}</div>
                        <div>scope: {humanizeSnakeCase(event.evidence_scope)}</div>
                        <div>
                          cleanup relevance:{" "}
                          {humanizeSnakeCase(event.cleanup_relevance)}
                        </div>
                        <div>{event.details || "no extra details"}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </details>
          </details>

          <details className="report-detail" open>
            <summary>
              <span>resident regions</span>
              <span className="pill neutral">
                {currentReport.llama_runtime.resident_regions.length}
              </span>
            </summary>
            {currentReport.llama_runtime.resident_regions.length === 0 ? (
              <p className="muted-text">no detailed region summary captured</p>
            ) : (
              <div className="report-list">
                {currentReport.llama_runtime.resident_regions.map((region) => (
                  <div
                    className="report-item"
                    key={`${region.region_type}-${region.resident_bytes}`}
                  >
                    <div className="report-item-header">
                      <strong>{region.region_type}</strong>
                      <span className="pill neutral">
                        {formatBytes(region.resident_bytes)} resident
                      </span>
                    </div>
                    <div className="report-path-list">
                      <div>virtual: {formatBytes(region.virtual_bytes)}</div>
                      <div>resident: {formatBytes(region.resident_bytes)}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>

          <details className="report-detail" open>
            <summary>
              <span>memory domains</span>
              <span className="pill neutral">
                {currentReport.llama_runtime.memory_domains.length}
              </span>
            </summary>
            <div className="audit-list">
              {currentReport.llama_runtime.memory_domains.map((domain) => (
                <details className="audit-item" key={`${domain.domain}-${domain.exposure_scope}`}>
                  <summary>
                    <code>{domain.domain}</code>
                    <span className={statusClass(domain.cleanup_status)}>
                      {domain.cleanup_status}
                    </span>
                  </summary>
                  <p>
                    <strong>scope:</strong> {domain.exposure_scope}
                  </p>
                  <p>{domain.notes}</p>
                </details>
              ))}
            </div>
          </details>

          {currentReport.llama_runtime.observation_notes.length > 0 && (
            <details className="report-detail" open>
              <summary>
                <span>observation notes</span>
                <span className="pill neutral">
                  {currentReport.llama_runtime.observation_notes.length}
                </span>
              </summary>
              <div className="report-risk-block">
                {currentReport.llama_runtime.observation_notes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </div>
            </details>
          )}

          <details className="report-detail" open>
            <summary>
              <span>post-shutdown resident regions</span>
              <span className="pill neutral">
                {currentReport.llama_runtime.resident_regions_after_shutdown.length}
              </span>
            </summary>
            {currentReport.llama_runtime.resident_regions_after_shutdown.length === 0 ? (
              <p className="muted-text">no post-shutdown region summary captured</p>
            ) : (
              <div className="report-list">
                {currentReport.llama_runtime.resident_regions_after_shutdown.map((region) => (
                  <div
                    className="report-item"
                    key={`post-${region.region_type}-${region.resident_bytes}`}
                  >
                    <div className="report-item-header">
                      <strong>{region.region_type}</strong>
                      <span className="pill neutral">
                        {formatBytes(region.resident_bytes)} resident
                      </span>
                    </div>
                    <div className="report-path-list">
                      <div>virtual: {formatBytes(region.virtual_bytes)}</div>
                      <div>resident: {formatBytes(region.resident_bytes)}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>

          <details className="report-detail" open>
            <summary>
              <span>resident region deltas</span>
              <span className="pill neutral">
                {currentReport.llama_runtime.resident_region_deltas.length}
              </span>
            </summary>
            {currentReport.llama_runtime.resident_region_deltas.length === 0 ? (
              <p className="muted-text">no region deltas available</p>
            ) : (
              <div className="report-list">
                {currentReport.llama_runtime.resident_region_deltas.map((region) => (
                  <div className="report-item" key={`delta-${region.region_type}`}>
                    <div className="report-item-header">
                      <strong>{region.region_type}</strong>
                      <span className="pill neutral">
                        {formatSignedBytesDelta(region.resident_delta_bytes)}
                      </span>
                    </div>
                    <div className="report-path-list">
                      <div>before: {formatBytes(region.before_resident_bytes)}</div>
                      <div>after: {formatBytes(region.after_resident_bytes)}</div>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>
        </section>
      )}

      {currentReport.process_scan && (
        <section className="report-section">
          <div className="panel-title">process memory scanning</div>

          <div className="report-risk-block">
            <p>{currentReport.process_scan.summary}</p>
          </div>

          <div className="registry-lifecycle-summary">
            <span className={inspectionStatusClass(currentReport.process_scan.overall_status)}>
              overall {humanizeSnakeCase(currentReport.process_scan.overall_status)}
            </span>
            <span
              className={inspectionStatusClass(currentReport.process_scan.implementation_status)}
            >
              impl {humanizeSnakeCase(currentReport.process_scan.implementation_status)}
            </span>
          </div>

          <ReportGrid
            entries={[
              {
                label: "target process",
                value: currentReport.process_scan.target_process_kind,
              },
              {
                label: "target runtime pid",
                value: currentReport.process_scan.target_runtime_pid?.toString() || "none",
              },
              {
                label: "report platform",
                value: currentReport.process_scan.platform,
              },
              {
                label: "planned platforms",
                value: currentReport.process_scan.planned_platforms.join(", ") || "none",
              },
            ]}
          />

          <div className="report-risk-block">
            <p>{currentReport.process_scan.residual_risk_summary}</p>
          </div>

          <details className="report-detail" open>
            <summary>
              process scan phases ({currentReport.process_scan.phases.length})
            </summary>
            <div className="report-list">
              {currentReport.process_scan.phases.map((phase) => (
                <div className="report-item" key={phase.phase}>
                  <div className="report-item-header">
                    <strong>{humanizeSnakeCase(phase.phase)}</strong>
                    <span className={inspectionStatusClass(phase.status)}>
                      {humanizeSnakeCase(phase.status)}
                    </span>
                  </div>
                  <div className="report-path-list">
                    <div>method: {humanizeSnakeCase(phase.method)}</div>
                    <div>scope: {phase.scope_summary}</div>
                    <div>target pid: {phase.target_pid?.toString() || "none"}</div>
                    <div>
                      bytes scanned:{" "}
                      {phase.bytes_scanned === undefined || phase.bytes_scanned === null
                        ? "none"
                        : formatBytes(phase.bytes_scanned)}
                    </div>
                    <div>
                      regions scanned:{" "}
                      {phase.regions_scanned === undefined || phase.regions_scanned === null
                        ? "none"
                        : phase.regions_scanned}
                    </div>
                    <div>
                      regions skipped:{" "}
                      {phase.regions_skipped === undefined || phase.regions_skipped === null
                        ? "none"
                        : phase.regions_skipped}
                    </div>
                  </div>

                  <div className="registry-lifecycle-summary">
                    {phase.patterns.map((pattern) => (
                      <span key={`${phase.phase}-${pattern.pattern_kind}`}>
                        {humanizeSnakeCase(pattern.pattern_kind)}:{" "}
                        {humanizeSnakeCase(pattern.status)}
                      </span>
                    ))}
                  </div>

                  {phase.notes.length > 0 && (
                    <ul className="report-note-list">
                      {phase.notes.map((note, index) => (
                        <li key={`${phase.phase}-note-${index}`}>{note}</li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
            </div>
          </details>

          {currentReport.process_scan.notes.length > 0 && (
            <details className="report-detail">
              <summary>process scan notes ({currentReport.process_scan.notes.length})</summary>
              <ul className="report-note-list">
                {currentReport.process_scan.notes.map((note, index) => (
                  <li key={`process-scan-note-${index}`}>{note}</li>
                ))}
              </ul>
            </details>
          )}
        </section>
      )}

      {currentReport.platform_capability_matrix && (
        <section className="report-section">
          <div className="panel-title">platform capability matrix</div>

          <div className="report-risk-block">
            <p>{currentReport.platform_capability_matrix.summary}</p>
          </div>

          <div className="registry-lifecycle-summary">
            <span
              className={inspectionStatusClass(
                currentReport.platform_capability_matrix.matrix_status
              )}
            >
              status {humanizeSnakeCase(currentReport.platform_capability_matrix.matrix_status)}
            </span>
          </div>

          <ReportGrid
            entries={[
              {
                label: "platform",
                value: currentReport.platform_capability_matrix.scope_platform,
              },
              {
                label: "model id",
                value: currentReport.platform_capability_matrix.scope_model_id || "unknown",
              },
              {
                label: "runtime build profile",
                value:
                  currentReport.platform_capability_matrix.runtime_build_profile || "unknown",
              },
              {
                label: "gpu offload requested",
                value:
                  currentReport.platform_capability_matrix.gpu_offload_requested === undefined ||
                  currentReport.platform_capability_matrix.gpu_offload_requested === null
                    ? "unknown"
                    : currentReport.platform_capability_matrix.gpu_offload_requested
                      ? "true"
                      : "false",
              },
              {
                label: "track entries",
                value: String(currentReport.platform_capability_matrix.capabilities.length),
              },
            ]}
          />

          <details className="report-detail" open>
            <summary>
              track readiness ({currentReport.platform_capability_matrix.capabilities.length})
            </summary>
            {currentReport.platform_capability_matrix.capabilities.length === 0 ? (
              <p className="muted-text">no capability entries available</p>
            ) : (
              <div className="report-list">
                {currentReport.platform_capability_matrix.capabilities.map((capability) => (
                  <div className="report-item" key={capability.capability_id}>
                    <div className="report-item-header">
                      <strong>{capability.capability_label}</strong>
                      <span className={inspectionStatusClass(capability.current_status)}>
                        {humanizeSnakeCase(capability.current_status)}
                      </span>
                    </div>

                    <div className="report-path-list">
                      <div>track: {humanizeSnakeCase(capability.roadmap_track)}</div>
                      <div>evidence: {humanizeSnakeCase(capability.evidence_level)}</div>
                      <div>v1 blocker: {capability.v1_blocker ? "yes" : "no"}</div>
                      <div>boundary: {capability.claim_boundary}</div>
                      <div>{capability.summary}</div>
                      {capability.notes.map((note) => (
                        <div key={`${capability.capability_id}-${note}`}>{note}</div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </details>

          {currentReport.platform_capability_matrix.notes.length > 0 && (
            <details className="report-detail">
              <summary>
                matrix notes ({currentReport.platform_capability_matrix.notes.length})
              </summary>
              <ul className="report-note-list">
                {currentReport.platform_capability_matrix.notes.map((note, index) => (
                  <li key={`platform-capability-note-${index}`}>{note}</li>
                ))}
              </ul>
            </details>
          )}
        </section>
      )}

      {currentReport.memory_validation && (
        <section className="report-section">
          <div className="panel-title">memory validation harness</div>

          <div className="report-risk-block">
            <p>{currentReport.memory_validation.summary}</p>
          </div>

          <div className="registry-lifecycle-summary">
            <span className={inspectionStatusClass(currentReport.memory_validation.validation_status)}>
              status {humanizeSnakeCase(currentReport.memory_validation.validation_status)}
            </span>
            <span
              className={inspectionStatusClass(
                currentReport.memory_validation.canary_execution_status
              )}
            >
              canary {humanizeSnakeCase(currentReport.memory_validation.canary_execution_status)}
            </span>
            <span
              className={inspectionStatusClass(
                currentReport.memory_validation.process_scan_signal_status
              )}
            >
              process scan{" "}
              {humanizeSnakeCase(currentReport.memory_validation.process_scan_signal_status)}
            </span>
          </div>

          <ReportGrid
            entries={[
              {
                label: "scope",
                value: humanizeSnakeCase(currentReport.memory_validation.harness_scope),
              },
              {
                label: "best stage",
                value: currentReport.memory_validation.best_stage_label || "none",
              },
              {
                label: "best stage id",
                value: currentReport.memory_validation.best_stage_id || "none",
              },
              {
                label: "best stage kind",
                value: currentReport.memory_validation.best_stage_kind
                  ? humanizeSnakeCase(currentReport.memory_validation.best_stage_kind)
                  : "none",
              },
              {
                label: "best stage score",
                value: `${currentReport.memory_validation.best_stage_score}/100`,
              },
              {
                label: "best verdict",
                value: humanizeSnakeCase(currentReport.memory_validation.best_stage_verdict),
              },
            ]}
          />

          <details className="report-detail" open>
            <summary>
              validation history
              <span
                className={inspectionStatusClass(
                  currentReport.memory_validation_history.history_status
                )}
              >
                {humanizeSnakeCase(currentReport.memory_validation_history.history_status)}
              </span>
            </summary>

            <div className="report-risk-block">
              <p>{currentReport.memory_validation_history.summary}</p>
            </div>

            <div className="report-risk-block">
              <p>
                <strong>release gate:</strong>{" "}
                {currentReport.memory_validation_history.release_gate.summary}
              </p>
            </div>

            <div className="report-risk-block">
              <p>
                <strong>controlled canary history:</strong>{" "}
                {currentReport.memory_validation_history.controlled_canary_history.summary}
              </p>
            </div>

            <div className="report-risk-block">
              <p>
                <strong>cleanup stage recommendation:</strong>{" "}
                {currentReport.memory_validation_history.cleanup_stage_recommendation.summary}
              </p>
              <p>
                <strong>clean-stage claim:</strong>{" "}
                {
                  currentReport.memory_validation_history.cleanup_stage_recommendation
                    .clean_claim_summary
                }
              </p>
            </div>

            <ReportGrid
              entries={[
                {
                  label: "scope key",
                  value: currentReport.memory_validation_history.scope_key,
                },
                {
                  label: "model id",
                  value: currentReport.memory_validation_history.scope_model_id || "unknown",
                },
                {
                  label: "platform",
                  value: currentReport.memory_validation_history.scope_platform || "unknown",
                },
                {
                  label: "gpu offload requested",
                  value:
                    currentReport.memory_validation_history.scope_gpu_offload_requested ===
                      undefined ||
                    currentReport.memory_validation_history.scope_gpu_offload_requested === null
                      ? "unknown"
                      : currentReport.memory_validation_history.scope_gpu_offload_requested
                        ? "true"
                        : "false",
                },
                {
                  label: "runs recorded",
                  value: String(currentReport.memory_validation_history.runs_recorded),
                },
                {
                  label: "marker-detection runs",
                  value: String(currentReport.memory_validation_history.marker_detection_runs),
                },
                {
                  label: "clear canary runs",
                  value: String(currentReport.memory_validation_history.clear_canary_runs),
                },
                {
                  label: "inconclusive or failed",
                  value: String(
                    currentReport.memory_validation_history.inconclusive_or_failed_runs
                  ),
                },
                {
                  label: "strong or moderate runs",
                  value: String(currentReport.memory_validation_history.strong_or_moderate_runs),
                },
                {
                  label: "best score range",
                  value:
                    currentReport.memory_validation_history.best_stage_score_min === undefined ||
                    currentReport.memory_validation_history.best_stage_score_min === null ||
                    currentReport.memory_validation_history.best_stage_score_max === undefined ||
                    currentReport.memory_validation_history.best_stage_score_max === null
                      ? "unavailable"
                      : `${currentReport.memory_validation_history.best_stage_score_min}-${currentReport.memory_validation_history.best_stage_score_max}/100`,
                },
                {
                  label: "best score avg",
                  value:
                    currentReport.memory_validation_history.best_stage_score_avg === undefined ||
                    currentReport.memory_validation_history.best_stage_score_avg === null
                      ? "unavailable"
                      : `${currentReport.memory_validation_history.best_stage_score_avg.toFixed(1)}/100`,
                },
                {
                  label: "last recorded",
                  value: currentReport.memory_validation_history.last_recorded_at || "unknown",
                },
              ]}
            />

            <details className="report-detail">
              <summary>
                release gate
                <span
                  className={inspectionStatusClass(
                    currentReport.memory_validation_history.release_gate.gate_status
                  )}
                >
                  {humanizeSnakeCase(
                    currentReport.memory_validation_history.release_gate.gate_status
                  )}
                </span>
              </summary>

              <ReportGrid
                entries={[
                  {
                    label: "cleanup-stage gate",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.release_gate
                        .cleanup_stage_gate_status
                    ),
                  },
                  {
                    label: "controlled-canary gate",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.release_gate
                        .controlled_canary_gate_status
                    ),
                  },
                  {
                    label: "stage gate passed",
                    value: currentReport.memory_validation_history.release_gate.stage_gate_passed
                      ? "true"
                      : "false",
                  },
                  {
                    label: "canary gate passed",
                    value: currentReport.memory_validation_history.release_gate
                      .controlled_canary_gate_passed
                      ? "true"
                      : "false",
                  },
                  {
                    label: "min stage runs",
                    value: String(
                      currentReport.memory_validation_history.release_gate
                        .min_stage_runs_required
                    ),
                  },
                  {
                    label: "min clear canary runs",
                    value: String(
                      currentReport.memory_validation_history.release_gate
                        .min_clear_canary_runs_required
                    ),
                  },
                  {
                    label: "max marker detections",
                    value: String(
                      currentReport.memory_validation_history.release_gate
                        .max_marker_detection_runs_allowed_for_clean_claim
                    ),
                  },
                  {
                    label: "max worsened runs",
                    value: String(
                      currentReport.memory_validation_history.release_gate
                        .max_worsened_runs_allowed_for_clean_stage
                    ),
                  },
                  {
                    label: "max inconclusive runs",
                    value: String(
                      currentReport.memory_validation_history.release_gate
                        .max_inconclusive_runs_allowed_for_clean_stage
                    ),
                  },
                  {
                    label: "observed stage evidence",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.release_gate
                        .observed_stage_evidence_support_status
                    ),
                  },
                  {
                    label: "required stage evidence",
                    value:
                      currentReport.memory_validation_history.release_gate
                        .required_stage_evidence_support_statuses.length > 0
                        ? currentReport.memory_validation_history.release_gate.required_stage_evidence_support_statuses
                            .map((status) => humanizeSnakeCase(status))
                            .join(" or ")
                        : "unavailable",
                  },
                ]}
              />

              {currentReport.memory_validation_history.release_gate.notes.length > 0 && (
                <ul className="report-note-list">
                  {currentReport.memory_validation_history.release_gate.notes.map(
                    (note, index) => (
                      <li key={`memory-validation-history-release-gate-note-${index}`}>
                        {note}
                      </li>
                    )
                  )}
                </ul>
              )}
            </details>

            <details className="report-detail">
              <summary>
                controlled canary history
                <span
                  className={inspectionStatusClass(
                    currentReport.memory_validation_history.controlled_canary_history
                      .recommendation_status
                  )}
                >
                  {humanizeSnakeCase(
                    currentReport.memory_validation_history.controlled_canary_history
                      .recommendation_status
                  )}
                </span>
              </summary>

              <ReportGrid
                entries={[
                  {
                    label: "history status",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.controlled_canary_history
                        .history_status
                    ),
                  },
                  {
                    label: "runs with canary",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .runs_with_canary_requested
                    ),
                  },
                  {
                    label: "runs with completed passes",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .runs_with_completed_passes
                    ),
                  },
                  {
                    label: "requested passes",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .total_requested_passes
                    ),
                  },
                  {
                    label: "completed passes",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .total_completed_passes
                    ),
                  },
                  {
                    label: "failed passes",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .total_failed_passes
                    ),
                  },
                  {
                    label: "clear runs",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history.clear_runs
                    ),
                  },
                  {
                    label: "marker-detection runs",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .marker_detection_runs
                    ),
                  },
                  {
                    label: "mixed or inconclusive runs",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .mixed_or_inconclusive_runs
                    ),
                  },
                  {
                    label: "backend unsupported runs",
                    value: String(
                      currentReport.memory_validation_history.controlled_canary_history
                        .backend_unsupported_runs
                    ),
                  },
                  {
                    label: "latest execution",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.controlled_canary_history
                        .latest_execution_status
                    ),
                  },
                  {
                    label: "latest aggregate signal",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.controlled_canary_history
                        .latest_aggregate_signal_status
                    ),
                  },
                ]}
              />

              {currentReport.memory_validation_history.controlled_canary_history.notes.length >
                0 && (
                <ul className="report-note-list">
                  {currentReport.memory_validation_history.controlled_canary_history.notes.map(
                    (note, index) => (
                      <li key={`memory-validation-history-canary-note-${index}`}>{note}</li>
                    )
                  )}
                </ul>
              )}
            </details>

            <details className="report-detail">
              <summary>
                cleanup stage recommendation
                <span
                  className={inspectionStatusClass(
                    currentReport.memory_validation_history.cleanup_stage_recommendation
                      .recommendation_status
                  )}
                >
                  {humanizeSnakeCase(
                    currentReport.memory_validation_history.cleanup_stage_recommendation
                      .recommendation_status
                  )}
                </span>
              </summary>

              <ReportGrid
                entries={[
                  {
                    label: "clean-claim status",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .clean_claim_status
                    ),
                  },
                  {
                    label: "evidence support",
                    value: humanizeSnakeCase(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .evidence_support_status
                    ),
                  },
                  {
                    label: "recommended stage",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .stage_label || "none",
                  },
                  {
                    label: "stage id",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .stage_id || "none",
                  },
                  {
                    label: "stage kind",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .stage_kind
                        ? humanizeSnakeCase(
                            currentReport.memory_validation_history.cleanup_stage_recommendation
                              .stage_kind
                          )
                        : "none",
                  },
                  {
                    label: "runner-up stage",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .runner_up_stage_label || "none",
                  },
                  {
                    label: "runner-up kind",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .runner_up_stage_kind
                        ? humanizeSnakeCase(
                            currentReport.memory_validation_history.cleanup_stage_recommendation
                              .runner_up_stage_kind
                          )
                        : "none",
                  },
                  {
                    label: "compared stages",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .compared_stage_count
                    ),
                  },
                  {
                    label: "runs recorded",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .runs_recorded
                    ),
                  },
                  {
                    label: "avg validation score",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .avg_validation_score === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .avg_validation_score === null
                        ? "unavailable"
                        : `${currentReport.memory_validation_history.cleanup_stage_recommendation.avg_validation_score.toFixed(1)}/100`,
                  },
                  {
                    label: "effectiveness score",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .effectiveness_score === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .effectiveness_score === null
                        ? "unavailable"
                        : currentReport.memory_validation_history.cleanup_stage_recommendation.effectiveness_score.toFixed(
                            1
                          ),
                  },
                  {
                    label: "runner-up score",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .runner_up_effectiveness_score === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .runner_up_effectiveness_score === null
                        ? "unavailable"
                        : currentReport.memory_validation_history.cleanup_stage_recommendation.runner_up_effectiveness_score.toFixed(
                            1
                          ),
                  },
                  {
                    label: "effectiveness gap",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .effectiveness_gap === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .effectiveness_gap === null
                        ? "unavailable"
                        : currentReport.memory_validation_history.cleanup_stage_recommendation.effectiveness_gap.toFixed(
                            1
                          ),
                  },
                  {
                    label: "avg score gap",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .avg_validation_score_gap === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .avg_validation_score_gap === null
                        ? "unavailable"
                        : `${currentReport.memory_validation_history.cleanup_stage_recommendation.avg_validation_score_gap.toFixed(1)}/100`,
                  },
                  {
                    label: "marker-detection gap",
                    value:
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .marker_detection_gap === undefined ||
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .marker_detection_gap === null
                        ? "unavailable"
                        : String(
                            currentReport.memory_validation_history.cleanup_stage_recommendation
                              .marker_detection_gap
                          ),
                  },
                  {
                    label: "improved runs",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .improved_runs
                    ),
                  },
                  {
                    label: "unchanged runs",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .unchanged_runs
                    ),
                  },
                  {
                    label: "worsened runs",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .worsened_runs
                    ),
                  },
                  {
                    label: "inconclusive runs",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .inconclusive_runs
                    ),
                  },
                  {
                    label: "marker-detection runs",
                    value: String(
                      currentReport.memory_validation_history.cleanup_stage_recommendation
                        .marker_detection_runs
                    ),
                  },
                ]}
              />

              <p className="report-summary">
                {
                  currentReport.memory_validation_history.cleanup_stage_recommendation
                    .evidence_support_summary
                }
              </p>

              {currentReport.memory_validation_history.cleanup_stage_recommendation.notes.length >
                0 && (
                <ul className="report-note-list">
                  {currentReport.memory_validation_history.cleanup_stage_recommendation.notes.map(
                    (note, index) => (
                      <li key={`memory-validation-history-recommendation-note-${index}`}>
                        {note}
                      </li>
                    )
                  )}
                </ul>
              )}
            </details>

            <details className="report-detail">
              <summary>
                cleanup stage trends ({currentReport.memory_validation_history.stage_trends.length})
              </summary>
              {currentReport.memory_validation_history.stage_trends.length === 0 ? (
                <p className="muted-text">no repeated cleanup stage trends available yet</p>
              ) : (
                <div className="report-list">
                  {currentReport.memory_validation_history.stage_trends.map((trend) => (
                    <div className="report-item" key={`history-stage-trend-${trend.stage_id}`}>
                      <div className="report-item-header">
                        <strong>{trend.stage_label}</strong>
                        <span className={inspectionStatusClass(trend.latest_validation_verdict)}>
                          {trend.avg_validation_score.toFixed(1)}/100
                        </span>
                      </div>
                      <div className="report-path-list">
                        <div>stage id: {trend.stage_id}</div>
                        <div>kind: {humanizeSnakeCase(trend.stage_kind)}</div>
                        <div>
                          evidence support:{" "}
                          {humanizeSnakeCase(trend.evidence_support_status)}
                        </div>
                        <div>runs recorded: {trend.runs_recorded}</div>
                        <div>best score: {trend.best_validation_score}/100</div>
                        <div>improved runs: {trend.improved_runs}</div>
                        <div>unchanged runs: {trend.unchanged_runs}</div>
                        <div>worsened runs: {trend.worsened_runs}</div>
                        <div>inconclusive runs: {trend.inconclusive_runs}</div>
                        <div>strong or moderate runs: {trend.strong_or_moderate_runs}</div>
                        <div>marker-detection runs: {trend.marker_detection_runs}</div>
                        <div>clear marker support runs: {trend.clear_marker_support_runs}</div>
                        <div>helper scan runs: {trend.helper_scan_runs}</div>
                        <div>helper scan clear runs: {trend.helper_scan_clear_runs}</div>
                        <div>
                          helper scan marker detections:{" "}
                          {trend.helper_scan_marker_detection_runs}
                        </div>
                        <div>
                          cleanup signal strong runs: {trend.cleanup_signal_strong_runs}
                        </div>
                        <div>
                          cleanup signal partial runs: {trend.cleanup_signal_partial_runs}
                        </div>
                        <div>
                          cleanup signal limited runs: {trend.cleanup_signal_limited_runs}
                        </div>
                        <div>
                          cleanup signal runtime-global-only runs:
                          {" "}{trend.cleanup_signal_runtime_global_only_runs}
                        </div>
                        <div>
                          cleanup signal declared-only runs:
                          {" "}{trend.cleanup_signal_declared_only_runs}
                        </div>
                        <div>
                          cleanup signal scope unavailable runs:
                          {" "}{trend.cleanup_signal_scope_unavailable_runs}
                        </div>
                        <div>stage-local scan runs: {trend.stage_local_scan_runs}</div>
                        <div>
                          stage-local scan clear runs: {trend.stage_local_scan_clear_runs}
                        </div>
                        <div>
                          stage-local scan marker detections:{" "}
                          {trend.stage_local_scan_marker_detection_runs}
                        </div>
                        <div>
                          stage-local scan limited runs: {trend.stage_local_scan_limited_runs}
                        </div>
                        <div>
                          session fallback scan runs: {trend.session_fallback_scan_runs}
                        </div>
                        <div>
                          latest vram evidence:{" "}
                          {humanizeSnakeCase(trend.latest_vram_evidence_status)}
                        </div>
                        <div>
                          latest verdict:{" "}
                          {humanizeSnakeCase(trend.latest_validation_verdict)}
                        </div>
                        <div>
                          latest marker evidence:{" "}
                          {humanizeSnakeCase(trend.latest_marker_evidence_status)}
                        </div>
                        <div>
                          latest cleanup signal support:{" "}
                          {humanizeSnakeCase(trend.latest_cleanup_signal_support_status)}
                        </div>
                        <div>
                          latest cleanup signal scope:{" "}
                          {humanizeSnakeCase(trend.latest_cleanup_signal_support_scope_status)}
                        </div>
                        <div>
                          latest contributing cleanup signals:{" "}
                          {trend.latest_contributing_cleanup_signals.length > 0
                            ? trend.latest_contributing_cleanup_signals.join(", ")
                            : "none"}
                        </div>
                        <div>
                          latest process scan context:{" "}
                          {humanizeSnakeCase(trend.latest_process_scan_context_status)}
                        </div>
                        <div>
                          latest process scan scope:{" "}
                          {humanizeSnakeCase(trend.latest_process_scan_context_scope)}
                        </div>
                        <div>{trend.evidence_support_summary}</div>
                        <div>{trend.summary}</div>
                        {trend.notes.map((note) => (
                          <div key={`${trend.stage_id}-${note}`}>{note}</div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </details>

            <details className="report-detail">
              <summary>
                cleanup stage effectiveness (
                {
                  currentReport.memory_validation_history.cleanup_stage_effectiveness.stages
                    .length
                }
                )
              </summary>
              {currentReport.memory_validation_history.cleanup_stage_effectiveness.stages.length ===
              0 ? (
                <p className="muted-text">
                  no repeated cleanup-stage effectiveness summary available yet
                </p>
              ) : (
                <>
                  <ReportGrid
                    entries={[
                      {
                        label: "summary status",
                        value: humanizeSnakeCase(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .summary_status
                        ),
                      },
                      {
                        label: "consistently helpful",
                        value: String(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .consistently_helpful_count
                        ),
                      },
                      {
                        label: "promising but limited",
                        value: String(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .promising_but_limited_count
                        ),
                      },
                      {
                        label: "ineffective or regressive",
                        value: String(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .ineffective_or_regressive_count
                        ),
                      },
                      {
                        label: "marker persistent",
                        value: String(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .marker_persistent_count
                        ),
                      },
                      {
                        label: "waiting for history",
                        value: String(
                          currentReport.memory_validation_history.cleanup_stage_effectiveness
                            .waiting_for_repeated_history_count
                        ),
                      },
                    ]}
                  />

                  <p className="report-summary">
                    {
                      currentReport.memory_validation_history.cleanup_stage_effectiveness
                        .summary
                    }
                  </p>

                  <div className="report-list">
                    {currentReport.memory_validation_history.cleanup_stage_effectiveness.stages.map(
                      (stage) => (
                        <div
                          className="report-item"
                          key={`effectiveness-${stage.stage_id}`}
                        >
                          <div className="report-item-header">
                            <strong>{stage.stage_label}</strong>
                            <span className={inspectionStatusClass(stage.effectiveness_class)}>
                              {humanizeSnakeCase(stage.effectiveness_class)}
                            </span>
                          </div>
                          <div className="report-path-list">
                            <div>stage id: {stage.stage_id}</div>
                            <div>kind: {humanizeSnakeCase(stage.stage_kind)}</div>
                            <div>runs recorded: {stage.runs_recorded}</div>
                            <div>
                              avg validation score: {stage.avg_validation_score.toFixed(1)}/100
                            </div>
                            <div>
                              evidence support:{" "}
                              {humanizeSnakeCase(stage.evidence_support_status)}
                            </div>
                            <div>
                              cleanup signal scope:{" "}
                              {humanizeSnakeCase(stage.cleanup_signal_scope_status)}
                            </div>
                            <div>improved runs: {stage.improved_runs}</div>
                            <div>unchanged runs: {stage.unchanged_runs}</div>
                            <div>worsened runs: {stage.worsened_runs}</div>
                            <div>inconclusive runs: {stage.inconclusive_runs}</div>
                            <div>marker-detection runs: {stage.marker_detection_runs}</div>
                            <div>
                              stage-local clear runs: {stage.stage_local_scan_clear_runs}
                            </div>
                            <div>{stage.summary}</div>
                          </div>
                        </div>
                      )
                    )}
                  </div>

                  {currentReport.memory_validation_history.cleanup_stage_effectiveness.notes
                    .length > 0 && (
                    <ul className="report-note-list">
                      {currentReport.memory_validation_history.cleanup_stage_effectiveness.notes.map(
                        (note, index) => (
                          <li key={`memory-validation-effectiveness-note-${index}`}>
                            {note}
                          </li>
                        )
                      )}
                    </ul>
                  )}
                </>
              )}
            </details>

            {currentReport.memory_validation_history.notes.length > 0 && (
              <details className="report-detail">
                <summary>
                  history notes ({currentReport.memory_validation_history.notes.length})
                </summary>
                <ul className="report-note-list">
                  {currentReport.memory_validation_history.notes.map((note, index) => (
                    <li key={`memory-validation-history-note-${index}`}>{note}</li>
                  ))}
                </ul>
              </details>
            )}
          </details>

          <details className="report-detail" open>
            <summary>
              controlled canary helper
              <span
                className={inspectionStatusClass(
                  currentReport.memory_validation.controlled_canary_run.execution_status
                )}
              >
                {humanizeSnakeCase(
                  currentReport.memory_validation.controlled_canary_run.execution_status
                )}
              </span>
            </summary>

            <div className="report-risk-block">
              <p>{currentReport.memory_validation.controlled_canary_run.summary}</p>
            </div>

            <ReportGrid
              entries={[
                {
                  label: "requested passes",
                  value: String(
                    currentReport.memory_validation.controlled_canary_run.requested_passes
                  ),
                },
                {
                  label: "completed passes",
                  value: String(
                    currentReport.memory_validation.controlled_canary_run.completed_passes
                  ),
                },
                {
                  label: "failed passes",
                  value: String(
                    currentReport.memory_validation.controlled_canary_run.failed_passes
                  ),
                },
                {
                  label: "aggregate signal",
                  value: humanizeSnakeCase(
                    currentReport.memory_validation.controlled_canary_run
                      .aggregate_signal_status
                  ),
                },
                {
                  label: "aggregate scan status",
                  value: humanizeSnakeCase(
                    currentReport.memory_validation.controlled_canary_run
                      .aggregate_process_scan_status
                  ),
                },
                {
                  label: "canary id",
                  value: currentReport.memory_validation.controlled_canary_run.canary_id,
                },
                {
                  label: "selected pass",
                  value:
                    currentReport.memory_validation.controlled_canary_run.selected_pass_index ===
                      undefined ||
                    currentReport.memory_validation.controlled_canary_run.selected_pass_index ===
                      null
                      ? "none"
                      : String(
                          currentReport.memory_validation.controlled_canary_run
                            .selected_pass_index
                        ),
                },
                {
                  label: "runtime pid",
                  value:
                    currentReport.memory_validation.controlled_canary_run.runtime_pid?.toString() ||
                    "none",
                },
                {
                  label: "runtime endpoint",
                  value:
                    currentReport.memory_validation.controlled_canary_run.runtime_endpoint ||
                    "none",
                },
                {
                  label: "response bytes",
                  value:
                    currentReport.memory_validation.controlled_canary_run.response_bytes ===
                      undefined ||
                    currentReport.memory_validation.controlled_canary_run.response_bytes === null
                      ? "none"
                      : String(
                          currentReport.memory_validation.controlled_canary_run.response_bytes
                        ),
                },
                {
                  label: "scan status",
                  value: humanizeSnakeCase(
                    currentReport.memory_validation.controlled_canary_run.process_scan
                      .overall_status
                  ),
                },
              ]}
            />

            <div className="report-risk-block">
              <p>
                <strong>selection reason:</strong>{" "}
                {currentReport.memory_validation.controlled_canary_run.selection_reason}
              </p>
            </div>

            <details className="report-detail">
              <summary>
                canary passes (
                {currentReport.memory_validation.controlled_canary_run.passes.length})
              </summary>
              {currentReport.memory_validation.controlled_canary_run.passes.length === 0 ? (
                <p className="muted-text">no individual canary passes available</p>
              ) : (
                <div className="report-list">
                  {currentReport.memory_validation.controlled_canary_run.passes.map((pass) => (
                    <div className="report-item" key={`canary-pass-${pass.pass_index}`}>
                      <div className="report-item-header">
                        <strong>pass {pass.pass_index}</strong>
                        <span className={inspectionStatusClass(pass.execution_status)}>
                          {humanizeSnakeCase(pass.execution_status)}
                        </span>
                      </div>
                      <div className="report-path-list">
                        <div>canary id: {pass.canary_id}</div>
                        <div>runtime pid: {pass.runtime_pid?.toString() || "none"}</div>
                        <div>runtime endpoint: {pass.runtime_endpoint || "none"}</div>
                        <div>
                          response bytes:{" "}
                          {pass.response_bytes === undefined || pass.response_bytes === null
                            ? "none"
                            : String(pass.response_bytes)}
                        </div>
                        <div>
                          process scan: {humanizeSnakeCase(pass.process_scan.overall_status)}
                        </div>
                        <div>{pass.summary}</div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </details>

            <details className="report-detail">
              <summary>
                canary process scan phases (
                {
                  currentReport.memory_validation.controlled_canary_run.process_scan.phases.length
                }
                )
              </summary>
              {currentReport.memory_validation.controlled_canary_run.process_scan.phases.length ===
              0 ? (
                <p className="muted-text">no canary process scan phases available</p>
              ) : (
                <div className="report-list">
                  {currentReport.memory_validation.controlled_canary_run.process_scan.phases.map(
                    (phase) => (
                      <div className="report-item" key={`canary-phase-${phase.phase}`}>
                        <div className="report-item-header">
                          <strong>{humanizeSnakeCase(phase.phase)}</strong>
                          <span className={inspectionStatusClass(phase.status)}>
                            {humanizeSnakeCase(phase.status)}
                          </span>
                        </div>
                        <div className="report-path-list">
                          <div>method: {humanizeSnakeCase(phase.method)}</div>
                          <div>scope: {phase.scope_summary}</div>
                          <div>target pid: {phase.target_pid?.toString() || "none"}</div>
                        </div>
                      </div>
                    )
                  )}
                </div>
              )}
            </details>

            {currentReport.memory_validation.controlled_canary_run.notes.length > 0 && (
              <details className="report-detail">
                <summary>
                  canary notes (
                  {currentReport.memory_validation.controlled_canary_run.notes.length})
                </summary>
                <ul className="report-note-list">
                  {currentReport.memory_validation.controlled_canary_run.notes.map(
                    (note, index) => (
                      <li key={`controlled-canary-note-${index}`}>{note}</li>
                    )
                  )}
                </ul>
              </details>
            )}
          </details>

          <details className="report-detail" open>
            <summary>
              memory validation stage scorecards (
              {currentReport.memory_validation.stage_scorecards.length})
            </summary>
            {currentReport.memory_validation.stage_scorecards.length === 0 ? (
              <p className="muted-text">no validation stage scorecards available</p>
            ) : (
              <div className="report-list">
                {currentReport.memory_validation.stage_scorecards.map((scorecard) => (
                  <div className="report-item" key={`validation-${scorecard.stage_id}`}>
                    <div className="report-item-header">
                      <strong>{scorecard.stage_label}</strong>
                      <span className={inspectionStatusClass(scorecard.validation_verdict)}>
                        {scorecard.validation_score}/100
                      </span>
                    </div>

                    <div className="report-path-list">
                      <div>verdict: {humanizeSnakeCase(scorecard.validation_verdict)}</div>
                      <div>stage id: {scorecard.stage_id}</div>
                      <div>kind: {humanizeSnakeCase(scorecard.stage_kind)}</div>
                      <div>action: {humanizeSnakeCase(scorecard.action_status)}</div>
                      <div>
                        vram evidence: {humanizeSnakeCase(scorecard.vram_evidence_status)}
                      </div>
                      <div>
                        marker evidence: {humanizeSnakeCase(scorecard.marker_evidence_status)}
                      </div>
                      <div>
                        process scan context:{" "}
                        {humanizeSnakeCase(scorecard.process_scan_context_status)}
                      </div>
                      <div>
                        process scan scope:{" "}
                        {humanizeSnakeCase(scorecard.process_scan_context_scope)}
                      </div>
                      <div>
                        cleanup signal support:{" "}
                        {humanizeSnakeCase(scorecard.cleanup_signal_support_status)}
                      </div>
                      <div>
                        cleanup signal scope:{" "}
                        {humanizeSnakeCase(scorecard.cleanup_signal_support_scope_status)}
                      </div>
                      <div>
                        contributing cleanup signals:{" "}
                        {scorecard.contributing_cleanup_signals.length > 0
                          ? scorecard.contributing_cleanup_signals.join(", ")
                          : "none"}
                      </div>
                      <div>
                        controlled canary:{" "}
                        {humanizeSnakeCase(scorecard.controlled_canary_signal_status)}
                      </div>
                      <div>{scorecard.summary}</div>
                      <div>{scorecard.cleanup_signal_support_summary}</div>
                      <div>{scorecard.cleanup_signal_support_scope_summary}</div>
                    </div>

                    {scorecard.strengths.length > 0 && (
                      <details className="report-detail">
                        <summary>strengths ({scorecard.strengths.length})</summary>
                        <ul className="report-note-list">
                          {scorecard.strengths.map((strength, index) => (
                            <li key={`${scorecard.stage_id}-strength-${index}`}>{strength}</li>
                          ))}
                        </ul>
                      </details>
                    )}

                    {scorecard.gaps.length > 0 && (
                      <details className="report-detail">
                        <summary>gaps ({scorecard.gaps.length})</summary>
                        <ul className="report-note-list">
                          {scorecard.gaps.map((gap, index) => (
                            <li key={`${scorecard.stage_id}-gap-${index}`}>{gap}</li>
                          ))}
                        </ul>
                      </details>
                    )}
                  </div>
                ))}
              </div>
            )}
          </details>

          {currentReport.memory_validation.notes.length > 0 && (
            <details className="report-detail">
              <summary>
                memory validation notes ({currentReport.memory_validation.notes.length})
              </summary>
              <ul className="report-note-list">
                {currentReport.memory_validation.notes.map((note, index) => (
                  <li key={`memory-validation-note-${index}`}>{note}</li>
                ))}
              </ul>
            </details>
          )}
        </section>
      )}

      {currentReport.retrieval && (
        <section className="report-section">
          <div className="panel-title">retrieval provenance</div>
          <ReportGrid
            entries={[
              {
                label: "corpus",
                value: `${currentReport.retrieval.corpus_name} (${currentReport.retrieval.corpus_id})`,
              },
              {
                label: "mode",
                value: humanizeSnakeCase(currentReport.retrieval.retrieval_mode),
              },
              {
                label: "grounded turns",
                value: String(currentReport.retrieval.grounded_turns),
              },
              {
                label: "retrieved chunks",
                value: String(currentReport.retrieval.retrieved_chunks),
              },
              {
                label: "top k",
                value: String(currentReport.retrieval.top_k),
              },
              {
                label: "context injected",
                value: formatBoolean(currentReport.retrieval.context_injected),
              },
            ]}
          />

          <div className="report-risk-block">
            <p>
              <strong>latest grounded query:</strong> {currentReport.retrieval.query}
            </p>
            <p>
              <strong>source files touched:</strong> {currentReport.retrieval.source_paths.length}
            </p>
            {currentReport.retrieval.page_hits.length > 0 && (
              <p>
                <strong>page-level hits:</strong> {currentReport.retrieval.page_hits.length}
              </p>
            )}
          </div>
        </section>
      )}

      <section className="report-section">
        <div className="panel-title">cleanup</div>
        <ReportGrid
          entries={[
            {
              label: "attempted",
              value: formatBoolean(currentReport.cleanup.attempted),
            },
            {
              label: "successful",
              value: formatBoolean(currentReport.cleanup.successful),
            },
            {
              label: "workspace deleted",
              value: formatBoolean(currentReport.cleanup.workspace_deleted),
            },
            { label: "files removed", value: String(currentReport.cleanup.files_removed) },
            {
              label: "directories removed",
              value: String(currentReport.cleanup.directories_removed),
            },
            {
              label: "cleanup error",
              value: currentReport.cleanup.error || "none",
            },
          ]}
        />
      </section>

      <details className="report-detail" open>
        <summary>
          <span>artifacts detected</span>
          <span className="pill neutral">{currentReport.cleanup.artifacts_detected.length}</span>
        </summary>
        {currentReport.cleanup.artifacts_detected.length === 0 ? (
          <p className="muted-text">no artifacts detected</p>
        ) : (
          <div className="report-list">
            {currentReport.cleanup.artifacts_detected.map((artifact) => (
              <div
                className="report-item"
                key={`${artifact.path}-${artifact.kind}-${artifact.size_bytes}`}
              >
                <div className="report-item-header">
                  <strong>{artifact.kind}</strong>
                  <span className="pill neutral">{formatBytes(artifact.size_bytes)}</span>
                </div>
                <div className="report-path-list">
                  <div>{artifact.path}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </details>

      <details className="report-detail" open>
        <summary>
          <span>sanitization operations</span>
          <span className="pill neutral">
            {currentReport.cleanup.sanitization_operations.length}
          </span>
        </summary>
        {currentReport.cleanup.sanitization_operations.length === 0 ? (
          <p className="muted-text">no sanitization operations recorded</p>
        ) : (
          <div className="audit-list">
            {currentReport.cleanup.sanitization_operations.map((operation, index) => (
              <details className="audit-item" key={`${operation.operation}-${index}`}>
                <summary>
                  <code>{operation.operation}</code>
                  <span className={statusClass(operation.status)}>{operation.status}</span>
                </summary>
                <p>{operation.details}</p>
              </details>
            ))}
          </div>
        )}
      </details>

      <section className="report-section">
        <div className="panel-title">residual risks</div>
        <div className="report-risk-block">
          <p>{currentReport.residual_risk}</p>
          {currentReport.session_profile?.active_runtime_residual_risk && (
            <p>{currentReport.session_profile.active_runtime_residual_risk}</p>
          )}
        </div>
      </section>

      {showRawReport && <pre>{rawReport}</pre>}
    </div>
  );
}
