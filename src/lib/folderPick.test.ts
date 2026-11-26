import { describe, expect, it } from "vitest";
import { descendText, enterTarget, moveIndex, startIndex, type Completion, type FolderHit } from "./folderPick";

const hit = (shown: string): FolderHit => ({
  name: shown.split("/").pop() ?? shown,
  path: shown.replace("~", "/home/u"),
  shown,
  git: false,
  known: false,
});

const list = (exact: boolean, ...shown: string[]): Completion => ({ hits: shown.map(hit), exact });

describe("startIndex", () => {
  it("rests on the first folder while the text is still a fragment", () => {
    expect(startIndex(list(false, "~/work/alpha", "~/work/alpha-old"), "~/work/al")).toBe(0);
  });

  it("rests on nothing when the text is already a folder: Enter takes the text", () => {
    expect(startIndex(list(true, "~/work/alpha", "~/work/alpha-old"), "~/work/alpha")).toBe(-1);
  });

  it("rests on nothing when there is nothing to offer", () => {
    expect(startIndex(list(false), "zz")).toBe(-1);
  });

  it("rests on nothing in an empty field: a stray Enter adds no project", () => {
    expect(startIndex(list(false, "~/work/alpha"), "  ")).toBe(-1);
  });
});

describe("moveIndex", () => {
  it("walks down and up through the folders", () => {
    expect(moveIndex(0, 3, 1)).toBe(1);
    expect(moveIndex(1, 3, -1)).toBe(0);
  });

  it("goes round at both ends", () => {
    expect(moveIndex(2, 3, 1)).toBe(0);
    expect(moveIndex(0, 3, -1)).toBe(2);
  });

  it("enters the list from the text at either end", () => {
    expect(moveIndex(-1, 3, 1)).toBe(0);
    expect(moveIndex(-1, 3, -1)).toBe(2);
  });

  it("stays out of an empty list", () => {
    expect(moveIndex(-1, 0, 1)).toBe(-1);
  });
});

describe("enterTarget", () => {
  it("is the folder the highlight rests on", () => {
    expect(enterTarget("~/work/al", list(false, "~/work/alpha", "~/work/alpha-old"), 1)).toBe("/home/u/work/alpha-old");
  });

  it("is the text itself when nothing is highlighted", () => {
    expect(enterTarget(" ~/work/alpha ", list(true, "~/work/alpha"), -1)).toBe("~/work/alpha");
  });

  it("is nothing when the field is empty and nothing is highlighted", () => {
    expect(enterTarget("  ", list(false), -1)).toBeNull();
  });

  it("ignores a highlight the list no longer has", () => {
    expect(enterTarget("~/x", list(false, "~/xa"), 4)).toBe("~/x");
  });
});

describe("descendText", () => {
  it("is the highlighted folder with a separator, so its own folders list next", () => {
    expect(descendText(list(false, "~/work/alpha", "~/work/beta"), 1)).toBe("~/work/beta/");
  });

  it("takes the first folder when nothing is highlighted", () => {
    expect(descendText(list(true, "~/work/alpha"), -1)).toBe("~/work/alpha/");
  });

  it("is nothing when there is no folder to go into", () => {
    expect(descendText(list(false), -1)).toBeNull();
  });
});
