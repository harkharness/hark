/**
 * The keyboard of the "add a project" field: which folder the highlight
 * rests on, what Enter takes, what Tab or a click goes into. The folders themselves
 * come from the backend (`folder_complete`).
 */

export type FolderHit = {
  name: string;
  /** Absolute: what becomes the project. */
  path: string;
  /** The same path as typed, home as `~`. */
  shown: string;
  git: boolean;
  /** Already a project. */
  known: boolean;
};

export type Completion = {
  hits: FolderHit[];
  /** The folder the typed text already is, by its real path: "name/" is
   *  wherever that name was found. Null while the text is a fragment. */
  folder: FolderHit | null;
};

export const EMPTY: Completion = { hits: [], folder: null };

/** Where the highlight starts: on the first folder, unless what was typed
 * is a folder already (Enter takes that folder) or nothing was
 * typed yet (a stray Enter must not add whatever came first). */
export function startIndex(done: Completion, typed: string): number {
  return done.folder || done.hits.length === 0 || !typed.trim() ? -1 : 0;
}

/** Arrows go round the list; from the text (-1) they enter at either end. */
export function moveIndex(index: number, count: number, delta: 1 | -1): number {
  if (count === 0) return -1;
  if (index < 0) return delta === 1 ? 0 : count - 1;
  return (index + delta + count) % count;
}

/** What Enter turns into a project: the highlighted folder; else the
 * folder the text is; else the text, for the backend to judge. */
export function enterTarget(typed: string, done: Completion, index: number): string | null {
  const hit = done.hits[index] ?? done.folder;
  if (hit) return hit.path;
  return typed.trim() || null;
}

/** What Tab or a click writes in the field: the folder with a separator after it, so
 * the next thing listed is what it holds. */
export function descendText(done: Completion, index: number): string | null {
  const hit = done.hits[index] ?? done.hits[0];
  return hit ? `${hit.shown}/` : null;
}
