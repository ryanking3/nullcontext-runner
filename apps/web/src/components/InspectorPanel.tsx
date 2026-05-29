import { PrivacyReportViewer } from "./PrivacyReportViewer";
import type { AuditOperation, InspectorView } from "../appTypes";
import { statusClass } from "../appUtils";

type InspectorTab = {
  id: InspectorView;
  label: string;
  count?: number;
  disabled?: boolean;
};

export function InspectorPanel({
  open,
  onClose,
  tabs,
  inspectorView,
  onInspectorViewChange,
  auditOperations,
  runtimeLogs,
  currentReportRaw,
  showRawReport,
  onToggleRaw,
  stderr,
}: {
  open: boolean;
  onClose: () => void;
  tabs: InspectorTab[];
  inspectorView: InspectorView;
  onInspectorViewChange: (view: InspectorView) => void;
  auditOperations: AuditOperation[];
  runtimeLogs: string;
  currentReportRaw: string;
  showRawReport: boolean;
  onToggleRaw: () => void;
  stderr: string;
}) {
  if (!open) {
    return null;
  }

  return (
    <aside className="inspector">
      <section className="panel inspector-shell">
        <div className="panel-header">
          <div className="panel-title">inspector</div>
          <button className="ghost-button" onClick={onClose}>
            hide
          </button>
        </div>

        <div className="inspector-tabs">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              className={inspectorView === tab.id ? "selected" : ""}
              disabled={tab.disabled}
              onClick={() => onInspectorViewChange(tab.id)}
              title={tab.disabled ? "No data yet" : undefined}
            >
              {tab.label}
              {typeof tab.count === "number" ? ` (${tab.count})` : ""}
            </button>
          ))}
        </div>

        <div className="inspector-panel">
          {inspectorView === "audit" && (
            <>
              {auditOperations.length === 0 ? (
                <p className="muted-text">audit operations appear during a run</p>
              ) : (
                <div className="audit-list">
                  {auditOperations.map((operation, index) => (
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
            </>
          )}

          {inspectorView === "runtime" && <pre>{runtimeLogs || "no runtime logs yet"}</pre>}

          {inspectorView === "report" && (
            <PrivacyReportViewer
              rawReport={currentReportRaw}
              showRawReport={showRawReport}
              onToggleRaw={onToggleRaw}
            />
          )}

          {inspectorView === "stderr" && <pre>{stderr || "no stderr captured"}</pre>}
        </div>
      </section>
    </aside>
  );
}
