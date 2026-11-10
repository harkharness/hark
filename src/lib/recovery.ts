/** A chat whose session log is gone from disk may take another session;
 *  nothing else may move a chat off its own session. */

export type Recoverable = { title: string; sessionId: string; workspace?: string | null };
export type Candidate = { session_id: string; title?: string; cwd?: string | null };

/** The backend says so in the error itself: only a missing log counts.
 *  A locked index, a parse error or a timeout is a failure to report,
 *  not a reason to open some other conversation (10/11: a topic match
 *  swapped a chat for a 237 MB desktop session held at a terminal). */
export function sessionVanished(err: unknown): boolean {
  const text = err instanceof Error ? err.message : String(err ?? "");
  return text.startsWith("vanished");
}

/** The replacement for a chat whose session vanished, or null: the hit
 *  must be another session, inside the chat's own folder when it has one. */
export function recoverSession(err: unknown, chat: Recoverable, hit: Candidate | null): Candidate | null {
  if (!sessionVanished(err) || !hit || hit.session_id === chat.sessionId) return null;
  if (chat.workspace) {
    const cwd = hit.cwd ?? "";
    if (cwd !== chat.workspace && !cwd.startsWith(`${chat.workspace}/`)) return null;
  }
  return hit;
}
