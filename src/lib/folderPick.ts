/**
 * The keyboard of the "add a project" field: which folder the highlight
 * rests on, what Enter takes, what Tab goes into. The folders themselves
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

export type Completion = { hits: FolderHit[]; exact: boolean };

export const EMPTY: Completion = { hits: [], exact: false };

/** Where the highlight starts: on the first folder, unless what was typed
 * is a folder already (Enter takes the text as written) or nothing was
 * typed yet (a stray Enter must not add whatever came first). */
export function startIndex(done: Completion, typed: string): number {
  return done.exact || done.hits.length === 0 || !typed.trim() ? -1 : 0;
}

/** Arrows go round the list; from the text (-1) they enter at either end. */
export function moveIndex(index: number, count: number, delta: 1 | -1): number {
  if (count === 0) return -1;
  if (index < 0) return delta === 1 ? 0 : count - 1;
  return (index + delta + count) % count;
}

/** What Enter turns into a project: the highlighted folder, else the text. */
export function enterTarget(typed: string, done: Completion, index: number): string | null {
  const hit = done.hits[index];
  if (hit) return hit.path;
  return typed.trim() || null;
}

/** What Tab writes in the field: the folder with a separator after it, so
 * the next thing listed is what it holds. */
export function descendText(done: Completion, index: number): string | null {
  const hit = done.hits[index] ?? done.hits[0];
  return hit ? `${hit.shown}/` : null;
}
