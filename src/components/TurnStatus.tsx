import { useEffect, useState } from "react";
import { st } from "../lib/i18n";

/**
 * What is happening RIGHT NOW, while a turn runs.
 *
 * The only feedback used to be a pulsing orb next to the speaker icon —
 * it said "something", never what, and never for how long. This says the
 * elapsed time, the phase (drawn from real stream events, not a guess),
 * and how much has come back so far.
 */
export type TurnPhase =
  | { kind: "thinking" }
  | { kind: "writing" }
  | { kind: "tool"; name: string }
  | { kind: "waiting"; label: string };

export type TurnState = {
  startedAt: number;
  phase: TurnPhase;
  /** Characters of assistant text streamed this turn (token estimate). */
  chars: number;
};

/** Elapsed, the way a person reads it. */
function elapsed(ms: number): string {
  const total = Math.floor(ms / 1000);
  if (total < 60) return `${total}s`;
  return `${Math.floor(total / 60)}m ${total % 60}s`;
}

/** Thinking gets more insistent the longer it takes — the wait is the
 *  information. Tool and writing phases say what they are instead. */
function label(phase: TurnPhase, seconds: number): string {
  switch (phase.kind) {
    case "tool":
      return st("sp_ts_tool", { t: phase.name });
    case "writing":
      return st("sp_ts_writing");
    case "waiting":
      return phase.label;
    default:
      if (seconds > 90) return st("sp_ts_thinking_long");
      if (seconds > 35) return st("sp_ts_thinking_more");
      return st("sp_ts_thinking");
  }
}

export default function TurnStatus({ state }: { state: TurnState }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const ms = Math.max(0, now - state.startedAt);
  const seconds = Math.floor(ms / 1000);
  // chars/4 is the same proxy the prompt budget uses. Marked "~" because
  // it IS an estimate: the real count arrives with the finished turn.
  const tokens = Math.round(state.chars / 4);

  return (
    <div className="turnstatus" role="status" aria-live="polite">
      <span className="ts-spark" aria-hidden="true">
        ✳
      </span>
      <span className="ts-time">{elapsed(ms)}</span>
      {tokens > 0 && (
        <>
          <span className="ts-dot">·</span>
          <span className="ts-tokens">{st("sp_ts_tokens", { n: tokens })}</span>
        </>
      )}
      <span className="ts-dot">·</span>
      <span className="ts-label">{label(state.phase, seconds)}</span>
    </div>
  );
}
