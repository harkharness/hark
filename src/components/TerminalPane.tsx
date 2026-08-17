import { useEffect, useRef } from "react";

/**
 * The task's own "terminal": the raw worker feed (tool payloads, results,
 * turn costs) exactly as it streams, one line per event. Read-only; the
 * transcript stays the human view, this is the machine view.
 */
export default function TerminalPane({
  label,
  lines,
  onClose,
}: {
  label: string;
  lines: string[];
  onClose: () => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    endRef.current?.scrollIntoView();
  }, [lines]);

  return (
    <div className="termpane">
      <div className="viewer-head">
        <span className="viewer-proj">terminal ·</span>
        <span className="viewer-path">{label}</span>
        <span className="viewer-actions">
          <button onClick={onClose}>×</button>
        </span>
      </div>
      <pre className="term-body">
        {lines.length === 0 ? "(sem eventos ainda nesta thread)" : lines.join("\n")}
        <div ref={endRef} />
      </pre>
    </div>
  );
}
