import { useEffect, useRef, useState } from "react";
import { Plus } from "lucide-react";

/**
 * The "Terminal" window body: one tab per task feed (raw worker events).
 * `+` opens any other thread that has produced output. Read-only; closing
 * a tab never stops the worker — the feed keeps accumulating underneath.
 */
export default function TerminalPane({
  rawLog,
  focusedLabel,
}: {
  rawLog: Record<string, string[]>;
  focusedLabel?: string;
}) {
  const [tabs, setTabs] = useState<string[]>(focusedLabel ? [focusedLabel] : []);
  const [active, setActive] = useState(0);
  const [picking, setPicking] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  // Following the focused thread: a new focus opens/activates its tab.
  useEffect(() => {
    if (!focusedLabel) return;
    setTabs((old) => (old.includes(focusedLabel) ? old : [...old, focusedLabel]));
    setActive((_) => {
      const idx = tabs.indexOf(focusedLabel);
      return idx >= 0 ? idx : tabs.length;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [focusedLabel]);

  const current = tabs[active];
  const lines = current ? (rawLog[current] ?? []) : [];
  useEffect(() => {
    endRef.current?.scrollIntoView();
  }, [lines]);

  const available = Object.keys(rawLog).filter((l) => !tabs.includes(l));

  return (
    <div className="termpane">
      <div className="filetabs">
        {tabs.map((label, i) => (
          <span
            key={label}
            className={`filetab ${i === active ? "on" : ""}`}
            title={label}
            onClick={() => setActive(i)}
          >
            {label.slice(0, 18)}
            <button
              className="filetab-close"
              title="fechar aba (task segue rodando)"
              onClick={(e) => {
                e.stopPropagation();
                setTabs((old) => old.filter((_, j) => j !== i));
                setActive((a) => Math.max(0, a > i ? a - 1 : Math.min(a, tabs.length - 2)));
              }}
            >
              ×
            </button>
          </span>
        ))}
        {available.length > 0 && (
          <span className="filetab addtab">
            <button title="abrir feed de outra task" onClick={() => setPicking((p) => !p)}>
              <Plus size={12} />
            </button>
            {picking && (
              <div className="side-menu">
                {available.map((label) => (
                  <button
                    key={label}
                    onClick={() => {
                      setTabs((old) => [...old, label]);
                      setActive(tabs.length);
                      setPicking(false);
                    }}
                  >
                    {label.slice(0, 30)}
                  </button>
                ))}
              </div>
            )}
          </span>
        )}
      </div>
      <pre className="term-body">
        {tabs.length === 0
          ? "(foca uma task pra acompanhar o feed dela)"
          : lines.length === 0
            ? "(sem eventos ainda nesta thread)"
            : lines.join("\n")}
        <div ref={endRef} />
      </pre>
    </div>
  );
}
