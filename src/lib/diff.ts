// The diff panel's pure rules: what counts as "seen", how a path splits,
// the totals line. Seen lives in localStorage per repository root; the
// signature is the change itself, so a file edited again is new again.
import type { FileChange } from "../types";

export type Seen = Record<string, string>;

export const seenKey = (root: string) => `hark.diff.seen.${root}`;

const signature = (c: FileChange) => `${c.added}:${c.removed}:${c.untracked ? "u" : "t"}`;

export function isSeen(seen: Seen, change: FileChange): boolean {
  return seen[change.path] === signature(change);
}

export function markSeen(seen: Seen, change: FileChange): Seen {
  return { ...seen, [change.path]: signature(change) };
}

export function splitPath(path: string): { name: string; dir: string } {
  const at = path.lastIndexOf("/");
  return at < 0 ? { name: path, dir: "" } : { name: path.slice(at + 1), dir: path.slice(0, at) };
}

export function totals(changes: FileChange[]): { added: number; removed: number; files: number } {
  return changes.reduce((t, c) => ({ added: t.added + c.added, removed: t.removed + c.removed, files: t.files + 1 }), { added: 0, removed: 0, files: 0 });
}
