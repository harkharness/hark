import { describe, expect, it } from "vitest";
import { isSeen, markSeen, seenKey, splitPath, totals } from "./diff";
import type { FileChange } from "../types";

const c = (path: string, added = 1, removed = 0): FileChange => ({ path, added, removed, untracked: false, binary: false });

describe("seen: a file is seen until its change changes", () => {
  it("marks by path and signature; a new count unmarks it", () => {
    const seen = markSeen({}, c("a.ts", 3, 1));
    expect(isSeen(seen, c("a.ts", 3, 1))).toBe(true);
    expect(isSeen(seen, c("a.ts", 4, 1))).toBe(false);
    expect(isSeen(seen, c("b.ts"))).toBe(false);
  });
  it("the storage key is per repository root", () => {
    expect(seenKey("/Users/x/p")).toBe("hark.diff.seen./Users/x/p");
  });
});

describe("splitPath / totals", () => {
  it("name and directory, the root having none", () => {
    expect(splitPath("src/components/ReportDialog.tsx")).toEqual({ name: "ReportDialog.tsx", dir: "src/components" });
    expect(splitPath("README.md")).toEqual({ name: "README.md", dir: "" });
  });
  it("sums every change", () => {
    expect(totals([c("a", 2, 1), c("b", 5, 0)])).toEqual({ added: 7, removed: 1, files: 2 });
  });
});
