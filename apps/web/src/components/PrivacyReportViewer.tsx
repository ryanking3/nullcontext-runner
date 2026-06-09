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
              <strong>cleanup boundary:</strong> {currentReport.llama_runtime.cleanup_summary}
            </p>
            <p>
              <strong>runtime-specific residual risk:</strong>{" "}
              {currentReport.llama_runtime.residual_risk_summary}
            </p>
          </div>

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
                {currentReport.llama_runtime.introspection.notes.map((note) => (
                  <p key={note}>{note}</p>
                ))}
              </div>
            )}

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
