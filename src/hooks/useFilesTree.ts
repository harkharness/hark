import { useCallback, useState } from "react";
import { readTreePref, treeOpenFor, writeTreePref } from "../lib/filesTree";

/**
 * The tree column of one files window: open or closed. The user's choice
 * (the toggle in the frame header) is saved and wins; before any choice
 * the frame's measured width decides — the window feeds it via `onWidth`.
 */
export function useFilesTree() {
  const [pref, setPref] = useState<boolean | null>(() => readTreePref());
  const [width, setWidth] = useState<number | null>(null);
  const open = treeOpenFor(pref, width);
  const set = useCallback((next: boolean) => {
    writeTreePref(next);
    setPref(next);
  }, []);
  const toggle = useCallback(() => set(!open), [open, set]);
  const onWidth = useCallback((px: number) => setWidth(px), []);
  return { open, toggle, set, onWidth };
}
