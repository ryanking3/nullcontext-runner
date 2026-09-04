import type { ReactNode } from "react";

export type EvidenceStatusListEntry = {
  key: string;
  label: string;
  status: string;
  details: Array<{
    key: string;
    content: ReactNode;
  }>;
};

export function EvidenceStatusList({
  entries,
  statusClassName,
  formatStatus,
}: {
  entries: EvidenceStatusListEntry[];
  statusClassName: (status: string) => string;
  formatStatus: (status: string) => string;
}) {
  return (
    <div className="report-list">
      {entries.map((entry) => (
        <div className="report-item" key={entry.key}>
          <div className="report-item-header">
            <strong>{entry.label}</strong>
            <span className={statusClassName(entry.status)}>
              {formatStatus(entry.status)}
            </span>
          </div>
          <div className="report-path-list">
            {entry.details.map((detail) => (
              <div key={detail.key}>{detail.content}</div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
