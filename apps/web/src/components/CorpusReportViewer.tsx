import { useState } from "react";
import type { CorpusIngestionReport } from "../appTypes";
import { formatBytes, formatTimestamp } from "../appUtils";
import { ReportGrid } from "./ReportGrid";

export function CorpusReportViewer({
  title,
  report,
  rawJson,
  compact = false,
}: {
  title: string;
  report: CorpusIngestionReport;
  rawJson?: string;
  compact?: boolean;
}) {
  const [showRaw, setShowRaw] = useState(false);

  return (
    <section className={`report-section${compact ? " compact-report-section" : ""}`}>
      <div className="panel-header">
        <div className="panel-title">{title}</div>
        {rawJson && (
          <button className="ghost-button" onClick={() => setShowRaw((current) => !current)}>
            {showRaw ? "hide raw json" : "view raw json"}
          </button>
        )}
      </div>

      <ReportGrid
        entries={[
          {
            label: "corpus id",
            value: report.corpus_id,
          },
          {
            label: "created",
            value: formatTimestamp(report.created_at),
          },
          {
            label: "persistent",
            value: report.persistent ? "yes" : "no",
          },
          {
            label: "OCR enabled",
            value: report.ocr_enabled ? "yes" : "no",
          },
          {
            label: "files discovered",
            value: String(report.files_discovered),
          },
          {
            label: "files ingested",
            value: String(report.files_ingested),
          },
          {
            label: "files failed",
            value: String(report.files_failed),
          },
          {
            label: "pdf pages seen",
            value: String(report.pdf_pages_seen),
          },
          {
            label: "pdf pages OCR'd",
            value: String(report.pdf_pages_ocrd),
          },
          {
            label: "chunks created",
            value: String(report.chunk_count),
          },
        ]}
      />

      {report.source_paths_requested.length > 0 && (
        <details className="report-detail" open={!compact}>
          <summary>
            <span>requested source paths</span>
            <span className="pill neutral">{report.source_paths_requested.length}</span>
          </summary>
          <div className="report-list">
            {report.source_paths_requested.map((path) => (
              <div className="report-item" key={path}>
                <div className="report-path-list">
                  <div>{path}</div>
                </div>
              </div>
            ))}
          </div>
        </details>
      )}

      {report.upload_staging && (
        <details className="report-detail" open>
          <summary>
            <span>upload staging</span>
            <span className="pill neutral">{report.upload_staging.staged_files}</span>
          </summary>
          <ReportGrid
            entries={[
              {
                label: "staging root",
                value: report.upload_staging.staging_root,
              },
              {
                label: "staged files",
                value: String(report.upload_staging.staged_files),
              },
              {
                label: "staged bytes",
                value: formatBytes(report.upload_staging.staged_bytes),
              },
              {
                label: "cleanup status",
                value: report.upload_staging.cleaned_up ? "cleaned up" : "retained/failed",
              },
              {
                label: "cleanup error",
                value: report.upload_staging.cleanup_error || "none",
              },
            ]}
          />

          <details className="report-detail" open={!compact}>
            <summary>
              <span>uploaded filenames</span>
              <span className="pill neutral">
                {report.upload_staging.source_filenames.length}
              </span>
            </summary>
            <div className="report-list">
              {report.upload_staging.source_filenames.map((name) => (
                <div className="report-item" key={name}>
                  <div className="report-path-list">
                    <div>{name}</div>
                  </div>
                </div>
              ))}
            </div>
          </details>
        </details>
      )}

      {report.warnings.length > 0 && (
        <details className="report-detail" open>
          <summary>
            <span>warnings</span>
            <span className="pill warning">{report.warnings.length}</span>
          </summary>
          <div className="report-risk-block">
            {report.warnings.map((warning) => (
              <p key={warning}>{warning}</p>
            ))}
          </div>
        </details>
      )}

      <div className="report-risk-block">
        <p>
          <strong>residual risk:</strong> {report.residual_risk}
        </p>
      </div>

      {rawJson && showRaw && (
        <details className="report-detail" open>
          <summary>
            <span>raw corpus report</span>
            <span className="pill neutral">json</span>
          </summary>
          <pre>{rawJson}</pre>
        </details>
      )}
    </section>
  );
}
