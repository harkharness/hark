import { useEffect, useRef, useState } from "react";
import { Plus } from "lucide-react";
import ShellTerminal, { disposeShell } from "./ShellTerminal";

/**
 * The "Terminal" window: REAL shells (one PTY per tab, the user's own
 * $SHELL in the project directory), plus a read-only "feed" tab with the
 * raw worker events of the focused task. `+` opens more shells. Closing a
 * shell tab kills that shell; closing the pane hides it, shells survive.
 */
export default function TerminalPane({
  rawLog,
  focusedLabel,
  shells,
  active,
  cwd,
  onAddShell,
  onCloseShell,
  onActivate,
}: {
  rawLog: Record<string, string[]>;
  focusedLabel?: string;
  /** Shell tab ids, owned by App (survive pane close/reopen). */
  shells: string[];
  /** Active tab: a shell id or "feed". */
  active: string;
  cwd?: string;
  onAddShell: () => void;
  onCloseShell: (id: string) => void;
  onActivate: (id: string) => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  const [feedThread, setFeedThread] = useState<string | undefined>(focusedLabel);

  useEffect(() => {
    if (focusedLabel) setFeedThread(focusedLabel);
  }, [focusedLabel]);

  const lines = feedThread ? (rawLog[feedThread] ?? []) : [];
  useEffect(() => {
    if (active === "feed") endRef.current?.scrollIntoView();
  }, [lines, active]);

  return (
    <div className="termpane">
      <div className="filetabs">
        {shells.map((id, i) => (
          <span
            key={id}
            className={`filetab ${active === id ? "on" : ""}`}
            onClick={() => onActivate(id)}
          >
            zsh {i + 1}
            <button
              className="filetab-close"
              title="fechar este shell"
              onClick={(e) => {
                e.stopPropagation();
                disposeShell(id);
                onCloseShell(id);
              }}
            >
              ×
            </button>
          </span>
        ))}
        <span className="filetab addtab">
          <button title="novo shell" onClick={onAddShell}>
            <Plus size={12} />
          </button>
        </span>
        <span
          className={`filetab feedtab ${active === "feed" ? "on" : ""}`}
          title="eventos brutos do worker da task focada"
          onClick={() => onActivate("feed")}
        >
          feed
        </span>
      </div>

      {/* Shells stay mounted (hidden) so switching tabs never loses the
          screen; the PTY lives on the Rust side either way. */}
      {shells.map((id) => (
        <div key={id} className="term-slot" style={{ display: active === id ? "flex" : "none" }}>
          <ShellTerminal id={id} cwd={cwd} onExit={() => onCloseShell(id)} />
        </div>
      ))}

      {active === "feed" && (
        <pre className="term-body">
          {!feedThread
            ? "(foca uma task pra acompanhar o feed dela)"
            : lines.length === 0
              ? "(sem eventos ainda nesta thread)"
              : lines.join("\n")}
          <div ref={endRef} />
        </pre>
      )}
    </div>
  );
}
