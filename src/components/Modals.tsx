import { t } from "../lib/i18n";
import type { DispatchWarning, LiveWorker, Msg, SessionHit } from "../types";

export type Pending =
  | {
      kind: "confirm-dispatch";
      instruction: string;
      sessionId?: string;
      /** Gate's mismatch aviso (already filtered for credibility). */
      warning?: string;
      /** Local prechecks: true numbers, each with actions. */
      warnings?: DispatchWarning[];
      /** The text was TYPED, not heard. Nothing to re-read and nothing to
       *  correct: the only open question is the warning's. */
      typed?: boolean;
    }
  | { kind: "pick-session"; query: string; candidates: SessionHit[] }
  /** Board tasks too close to call: the user picks, never a silent guess. */
  | {
      kind: "pick-task";
      query: string;
      candidates: { title: string; session_id?: string | null; workspace?: string | null }[];
    }
  | {
      kind: "choice";
      instruction: string;
      candidates: { session_id: string; title: string; last_ts: string }[];
    }
  /** Spoken/typed project not registered, but disk directories match:
   *  picking one registers it and opens its window — never a dead end. */
  | { kind: "project-offer"; query: string; candidates: string[] }
  | { kind: "resume-task"; title: string; sessionId?: string; instruction: string }
  | null;

/** Confirmation/choice/resume/summary dialogs. Permission asks are NOT
 * here: those are inline cards in the thread. */
export default function Modals({
  pending,
  setPending,
  focusedTaskTitle,
  onDispatch,
  onPickSession,
  onPickProject,
  onPickTask,
  onCompactFirst,
}: {
  pending: Pending;
  setPending: (p: Pending) => void;
  focusedTaskTitle?: string;
  onDispatch: (instruction: string, sessionId?: string, taskTitle?: string) => void;
  /** A recovered session becomes a task and opens its chat. */
  onPickSession: (hit: SessionHit) => void;
  /** project-offer pick: register the path and open its window. */
  onPickProject: (path: string) => void;
  /** An ambiguous spoken target resolved by hand. */
  onPickTask: (title: string, sessionId?: string) => void;
  /** "compactar antes": /compact as its own turn, then the message. */
  onCompactFirst: (instruction: string, sessionId?: string) => void;
}) {
  if (!pending) return null;

  if (pending.kind === "pick-task") {
    return (
      <div className="modal-backdrop" onClick={() => setPending(null)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <h2>{t("m_tasks_about", { q: pending.query })}</h2>
          {pending.candidates.map((c) => (
            <button
              key={c.title}
              className="choice"
              onClick={() => {
                setPending(null);
                onPickTask(c.title, c.session_id ?? undefined);
              }}
            >
              <b>{c.title}</b>
              {c.workspace && (
                <span className="reader-meta">
                  {" "}· {c.workspace.split("/").filter(Boolean).pop()}
                </span>
              )}
            </button>
          ))}
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (pending.kind === "pick-session") {
    return (
      <div className="modal-backdrop" onClick={() => setPending(null)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <h2>{t("m_sessions_about", { q: pending.query })}</h2>
          {pending.candidates.map((c) => (
            <button
              key={c.session_id}
              className="choice"
              onClick={() => {
                setPending(null);
                onPickSession(c);
              }}
            >
              <b>{c.title}</b>
              <span className="reader-meta">
                {c.last_ts ? ` · ${c.last_ts.slice(0, 16).replace("T", " ")}` : ""}
                {c.cwd ? ` · ${c.cwd.split("/").filter(Boolean).pop()}` : ""}
              </span>
              {c.last_prompt && <div className="choice-note">{c.last_prompt}</div>}
            </button>
          ))}
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (pending.kind === "project-offer") {
    return (
      <div className="modal-backdrop" onClick={() => setPending(null)}>
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <h2>{t("m_dirs_about", { q: pending.query })}</h2>
          {pending.candidates.map((path) => (
            <button
              key={path}
              className="choice"
              onClick={() => {
                setPending(null);
                onPickProject(path);
              }}
            >
              <b>{path.split("/").pop()}</b>
              <span className="reader-meta"> · {path}</span>
            </button>
          ))}
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (pending.kind === "confirm-dispatch") {
    const confirm = () => {
      const { instruction, sessionId } = pending;
      setPending(null);
      onDispatch(instruction, sessionId);
    };
    return (
      <div className="modal-backdrop">
        <div className="modal">
          <h2>
            {pending.typed ? t("m_heavy_q") : t("m_dispatch_q")}
            {pending.sessionId && focusedTaskTitle && (
              <span className="reader-meta"> → {focusedTaskTitle}</span>
            )}
          </h2>
          {pending.warning && <div className="gate-warning">⚠ {pending.warning}</div>}
          {/* Local warnings carry ACTIONS, not just anxiety: compact the
              session first, or proceed as-is (the confirm button). */}
          {(pending.warnings ?? []).map((w) => (
            <div key={w.kind} className="gate-warning warn-actions">
              <span>⚠ {w.text}</span>
              {w.actions.includes("compact_first") && (
                <button
                  className="warn-act"
                  onClick={() => {
                    const { instruction, sessionId } = pending;
                    setPending(null);
                    onCompactFirst(instruction, sessionId);
                  }}
                >
                  {t("m_compact_first")}
                </button>
              )}
            </div>
          ))}
          {/* EDITABLE only when the words were HEARD: STT gets them wrong,
              so fix them here (or say the whole thing again — the voice
              loop swaps this text). Typed, they are already right, and
              reprinting them to be confirmed was the whole complaint. */}
          {!pending.typed && (
          <textarea
            className="resume-input"
            autoFocus
            value={pending.instruction}
            onChange={(e) => setPending({ ...pending, instruction: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                confirm();
              }
              if (e.key === "Escape") setPending(null);
            }}
            rows={5}
          />
          )}
          <div className="row">
            <span className="modal-hint">{pending.typed ? "" : t("m_dispatch_hint")}</span>
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
            <button className="allow" onClick={confirm}>
              {pending.typed ? t("m_send_anyway") : t("m_confirm")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (pending.kind === "choice") {
    return (
      <div className="modal-backdrop">
        <div className="modal">
          <h2>{t("m_which_session")}</h2>
          {pending.candidates.map((c) => (
            <button
              key={c.session_id}
              className="choice"
              onClick={() => {
                const inst = pending.instruction;
                setPending(null);
                onDispatch(inst, c.session_id);
              }}
            >
              {c.title || c.session_id} · {c.last_ts}
            </button>
          ))}
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (pending.kind === "resume-task") {
    return (
      <div className="modal-backdrop">
        <div className="modal">
          <h2>{t("m_resume", { t: pending.title })}</h2>
          <textarea
            className="resume-input"
            value={pending.instruction}
            onChange={(e) => setPending({ ...pending, instruction: e.target.value })}
            rows={4}
          />
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              {t("m_cancel")}
            </button>
            <button
              className="allow"
              onClick={() => {
                const { instruction, sessionId, title } = pending;
                setPending(null);
                onDispatch(instruction, sessionId, title);
              }}
            >
              {t("m_dispatch")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return null;
}
