// The voice round's pure rules: how a plan is confirmed, what its chip
// says. The hook (useVoiceRound) owns the state; this owns no state.
import type { VoicePlan } from "../types";

export type WorkPlan = Extract<VoicePlan, { kind: "work" }>;

/** Two speeds: the chat on screen (high confidence, existing, no local
 *  warning) executes after a silent beat; anything else is spoken and
 *  waits for a verdict, never on silence. */
export function confirmSpeed(plan: WorkPlan): "fast" | "verdict" {
  return plan.confidence === "high" && !plan.new_task && plan.warnings.length === 0 ? "fast" : "verdict";
}

/** The destination as one word: the task, else the project, else what
 *  the caller says a brand-new chat is called. */
export function targetLabel(plan: WorkPlan, fallback: string): string {
  return plan.task_title ?? plan.project_name ?? fallback;
}

/** The stages of a round, by name (the hook's `Stage["s"]`). */
export type StageName =
  | "idle"
  | "listening"
  | "loading"
  | "transcribing"
  | "thinking"
  | "confirm"
  | "candidates"
  | "running"
  | "note"
  | "type"
  | "asking"
  | "answer";

/** What the microphone button means in each stage. While it listens the
 *  button is "I am done": the capture ends and the words go on. While the
 *  words are being read or acted on it waits: pressing it again used to
 *  abort the turn and start over, throwing away what had just been said.
 *  From rest, or from something that only lingers on screen, it starts. */
export function micPress(stage: StageName): "start" | "send" | "wait" {
  if (stage === "listening") return "send";
  if (stage === "transcribing" || stage === "loading" || stage === "thinking" || stage === "asking" || stage === "running") {
    return "wait";
  }
  return "start";
}

/** How long a first Esc keeps its warning up. */
export const ESC_CONFIRM_MS = 4000;

/** What Esc means in each stage. Listening: end the capture, keep the
 *  words. Reading the words: the first press only warns, because it is
 *  the same key that sends a moment earlier and the capture may have
 *  ended by itself in between; the second, while the warning is up,
 *  cancels. A model still loading has no words to lose. Anything else
 *  on screen just closes. */
export function escapePress(stage: StageName, warnedAt: number, now: number): "send" | "warn" | "abort" | "close" {
  if (stage === "listening") return "send";
  if (stage === "transcribing") return warnedAt > 0 && now - warnedAt <= ESC_CONFIRM_MS ? "abort" : "warn";
  if (stage === "loading") return "abort";
  return "close";
}
