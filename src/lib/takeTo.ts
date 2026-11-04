/** Where a page can be taken ("levar para…"): never a silent default. */
export type Destination =
  | { kind: "hark" }
  | { kind: "origin"; title: string; sessionId?: string }
  | { kind: "chat"; title: string; sessionId?: string }
  | { kind: "new"; project: string };

export type RecentChat = { title: string; sessionId?: string };

/** How many recent chats the menu lists besides the origin. */
const RECENT = 3;

/**
 * The menu's rows, in order: Hark (Início), the chat the page was born in
 * when it is on the board, a few recent chats, a new chat in the page's
 * project. The origin is highlighted by its place, not chosen for you.
 */
export function destinations(origin: string | undefined, recent: RecentChat[], project: string): Destination[] {
  const out: Destination[] = [{ kind: "hark" }];
  const born = origin ? recent.find((c) => c.title === origin) : undefined;
  if (born) out.push({ kind: "origin", title: born.title, sessionId: born.sessionId });
  for (const c of recent.filter((c) => c.title !== origin).slice(0, RECENT)) {
    out.push({ kind: "chat", title: c.title, sessionId: c.sessionId });
  }
  out.push({ kind: "new", project });
  return out;
}
