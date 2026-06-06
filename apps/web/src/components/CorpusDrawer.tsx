import { CorpusReportViewer } from "./CorpusReportViewer";
import type {
  CorpusIndexEntry,
  CorpusIngestionReport,
  CorpusLifecycleActionResponse,
} from "../appTypes";
import {
  formatBoolean,
  formatBytes,
  formatTimestamp,
  humanizeSnakeCase,
  lifecycleStateClass,
  shortId,
} from "../appUtils";

export function CorpusDrawer({
  open,
  onRefresh,
  onClose,
  corporaLoadedAt,
  corpusQuery,
  onCorpusQueryChange,
  corpusLoadError,
  corpora,
  filteredCorpora,
  selectedCorpusId,
  onSelectCorpus,
  selectedCorpus,
  selectedCorpusLifecycleResult,
  corpusActionMessage,
  corpusActionFailed,
  corpusRetentionPolicyDraft,
  onCorpusRetentionPolicyDraftChange,
  corpusRetentionMinutesDraft,
  onCorpusRetentionMinutesDraftChange,
  corpusActionPending,
  onSaveCorpusRetentionPolicy,
  onUseCorpusForOneShot,
  onOpenCorpusReport,
  onRunCorpusLifecycleAction,
  selectedCorpusReport,
  currentCorpusReport,
  corpusIngestMessage,
  corpusIngestFailed,
  corpusIngestName,
  onCorpusIngestNameChange,
  corpusIngestPaths,
  onCorpusIngestPathsChange,
  corpusUploadDragActive,
  corpusIngestPending,
  onCorpusUploadDragActiveChange,
  onCorpusUploadSelection,
  corpusUploadInputKey,
  corpusUploadFiles,
  corpusIngestPersistent,
  onCorpusIngestPersistentChange,
  corpusIngestOcrEnabled,
  onCorpusIngestOcrEnabledChange,
  onIngestCorpusFromPaths,
  onIngestUploadedCorpus,
  corpusUploadProgressPercent,
  corpusUploadProgressLabel,
  lastIngestedCorpusReport,
}: {
  open: boolean;
  onRefresh: () => void;
  onClose: () => void;
  corporaLoadedAt: string;
  corpusQuery: string;
  onCorpusQueryChange: (value: string) => void;
  corpusLoadError: string;
  corpora: CorpusIndexEntry[];
  filteredCorpora: CorpusIndexEntry[];
  selectedCorpusId: string;
  onSelectCorpus: (corpusId: string) => void;
  selectedCorpus: CorpusIndexEntry | null;
  selectedCorpusLifecycleResult: CorpusLifecycleActionResponse | null;
  corpusActionMessage: string;
  corpusActionFailed: boolean;
  corpusRetentionPolicyDraft: string;
  onCorpusRetentionPolicyDraftChange: (value: string) => void;
  corpusRetentionMinutesDraft: string;
  onCorpusRetentionMinutesDraftChange: (value: string) => void;
  corpusActionPending: string | null;
  onSaveCorpusRetentionPolicy: (corpusId: string) => void;
  onUseCorpusForOneShot: (corpusId: string) => void;
  onOpenCorpusReport: (corpusId: string) => void;
  onRunCorpusLifecycleAction: (corpusId: string, action: "reconcile" | "cleanup") => void;
  selectedCorpusReport: string;
  currentCorpusReport: CorpusIngestionReport | null;
  corpusIngestMessage: string;
  corpusIngestFailed: boolean;
  corpusIngestName: string;
  onCorpusIngestNameChange: (value: string) => void;
  corpusIngestPaths: string;
  onCorpusIngestPathsChange: (value: string) => void;
  corpusUploadDragActive: boolean;
  corpusIngestPending: boolean;
  onCorpusUploadDragActiveChange: (active: boolean) => void;
  onCorpusUploadSelection: (files: File[]) => void;
  corpusUploadInputKey: number;
  corpusUploadFiles: File[];
  corpusIngestPersistent: boolean;
  onCorpusIngestPersistentChange: (checked: boolean) => void;
  corpusIngestOcrEnabled: boolean;
  onCorpusIngestOcrEnabledChange: (checked: boolean) => void;
  onIngestCorpusFromPaths: () => void;
  onIngestUploadedCorpus: () => void;
  corpusUploadProgressPercent: number | null;
  corpusUploadProgressLabel: string;
  lastIngestedCorpusReport: CorpusIngestionReport | null;
}) {
  const selectedCorpusUsableForRetrieval =
    !!selectedCorpus &&
    selectedCorpus.lifecycle.state === "ready" &&
    selectedCorpus.root_exists &&
    selectedCorpus.manifest_exists;

  return (
    <aside className={`corpus-drawer${open ? " open" : ""}`}>
      <div className="drawer-header">
        <div>
          <h3>corpus registry</h3>
          <p>inspect local grounding corpora, manage retention, and ingest new material</p>
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

      <div className="drawer-body corpus-drawer-body">
        <section className="panel corpus-list-panel">
          <div className="panel-header">
            <div className="panel-title">corpora</div>
            <span className="mini-status">loaded:{corporaLoadedAt}</span>
          </div>

          <div className="registry-toolbar">
            <label>
              search
              <input
                type="search"
                value={corpusQuery}
                onChange={(event) => onCorpusQueryChange(event.target.value)}
                placeholder="name, id, path, backend..."
              />
            </label>
          </div>

          {corpusLoadError ? (
            <p className="muted-text">corpus registry unavailable: {corpusLoadError}</p>
          ) : filteredCorpora.length === 0 ? (
            <p className="muted-text">
              {corpora.length === 0
                ? "no corpora have been ingested yet"
                : "no corpora match the current search"}
            </p>
          ) : (
            <div className="session-list model-session-list">
              {filteredCorpora.map((corpus) => (
                <button
                  className={
                    selectedCorpusId === corpus.corpus_id ? "session-item selected" : "session-item"
                  }
                  key={corpus.corpus_id}
                  onClick={() => onSelectCorpus(corpus.corpus_id)}
                >
                  <div className="registry-session-header">
                    <span>{corpus.name}</span>
                    <span className={lifecycleStateClass(corpus.lifecycle.state)}>
                      {humanizeSnakeCase(corpus.lifecycle.state)}
                    </span>
                  </div>
                  <div className="registry-session-meta">
                    <small>{corpus.persistent ? "persistent" : "ephemeral"}</small>
                    <small>{new Date(corpus.created_at).toLocaleString()}</small>
                  </div>
                  <small>{shortId(corpus.corpus_id)}</small>
                  <small>
                    {corpus.source_count} sources | {corpus.chunk_count} chunks
                  </small>
                </button>
              ))}
            </div>
          )}
        </section>

        <section className="panel corpus-detail-panel">
          <div className="detail-stack">
            <section className="corpus-detail-block">
              <div className="panel-header">
                <div className="panel-title">details</div>
                {selectedCorpus && <span className="mini-status">id:{shortId(selectedCorpus.corpus_id)}</span>}
              </div>

              {!selectedCorpus ? (
                <p className="muted-text">select a corpus to inspect its lifecycle and artifact paths</p>
              ) : (
                <>
                  <div className="registry-lifecycle-summary">
                    <span className={lifecycleStateClass(selectedCorpus.lifecycle.state)}>
                      {humanizeSnakeCase(selectedCorpus.lifecycle.state)}
                    </span>
                    <span className="pill neutral">
                      {selectedCorpus.persistent ? "persistent" : "ephemeral"}
                    </span>
                  </div>

                  {corpusActionMessage && (
                    <div
                      className={
                        corpusActionFailed ? "registry-action-banner failed" : "registry-action-banner"
                      }
                    >
                      {corpusActionMessage}
                    </div>
                  )}

                  {selectedCorpusLifecycleResult && !corpusActionFailed && (
                    <div className="config-summary">
                      <span>
                        latest action state:{" "}
                        {humanizeSnakeCase(selectedCorpusLifecycleResult.lifecycle_state)}
                      </span>
                      <span>
                        root exists: {formatBoolean(selectedCorpusLifecycleResult.root_exists)}
                      </span>
                      <span>
                        manifest exists:{" "}
                        {formatBoolean(selectedCorpusLifecycleResult.manifest_exists ?? false)}
                      </span>
                      <span>
                        report available:{" "}
                        {formatBoolean(selectedCorpusLifecycleResult.report_available)}
                      </span>
                      <span>
                        report source:{" "}
                        {humanizeSnakeCase(selectedCorpusLifecycleResult.report_storage)}
                      </span>
                      <span>
                        updated:{" "}
                        {selectedCorpusLifecycleResult.updated_at
                          ? formatTimestamp(selectedCorpusLifecycleResult.updated_at)
                          : "unknown"}
                      </span>
                      {selectedCorpusLifecycleResult.state_note && (
                        <span>note: {selectedCorpusLifecycleResult.state_note}</span>
                      )}
                    </div>
                  )}

                  <dl className="registry-detail-grid">
                    <dt>name</dt>
                    <dd>{selectedCorpus.name}</dd>
                    <dt>created</dt>
                    <dd>{formatTimestamp(selectedCorpus.created_at)}</dd>
                    <dt>retention policy</dt>
                    <dd>{humanizeSnakeCase(selectedCorpus.lifecycle.retention_policy)}</dd>
                    <dt>retention deadline</dt>
                    <dd>
                      {selectedCorpus.lifecycle.retention_deadline
                        ? formatTimestamp(selectedCorpus.lifecycle.retention_deadline)
                        : "none"}
                    </dd>
                    <dt>cleanup requested</dt>
                    <dd>
                      {selectedCorpus.lifecycle.cleanup_requested_at
                        ? formatTimestamp(selectedCorpus.lifecycle.cleanup_requested_at)
                        : "not requested"}
                    </dd>
                    <dt>cleanup completed</dt>
                    <dd>
                      {selectedCorpus.lifecycle.cleanup_completed_at
                        ? formatTimestamp(selectedCorpus.lifecycle.cleanup_completed_at)
                        : "not completed"}
                    </dd>
                    <dt>cleanup reason</dt>
                    <dd>
                      {selectedCorpus.lifecycle.cleanup_reason
                        ? humanizeSnakeCase(selectedCorpus.lifecycle.cleanup_reason)
                        : "none"}
                    </dd>
                    <dt>state note</dt>
                    <dd>{selectedCorpus.lifecycle.state_note || "none"}</dd>
                    <dt>lifecycle updated</dt>
                    <dd>
                      {selectedCorpus.lifecycle.updated_at
                        ? formatTimestamp(selectedCorpus.lifecycle.updated_at)
                        : "unknown"}
                    </dd>
                    <dt>sources</dt>
                    <dd>{selectedCorpus.source_count}</dd>
                    <dt>chunks</dt>
                    <dd>{selectedCorpus.chunk_count}</dd>
                    <dt>embedding backend</dt>
                    <dd>{selectedCorpus.embedding_backend || "unknown"}</dd>
                    <dt>embedding model</dt>
                    <dd>{selectedCorpus.embedding_model || "unknown"}</dd>
                    <dt>ocr backend</dt>
                    <dd>{selectedCorpus.ocr_backend || "unknown"}</dd>
                    <dt>root path</dt>
                    <dd className="registry-path">{selectedCorpus.root_path}</dd>
                    <dt>manifest path</dt>
                    <dd className="registry-path">{selectedCorpus.manifest_path}</dd>
                    <dt>report path</dt>
                    <dd className="registry-path">{selectedCorpus.report_path}</dd>
                    <dt>root exists</dt>
                    <dd>
                      {formatBoolean(
                        selectedCorpusLifecycleResult?.root_exists ?? selectedCorpus.root_exists
                      )}
                    </dd>
                    <dt>manifest exists</dt>
                    <dd>
                      {formatBoolean(
                        selectedCorpusLifecycleResult?.manifest_exists ??
                          selectedCorpus.manifest_exists
                      )}
                    </dd>
                    <dt>report exists at recorded path</dt>
                    <dd>
                      {formatBoolean(
                        selectedCorpusLifecycleResult?.report_exists ?? selectedCorpus.report_exists
                      )}
                    </dd>
                    <dt>report available</dt>
                    <dd>
                      {formatBoolean(
                        selectedCorpusLifecycleResult?.report_available ??
                          selectedCorpus.report_available
                      )}
                    </dd>
                    <dt>report source</dt>
                    <dd>
                      {humanizeSnakeCase(
                        selectedCorpusLifecycleResult?.report_storage ??
                          selectedCorpus.report_storage
                      )}
                    </dd>
                    <dt>loadable report path</dt>
                    <dd className="registry-path">
                      {selectedCorpusLifecycleResult?.loadable_report_path ??
                        selectedCorpus.loadable_report_path ??
                        "none"}
                    </dd>
                  </dl>

                  <section className="registry-retention-controls">
                    <div className="panel-title">retention policy</div>
                    <div className="registry-filter-row">
                      <label>
                        policy
                        <select
                          value={corpusRetentionPolicyDraft}
                          onChange={(event) => onCorpusRetentionPolicyDraftChange(event.target.value)}
                          disabled={corpusActionPending !== null}
                        >
                          <option value="retain_until_manual_cleanup">manual cleanup</option>
                          <option value="retain_for_duration">timed retention</option>
                          <option value="ephemeral_immediate">ephemeral immediate</option>
                        </select>
                      </label>

                      {corpusRetentionPolicyDraft === "retain_for_duration" && (
                        <label>
                          minutes
                          <input
                            type="number"
                            min={1}
                            step={1}
                            value={corpusRetentionMinutesDraft}
                            onChange={(event) => onCorpusRetentionMinutesDraftChange(event.target.value)}
                            disabled={corpusActionPending !== null}
                          />
                        </label>
                      )}
                    </div>

                    <div className="registry-retention-meta">
                      <span>
                        current deadline:{" "}
                        {selectedCorpus.lifecycle.retention_deadline
                          ? formatTimestamp(selectedCorpus.lifecycle.retention_deadline)
                          : "none"}
                      </span>
                      <button
                        onClick={() => onSaveCorpusRetentionPolicy(selectedCorpus.corpus_id)}
                        disabled={corpusActionPending !== null}
                      >
                        {corpusActionPending === "retention" ? "saving..." : "save retention policy"}
                      </button>
                    </div>
                  </section>

                  <div className="registry-actions">
                    <button
                      onClick={() => onUseCorpusForOneShot(selectedCorpus.corpus_id)}
                      disabled={!selectedCorpusUsableForRetrieval}
                      title={
                        !selectedCorpusUsableForRetrieval
                          ? "Only ready corpora with intact root and manifest artifacts can be bound for retrieval."
                          : undefined
                      }
                    >
                      use for one-shot
                    </button>
                    <button
                      onClick={() => onOpenCorpusReport(selectedCorpus.corpus_id)}
                      disabled={corpusActionPending !== null || !selectedCorpus.report_available}
                      title={
                        !selectedCorpus.report_available
                          ? "No loadable corpus report is currently available for this corpus."
                          : undefined
                      }
                    >
                      load report
                    </button>
                    <button
                      onClick={() => onRunCorpusLifecycleAction(selectedCorpus.corpus_id, "reconcile")}
                      disabled={corpusActionPending !== null}
                    >
                      {corpusActionPending === "reconcile" ? "reconciling..." : "reconcile"}
                    </button>
                    <button
                      className="danger-button"
                      onClick={() => onRunCorpusLifecycleAction(selectedCorpus.corpus_id, "cleanup")}
                      disabled={corpusActionPending !== null}
                    >
                      {corpusActionPending === "cleanup" ? "cleaning up..." : "cleanup now"}
                    </button>
                  </div>

                  {selectedCorpusReport && currentCorpusReport && (
                    <CorpusReportViewer
                      title="corpus report"
                      report={currentCorpusReport}
                      rawJson={selectedCorpusReport}
                    />
                  )}
                  {selectedCorpusReport && !currentCorpusReport && (
                    <div className="registry-action-banner failed">{selectedCorpusReport}</div>
                  )}
                </>
              )}
            </section>

            <section className="corpus-ingest-panel">
              <div className="panel-header">
                <div className="panel-title">ingest corpus</div>
                <span className="mini-status">txt | md | pdf</span>
              </div>

              {corpusIngestMessage && (
                <div
                  className={corpusIngestFailed ? "registry-action-banner failed" : "registry-action-banner"}
                >
                  {corpusIngestMessage}
                </div>
              )}

              <label>
                corpus name
                <input
                  value={corpusIngestName}
                  onChange={(event) => onCorpusIngestNameChange(event.target.value)}
                  placeholder="incident-response-briefing"
                  disabled={corpusIngestPending}
                />
              </label>

              <label>
                local paths
                <textarea
                  value={corpusIngestPaths}
                  onChange={(event) => onCorpusIngestPathsChange(event.target.value)}
                  placeholder={"/Users/you/docs/briefing.pdf\n/Users/you/docs/notes"}
                  disabled={corpusIngestPending}
                />
              </label>

              <label>
                upload files
                <div
                  className={`upload-dropzone${corpusUploadDragActive ? " active" : ""}${
                    corpusIngestPending ? " disabled" : ""
                  }`}
                  onDragEnter={(event) => {
                    event.preventDefault();
                    if (!corpusIngestPending) {
                      onCorpusUploadDragActiveChange(true);
                    }
                  }}
                  onDragOver={(event) => {
                    event.preventDefault();
                    if (!corpusIngestPending) {
                      onCorpusUploadDragActiveChange(true);
                    }
                  }}
                  onDragLeave={(event) => {
                    event.preventDefault();
                    if (event.currentTarget.contains(event.relatedTarget as Node | null)) {
                      return;
                    }
                    onCorpusUploadDragActiveChange(false);
                  }}
                  onDrop={(event) => {
                    event.preventDefault();
                    onCorpusUploadDragActiveChange(false);
                    if (corpusIngestPending) {
                      return;
                    }
                    onCorpusUploadSelection(Array.from(event.dataTransfer.files ?? []));
                  }}
                >
                  <input
                    key={corpusUploadInputKey}
                    className="upload-input"
                    type="file"
                    accept=".txt,.md,.pdf,text/plain,text/markdown,application/pdf"
                    multiple
                    disabled={corpusIngestPending}
                    onChange={(event) => onCorpusUploadSelection(Array.from(event.target.files ?? []))}
                  />
                  <div className="upload-dropzone-copy">
                    <strong>drop files here or click to browse</strong>
                    <span>Finder / File Explorer upload for .txt, .md, and .pdf</span>
                  </div>
                </div>
              </label>

              {corpusUploadFiles.length > 0 && (
                <div className="config-summary corpus-upload-summary">
                  <span>
                    selected: {corpusUploadFiles.length} file
                    {corpusUploadFiles.length === 1 ? "" : "s"}
                  </span>
                  {corpusUploadFiles.slice(0, 4).map((file) => (
                    <span key={`${file.name}-${file.size}`}>
                      {file.name} | {formatBytes(file.size)}
                    </span>
                  ))}
                  {corpusUploadFiles.length > 4 && (
                    <span>…and {corpusUploadFiles.length - 4} more</span>
                  )}
                </div>
              )}

              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={corpusIngestPersistent}
                  onChange={(event) => onCorpusIngestPersistentChange(event.target.checked)}
                  disabled={corpusIngestPending}
                />
                persistent corpus
              </label>

              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={corpusIngestOcrEnabled}
                  onChange={(event) => onCorpusIngestOcrEnabledChange(event.target.checked)}
                  disabled={corpusIngestPending}
                />
                enable hybrid OCR for sparse PDF pages
              </label>

              <div className="registry-actions">
                <button onClick={onIngestCorpusFromPaths} disabled={corpusIngestPending}>
                  {corpusIngestPending ? "ingesting..." : "ingest from paths"}
                </button>
                <button
                  onClick={onIngestUploadedCorpus}
                  disabled={corpusIngestPending || corpusUploadFiles.length === 0}
                >
                  {corpusIngestPending ? "uploading..." : "upload + ingest"}
                </button>
              </div>

              {corpusIngestPending && corpusUploadProgressPercent !== null && (
                <div className="upload-progress">
                  <div className="upload-progress-bar">
                    <div
                      className="upload-progress-fill"
                      style={{ width: `${corpusUploadProgressPercent}%` }}
                    />
                  </div>
                  <div className="upload-progress-meta">
                    <span>{corpusUploadProgressPercent}%</span>
                    <span>{corpusUploadProgressLabel || "Uploading corpus files..."}</span>
                  </div>
                </div>
              )}

              <p className="microcopy">
                Upload files to open Finder/File Explorer like a normal chatbot, or enter
                absolute local file and directory paths if you want an operator-style ingest.
                PDF ingestion uses native text extraction first, then OCRs sparse pages when enabled.
              </p>

              {lastIngestedCorpusReport && (
                <div className="config-summary corpus-ingest-summary">
                  <span>last ingest: {shortId(lastIngestedCorpusReport.corpus_id)}</span>
                  <span>
                    discovered: {lastIngestedCorpusReport.files_discovered} | ingested:{" "}
                    {lastIngestedCorpusReport.files_ingested}
                  </span>
                  <span>
                    pdf pages: {lastIngestedCorpusReport.pdf_pages_seen} | OCR:{" "}
                    {lastIngestedCorpusReport.pdf_pages_ocrd}
                  </span>
                  <span>chunks: {lastIngestedCorpusReport.chunk_count}</span>
                  {lastIngestedCorpusReport.upload_staging && (
                    <span>
                      upload staging:{" "}
                      {lastIngestedCorpusReport.upload_staging.cleaned_up ? "cleaned" : "retained/failed"}
                    </span>
                  )}
                </div>
              )}

              {lastIngestedCorpusReport && (
                <CorpusReportViewer
                  title="latest ingest report"
                  report={lastIngestedCorpusReport}
                  compact
                />
              )}
            </section>
          </div>
        </section>
      </div>
    </aside>
  );
}
