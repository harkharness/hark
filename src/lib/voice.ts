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
