import { useEffect, useState } from "react";
import * as ipc from "../lib/ipc";
import type { RepoState } from "../types";

/** How often the working tree is re-read. Git is cheap and cached in
 *  Rust for a few seconds, but nobody needs sub-second branch news. */
const EVERY_MS = 10_000;

/**
 * Git state for a set of workspaces, keyed by the path asked for (not by
 * the repo root — the caller knows its own paths). Paths that are not
 * repositories are simply absent, so a lookup returning undefined means
 * "no git here", never "still loading".
 */
export function useRepoStates(paths: string[]): Record<string, RepoState> {
  const [states, setStates] = useState<Record<string, RepoState>>({});
  // Effects compare deps by identity, and the caller rebuilds this array
  // every render; the joined key is what actually changed.
  const key = [...new Set(paths.filter(Boolean))].sort().join("|");

  useEffect(() => {
    if (!key) {
      setStates({});
      return;
    }
    let alive = true;
    const read = () =>
      ipc
        .repoStates(key.split("|"))
        .then((next) => alive && setStates(next))
        // A missing git, an unreadable repo: the chips just stay away.
        .catch(() => {});
    read();
    const timer = window.setInterval(read, EVERY_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [key]);

  return states;
}
