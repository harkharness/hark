import { describe, expect, it } from "vitest";
import { boardSummary, filterTasks, idleDays, inProject } from "./board";
import type { BoardTask, Project } from "../types";

const hark: Project = { name: "hark", path: "/Users/x/Projects/hark" };
const landing: Project = { name: "hark-landing", path: "/Users/x/Projects/hark-landing" };
const task = (title: string, workspace: string | null, status: BoardTask["status"] = "doing"): BoardTask => ({
  title,
  status,
  updated_at: "2026-10-28T10:00:00Z",
  session_ids: [],
  pinned: false,
  workspace,
});

describe("inProject / filterTasks: a task belongs to the project its workspace sits in", () => {
  it("the root or a folder under it, never a sibling sharing the prefix", () => {
    expect(inProject(task("a", "/Users/x/Projects/hark"), hark)).toBe(true);
    expect(inProject(task("a", "/Users/x/Projects/hark/crates"), hark)).toBe(true);
    expect(inProject(task("a", "/Users/x/Projects/hark-landing"), hark)).toBe(false);
    expect(inProject(task("a", null), hark)).toBe(false);
  });
  it("no filter is every task; a project keeps only its own", () => {
    const tasks = [task("a", hark.path), task("b", landing.path), task("c", null)];
    expect(filterTasks(tasks, null)).toHaveLength(3);
    expect(filterTasks(tasks, hark).map((t) => t.title)).toEqual(["a"]);
  });
});

describe("boardSummary: the line over the board", () => {
  it("counts cards, live workers and open asks among the tasks shown", () => {
    const tasks = [task("a", hark.path), task("b", hark.path), task("c", hark.path, "done")];
    expect(boardSummary(tasks, new Set(["a", "zzz"]), new Set(["b"]))).toEqual({ cards: 3, live: 1, asking: 1 });
  });
});

describe("idleDays: how long a card sat still", () => {
  it("whole days, zero inside the first one", () => {
    const now = Date.parse("2026-10-28T10:00:00Z");
    expect(idleDays("2026-10-28T02:00:00Z", now)).toBe(0);
    expect(idleDays("2026-10-24T09:00:00Z", now)).toBe(4);
    expect(idleDays("not a date", now)).toBe(0);
  });
});
