// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import { TREE_MIN_WIDTH, TREE_PREF_KEY, readTreePref, treeOpenFor, writeTreePref } from "./filesTree";

describe("whether the files window shows its tree", () => {
  it("opens by default when the frame is wide enough", () => {
    expect(treeOpenFor(null, 900)).toBe(true);
  });

  it("is born collapsed in a narrow frame, the rail case", () => {
    expect(treeOpenFor(null, TREE_MIN_WIDTH - 1)).toBe(false);
  });

  it("assumes a wide frame before the first measure", () => {
    expect(treeOpenFor(null, null)).toBe(true);
  });

  it("lets the saved choice win over the width, both ways", () => {
    expect(treeOpenFor(false, 900)).toBe(false);
    expect(treeOpenFor(true, 300)).toBe(true);
  });
});

describe("the saved choice", () => {
  beforeEach(() => localStorage.clear());

  it("survives a remount and ignores garbage", () => {
    expect(readTreePref()).toBe(null);
    writeTreePref(false);
    expect(readTreePref()).toBe(false);
    writeTreePref(true);
    expect(readTreePref()).toBe(true);
    localStorage.setItem(TREE_PREF_KEY, "meh");
    expect(readTreePref()).toBe(null);
  });
});
