// The identity of a thread on screen is its label (Msg.task). A label can
// change under a live chat: the light model names it after the first
// reply, the index shows that name, and the person reopens the chat by
// it. These helpers keep one key per chat when that happens, and tell a
// chat that belongs to Hark itself (its own session, in the data dir)
// from a project chat. Pure.
import type { Msg } from "../types";

/** A "+" draft is a page of its own until its first message opens the
 *  session: keyed by the project, with a prefix no chat title can carry
 *  (a control character), so it never collides with a chat or with the
 *  general thread (the empty label). */
const DRAFT_PREFIX = "\u0001draft:";
export const draftThread = (projectPath: string): string => `${DRAFT_PREFIX}${projectPath}`;
export const isDraftThread = (thread: string): boolean => thread.startsWith(DRAFT_PREFIX);

/** Início holds several conversations a day; each is a thread of its
 *  own, keyed by its id the same way a draft is by its project. The
 *  day's own thread (no id) stays the general one, the empty label. */
const INICIO_PREFIX = "\u0001inicio:";
export const inicioThread = (conversation: string): string => `${INICIO_PREFIX}${conversation}`;
/** A fresh id: unique on this machine, sortable by when it began. */
export const newConversationId = (now = Date.now()): string =>
  `c-${now.toString(36)}${Math.floor(Math.random() * 1296).toString(36).padStart(2, "0")}`;

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

/** What `chatFolder` reads: only the fields it needs of each source. */
type FolderSources = {
  /** The live worker behind the chat, from the registry. */
  worker?: { workspace?: string | null };
  board: { title: string; workspace?: string | null; session_ids: string[] }[];
  sessions: { session_id: string; cwd?: string | null }[];
};

/**
 * The folder the chat on screen works in: where its terminal opens, whose
 * files and repository the panels beside it show. Asked of whoever knows,
 * surest first: the folder the chat was opened in, the live worker's, the
 * board card (by title, then by session: a chat just born is on screen
 * under the first words of its message while its card already carries a
 * written title), and at last the folder the session itself ran in.
 */
export function chatFolder(
  chat: { title: string; sessionId: string; workspace?: string } | null,
  from: FolderSources,
): string | undefined {
  if (!chat) return undefined;
  const card =
    from.board.find((t) => t.title === chat.title && t.workspace) ??
    (chat.sessionId ? from.board.find((t) => t.session_ids.includes(chat.sessionId) && t.workspace) : undefined);
  const session = chat.sessionId ? from.sessions.find((s) => s.session_id === chat.sessionId) : undefined;
  return chat.workspace || from.worker?.workspace || card?.workspace || session?.cwd || undefined;
}
