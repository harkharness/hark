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
 * The "Terminal" window: REAL shells, one PTY per tab, the user's own
 * $SHELL in the project directory. `+` opens more. Closing a tab kills
 * that shell; closing the pane only hides them.
 *
 * There is no worker "feed" tab: the agent's tool calls already read as
 * folds in the transcript, which is where the conversation is — a second
 * transcription of the same events, in a panel, was one more place to
 * look and a dead panel whenever the thread was fresh.
 */
export default function TerminalPane({
  shells,
  active,
  cwd,
  onAddShell,
  onCloseShell,
  onActivate,
}: {
  /** Shell tab ids, owned by App (survive pane close/reopen). */
  shells: string[];
  /** Active shell id. */
  active: string;
  cwd?: string;
  onAddShell: () => void;
  onCloseShell: (id: string) => void;
  onActivate: (id: string) => void;
}) {
  return (
    <div className="termpane">
      {/* Shells stay mounted (hidden) so switching tabs never loses the
          screen; the PTY lives on the Rust side either way. */}
      {shells.map((id) => (
        <div key={id} className="term-slot" style={{ display: active === id ? "flex" : "none" }}>
          <ShellTerminal id={id} cwd={cwd} onExit={() => onCloseShell(id)} />
        </div>
      ))}

    </div>
  );
}
