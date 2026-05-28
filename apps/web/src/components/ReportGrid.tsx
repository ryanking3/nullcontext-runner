export function ReportGrid({
  entries,
}: {
  entries: Array<{ label: string; value: string }>;
}) {
  return (
    <dl className="report-grid">
      {entries.map((entry) => (
        <div className="report-grid-row" key={entry.label}>
          <dt>{entry.label}</dt>
          <dd>{entry.value}</dd>
        </div>
      ))}
    </dl>
  );
}
