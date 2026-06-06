import type {
  RegistryModeFilter,
  RegistryOutcomeFilter,
  RegistrySortOrder,
  SessionIndexEntry,
  SessionLifecycleActionResponse,
} from "../appTypes";
import {
  formatBoolean,
  formatTimestamp,
  humanizeSnakeCase,
  lifecycleStateClass,
  shortId,
} from "../appUtils";

export function SessionRegistryDrawer({
  open,
  onRefresh,
  onClose,
  registryLoadedAt,
  registryQuery,
  onRegistryQueryChange,
  registryModeFilter,
  onRegistryModeFilterChange,
  registryOutcomeFilter,
  onRegistryOutcomeFilterChange,
  registrySortOrder,
  onRegistrySortOrderChange,
  filteredSessions,
  sessions,
  latestSession,
  latestReportableSession,
  onClearFilters,
  onOpenLatestReport,
  selectedSessionId,
  onSelectSession,
  selectedSession,
  selectedLifecycleResult,
  registryActionMessage,
  registryActionFailed,
  retentionPolicyDraft,
  onRetentionPolicyDraftChange,
  retentionMinutesDraft,
  onRetentionMinutesDraftChange,
  registryActionPending,
  onSaveRetentionPolicy,
  onOpenSessionReport,
  onRunLifecycleAction,
}: {
  open: boolean;
  onRefresh: () => void;
  onClose: () => void;
  registryLoadedAt: string;
  registryQuery: string;
  onRegistryQueryChange: (value: string) => void;
  registryModeFilter: RegistryModeFilter;
  onRegistryModeFilterChange: (value: RegistryModeFilter) => void;
  registryOutcomeFilter: RegistryOutcomeFilter;
  onRegistryOutcomeFilterChange: (value: RegistryOutcomeFilter) => void;
  registrySortOrder: RegistrySortOrder;
  onRegistrySortOrderChange: (value: RegistrySortOrder) => void;
  filteredSessions: SessionIndexEntry[];
  sessions: SessionIndexEntry[];
  latestSession?: SessionIndexEntry | null;
  latestReportableSession?: SessionIndexEntry | null;
  onClearFilters: () => void;
  onOpenLatestReport: () => void;
  selectedSessionId: string;
  onSelectSession: (sessionId: string) => void;
  selectedSession: SessionIndexEntry | null;
  selectedLifecycleResult: SessionLifecycleActionResponse | null;
  registryActionMessage: string;
  registryActionFailed: boolean;
  retentionPolicyDraft: string;
  onRetentionPolicyDraftChange: (value: string) => void;
  retentionMinutesDraft: string;
  onRetentionMinutesDraftChange: (value: string) => void;
  registryActionPending: string | null;
  onSaveRetentionPolicy: (sessionId: string) => void;
  onOpenSessionReport: (sessionId: string) => void;
  onRunLifecycleAction: (sessionId: string, action: "reconcile" | "cleanup") => void;
}) {
  const selectedSessionIsActive = selectedSession?.lifecycle.state === "active";
  const latestSessionIsActive = latestSession?.lifecycle.state === "active";

  return (
    <aside className={`registry-drawer${open ? " open" : ""}`}>
      <div className="drawer-header">
        <div>
          <h3>session registry</h3>
          <p>browse retained sessions and open stored reports without packing the sidebar</p>
        </div>
        <div className="drawer-actions">
          <button className="ghost-button" onClick={onRefresh}>
            refresh
          </button>
          <button className="ghost-button" onClick={onClose}>
            close
          </button>
        </div>
      </div>

      <div className="drawer-body registry-drawer-body">
        <section className="panel registry-list-panel">
          <div className="panel-header">
            <div className="panel-title">sessions</div>
            <span className="mini-status">loaded:{registryLoadedAt}</span>
          </div>

          <div className="registry-toolbar">
            <label>
              search
              <input
                type="search"
                value={registryQuery}
                onChange={(event) => onRegistryQueryChange(event.target.value)}
                placeholder="session id, backend, path..."
              />
            </label>

            <div className="registry-filter-row">
              <label>
                mode
                <select
                  value={registryModeFilter}
                  onChange={(event) =>
                    onRegistryModeFilterChange(event.target.value as RegistryModeFilter)
                  }
                >
                  <option value="all">all</option>
                  <option value="secure">secure</option>
                  <option value="standard">standard</option>
                  <option value="air-gapped">air-gapped</option>
                </select>
              </label>

              <label>
                outcome
                <select
                  value={registryOutcomeFilter}
                  onChange={(event) =>
                    onRegistryOutcomeFilterChange(event.target.value as RegistryOutcomeFilter)
                  }
                >
                  <option value="all">all</option>
                  <option value="cleanup-failed">cleanup failed</option>
                  <option value="workspace-retained">workspace retained</option>
                  <option value="artifacts">artifacts &gt; 0</option>
                  <option value="history-stored">history stored</option>
                </select>
              </label>

              <label>
                sort
                <select
                  value={registrySortOrder}
                  onChange={(event) =>
                    onRegistrySortOrderChange(event.target.value as RegistrySortOrder)
                  }
                >
                  <option value="newest">newest first</option>
                  <option value="oldest">oldest first</option>
                </select>
              </label>
            </div>

            <div className="registry-chip-row">
              <button
                className={
                  registryOutcomeFilter === "cleanup-failed" ? "selected chip-button" : "chip-button"
                }
                onClick={() =>
                  onRegistryOutcomeFilterChange(
                    registryOutcomeFilter === "cleanup-failed" ? "all" : "cleanup-failed"
                  )
                }
              >
                cleanup failed
              </button>
              <button
                className={
                  registryOutcomeFilter === "workspace-retained"
                    ? "selected chip-button"
                    : "chip-button"
                }
                onClick={() =>
                  onRegistryOutcomeFilterChange(
                    registryOutcomeFilter === "workspace-retained"
                      ? "all"
                      : "workspace-retained"
                  )
                }
              >
                workspace retained
              </button>
              <button
                className={
                  registryOutcomeFilter === "artifacts" ? "selected chip-button" : "chip-button"
                }
                onClick={() =>
                  onRegistryOutcomeFilterChange(
                    registryOutcomeFilter === "artifacts" ? "all" : "artifacts"
                  )
                }
              >
                artifacts &gt; 0
              </button>
              <button
                className={
                  registryOutcomeFilter === "history-stored"
                    ? "selected chip-button"
                    : "chip-button"
                }
                onClick={() =>
                  onRegistryOutcomeFilterChange(
                    registryOutcomeFilter === "history-stored" ? "all" : "history-stored"
                  )
                }
              >
                history stored
              </button>
            </div>

            <div className="registry-toolbar-meta">
              <span>
                showing {filteredSessions.length} of {sessions.length}
              </span>
              <div className="registry-toolbar-actions">
                <button className="ghost-button" onClick={onClearFilters}>
                  clear filters
                </button>
                <button
                  className="ghost-button"
                  disabled={!latestReportableSession}
                  onClick={onOpenLatestReport}
                  title={
                    !latestReportableSession && latestSessionIsActive
                      ? "The newest retained session is still active, so its report is not available yet."
                      : undefined
                  }
                >
                  latest report
                </button>
              </div>
            </div>
          </div>

          {sessions.length === 0 ? (
            <p className="muted-text">no persistent sessions</p>
          ) : filteredSessions.length === 0 ? (
            <p className="muted-text">no sessions match the current registry filters</p>
          ) : (
            <div className="session-list registry-session-list">
              {filteredSessions.map((session) => (
                <button
                  className={
                    selectedSessionId === session.session_id
                      ? "session-item selected"
                      : "session-item"
                  }
                  key={session.session_id}
                  onClick={() => onSelectSession(session.session_id)}
                >
                  <div className="registry-session-header">
                    <span>{shortId(session.session_id)}</span>
                    <span className={lifecycleStateClass(session.lifecycle.state)}>
                      {humanizeSnakeCase(session.lifecycle.state)}
                    </span>
                  </div>
                  <div className="registry-session-meta">
                    <small>{session.security_mode}</small>
                    <small>{new Date(session.started_at).toLocaleString()}</small>
                  </div>
                  {session.model_name && <small>{session.model_name}</small>}
                  <small>{humanizeSnakeCase(session.lifecycle.retention_policy)}</small>
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="panel registry-detail-panel">
          <div className="panel-header">
            <div className="panel-title">details</div>
            {selectedSession && <span className="mini-status">id:{shortId(selectedSession.session_id)}</span>}
          </div>

          {!selectedSession ? (
            <p className="muted-text">
              {filteredSessions.length === 0 && sessions.length > 0
                ? "adjust the registry filters to inspect a matching session"
                : "select a persistent session to inspect its metadata"}
            </p>
          ) : (
            <>
              <div className="registry-lifecycle-summary">
                <span className={lifecycleStateClass(selectedSession.lifecycle.state)}>
                  {humanizeSnakeCase(selectedSession.lifecycle.state)}
                </span>
                <span className="pill neutral">
                  {humanizeSnakeCase(selectedSession.lifecycle.retention_policy)}
                </span>
              </div>

              {registryActionMessage && (
                <div
                  className={
                    registryActionFailed ? "registry-action-banner failed" : "registry-action-banner"
                  }
                >
                  {registryActionMessage}
                </div>
              )}

              {selectedLifecycleResult && !registryActionFailed && (
                <div className="config-summary">
                  <span>
                    latest action state:{" "}
                    {humanizeSnakeCase(selectedLifecycleResult.lifecycle_state)}
                  </span>
                  <span>
                    report exists: {formatBoolean(selectedLifecycleResult.report_exists)}
                  </span>
                  <span>
                    workspace exists: {formatBoolean(selectedLifecycleResult.workspace_exists)}
                  </span>
                  <span>
                    updated:{" "}
                    {selectedLifecycleResult.updated_at
                      ? formatTimestamp(selectedLifecycleResult.updated_at)
                      : "unknown"}
                  </span>
                  {selectedLifecycleResult.state_note && (
                    <span>note: {selectedLifecycleResult.state_note}</span>
                  )}
                </div>
              )}

              <dl className="registry-detail-grid">
                <dt>started</dt>
                <dd>{new Date(selectedSession.started_at).toLocaleString()}</dd>
                <dt>lifecycle state</dt>
                <dd>{humanizeSnakeCase(selectedSession.lifecycle.state)}</dd>
                <dt>retention policy</dt>
                <dd>{humanizeSnakeCase(selectedSession.lifecycle.retention_policy)}</dd>
                <dt>retention deadline</dt>
                <dd>
                  {selectedSession.lifecycle.retention_deadline
                    ? formatTimestamp(selectedSession.lifecycle.retention_deadline)
                    : "none"}
                </dd>
                <dt>cleanup requested</dt>
                <dd>
                  {selectedSession.lifecycle.cleanup_requested_at
                    ? formatTimestamp(selectedSession.lifecycle.cleanup_requested_at)
                    : "not requested"}
                </dd>
                <dt>cleanup completed</dt>
                <dd>
                  {selectedSession.lifecycle.cleanup_completed_at
                    ? formatTimestamp(selectedSession.lifecycle.cleanup_completed_at)
                    : "not completed"}
                </dd>
                <dt>cleanup reason</dt>
                <dd>
                  {selectedSession.lifecycle.cleanup_reason
                    ? humanizeSnakeCase(selectedSession.lifecycle.cleanup_reason)
                    : "none"}
                </dd>
                <dt>state note</dt>
                <dd>{selectedSession.lifecycle.state_note || "none"}</dd>
                <dt>lifecycle updated</dt>
                <dd>
                  {selectedSession.lifecycle.updated_at
                    ? formatTimestamp(selectedSession.lifecycle.updated_at)
                    : "unknown"}
                </dd>
                <dt>mode</dt>
                <dd>{selectedSession.security_mode}</dd>
                <dt>prompt source</dt>
                <dd>{selectedSession.prompt_source}</dd>
                <dt>history stored</dt>
                <dd>{selectedSession.history_stored ? "yes" : "no"}</dd>
                <dt>backend</dt>
                <dd>{selectedSession.backend}</dd>
                <dt>model id</dt>
                <dd>{selectedSession.model_id || "legacy/default"}</dd>
                <dt>model name</dt>
                <dd>{selectedSession.model_name || "unknown"}</dd>
                <dt>artifacts</dt>
                <dd>{selectedSession.artifacts_detected}</dd>
                <dt>cleanup attempted</dt>
                <dd>{selectedSession.cleanup_attempted ? "yes" : "no"}</dd>
                <dt>cleanup successful</dt>
                <dd>{selectedSession.cleanup_successful ? "yes" : "no"}</dd>
                <dt>workspace deleted</dt>
                <dd>{selectedSession.workspace_deleted ? "yes" : "no"}</dd>
                <dt>workspace</dt>
                <dd className="registry-path">{selectedSession.workspace}</dd>
                <dt>model path</dt>
                <dd className="registry-path">{selectedSession.model_path}</dd>
                <dt>report path</dt>
                <dd className="registry-path">{selectedSession.report_path}</dd>
                <dt>workspace exists</dt>
                <dd>
                  {formatBoolean(
                    selectedLifecycleResult?.workspace_exists ?? selectedSession.workspace_exists
                  )}
                </dd>
                <dt>report exists</dt>
                <dd>
                  {formatBoolean(
                    selectedLifecycleResult?.report_exists ?? selectedSession.report_exists
                  )}
                </dd>
              </dl>

              <section className="registry-retention-controls">
                <div className="panel-title">retention policy</div>
                <div className="registry-filter-row">
                  <label>
                    policy
                    <select
                      value={retentionPolicyDraft}
                      onChange={(event) => onRetentionPolicyDraftChange(event.target.value)}
                      disabled={registryActionPending !== null}
                    >
                      <option value="retain_until_manual_cleanup">manual cleanup</option>
                      <option value="retain_for_duration">timed retention</option>
                    </select>
                  </label>

                  {retentionPolicyDraft === "retain_for_duration" && (
                    <label>
                      minutes
                      <input
                        type="number"
                        min={1}
                        step={1}
                        value={retentionMinutesDraft}
                        onChange={(event) => onRetentionMinutesDraftChange(event.target.value)}
                        disabled={registryActionPending !== null}
                      />
                    </label>
                  )}
                </div>

                <div className="registry-retention-meta">
                  <span>
                    current deadline:{" "}
                    {selectedSession.lifecycle.retention_deadline
                      ? formatTimestamp(selectedSession.lifecycle.retention_deadline)
                      : "none"}
                  </span>
                  <button
                    onClick={() => onSaveRetentionPolicy(selectedSession.session_id)}
                    disabled={registryActionPending !== null}
                  >
                    {registryActionPending === "retention" ? "saving..." : "save retention policy"}
                  </button>
                </div>
              </section>

              <div className="registry-actions">
                <button
                  onClick={() => onOpenSessionReport(selectedSession.session_id)}
                  disabled={selectedSessionIsActive || !selectedSession.report_exists}
                  title={
                    selectedSessionIsActive
                      ? "End + Sanitize first. Active chat reports are written when the session ends."
                      : !selectedSession.report_exists
                        ? "No saved report is currently available for this retained session."
                      : undefined
                  }
                >
                  open report in inspector
                </button>
                <button
                  onClick={() => onRunLifecycleAction(selectedSession.session_id, "reconcile")}
                  disabled={registryActionPending !== null || selectedSessionIsActive}
                  title={
                    selectedSessionIsActive
                      ? "End + Sanitize first. Reconciliation only applies after the retained chat is no longer live."
                      : undefined
                  }
                >
                  {registryActionPending === "reconcile" ? "reconciling..." : "reconcile"}
                </button>
                <button
                  className="danger-button"
                  onClick={() => onRunLifecycleAction(selectedSession.session_id, "cleanup")}
                  disabled={registryActionPending !== null || selectedSessionIsActive}
                  title={
                    selectedSessionIsActive
                      ? "End + Sanitize first. Lifecycle cleanup only runs after the retained chat is no longer live."
                      : undefined
                  }
                >
                  {registryActionPending === "cleanup" ? "cleaning up..." : "cleanup now"}
                </button>
              </div>

              <p className="microcopy">
                Registry browsing stays separate from the live runtime shell so report inspection and
                lifecycle actions don&apos;t compete with conversation and runtime controls.
              </p>
              {selectedSessionIsActive && (
                <p className="microcopy">
                  This retained chat is still active, so its privacy report will appear only after
                  End + Sanitize completes.
                </p>
              )}
            </>
          )}
        </section>
      </div>
    </aside>
  );
}
