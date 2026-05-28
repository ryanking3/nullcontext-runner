export function Hint({ text }: { text: string }) {
  return (
    <span className="hint" tabIndex={0}>
      <span className="hint-trigger">?</span>
      <span className="hint-bubble">{text}</span>
    </span>
  );
}
