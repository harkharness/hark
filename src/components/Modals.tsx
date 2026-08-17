import type { LiveWorker, Msg } from "../types";

export type Pending =
  | { kind: "confirm-dispatch"; instruction: string; sessionId?: string; warning?: string }
  | {
      kind: "choice";
      instruction: string;
      candidates: { session_id: string; title: string; last_ts: string }[];
    }
  | { kind: "resume-task"; title: string; sessionId?: string; instruction: string }
  | { kind: "task-summary"; taskId: string }
  | null;

/** Confirmation/choice/resume/summary dialogs. Permission asks are NOT
 * here: those are inline cards in the thread. */
export default function Modals({
  pending,
  setPending,
  focusedTaskTitle,
  liveWorkers,
  messages,
  onDispatch,
  onFocusWorker,
}: {
  pending: Pending;
  setPending: (p: Pending) => void;
  focusedTaskTitle?: string;
  liveWorkers: Record<string, LiveWorker>;
  messages: Msg[];
  onDispatch: (instruction: string, sessionId?: string) => void;
  onFocusWorker: (taskId: string) => void;
}) {
  if (!pending) return null;

  if (pending.kind === "confirm-dispatch") {
    return (
      <div className="modal-backdrop">
        <div className="modal">
          <h2>
            Despachar tarefa?
            {pending.sessionId && focusedTaskTitle && (
              <span className="reader-meta"> → {focusedTaskTitle}</span>
            )}
          </h2>
          {pending.warning && <div className="gate-warning">⚠ {pending.warning}</div>}
          <pre>{pending.instruction}</pre>
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              cancelar
            </button>
            <button
              className="allow"
              onClick={() => {
                const { instruction, sessionId } = pending;
                setPending(null);
                onDispatch(instruction, sessionId);
              }}
            >
              confirmar
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
          <h2>Qual sessão?</h2>
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
              cancelar
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
          <h2>▶ Retomar: {pending.title}</h2>
          <textarea
            className="resume-input"
            value={pending.instruction}
            onChange={(e) => setPending({ ...pending, instruction: e.target.value })}
            rows={4}
          />
          <div className="row">
            <button className="plain" onClick={() => setPending(null)}>
              cancelar
            </button>
            <button
              className="allow"
              onClick={() => {
                const { instruction, sessionId } = pending;
                setPending(null);
                onDispatch(instruction, sessionId);
              }}
            >
              despachar
            </button>
          </div>
        </div>
      </div>
    );
  }

  // task-summary
  const worker = liveWorkers[pending.taskId];
  return (
    <div className="modal-backdrop" onClick={() => setPending(null)}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>
          ℹ {worker?.label ?? pending.taskId}
          {worker && ` · ${worker.status === "running" ? "rodando" : "aguardando você"}`}
        </h2>
        <pre>
          {messages
            .filter((m) => "task" in m && m.task === (worker?.label ?? pending.taskId))
            .slice(-14)
            .map((m) => {
              switch (m.who) {
                case "user":
                  return `você: ${m.text}`;
                case "tool":
                  return `  ⚙ ${m.name}`;
                case "output":
                  return `  ${m.error ? "✗" : "✓"} ${m.content.split("\n")[0]}`;
                case "permission":
                  return `  🔐 ${m.tool} ${m.decision ?? "aguardando"}`;
                default:
                  return `vox: ${"text" in m ? m.text : ""}`;
              }
            })
            .join("\n") || "(sem eventos ainda)"}
        </pre>
        <div className="row">
          <button
            className="plain"
            onClick={() => {
              onFocusWorker(pending.taskId);
              setPending(null);
            }}
          >
            focar nela
          </button>
          <button className="plain" onClick={() => setPending(null)}>
            fechar
          </button>
        </div>
      </div>
    </div>
  );
}
