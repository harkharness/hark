import { useEffect, useRef, useState } from "react";
import { Plus, SquareChevronRight } from "lucide-react";
import ShellTerminal, { disposeShell } from "./ShellTerminal";
import { t } from "../lib/i18n";

/** The Terminal window's tab strip — lives in the PanelFrame header. */
export function TerminalTabs({
  shells,
  active,
  onActivate,
  onAddShell,
  onCloseShell,
  onResumeSession,
  resumeSpent,
}: {
  shells: string[];
  active: string;
  onActivate: (id: string) => void;
  onAddShell: () => void;
  onCloseShell: (id: string) => void;
  /** Paste the focused chat's resume command into the shell. Absent when
   *  no chat is focused — there would be no session to name. */
  onResumeSession?: () => void;
  /** The session is already running here: the button has nothing to do. */
  resumeSpent?: boolean;
}) {
  return (
    <>
      {shells.map((id, i) => (
        <span
          key={id}
          className={`filetab ${active === id ? "on" : ""}`}
          onClick={() => onActivate(id)}
        >
          zsh {i + 1}
          <button
            className="filetab-close"
            title={t("term_close_shell")}
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
        <button title={t("term_new_shell")} onClick={onAddShell}>
          <Plus size={12} />
        </button>
      </span>
      <span
        className={`filetab feedtab ${active === "feed" ? "on" : ""}`}
        title={t("term_feed_hint")}
        onClick={() => onActivate("feed")}
      >
        feed
      </span>
      {onResumeSession && (
        <button
          className={`term-resume ${resumeSpent ? "spent" : ""}`}
          title={t("term_resume_hint")}
          disabled={resumeSpent}
          onClick={onResumeSession}
        >
          <SquareChevronRight size={12} /> {t("term_resume")}
        </button>
      )}
    </>
  );
}

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
            ? t("feed_no_task")
            : lines.length === 0
              ? t("feed_empty")
              : lines.join("\n")}
          <div ref={endRef} />
        </pre>
      )}
    </div>
  );
}
