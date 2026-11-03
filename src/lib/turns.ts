/** The least a clock must have run before a reconcile may end it: a turn
 *  just sent is on screen before the backend marks it in flight. */
export const TURN_GRACE_MS = 8_000;

/**
 * Which clocks on screen have no turn behind them. The backend says which
 * tasks have a turn in flight (and their sessions); a chat's clock is
 * keyed by its title, so `sessionOf` maps a title to its session. A clock
 * nobody can map stays: better a stale clock than a turn shown as done
 * while it runs. The general chat's clock follows the hark-chat task.
 */
export function staleTurns(
  turns: Record<string, { startedAt: number }>,
  inFlight: { task_id: string; session_id: string }[],
  sessionOf: (key: string) => string | null,
  general: string,
  now = Date.now(),
): string[] {
  const sessions = new Set(inFlight.map((w) => w.session_id).filter(Boolean));
  const harkChat = inFlight.some((w) => w.task_id === "hark-chat");
  const out: string[] = [];
  for (const [key, state] of Object.entries(turns)) {
    if (now - state.startedAt < TURN_GRACE_MS) continue;
    if (key === general) {
      if (!harkChat) out.push(key);
      continue;
    }
    const session = sessionOf(key);
    if (session && !sessions.has(session)) out.push(key);
  }
  return out;
}
