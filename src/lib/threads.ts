// The identity of a thread on screen is its label (Msg.task). A label can
// change under a live chat: the light model names it after the first
// reply, the index shows that name, and the person reopens the chat by
// it. These helpers keep one key per chat when that happens, and tell a
// chat that belongs to Hark itself (its own session, in the data dir)
// from a project chat. Pure.
import type { Msg } from "../types";

/** Every message of thread `from` now belongs to thread `to`. */
export function rekeyMessages(messages: Msg[], from: string, to: string): Msg[] {
  if (from === to) return messages;
  let touched = false;
  const out = messages.map((m) => {
    if ("task" in m && m.task === from) {
      touched = true;
      return { ...m, task: to } as Msg;
    }
    return m;
  });
  return touched ? out : messages;
}

/** A map keyed by thread, with `from` moved to `to` (the target wins when
 *  both exist, as the one on screen). */
export function rekeyRecord<T>(rec: Record<string, T>, from: string, to: string): Record<string, T> {
  if (from === to || !(from in rec)) return rec;
  const next = { ...rec };
  const moved = next[from];
  delete next[from];
  if (!(to in next)) next[to] = moved;
  return next;
}

/** Hark's own chat lives in the data dir and in the session the state
 *  file names. Opening it as a project chat spawned a second worker on
 *  the same session, whose replies landed in the general thread. */
export function harkOwnedSession(
  hit: { session_id: string; cwd?: string | null },
  dataDir: string | null | undefined,
  harkSession: string | null | undefined,
): boolean {
  if (harkSession && hit.session_id === harkSession) return true;
  const root = (dataDir ?? "").replace(/\/+$/, "");
  const cwd = hit.cwd ?? "";
  return !!root && (cwd === root || cwd.startsWith(`${root}/`));
}
