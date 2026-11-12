import { describe, expect, it } from "vitest";
import { harkOwnedSession, rekeyMessages, rekeyRecord } from "./threads";
import type { Msg } from "../types";

const msgs: Msg[] = [
  { who: "user", text: "a", task: "old name" } as Msg,
  { who: "hark", text: "b", task: "other" } as Msg,
  { who: "user", text: "c" } as Msg,
];

describe("a chat renamed under a live thread", () => {
  it("moves every message of the old label to the new one, leaving the rest", () => {
    const out = rekeyMessages(msgs, "old name", "New name");
    expect(out.map((m) => ("task" in m ? m.task : null))).toEqual(["New name", "other", null]);
    expect(out[1]).toBe(msgs[1]);
  });
  it("returns the same array when nothing matches or the names are equal", () => {
    expect(rekeyMessages(msgs, "nope", "x")).toBe(msgs);
    expect(rekeyMessages(msgs, "old name", "old name")).toBe(msgs);
  });
  it("moves a per-thread record the same way, the target winning a clash", () => {
    expect(rekeyRecord({ a: 1, b: 2 }, "a", "c")).toEqual({ b: 2, c: 1 });
    expect(rekeyRecord({ a: 1, b: 2 }, "a", "b")).toEqual({ b: 2 });
    const same = { a: 1 };
    expect(rekeyRecord(same, "z", "y")).toBe(same);
  });
});

describe("a session that is Hark's own", () => {
  it("is the one the state file names, or any one whose cwd is the data dir", () => {
    expect(harkOwnedSession({ session_id: "s1", cwd: "/Users/ana/Projects/x" }, "/Users/ana/.hark", "s1")).toBe(true);
    expect(harkOwnedSession({ session_id: "s2", cwd: "/Users/ana/.hark" }, "/Users/ana/.hark/", null)).toBe(true);
    expect(harkOwnedSession({ session_id: "s3", cwd: "/Users/ana/.hark/sub" }, "/Users/ana/.hark", null)).toBe(true);
  });
  it("is not a project chat", () => {
    expect(harkOwnedSession({ session_id: "s4", cwd: "/Users/ana/Projects/x" }, "/Users/ana/.hark", "s1")).toBe(false);
    expect(harkOwnedSession({ session_id: "s5", cwd: "/Users/ana/.harkness" }, "/Users/ana/.hark", null)).toBe(false);
    expect(harkOwnedSession({ session_id: "s6" }, null, null)).toBe(false);
  });
});
