import { Info, Lock, X } from "lucide-react";
import { directiveLabels } from "../lib/format";
import type { LiveWorker } from "../types";

/**
 * One chip per LIVE worker (status, directives, stop). The focused task
 * needs no chip: the sidebar already highlights it — clicking the active
 * item there again releases the focus.
 */
export default function WorkerChips({
  liveWorkers,
  focused,
  onToggleFocus,
  onInfo,
  onStop,
}: {
  liveWorkers: Record<string, LiveWorker>;
  focused: string | null;
  onToggleFocus: (taskId: string) => void;
  onInfo: (taskId: string) => void;
  onStop: (taskId: string) => void;
}) {
  return (
    <>
      {Object.entries(liveWorkers).map(([taskId, w]) => (
        <span
          key={taskId}
          className={`worker-chip ${w.status} ${focused === taskId ? "focused" : ""}`}
          onClick={() => onToggleFocus(taskId)}
          title={focused === taskId ? "focado (clique para soltar)" : "clique para focar"}
        >
          <span className="dot" />
          {w.status === "awaiting" && <Lock size={11} />}
          {w.label}
          {directiveLabels(w.directives).length > 0 && (
            <span className="chip-mode">{directiveLabels(w.directives).join(" ")}</span>
          )}
          <button
            className="info"
            title="resumo da task"
            onClick={(e) => {
              e.stopPropagation();
              onInfo(taskId);
            }}
          >
            <Info size={11} />
          </button>
          <button
            className="close"
            title="finalizar worker"
            onClick={(e) => {
              e.stopPropagation();
              onStop(taskId);
            }}
          >
            <X size={11} />
          </button>
        </span>
      ))}
    </>
  );
}
