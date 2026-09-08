import { Lock, RotateCcw, X } from "lucide-react";
import { directiveLabels } from "../lib/format";
import type { LiveWorker } from "../types";
import { t } from "../lib/i18n";

/**
 * One chip per live worker you are NOT in — they are how you reach the
 * other work, and their name is what makes them readable.
 *
 * The focused task gets NO chip. It used to get one with its label
 * suppressed, which left a nameless card under the composer holding a
 * dot, an ⓘ and an ✕ — three controls with nothing saying what they
 * belonged to. The ✕ on it ended the whole session, which is not what an
 * ✕ next to a running turn reads as. What that card was for now lives
 * where it belongs: the turn's own status line, and Parar in the
 * composer.
 */
export default function WorkerChips({
  liveWorkers,
  focused,
  onToggleFocus,
  onStop,
  onRestartLight,
}: {
  liveWorkers: Record<string, LiveWorker>;
  focused: string | null;
  onToggleFocus: (taskId: string) => void;
  onStop: (taskId: string) => void;
  /** Heavy session (context ≥70%): fresh session with a local brief. */
  onRestartLight?: (taskId: string) => void;
}) {
  return (
    <>
      {Object.entries(liveWorkers)
        .filter(([taskId]) => taskId !== focused)
        .map(([taskId, w]) => (
        <span
          key={taskId}
          className={`worker-chip ${w.status}`}
          onClick={() => onToggleFocus(taskId)}
          title={t("chip_focus")}
        >
          <span className="dot" />
          {w.status === "awaiting" && <Lock size={11} />}
          {w.label}
          {directiveLabels(w.directives).length > 0 && (
            <span className="chip-mode">{directiveLabels(w.directives).join(" ")}</span>
          )}
          {(w.context_pct ?? 0) >= 0.7 && onRestartLight && (
            <button
              className="light"
              title={t("chip_light", { n: Math.round((w.context_pct ?? 0) * 100) })}
              onClick={(e) => {
                e.stopPropagation();
                onRestartLight(taskId);
              }}
            >
              <RotateCcw size={11} />
            </button>
          )}
          <button
            className="close"
            title={t("chip_stop")}
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
