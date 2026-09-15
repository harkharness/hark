/**
 * Whether the files window shows its tree column. Pure: the saved choice
 * wins; without one the frame's width decides, because in the rail (≈42%
 * of the work area) tree + viewer would be two squeezed columns.
 */

/** Below this frame width the tree is born collapsed. */
export const TREE_MIN_WIDTH = 560;
export const TREE_PREF_KEY = "hark-files-tree";

/** An unmeasured frame counts as wide: full screen is the common first paint. */
export function treeOpenFor(pref: boolean | null, width: number | null): boolean {
  if (pref !== null) return pref;
  return width === null || width >= TREE_MIN_WIDTH;
}

export function readTreePref(): boolean | null {
  try {
    const raw = localStorage.getItem(TREE_PREF_KEY);
    return raw === "open" ? true : raw === "closed" ? false : null;
  } catch {
    return null;
  }
}

export function writeTreePref(open: boolean) {
  try {
    localStorage.setItem(TREE_PREF_KEY, open ? "open" : "closed");
  } catch {
    // Storage refused (private mode): the choice lasts the session.
  }
}
