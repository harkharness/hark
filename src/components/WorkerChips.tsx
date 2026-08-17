import { Info, Lock, Target, X } from "lucide-react";
import { directiveLabels } from "../lib/format";
import type { LiveWorker } from "../types";

/**
 * One chip per live worker plus the focused-task chip. Clicking a chip
 * swaps the visible thread; ℹ shows a summary; × ends the worker.
 */
export default function WorkerChips({
  liveWorkers,
  focused,
  focusedTaskTitle,
  onToggleFocus,
  onReleaseFocusedTask,
  onInfo,
  onStop,
}: {
  liveWorkers: Record<string, LiveWorker>;
  focused: string | null;
  focusedTaskTitle?: string;
  onToggleFocus: (taskId: string) => void;
  onReleaseFocusedTask: () => void;
  onInfo: (taskId: string) => void;
  onStop: (taskId: string) => void;
}) {
  const focusedIsLive =
    !!focusedTaskTitle &&
    Object.values(liveWorkers).some((w) => w.label === focusedTaskTitle);

  return (
    <>
      {focusedTaskTitle && !focusedIsLive && (
        <span className="worker-chip focused">
          <Target size={12} /> {focusedTaskTitle.slice(0, 28)}
          <button
            className="close"
            title="soltar a task (voltar ao modo pergunta)"
            onClick={onReleaseFocusedTask}
          >
            <X size={11} />
          </button>
        </span>
      )}
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
